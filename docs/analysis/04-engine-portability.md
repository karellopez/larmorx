<!-- Generated 2026-10-08 by a read-only analysis agent against shallow clones in the parent directory.
     fmriprep @ 21a490fb (26.0.0.dev). Line numbers refer to those clones and will drift. -->

# nipype engine and cross-platform portability analysis for larmorx

Analysis performed read-only against the shallow clones in the parent directory.

## Headline findings

1. **Stock nipype can't run any workflow on Windows.** `Workflow.run()` lazily imports `nipype.pipeline.plugins` (`nipype/pipeline/engine/workflows.py:600-606`). That package's `__init__.py:6` imports `sge.py`, which runs `import pwd` at line 4, so every plugin (including Linear and MultiProc) fails. Separately, `MultiProcPlugin.__init__` calls `get_system_total_memory_gb()` (`multiproc.py:129-132`). That function raises on anything other than Linux or macOS (`utils/profiler.py:208-220`), and because it is the default argument of `dict.get`, it runs even when `memory_gb` is passed.
2. **Importing `fmriprep.config` crashes on Windows**, for three separate reasons:
   - `set_start_method('forkserver')` raises **ValueError** on Windows, but the code only catches RuntimeError (`fmriprep/config.py:103-106`; checked against the stdlib `get_context`).
   - `os.path.join(os.getenv('HOME'), ...)` is evaluated eagerly as a default argument, so it raises TypeError when HOME is unset, even if TEMPLATEFLOW_HOME is set (`config.py:177`).
   - `from os import EX_SOFTWARE` raises ImportError (`cli/run.py:33`).

   This import also happens inside **worker processes**: `fmriprep/interfaces/bids.py:17` imports `utils/bids.py`, whose line 39 imports `config`.
3. **fmriprep only needs a small part of nipype.** It uses MultiProc and Linear, timestamp hashing, the simple forms of iterables, JoinNode and MapNode, `run_without_submitting`, and the traits-based interface specs. Everything that matters is about 5.7k lines (engine 4.4k + plugins base/linear/multiproc/tools 1.35k), plus about 3.4k lines of `interfaces/base` and `utility`. All of nipype is about 125k lines.
4. **fmriprep's `maxtasksperchild: 1` has no effect.** It is set at `config.py:324-327`, but only LegacyMultiProc reads it (`legacymultiproc.py:207`). MultiProc's `ProcessPoolExecutor` (`multiproc.py:159-167`) never passes `max_tasks_per_child`, so workers are reused.
5. **No Pydra traces** in any of the 11 repos (`grep -ri pydra` returns nothing).
6. **CI coverage:** nipype, fmriprep, niworkflows, smriprep, sdcflows, templateflow, nitransforms and nireports test on Ubuntu only. pybids tests Ubuntu and macOS. Only acres tests Windows.

---

## 1. nipype engine architecture and what fmriprep uses

### Components

| Area | Where | Behaviour |
|---|---|---|
| Workflow run | `pipeline/engine/workflows.py:581-639` | Builds a flat graph, then `_set_needed_outputs`, then `generate_expanded_graph(deepcopy(flatgraph))`. Merges config into each node, writes the report info, then calls `runner.run`. |
| Graph expansion | `engine/utils.py:941-1175` | Expands iterables as a cartesian product (with `synchronize`/`itersource`), adds JoinNode slot fields, and removes IdentityInterface nodes that aren't joins (`utils.py:827-868`). |
| Working-dir path | `nodes.py:302-324` | `base_dir/<workflow hierarchy split on '.'>/<_field_value param dirs>/<node>`, then `realpath`. Param strings are sanitised (`utils.py:591-602`) and replaced by SHA-1 if longer than 252 characters (`utils.py:54-63`). MapNode sub-nodes go in `<node>/mapflow/_<node>N`. |
| Hashing | `interfaces/base/specs.py:218-320`, `nodes.py:565-579` | md5 of sorted, defined, non-`nohash` inputs. For files only the file's hash is used, not its path. Default `hash_method = timestamp` (`utils/config.py:44`; fmriprep doesn't override it), which is md5 of size + mtime (`filemanip.py:189-198`). `content` hashes the bytes with chunked md5. `needed_outputs` is mixed into the hash when `remove_unnecessary_outputs` is on. **The interface's code or version is not part of the hash.** |
| Cache check / resume | `nodes.py:341-563` | `_0x<hash>.json` marks the node as cached; `_0x<hash>_unfinished.json` exists while it runs. On a hash mismatch the directory is emptied (`emptydirs`) and the node re-runs. A cached node loads `result_<name>.pklz`; if unpickling fails it re-aggregates the outputs (`nodes.py:663-707`). |
| Result pickles | `engine/utils.py:227-270`, `filemanip.py:682-705` | `result_<node>.pklz`, `_node.pklz` and `_inputs.pklz` are gzip pickles written to a tmp file then `os.rename`d. Results hold absolute paths (`use_relative_paths=false`) and the full environment dict. |
| Crash files | `plugins/tools.py:19-68`, `filemanip.py:650-659` | `crash-<ts>-<user>-<node>-<uuid>.txt|pklz`. The txt format (`Node:` / `Working directory:` / `Node inputs:` / traceback) is parsed by `niworkflows/utils/misc.py:182-230`, `nireports/assembler/misc.py:34-80` and `fmriprep/utils/telemetry.py:85-150`. |
| Scheduler loop | `plugins/base.py:124-228` | Polls with a fixed sleep of `poll_sleep_duration=2` s (`base.py:206-207`). The dependency matrix is a `scipy.sparse.lil_matrix` (`base.py:25-34`). A failure cleans the queue and drops descendants. `stop_on_first_crash` raises immediately. |
| MultiProc | `plugins/multiproc.py:116-431` | Defaults: `n_procs` = cpu_count, `memory_gb` = 90% of RAM, plus GPU slots. Ready jobs are greedily fitted by declared `mem_gb` (node default 0.20) and `n_procs` (`multiproc.py:301-390`). Sort order is `tsort` or `mem_thread`. Cached nodes are finished in the master via `_local_hash_check`. `run_without_submitting` nodes run synchronously in the master. Other nodes are `deepcopy`'d and pickled to a ProcessPoolExecutor (`mp_context` from plugin_args or the default). MapNodes become per-sub-node jobs (`base.py:272-299`). Memory is **declared, not enforced**. |
| CommandLine | `interfaces/base/core.py:719-770`, `utils/subprocess.py:73-198` | Builds a string cmdline from `argstr`, then `which()` (PATHEXT-aware, `filemanip.py:804-827`), optionally `ldd`/`otool`, then `Popen(cmdline, shell=True, cwd=node_dir, env=...)`. Output modes are stream (uses `select.select`), allatonce, or file*. `RuntimeContext` does `os.chdir` into the node dir (`support.py:80,94`). |
| Resource monitor | `utils/profiler.py:45-147` | psutil sampling thread. Off in fmriprep unless `--resource-monitor`. |

### How fmriprep configures the engine

- `config.nipype` (`fmriprep/config.py:309-382`):
  - `plugin='MultiProc'`, `plugin_args={'maxtasksperchild':1,'raise_insufficient':False}`, plus `n_procs` and `memory_gb`.
  - `crashfile_format='txt'`, `get_linked_libs=False`, `remove_unnecessary_outputs=True`, `check_version=False`.
  - `omp_nthreads = min(nprocs-1, 8)`.
- `--use-plugin` YAML can swap the plugin (`parser.py:883-897`); `--debug pdb` switches to Linear (`run.py:59-63`).
- The workflow is built in a child `Process` that returns results through a `Manager` dict (`run.py:81-93`), and the boilerplate is generated in another child Process (`run.py:116-122`). The run itself is `fmriprep_wf.run(**get_plugin())` (`run.py:145`).
- Crash dumps go to a per-subject directory set through per-node config overrides (`workflows/base.py:104-112`).
- sdcflows instead uses niworkflows' own MultiProc copy (`niworkflows/engine/plugin.py`, via `sdcflows/cli/main.py:172-176`).

### Feature usage (grep occurrence counts, non-test, fmriprep + smriprep + niworkflows + sdcflows; approximate)

| Feature | Count | Needed by a replacement? |
|---|---|---|
| Node( | 311 / 220 / 137 / 119 | yes |
| `.connect(` blocks (nested `'inputnode.x'` addressing) | 266 | yes |
| connect-functions `(('out', fn), 'in')` | 48 | yes (edge transforms) |
| IdentityInterface | 199 | yes (pruned at expansion) |
| `run_without_submitting` | 189 | yes (performance) |
| `mem_gb=` / `n_procs=` | 174 / 51 | yes |
| SimpleInterface classes (incl. nireports) | 178 | yes (traits specs) |
| niu.Function | 57 | yes (source serialisation) |
| MapNode (all zipped iterfields; no `nested`/`serial`) | 63 | yes, simple form only |
| iterables sites (hemi L/R, echoes, templates, spaces) | ~12 | yes, simple lists |
| `synchronize` / `itersource=` | 0 | no |
| JoinNode (`joinsource`, 2 with `joinfield`) | 9 | yes |
| LiterateWorkflow `__desc__`/`__postdesc__` (boilerplate via `topological_sort`, `niworkflows/engine/workflows.py:47-73`) | 52 | yes |
| `_always_run` / `overwrite=` | 9 / 1 (FreeSurfer steps in smriprep) | optional |
| `updatehash`, provenance (`write_provenance`), `status_callback`, SGE/SLURM plugins | 0 in fmriprep | no |
| `write_graph` (pydot + `dot`) | `--write-graph` only | optional |

nipype imports beyond the engine:
- `interfaces.base`, `utility`, `mixins.reporting`
- `io.add_traits`, `FreeSurferSource`, `DataSink` (smriprep FreeSurfer path)
- `algorithms.confounds` (ACompCor/TCompCor/DVARS/TSNR), `rapidart._calc_norm_affine`
- `filemanip` (`fname_presuffix`, `copyfile`, `which`, `hash_infile`)

---

## 2. Windows/macOS portability blockers

### Grep counts (source / tests)

| Pattern | Hits |
|---|---|
| `os.symlink` / `symlink_to` | nipype 1/0, pybids 1/4, niworkflows 0/1 |
| `os.link` | nipype 1/0 |
| `'fork'` / `forkserver` | nipype 2, fmriprep 3, smriprep 1, sdcflows 2 |
| signal handling (`killpg`/`SIGKILL`/`SIGALRM`/`signal.signal`) | migas 3/1 only |
| `fcntl`, `import resource`, `sched_getaffinity`, `ulimit` | **0 everywhere** |
| `shell=True` | nipype 11/0, niworkflows 3, nireports 2, nitransforms 0/10, sdcflows 0/1 |
| `getuid` | nipype 3, migas 1, fmriprep tests 2 |
| `'/tmp` | nipype 17/21, fmriprep 7 (docker wrapper), smriprep 2, sdcflows 1, niworkflows 1/1 |
| `import pwd` | nipype 1 |
| `os.EX_*` | fmriprep 3, sdcflows 3 |
| `select.select` | nipype 1 |
| `/proc/` | nipype 1/11, fmriprep 6, sdcflows 6 |
| `getenv('HOME')` | fmriprep 1, sdcflows 1 |
| `mmap` | nipype 4, niworkflows 8/3 |
| filelock | nipype 12 |
| xvfb | nipype 11/30 |

### Findings table

Severity: **Blocker** = crashes at import or run time on that OS; **High** = breaks a common path; **Medium** = breaks edge cases or costs performance; **Low** = rare or cosmetic.

| # | Pattern | file:line | Repo | Severity | Suggested fix |
|---|---|---|---|---|---|
| 1 | `import pwd` in SGE plugin, imported by `plugins/__init__` and pulled in by `Workflow.run` | `nipype/pipeline/plugins/sge.py:4`, `plugins/__init__.py:6`, `engine/workflows.py:600` | nipype | **Blocker (Win)** | Lazy-import `pwd` inside the SGE functions or use `getpass.getuser()`; make the plugin registry lazy |
| 2 | Total RAM: Linux `/proc`, macOS `os.popen sysctl`, else `raise`; evaluated eagerly as `.get` default | `nipype/utils/profiler.py:199-223`; `multiproc.py:129-132`; `niworkflows/engine/plugin.py:439-457` | nipype, niworkflows | **Blocker (Win)** | `psutil.virtual_memory().total`, computed lazily |
| 3 | `set_start_method('forkserver')` catching only RuntimeError | `fmriprep/config.py:103-106` (also runs in workers) | fmriprep | **Blocker (Win)** | Choose `'forkserver' if in mp.get_all_start_methods() else 'spawn'`; catch ValueError |
| 4 | `set_start_method('forkserver')` with no try | `smriprep/src/smriprep/cli/run.py:373` | smriprep | **Blocker (Win)** | Same as #3 |
| 5 | `mp.set_start_method('fork')` inside `suppress(RuntimeError)` | `sdcflows/sdcflows/cli/main.py:100` | sdcflows | **Blocker (Win)**; High (macOS fork safety) | Use spawn/forkserver |
| 6 | `os.path.join(os.getenv('HOME'),...)` as eager default | `fmriprep/config.py:177`, `sdcflows/config.py:173` | fmriprep, sdcflows | **Blocker (Win, HOME unset)** | Use `templateflow.conf.TF_HOME`. This also fixes the default mismatching platformdirs on macOS and Windows (`templateflow/conf/env.py`) |
| 7 | `from os import EX_SOFTWARE`; `os.EX_DATAERR`/`EX_USAGE` | `fmriprep/cli/run.py:33,112`; `fmriprep/workflows/base.py:718`; `sdcflows/cli/main.py:135,143,162` | fmriprep, sdcflows | **Blocker** (run.py), Medium (error paths) | Local constants 64/65/70 |
| 8 | FS licence check always runs `mri_convert`; FileNotFoundError not caught | `niworkflows/utils/misc.py:358-378`, called at `fmriprep/cli/workflow.py:119` | niworkflows, fmriprep | **Blocker for a FreeSurfer-free build** | Run only when FreeSurfer binaries are actually used; catch OSError |
| 9 | `os.rename(tmp, dst)` over an existing file; retry catches only FileNotFoundError | `nipype/utils/filemanip.py:699` | nipype | **High (Win)**: FileExistsError on MapNode re-runs, re-aggregation, `updatehash` | `os.replace`; retry on PermissionError (antivirus) |
| 10 | `select.select` on pipes (`terminal_output='stream'`, the CommandLine default) | `nipype/utils/subprocess.py:127` | nipype | **High (Win)**: Linear plugin and master-run CommandLine nodes. MultiProc only forces allatonce for submitted jobs (`multiproc.py:182-183`) | `communicate()` or reader threads |
| 11 | `Popen(cmdline_string, shell=True)` with POSIX-style quoting | `nipype/utils/subprocess.py:105-112`; `core.py:711` | nipype | **High (Win)** for any CommandLine that remains (cmd.exe quoting, spaces in `C:\Users\First Last`) | argv list with `shell=False`, or drop CommandLine in favour of PyO3 calls |
| 12 | `ProcessPoolExecutor(max_workers=n_procs)`; Windows caps at 61; fmriprep default is `os.cpu_count()` | `multiproc.py:159-167`; `niworkflows/engine/plugin.py:462`; `fmriprep/config.py:318` | nipype, niworkflows | **High (Win, 62+ threads)** | `min(n, 61)` |
| 13 | Deep node dirs plus `os.chdir` per interface or worker | `nodes.py:302-324`; `support.py:80`; `multiproc.py:78` | nipype | **High (Win)** (see path-depth estimate below) | Short hashed node dirs, no chdir, short work dir, enable LongPathsEnabled |
| 14 | Symlinks only when `os.name=='posix'`; otherwise copy | `filemanip.py:400-407` | nipype | Medium (Win: disk and time for `copyfile=False` inputs) | Try `os.link` (works on NTFS without privileges) before copying |
| 15 | Deleting files while nibabel memmaps or handles are open (Win error 32) | engine cleanup at `engine/utils.py:1436-1500`, `filemanip.py:741-780` | nipype, all in-process interfaces | Medium (Win) | `mmap=False` in in-process nodes, `gc.collect()` plus retry; avoid persistent memmaps on the Rust side |
| 16 | `NamedTemporaryFile` re-opened by a child process | `fmriprep/utils/bids.py:448-452` | fmriprep | Medium (Win) | `delete=False` or `delete_on_close=False` |
| 17 | `check_call(['bids-validator',...])` without shell; npm `.cmd` shim not found | `fmriprep/utils/bids.py:452` | fmriprep | Medium (Win: validation silently skipped) | Resolve with `shutil.which` first |
| 18 | Ignore-pattern matching on `str(Path)`, which has backslashes on Windows; fmriprep regexes use `/` | `pybids/src/bids/layout/index.py:40-58` (line 54); patterns at `fmriprep/config.py:493-506`; templateflow `cache.py` | pybids | Medium (Win: `code`/`sourcedata`/`dwi` etc. not ignored) | `.as_posix()` |
| 19 | TemplateFlow S3 download writes the final path directly, with no lock | `python-client/templateflow/client.py:396-405` | templateflow | Medium (all OS; parallel workers) | Write to tmp, `os.replace`, guard with filelock |
| 20 | Skeleton update compares zip names (`/`) with `str(Path)` | `templateflow/conf/_s3.py:92-95` | templateflow | Low (Win) | `.as_posix()` |
| 21 | `if not filepath.is_file(): filepath.unlink()` | `client.py:396-397` | templateflow | Low (bug) | `unlink(missing_ok=True)` |
| 22 | `iterables` sanitiser replaces only `os.sep` | `engine/utils.py:599` | nipype | Low (Win) | Replace both separators; strip `*` |
| 23 | `mount` shell probe at import (in every worker) | `filemanip.py:251-255` | nipype | Low | Skip on Windows / make lazy |
| 24 | `os.getuid()` fallback | `plugins/tools.py:51`; `migas/config.py:349` | nipype, migas | Low | Fall back to `'unknown'` |
| 25 | ldd/otool with backticks and `shell=True` | `filemanip.py:829-846` | nipype | Low (fmriprep disables it) | n/a |
| 26 | svgo/cwebp via `shell=True` | `niworkflows/viz/utils.py:62-104`; `nireports/reportlets/utils.py:110-150` | niworkflows, nireports | Low (auto-detected and optional) | argv lists, or Python/Rust minifier |
| 27 | `mri_info` via `shell=True` | `niworkflows/interfaces/freesurfer.py:569` | niworkflows | Low (FreeSurfer-only) | Replace |
| 28 | `/proc` reads (guarded by `exists()`) | `fmriprep/config.py:163-201`, `utils/misc.py:72`; `sdcflows/config.py:164-205` | fmriprep, sdcflows | Low | psutil |
| 29 | Text I/O without `encoding=` (104 `read_text`/`write_text`, ~85 `open`) | e.g. `fmriprep/utils/bids.py:346,502`; `filemanip.py:652` (crash txt) | many | Low–Medium (Win cp1252). pybids reads sidecars as utf-8 (`index.py:326`) | Ship with `PYTHONUTF8=1` / `-X utf8` |
| 30 | HTML anchors use `str(WindowsPath)` | `nireports/assembler/reportlet.py:240-242`; `niworkflows/reports/core.py:202-204` | nireports, niworkflows | Low (browsers normalise `\`) | `.as_posix()` |
| 31 | No subprocess-tree cleanup on Ctrl-C (no process groups or job objects) | absent in plugins | nipype | Medium (Win orphans) | psutil tree-kill / Windows Job Objects |
| 32 | SoftFileLock on nipype data file and log handler | `utils/config.py:208-226`, `external/cloghandler.py:162` | nipype | Low (stale lock after a crash) | n/a |
| 33 | Startup network calls | `fmriprep/config.py:150-155` (rig.mit.edu), `cli/version.py:69,92`, migas, sentry | fmriprep | Low (short timeouts) | Off by default |
| 34 | Import side effects in every spawned worker: `TF_LAYOUT` (pybids indexing of TemplateFlow home), start method, psutil | `fmriprep/config.py:94` reached via `interfaces/bids.py:17` → `utils/bids.py:39` | fmriprep | Medium (startup cost per worker on spawn platforms) | Remove the config import from interfaces; make the layout lazy |

**Not a problem:**
- **Case-insensitive filesystems:** no node names collide case-insensitively within any file of fmriprep, smriprep, niworkflows or sdcflows. The TemplateFlow skeleton (2,648 entries) has no case collisions and its longest relative path is 112 characters.
- **xvfb:** nipype's xvfb support (`utils/config.py:300-360`) is only for `_redirect_x` interfaces. The only one reachable from these repos is `afni.SkullStrip` at `niworkflows/anat/skullstrip.py:96`, which fmriprep never uses.

**Path-depth estimate.** These are Windows paths assembled from real workflow and node names:
- `fmriprep_25_2_wf\sub_01_ses_01_wf\bold_apply_<46-char id>_wf\bold_std_wf\_in_tuple_MNI152NLin2009cAsym.res2\resample\<85-char file>` is **267** characters (directory 183).
- An `_hemi_L` MapNode variant under the fsLR resampling workflow is **303** characters (directory 197).
- The `bold_fit…\bold_hmc_wf\mcflirt\…par` path is **227** characters.
- A short single-session name under `C:\Users\k\work` is **135** characters.

So the 260-character limit is routinely exceeded for multi-entity BIDS names. To verify: as far as I know, the working directory and CreateProcess's `lpCurrentDirectory` stay limited to MAX_PATH even with LongPathsEnabled, which makes nipype's chdir-per-node design a hard limit.

---

## 3. TemplateFlow, pybids and bids-validator

**TemplateFlow** (`python-client/templateflow`):
- Default backend is **S3** at `https://templateflow.s3.amazonaws.com`. On import, if the home directory is empty, a bundled zip of zero-byte placeholder files is extracted (`conf/__init__.py:39-48`, `cache.py` `ensure()`, `_s3.py:20-110`); this needs no network.
- Real files are downloaded lazily by `api.get()` when a file has zero size, streamed with requests and tqdm (`client.py:376-407`).
- **DataLad mode** (`TEMPLATEFLOW_USE_DATALAD`; needs datalad and git-annex, which uses symlinks and works poorly on Windows) is in `cache.py` `DataladManager` and `client.py:359-374`. fmriprep only pulls datalad, datalad-osf and git-annex in its `container` extra and pixi feature. The client itself has no OSF code.
- Environment variables: `TEMPLATEFLOW_HOME` (default `platformdirs.user_cache_dir('templateflow')`, `conf/env.py`), `TEMPLATEFLOW_USE_DATALAD`, `TEMPLATEFLOW_AUTOUPDATE`.
- **There is no explicit offline mode.** It works offline if the home directory is pre-populated with non-empty files. The skeleton is only refreshed from the network by `update(local=False)` (`_s3.py:23-51`).
- `TF_LAYOUT` is an in-memory pybids layout built at import time.
- Portability issues are #18–21 in the table above.

**pybids:**
- Index is SQLite via SQLAlchemy (`layout/db.py:45-78`). fmriprep stores it at `work/<run_uuid>/bids_db/layout_index.sqlite`, resets it unless `--bids-database-dir` is given, and passes `validate=False` (`fmriprep/config.py:485-518`).
- pybids' own validation uses the Python `bids_validator` regex package (`pybids/pyproject.toml` requires `bids-validator>=1.14.7`; `index.py:9,128,175-185`), not the Node tool.
- The `symlink_to` call (`writing.py:285`) is only used with `link_to`, which fmriprep doesn't use.

**Full dataset validation:** fmriprep shells out to the external Node/Deno `bids-validator` CLI in `validate_input_dir` (`fmriprep/utils/bids.py:364-454`). If the tool is missing it prints a message and continues. `--skip-bids-validation` turns it off (`parser.py:213-217,968-980`). Windows issues are #16–17.

---

## 4. Other runtime dependencies

| Dependency | Where | Required? | Recommendation |
|---|---|---|---|
| FreeSurfer licence | discovery `fmriprep/config.py:169-173,480-481`; check `cli/workflow.py:119` → `niworkflows/utils/misc.py:358` | Effectively hard (always runs `mri_convert`) | Make conditional, or remove when FreeSurfer is replaced |
| `$FREESURFER_HOME/subjects/fsaverage*` | `BIDSFreeSurferDir` (`niworkflows/interfaces/bids.py:1376-1439`; `fmriprep/workflows/base.py:84`) | For surface outputs | Source `tpl-fsaverage` from TemplateFlow |
| Tool version probes for the boilerplate | `fmriprep/workflows/bold/stc.py:109`, `hmc.py:84`; `smriprep/.../anatomical.py:780,1443`, `fit/registration.py:122`, `surfaces.py:189,386` | They run binaries at build time | Have the Rust kernels report their versions |
| migas | `fmriprep/utils/telemetry.py:34,189-216`; `run.py:69-72` | Optional (`telemetry` extra) | Default off |
| sentry-sdk | `telemetry.py:33,62-150`; `run.py:67`; auto-disabled if missing (`parser.py:862-871`) | Optional | Default off |
| codecarbon | `run.py:43-57,207` (only with `--track-carbon`), but a **hard** dependency (`pyproject.toml:46`) | Runtime-optional | Move to an extra |
| APScheduler | `pyproject.toml:47`; **zero imports** in any repo, and the installed codecarbon 3.3.1 doesn't require it | Unused | Drop |
| etelemetry / rig.mit.edu ping | nipype dependency; `fmriprep/config.py:150-155` | Unused | Drop |
| graphviz `dot` + pydot | `--write-graph` only (`run.py:109`; `engine/utils.py:1377-1397`) | Optional | Keep optional |
| svgo / cwebp (Node) | auto-detected (`nireports/reportlets/__init__.py:28-29`) | Optional | Python or Rust replacement |
| pandoc | CITATION html/tex (`cli/workflow.py:186-222`); FileNotFoundError tolerated | Optional | Optional |
| bids-validator (Node/Deno) | `utils/bids.py:452` | Optional | Optional |
| xvfb / xvfbwrapper | nipype only | Not needed | Drop |
| datalad / git-annex | `container` extra | Optional | Avoid on Windows; use S3 |
| psutil | free memory at startup (`config.py:182`), resource monitor | Required, cross-platform | Use it to replace `/proc`/sysctl |
| prov, rdflib, lxml, pydot, simplejson, click | nipype dependencies, unused by fmriprep (provenance off) | No | Drop in a vendored engine |
| traits (C extension) | interface specs | Yes | arm64 macOS wheel confirmed in `.venv`; Windows-on-ARM wheels unverified |

---

## 5. Minimal feature set for a replacement engine, and options

**Must have:**
- Nested workflows with `'subwf.inputnode.x'` addressing.
- Typed interface specs (traits or compatible).
- SimpleInterface, Function and IdentityInterface (with pruning), Merge/Select.
- Connect-functions.
- MapNode with zipped iterfields.
- Single-field list iterables with JoinNode (`joinsource`/`joinfield`).
- `run_without_submitting`.
- `mem_gb`/`n_procs`-aware scheduling, with `nprocs`/`omp_nthreads` passed to kernels.
- `needed_outputs` pruning (`remove_unnecessary_outputs`).
- Input-hash caching (timestamp by default, content optional) with resume after a crash.
- Per-node config overrides (crash dir).
- `stop_on_first_crash`.
- Crash files in the txt format the existing readers parse.
- LiterateWorkflow `__desc__` traversal for the boilerplate.
- Linear (debug) and pooled parallel execution.

**Recommended additions:** put the implementation (Rust kernel) version in the hash; store JSON results instead of pickles; use short content-addressed directories with no chdir; schedule on events (`wait(FIRST_COMPLETED)`) instead of the 2 s poll; use a thread pool for GIL-releasing PyO3 nodes and a spawn process pool for Python nodes; kill whole process trees (Windows Job Objects) on cancel.

**Not needed:** synchronize/itersource, nested or serial MapNode, updatehash, provenance, SGE/SLURM/PBS plugins, `status_callback`, xvfb.

fmriprep's `--derivatives` fit/apply reuse (`fmriprep/utils/bids.py:63-130`) already reduces how much it depends on the engine cache.

| Option | Pros (from the code) | Cons (from the code) |
|---|---|---|
| **(a) nipype as-is** | No porting of the ~900 Node calls and 266 connect blocks; crash, report and boilerplate formats all work | Windows blockers #1, #2, #9–13 inside nipype; Linux-only CI; deep directories plus chdir; pickle results; 2 s polling; heavy dependency tree (scipy just for the sparse dependency matrix). Workable for Linux and macOS only |
| **(b) Vendor and patch** | Small, mostly upstreamable patches (#1, 2, 9, 10, 11, 12, 22, 23, plus fmriprep #3, 6, 7, 8, 16, 17); keeps the API and semantics; can be trimmed to about 9–10k lines | Still traits-bound, pickles and deep directories; fork drift |
| **(c) Pydra** | (External knowledge, verify) flat hash-named cache directories (short paths); split/combine covers iterables, MapNode and JoinNode | No migration traces at all, so every workflow and all 178 SimpleInterface specs plus the reporting mixin would be rewritten; no LiterateWorkflow equivalent; crash-file consumers would break; (verify) no memory-aware scheduling like MultiProc's `mem_gb` |
| **(d) Custom engine with a nipype-compatible facade** | Full control over the Windows issues (#13, 15, 31); can reuse nipype's traits specs and keep nipreps workflow code almost unchanged; Rust-aware scheduling | Must faithfully reimplement iterables/JoinNode expansion (`engine/utils.py:941-1175`), `needed_outputs` and the crash format; ongoing maintenance |

**Recommendation:** do it in two phases.
1. **Phase 1 = (b):** vendor a trimmed nipype (`pipeline.engine`, plugins base/linear/multiproc/tools, `interfaces.base`/`utility`, `io.add_traits`, `algorithms.confounds`/`rapidart`, `utils.filemanip`) and apply the patches above. This gets Windows working quickly and serves as a reference implementation.
2. **Phase 2 = (d):** replace `pipeline.engine` and the plugins behind the same `pe.*`/`niu.*` API, keeping the traits-based interface specs at first. Test equivalence against Phase 1 using hashes and outputs on a reference dataset.

Pydra only makes sense if a full rewrite of the nipreps workflows is acceptable anyway.

To measure rather than estimate: the actual node count per subject, which fmriprep logs at `fmriprep/cli/workflow.py:149-151`.
