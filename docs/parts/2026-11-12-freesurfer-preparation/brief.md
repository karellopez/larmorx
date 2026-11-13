You are a relay agent of **FreeSurfer's preparation** (step 3.0 of `docs/freesurfer-plan.md`; larmorx,
github.com/karellopez/larmorx), in the **replica-side role**: you may read FreeSurfer's source, and
nothing you write may ever reach a FreeSurfer clean-room implementer. The plan and its reasons:
`docs/freesurfer-plan.md`, `docs/next-steps.md` ("The order from here"), the decision log's entry
"2026-11-12: the plan to the end". Read them first, with the repository's `CLAUDE.md` and
`docs/porting-guide.md` (its §1, who may read what).

## How this part runs: relay agents (CLAUDE.md "Relay agents and fewer, fuller steps")
- One sub-task per relay; commit; bring `<scratch>/NOTES.md` up to date (done with commits, the exact
  state, the next sub-task; keep any section headed "FROM THE ORCHESTRATOR"); stop with a short report
  (5 to 10 lines).
- **If `<scratch>/NOTES.md` does not exist, you are relay 1**: you run in a fresh harness worktree from
  main; write its absolute path at the top of NOTES.md, and commit early (a relay that commits nothing
  loses its worktree). **Otherwise** work in the worktree NOTES.md names (`cd` into it).
- **Beside you** (about six agents at once, the user's burst; you never touch the same files): the
  confounds' port (`crates/larmorx-confounds`, `lx.confounds`), the documentation revision (the site and
  the documents' wording), and later the foundations' agents of FreeSurfer, who start from your
  inventory. Shared documents only through the handoff (CLAUDE.md, "Shared documents through a handoff").

## The sub-tasks (split one into several relays where it is large; say so in NOTES.md)
1. **The programs.** Every program `recon-all` 8.2.0 runs in its two streams (the default, v8, with
   `FS_V8_XOPTS=1`, and `-no-v8`; `docs/freesurfer-plan.md`, "The version and the files"), from the
   script (`<workspace>/tags/freesurfer-8.2.0/scripts/recon-all`, `distribution/etc/recon-config.yaml`,
   `global-expert-options.v8.txt`) and, as they arrive, from the recordings (below); and the FreeSurfer
   programs fMRIPrep 25.2.x and sMRIPrep run outside it (`docs/analysis/02-anatomical-smriprep-niworkflows.md`,
   the workspace's `fmriprep/`, `smriprep/`, `niworkflows/` clones): each with the options actually used
   and the step that runs it. Scripts recon-all calls (`mri_nu_correct.mni`, `talairach_avi`,
   `pctsurfcon`, `fs-synthmorph-reg`, ...) are programs too, with what they call in turn.
2. **The code each needs.** Per program: its source files in `<workspace>/tags/freesurfer-8.2.0`, the
   shared library functions it reaches (the call graph into `utils/`, `include/` and any other library),
   their size in lines; which functions many programs share (the foundations); the external libraries
   linked (ITK, VXL, Eigen, OpenMP, Python packages, ...).
3. **The third-party audit** (CLAUDE.md rule 3: "Audit each ported file for third-party code first"):
   every file in those call graphs, its licence or origin as its header and FreeSurfer's own notices say
   (FreeSurfer's licence, MNI's, Numerical Recipes, GPL, BSD, public domain, Washington University's
   AVI code, ...), and what each finding means for porting (portable into `larmorx-freesurfer`; a clean
   room only; to be written from a published description). List, do not decide: questions go to
   NOTES.md for the orchestrator.
4. **The data files.** Every file each program reads at run time (atlases `.gca`, `.gcs`, `.tif`,
   `mni305`, colour tables, `fsaverage`, the Talairach atlas, the networks' weights of SynthStrip,
   SynthSeg and SynthMorph and what reads them): its size, its licence and origin, and whether FreeSurfer
   serves it on its own (stable address and checksum) or only inside its 9.6 GB package (look at
   FreeSurfer's public distribution site and its git-annex remote; download nothing large). Into the
   list the plan's rule 4 asks for (`docs/freesurfer/data-files.md`, with a machine-readable table).
5. **Variation.** The programs whose results can depend on the thread count (OpenMP regions with
   reductions or races, ITK's threads) or on a random seed (`setRandomSeed`, environment variables), and
   the floating-point settings FreeSurfer is built with (its CMake files): whether the official programs
   could differ from what the source defines (as GCC 11 did for ANTs, CLAUDE.md rule 8).
6. **The waves and the layout.** The dependency map, the waves of `docs/freesurfer-plan.md` checked and
   corrected, the crates of `crates-freesurfer/` and of the clean rooms (names, what goes where), the queue
   `docs/freesurfer/queue.md` (the order of work for normal weeks: each item a bounded relay-sized task,
   its inputs and its checks), and one **porting card** per program `docs/freesurfer/cards/<program>.md`
   (inputs, outputs, the options recon-all and fMRIPrep use, every option, its source files and shared
   functions, its data files, its variation, an estimate of its size, its wave).
7. **The tools into the repository.** The recorder the orchestrator wrote
   (`<workspace>/tools/freesurfer-oracle/`: `fs_time_recorder.py`, `run_recon.sh`, `install_recorder.sh`)
   into `validation/investigations/freesurfer-oracle/` with a README (what a recording holds, how to run
   one, how to rebuild the subject directory before a step); then **the replay tool**
   (`larmorx_validation`): rebuild the subject directory before step k from a recording, run a program
   there (FreeSurfer's own, or later LARMORX's), and compare files with understanding of MGZ volumes,
   surfaces, curvature, labels, annotations, LTA and `.xfm` transforms, `.m3z` and the text files
   (nibabel reads most of them); FreeSurfer's own runs through the oracle-output cache (`oracle_cache.run`
   with a Policy), one thread by default. Prove it on a few recorded steps: FreeSurfer's step replayed at
   the recording's settings must give the recorded files (or the difference is measured and explained).
8. **The restricted material.** Everything you write from FreeSurfer's source (`docs/freesurfer/`, the
   cards, findings) is closed to FreeSurfer clean-room implementers: say so at the top of each file, and
   write in NOTES.md the exact paths and patterns the clean-room audit's restriction table needs
   (FreeSurfer's clone and tags, the installed package's scripts and Python, `crates-freesurfer/`,
   `docs/freesurfer/`, `docs/findings/freesurfer-*.md`) for the orchestrator, who edits the audit.
9. **The handoff and the final report** (the last relay): `<scratch>/HANDOFF.toml` and `HANDOFF.md` (the
   plan's corrections for `docs/freesurfer-plan.md` and next-steps, `upstream.tsv`'s FreeSurfer 8.2.0
   line, CLAUDE.md's rule 3 if the layout changes), the part's agents' ids in NOTES.md, the final report.

## The reference runs (the orchestrator's, running from tonight)
- **Main machine, the critical path:** six heads, each in both streams, every step recorded, under
  `<workspace>/oracles/freesurfer-8.2.0/runs/<head>-<stream>/` (`RUN_INFO.txt`, `record/steps/*.json`,
  `record/blobs/`, `subjects/<id>/`, `recon-all.out`). FreeSurfer 8.2.0 unpacked under
  `<workspace>/oracles/freesurfer-8.2.0/root/` (its `fs_time` is the recorder; `fs_time.orig` is
  FreeSurfer's). Use them as they finish; never start a full recon-all yourself (single steps through the
  replay tool are fine, boxed).
- **The lab's server:** extras the orchestrator starts; not on your path.

## The machine
FreeSurfer's runs take about 24 threads and up to 60 GiB from tonight. Keep your boxes within **12 GiB and
4 threads** (`scripts/orchestration/boxed.sh`). Long jobs in the background with logs in `oracles/logs/`;
one background wait on the log's final line, with a time limit; never a wait that can match its own
command line; never `pkill -f` with a pattern your own command line holds.

## Rules that stay
- Git: commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge
  into main. `source scripts/orchestration/env.sh` (or a wrapper script); never `maturin develop`. Gates
  before the final commits for what you change (`ruff`, `check_spdx.py`, `tables --check`, the Python
  tests you touch; Rust gates only if you add Rust).
- Public repository: no addresses, account names, host names or keys (placeholders `<workspace>`,
  `<scratch>`, `<lab-server>`). Never delete work: archive under `<workspace>/archive/`.
- Scratch: `<scratch>` = `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/b322ad6c-679d-4079-be53-839676da5898/scratchpad/freesurfer-0`
  (this brief is BRIEF.md there); never write under a `.claude/` folder (your harness worktree itself
  excepted). Keep every tool behind a finding in the repository (`validation/investigations/<topic>/`).
- Explain in plain words; numbers with their units; every claim from the source or a measurement.

## The final report (the last relay only)
The programs per stream and fMRIPrep's; the code (files, shared functions, sizes); the third-party audit's
findings and questions; the data files and their licences and sources; variation; the waves, the layout
and the queue; the replay tool and its proof; the restricted paths; commits; open questions.
