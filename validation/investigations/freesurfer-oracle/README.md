# FreeSurfer's reference runs, every step recorded, and replayed one step at a time

FreeSurfer 8.2.0's `recon-all` runs about 190 programs one after the other, each reading what the
earlier ones wrote into the subject's directory. To compare a port with FreeSurfer program by
program, the reference runs record every step: what it ran, the subject directory as it was just
before, and what it wrote. Any step can then be run again alone, by FreeSurfer or by LARMORX, in a
copy of the directory rebuilt as it was before that step (`docs/freesurfer-plan.md`, step 3.0).

The scripts here are the orchestrator's (written 2026-11-12, kept byte-identical to the copies that
made the reference runs: the recorder's SHA-256, `e510994e9f67891f41ebd4f85c37864071d53e49e265fcaf876934be59b4b4e1`,
is the one every run's `RUN_INFO.txt` names). The replay tool is `larmorx_validation.freesurfer`
(below).

| File | What it does |
|---|---|
| `fs_time_recorder.py` | stands in for FreeSurfer's `fs_time`, through which `recon-all` runs each step; it records the step around the call (standard library only, silent) |
| `install_recorder.sh <freesurfer home>` | puts the recorder in place of `bin/fs_time` (FreeSurfer's own kept as `bin/fs_time.orig`); without `LARMORX_FS_RECORD` set, the recorder only runs `fs_time.orig`: the installation behaves as shipped |
| `run_recon.sh <freesurfer home> <licence> <T1w> <subject> <out> <threads> [recon-all options]` | one head: `recon-all -s <subject> -i <T1w> -all -openmp N -itkthreads N` (never `-parallel`, so each change belongs to one step), recorded; writes `RUN_INFO.txt` |
| `run_queue.sh <freesurfer home> <licence> <jobs.tsv> <runs dir> <parallel> <threads>` | a list of heads, `<parallel>` at a time, skipping finished ones; ends with `QUEUE-DONE <failed>` |
| `retry_failed.sh` (the same arguments) | moves the runs that did not end with `EXIT 0` to `<runs>/../failed/<run>-<time>/` (kept: `recon-all` refuses an existing subject) and runs the queue again |

`jobs.tsv` holds one head per line, tab-separated: the run's name, the T1w, the subject's name and
recon-all's extra options (`-no-v8` for the classic stream; nothing for 8.2.0's default, v8).

## Running one

```
<repo>/validation/investigations/freesurfer-oracle/install_recorder.sh <freesurfer home>
<repo>/scripts/orchestration/boxed.sh 24 \
    <repo>/validation/investigations/freesurfer-oracle/run_recon.sh \
    <freesurfer home> <licence file> <T1w> sub-01 <workspace>/oracles/freesurfer-8.2.0/runs/<run> 4 -no-v8
```

`<freesurfer home>` is the unpacked package (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0`
for the reference runs); `<licence file>` is read in place, never copied into a run. A whole head
takes 1.5 to 3 hours at 4 threads; the v8 stream's `mri_synthseg` needs about 23 GiB and
SynthMorph's deformable step (`fs-synthmorph-reg`) about 18 GiB, so the box must allow them.

## What a recording holds

```
<run>/RUN_INFO.txt                       the command, the T1w and its SHA-256, FreeSurfer's build stamp,
                                         the package's and the recorder's SHA-256, threads, CPU, OS,
                                         start and end; the last line "EXIT <status>"
<run>/recon-all.out                      recon-all's terminal output
<run>/subjects/<id>/                     the finished subject directory (recon-all's own; with its links)
<run>/subjects/fsaverage                 the link recon-all makes to FreeSurfer's fsaverage
<run>/record/steps/NNNN-<program>.json   one file per step recon-all ran through fs_time
<run>/record/blobs/<aa>/<sha256>         every file's content, once, by its SHA-256
<run>/record/state.json                  the recorder's own state after the last step
```

A step's file:

| Key | What |
|---|---|
| `index`, `argv`, `cwd` | its number, its command line (the program first, as recon-all wrote it), its working directory |
| `env` | `OMP_NUM_THREADS`, `FS_OMP_NUM_THREADS`, `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, `FS_LICENSE`, `FREESURFER_HOME`, `SUBJECTS_DIR` as the step had them |
| `start`, `end`, `seconds`, `exit` | its times and exit status |
| `before` | the subject directory just before the step: every file's path and SHA-256 |
| `outputs`, `deleted` | the files it wrote with new content (path and SHA-256), the files it deleted |
| `between`, `between_deleted` | what changed between the previous step and this one: what `recon-all` did itself (its `touch` files, links, copies) and the programs it ran without `fs_time` |

The state after a step is `before` with `outputs` applied and `deleted` removed. The run script
records a last pseudo-step, `NNNN-final.json` (`argv` `["--larmorx-final"]`), with what changed after
the last step.

What a step is, and what is left out:

- **One step per top-level `fs_time` call.** A script `recon-all` runs as one step (`rca-talairach`,
  `fs-synthmorph-reg`, `mri_nu_correct.mni`, `talairach_avi`, `seg2cc`, `label-cortex`, ...) is one
  step with everything its own programs wrote: the calls it makes through `fs_time` inside run
  `fs_time.orig` directly (`LARMORX_FS_NESTED`) and get no file of their own. A program inside a
  script is replayed by replaying the script's step, or by rebuilding before it and running it by
  hand with the arguments the script's log shows.
- **Programs `recon-all` runs without `fs_time`** appear only as the next step's `between`.
- **Left out of the subject directory:** the top-level `tmp/`, and `scripts/recon-all.log`,
  `recon-all-status.log`, `recon-all.cmd`, `recon-all.done`, `recon-all.error`,
  `recon-all.local-copy` (recon-all's copy of itself), `build-stamp.txt` and `IsRunning*`, which
  change at every step. `scripts/recon-all.env`, `recon-config.yaml` and the logs the programs
  write are kept.
- **Links are recorded as the files they name** (their content). The finished subject directory
  still has them; the rebuild makes them again where the state before the step allows it.
- A file rewritten with the same content is not an output (FreeSurfer's provenance records make
  that rare: every volume and surface it writes names its time).

**The recordings stay private** (outside git, like every oracle output): they hold the machine's
paths, the user's and the machine's names in FreeSurfer's provenance records, and
`scripts/recon-all.env`, a dump of the whole session environment `recon-all` ran in.

## Rebuilding the subject directory before a step, and replaying it

```
cd <repo>
export PYTHONPATH=<repo>/python:<repo>/validation/src
python -m larmorx_validation freesurfer-replay steps ds000005-sub-01-v8
python -m larmorx_validation freesurfer-replay rebuild ds000005-sub-01-v8 mri_normalize <dest>
scripts/orchestration/boxed.sh 12 python -m larmorx_validation freesurfer-replay run \
    ds000005-sub-01-v8 11 mri_fill mri_label2label:3 --threads recorded --json <scratch>/replay.json
python -m larmorx_validation freesurfer-replay compare <reference file> <replayed file> --pair OLD=NEW
```

A run is a recording's directory or its name under `<workspace>/oracles/freesurfer-8.2.0/runs/`; a
step its number or `program[:n]` (the n-th step that runs it).

`rebuild` makes `<dest>/subjects/<id>/`:

1. every file of the step's `before`, copied from the blobs (copies, never links to the blobs: a
   program that writes a file in place must not change the recording);
2. the directories that existed before the step: those of the finished run whose birth time
   (GNU `find`'s `%B`, from Linux's `statx`) is before the step's `start`, and every file's own;
3. the links of the finished subject directory, where both the link and the file it names are in
   `before` with the same content; and the links beside the subject (`subjects/fsaverage`) born
   before the step;
4. the step's command line and working directory with the recorded subjects directory replaced by
   `<dest>/subjects`, the program by its name;
5. the environment: `recon-all`'s own as `scripts/recon-all.env` recorded it (FreeSurfer's
   variables, `PATH`, `LD_LIBRARY_PATH`, the locale, the shell's, the MNI tools'; not the rest of
   the session, and never the recorder's `LARMORX_*`), the step's six variables, `SUBJECTS_DIR`
   and `PWD` for the new place, and the thread count.

**Threads.** One by default (`--threads 1`): the three thread variables and the programs' thread
options (`--threads`, `-threads`, `-nthreads`, `-openmp`, `-j`) are set to 1. `--threads recorded`
keeps the recording's settings (the reference runs: 4), as the proof below needs.

`run` runs FreeSurfer's own program in the rebuilt directory **through the oracle-output cache**
(`oracle_cache.run`, docs/validation/oracle-cache.md): the program is started by a launcher,
`<workspace>/oracles/inputs/freesurfer-replay/launchers/<tree>/<program>/<program>`, beside a
`PACKAGE.json` naming the whole installation (its build stamp, the package's SHA-256, the SHA-256
of a manifest of all its 70,117 files: 19 GB hashed once, remembered by size and time), so a run's
key covers the data files and scripts the program reads besides itself; every file of the rebuilt
subject directory is an input, by content; the subjects directory is the place it writes; the
environment family `freesurfer` (FreeSurfer's, MNI's, OpenMP's and ITK's variables and `PATH`) is
in the key; `Policy.formats = "freesurfer"` makes the cache compare a fresh run with its entry
provenance aside. At one thread a run is served from its first run; above one thread after three
identical runs (the cache's rule). `--command PREFIX` runs another program instead (LARMORX's,
later), uncached: `--command "larmorx-freesurfer {program}"`.

Then it compares the subject directory after the run with the recorded state after the step:
every file either side changed, by `larmorx_validation.freesurfer.files`. FreeSurfer writes
provenance into most files (a command-line record with the time, the user and the machine in every
volume and surface, a surface's "created by" stamp, the run's paths, the header lines of LTA and
statistics files); these are left out, and what still differs is measured (voxels, vertices,
values, labels, lines). Verdicts: `identical`, `provenance` (the same result), `log` (a log's text
differs beyond times and dates: messages, not results), `differs`, `missing`, `extra`. A step is
**reproduced** when every file is `identical` or `provenance` (or `log`) and the exit status is the
recorded one.

The replays' directories are `<workspace>/oracles/inputs/freesurfer-replay/work/<run>/<step>-t<threads>/`
(regenerable; a directory without the replay's marker file is never emptied).

## The proof

FreeSurfer's steps replayed at the recordings' settings: `docs/validation/freesurfer-replay.md` (the
numbers, the steps, what differed and why), made with the `run` command above.
