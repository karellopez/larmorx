# Moving to the new main machine: the plan

From 2026-10-27 the new laptop (Alienware m18 R1: AMD Ryzen 9 7945HX, 32 threads, 124 GiB,
Ubuntu 22.04) is the main machine. The old laptop (MSI GE75: Intel i7-8750H, 12 threads, 31 GiB)
becomes a lean helper for speed measurements and extra tests when the main machine is busy
(`docs/helper-machines.md`). The scripts are in `scripts/orchestration/move/`.

**Same user, same paths:** user `karelo`, workspace `/home/karelo/VScode_projects/super_fmriprep`,
so every absolute path in the scripts, records, the oracle cache and Claude Code's notes stays
valid. Nothing is cloned again: the repositories move with their history and unpushed commits.

## The plan at a glance

| Phase | What | Who, when | Status (2026-10-27) |
|---|---|---|---|
| 1 | Prepare both laptops: SSH keys both ways, packages, the FreeSurfer licence, glibc | the user (`sudo` lines) and the old laptop's session | **done** |
| 2 | The copy: what the work needs (below), about 160 GiB; a final pass that compares every file's content; the check | by itself on the new laptop, over Wi-Fi (about 11 hours) | running |
| 3 | The new laptop's first session: settings, build, the proof that results do not depend on the machine, CLAUDE.md, push | the new session ("step 8" below), whatever the first message says | after 2 |
| 4 | The old laptop cleaned to a helper (the list below, approved by the user) | the new session over SSH, only after the proof | after 3 |
| 5 | The newest versions: FSL's newest official build and AFNI's newest release installed on the new laptop, SciPy, NumPy and OpenBLAS checked (the version-update part, decision log 2026-10-27); the old laptop gets them afterwards for its speed measurements | an agent part on the new laptop | after 4 |

## Phase 1: done

- SSH with keys, no passwords stored: new -> old as `old-laptop` (10.2.167.242, Wi-Fi; in the new
  laptop's `~/.ssh/config`), old -> new as `karelo@10.2.165.26` (the new laptop's wired address).
- On the new laptop the user installed (sudo): openssh-server, tmux, git, rsync, build tools,
  Apptainer 1.4.5, python3.12 from deadsnakes (`/usr/bin/python3.12`, the copied `.venv`'s base:
  3.12.16 there, 3.12.10 here), glibc upgraded to the old laptop's 2.35-0ubuntu3.15, the
  FreeSurfer licence (`/usr/local/freesurfer/license.txt`, never committed). The deadsnakes source
  and key were copied from the old laptop (`add-apt-repository` hung on Launchpad).
- On the new laptop, moved aside (never deleted) to `~/archive-before-move-2026-10-27/` (README
  inside): the user's own `.venv` (uv's Python, pip only) and FSL 6.0.7.10 in `~/fsl`; the
  workspace's `.vscode/settings.json` kept.
- A first copy was stopped and its partial pieces that the plan does not copy were removed from
  the new laptop (a fragment of `archive/`, the agents' worktrees); its correct parts stay and are
  checked by the final pass.

## Phase 2: the copy

**Copied** (about 160 GiB; `pull.sh`):

| What | Size | Why |
|---|---|---|
| fMRIPrep 25.2.6's fourteen runs (`oracles/fmriprep-25.2.6`) | 75 GB | the tests take fMRIPrep's real images and commands from them (the data panel); the proof re-runs the ANTs panel records on them; about two days to compute again and not exactly repeatable |
| The oracle cache (`oracles/cache`) | 40 GB | the stored runs of the originals; on the new CPU they are not hits (the key includes the CPU), but `verify` re-runs them there: the direct proof that the originals compute the same on the AMD CPU |
| The website's evidence data (`oracles/figures-cache`) | 8 GB | needed to update the tools' pages without computing everything again |
| fMRIPrep's container (`oracles/images`) | 4.8 GB | one comparison mode runs the originals inside fMRIPrep's own image; byte-identical copy (a new pull could differ) |
| The original programs | 12.6 GB | ANTs 2.6.5 (GCC 14.3 build), AFNI 25.2.09, CORE-MATH preload, the GCC 14.3 toolchain, FSL 6.0.7.17 (`~/fsl`, the current records' FSL: kept until the version update has compared it with the newest) |
| mcflirt's comparison runs (`oracles/fsl-6.0.7/mcflirt`, without the raw band runs) | 1.8 GB | read by the mcflirt replica's and hmc's records; `band/band.json` included |
| The GCC 11 ANTs build's programs, build info, logs and thread band | 0.9 GB | the evidence of the compiler bug (GCC bug 110182) |
| Test inputs (`bids-views`, `inputs`, `downloads`, `templateflow`, `difftest`, `logs`) | 2 GB | inputs of the suites |
| Code and environments | 16 GB | the three repositories (history, branches, unpushed commits), `tags/` (the exact sources ported), the upstream clones, `.venv`, Rust, Node 22, the test data (`~/.cache/larmorx-testdata`), `tools/`, `CLAUDE.md`, `ORCHESTRATOR-STATE.md`, Claude Code's memory notes and transcripts of this project, the usage tool's records |
| The archive's documents | under 1 GB | from `archive/`, only the documents: notes, reports, READMEs, scripts, source drafts, logs, tables and the agents' conversation records (text files under 1 MB; without the copies of third-party code: AFNI's and fMRIPrep's sources, Python environments, SciPy). Among them 2,332 distinct scripts of the agents that are in no repository: triaged into `validation/investigations/` after the move (next-steps) |

**Not copied** (`manifest.py` `LEFT_ON_OLD`; deleted from the old laptop in phase 4 if approved):

| What | Size | Why not |
|---|---|---|
| Build caches (`target/`) | 41 GB | rebuilt in minutes on the new laptop |
| The agents' worktrees (`larmorx/.claude/worktrees`) | 2 GB | all 46 are clean and merged into main (checked 2026-10-27): their work is in git; `git worktree prune` on the new laptop removes their entries |
| `archive/`'s bulky temporary data | 54 GB | test outputs, copies of the cache, clones and large reference tables of past sessions; their results are committed |
| `oracles/compare` | 22 GB | the 2026-10-12 stage comparisons with fMRIPrep, partly made with the GCC 11 ANTs build; redone at the end with the newest fMRIPrep |
| mcflirt's raw band runs | 18 GB | the records read only their summary `band/band.json` (copied); the FSL update measures the band again |
| The GCC 11 build's build folders | 3.4 GB | object files; its programs are copied |

**Commands** (on the new laptop; resumable):

```bash
bash /tmp/pull.sh old-laptop            # the copy (tmux session "move"); run again if it stops
bash /tmp/pull.sh old-laptop --verify   # the final pass: compares every file's content
python3 /home/karelo/VScode_projects/super_fmriprep/larmorx/scripts/orchestration/move/check.py
```

`second-pass.sh` (tmux session `move2`) does the last two by itself when the copy ends, sets up the
shell (Rust on `PATH`, Node 22 as nvm's default) and writes `~/MOVE-READY.txt` (the copy's and
the check's result) and `~/move-check.log`. `check.py` compares the new laptop with
`MOVE-MANIFEST.json`: every original program's SHA-256 (1,452), the system libraries the originals'
maths depends on, the tools' versions, each copied tree's files and size.

## Phase 3 ("step 8"): the new laptop's first session

The new session starts here, in this order, whatever the user's first message says:

1. **Where am I.** `hostname` must print `karelo-Alienware-m18-R1-AMD` (the old laptop is
   `karelo-GE75-Raider-8SF`). The session-start checks of CLAUDE.md: `uname -m`, `nproc`, free
   memory, `apptainer --version`, `rustc --version`.
2. **Usage.** `python3 tools/claude-usage/usage.py check` (as before every part). It must print
   GO, SLOW DOWN or PAUSE with the windows read "0 min ago"; "could not read the limits" means
   Claude Code's program was not found or is not signed in. SLOW DOWN or PAUSE: do 3 to 5
   (machine time, few tokens) and leave 8 for later.
3. **Claude Code's settings.** Done by the old laptop's session on 2026-10-27 (the user asked):
   `~/.claude/settings.json` there keeps its own keys and has `statusLine`,
   `subagentPromptCacheTtl: "1h"` and the attribution off (`"attribution": {"commit": "", "pr": "",
   "sessionUrl": false}`); its earlier version is in `~/archive-before-move-2026-10-27/`. check.py
   verifies the attribution and the commit-msg guard in each repository.
4. **The copy's result.** Read `~/MOVE-READY.txt` and `~/move-check.log`: every FAIL fixed, each
   WARN fixed or named in the proof as a possible cause. `git worktree prune` in `larmorx/`.
5. **Build and prove** (machine time; boxes up to about 100 GiB on this machine):
   - **Build:** `source larmorx/scripts/orchestration/env.sh`, then in `larmorx/`:
     `maturin develop --release` (main checkout only), `scripts/orchestration/build-gpl.sh .`,
     `scripts/orchestration/build-nc.sh .`; the gates (`gates-rust.sh . all`, `gates-python.sh .`).
   - **The proof** (next-steps, item M): the results do not depend on the machine.
     - `pytest tests/parity` (the smoke tier) and the golden suite: unchanged checksums.
     - Every record's standard tier re-run (`python -m larmorx_validation parity <suite> --tier
       standard`, boxed), compared case by case with the committed records
       (`scripts/orchestration/compare_records.py`): identical results, timings aside.
     - The originals themselves: a sample of the oracle cache re-run and compared
       (`python -m larmorx_validation oracle-cache verify`). The cache's key includes the CPU
       (model and flags: glibc picks its maths kernels by them), so the old laptop's entries are
       never hits here: every original runs again and fills its own entries. `verify` re-runs an
       entry whatever the machine: the direct check that the originals compute the same on the
       AMD CPU. If a large sample is identical, propose narrowing the key's machine part
       (next-steps, "The oracle cache across CPUs").
     - Any difference is a finding (`docs/findings/`), explained before new work.
     - The new machine is AMD (AVX-512), the old one Intel (AVX2): runtime SIMD dispatch takes
       other paths here, so the proof covers that too.
6. **The machine's settings in CLAUDE.md** ("Working on this machine", "Machines", the build-cache
   seed line): 32 threads and 124 GiB (one agent may use up to 28 threads, parity `--jobs` to
   match; boxes together up to about 100 GiB); the old laptop as the helper, reached by SSH
   (`docs/helper-machines.md`; add the `old-laptop` git remote).
7. **Push** what the old laptop committed and did not push (named in `ORCHESTRATOR-STATE.md`),
   after the gates and the proof: GitHub sign-in through VS Code (the askpass socket, as before).
8. **Then phase 4**, then the queued work: the panel's fourteen whole template registrations
   (`parity ants-registration --tier panel --case 'register_template.*/registration$'`, about 67
   hours of CPU, several at once) and antsRegistration's standard record re-made (513 cases); the
   25 heavy random-testing cases; the small part for antsAI and antsAffineInitializer; the version
   update (phase 5); the data panel's part (b) on the new versions (`docs/next-steps.md`). The
   Windows laptop's SSH when the user asks (`docs/helper-machines.md`).

## Phase 4: the old laptop cleaned to a helper

Only after phase 3's proof has passed, and only what the user approved. The new session writes
the deletions as a script with this exact list, runs it first as a dry run (it prints every path
and size), then for real, and records what it freed.

**Deleted from the old laptop** (about 206 GB; **approved by the user on 2026-10-27**, with the two
choices below):

| What | Size | Why it can go |
|---|---|---|
| The agents' worktrees and their build caches (`larmorx/.claude/worktrees`) | 43 GB | all clean and merged into main; the main checkout keeps its own build |
| The sessions' scratch in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/` | 56 GB | temporary files; what mattered is committed or in the archive's documents |
| `archive/` | 55 GB | its documents are on the new laptop; the rest is temporary data |
| `oracles/compare` | 22 GB | superseded stage comparisons, redone at the end |
| mcflirt's raw band runs | 18 GB | summarised in `band.json`; re-measured by the FSL update |
| The GCC 11 ANTs build (`oracles/ants-2.6.5-gcc11-O3`) | 4.3 GB | its evidence is on the new laptop |
| The website's evidence data (`oracles/figures-cache`) | 8 GB | site work happens on the new laptop |

**What the old laptop keeps** (its helper kit): the `larmorx` main checkout and its build,
`.venv`, Rust and Node; every original program (ANTs, AFNI, FSL, CORE-MATH, the toolchain, the
fMRIPrep image); the test data; its own oracle cache (keyed to its CPU, so tests there reuse it);
the fMRIPrep runs (so it can run any test, the data panel included). (Approved 2026-10-27, as is keeping FSL 6.0.7.17 on the
new laptop until the version update has compared it with the newest.) Never touched: the user's
own files outside the workspace (other projects, conda, `~/ants`, ...).

## Phase 5: the newest versions

The version-update part (decision log 2026-10-27), on the new laptop: the upstream watch first;
FSL's newest official build installed with FSL's installer beside 6.0.7.17; AFNI's newest release
compiled from its tag beside 25.2.09; SciPy, NumPy and OpenBLAS checked against their newest;
changes ported; records re-made. Then the old versions leave the new laptop (FSL 6.0.7.17 once the
update has compared the two), and the old laptop receives the new programs for its speed
measurements (`rsync` from the new laptop).

## Afterwards

- Work continues only on the new machine; the old laptop serves speed measurements and extra
  tests (`docs/helper-machines.md`).
- `MOVE-MANIFEST.json` (workspace root, not in git) records the old machine's facts and every
  oracle program's SHA-256; `check.py` can be run again at any time.
