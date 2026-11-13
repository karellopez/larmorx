# FreeSurfer: the plan

Agreed with the user on 2026-11-12 (decision log, "2026-11-12: the plan to the end"). Every
FreeSurfer program `recon-all` 8.2.0 runs in its two streams (below), and the FreeSurfer programs
fMRIPrep and sMRIPrep run outside it, each ported twice: a **replica** in `larmorx-freesurfer`
(FreeSurfer's licence) and a **clean room** in the main package (Apache-2.0), both complete
(every option, message, exit status and failure case, with D13's switches), and `recon-all`
itself in both. The order of the whole project is in [next-steps.md](next-steps.md), "The order
from here".

## The version and the files

- **FreeSurfer 8.2.0** (D5, decision log 2026-10-26). Its source: `<workspace>/tags/freesurfer-8.2.0`
  (GitHub's `v8.2.0` archive, SHA-256 `bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`
  in `.upstream-sha256`; the atlases are not in it). Its programs: the official
  `freesurfer_ubuntu22-8.2.0_amd64.deb` (9.6 GB; FreeSurfer 8.2.0 ships no tarball) unpacked with
  `dpkg-deb -x`, no administrator rights, under `<workspace>/oracles/freesurfer-8.2.0/`; the same
  file on every machine that runs it, its SHA-256 in each run's `RUN_INFO.txt`.
- **The official build** (measured on the package by the preparation; details in `docs/freesurfer/`,
  replica side): every program names one compiler, GCC 8.4.0 (`Ubuntu 8.4.0-3ubuntu2`), the 4dfp tools
  included (GNU Fortran, not Intel's), and links ITK 4.13.2 (not 5.4.5; `tags/ITK-v4.13.2`) and, where
  it uses them, the reference Netlib BLAS and LAPACK. GCC 8.4.0 keeps the float roundings in N4's B-spline
  fit that GCC 11 drops (CLAUDE.md's GCC 11 note does not apply to FreeSurfer's programs). sMRIPrep's and
  fMRIPrep's calls of `mri_robust_register`, `mri_coreg` and `mri_robust_template` give the same files at
  1, 2 and 4 threads.
  FreeSurfer's post-release update for 8.2.0 (`patch_data_20260625.tgz`) changes only how `recon-all` and
  `rca-surfreg` read expert-option files, and no data file: the reference runs stay on the package as
  released; the ports follow the newest version.
- **Two streams.** `recon-all` 8.2.0 runs, by default, the **v8 stream**: skull stripping by SynthStrip, the
  labels by SynthSeg and the registration by SynthMorph, neural networks (pretrained models: Rust inference with
  the original weights, decision log 2026-10-19), with several new correction steps. Its `-no-v8` flag gives the
  **classic stream**, what fMRIPrep 25.x's FreeSurfer 7.3.2 runs (`mri_watershed`, the Talairach chain,
  `mri_ca_label`, ...). Most programs, the surface steps among them, are shared by both; both correct the intensity
  with ITK's N4 (`AntsN4BiasFieldCorrectionFs`, which LARMORX's N4 replica may serve). The programs of each stream,
  measured on the recordings: `docs/freesurfer/programs.md` (replica side). **The order (the user, 2026-11-12):** the
  programs both streams share first, then the classic-only ones (fMRIPrep's), with the v8-only networks as a
  parallel track; FreeSurfer 7.3.2 is not ported (a reference for the final comparisons only).
- **The licence key** is a check inside FreeSurfer's own programs: the oracle runs read the user's
  licence file in place (the user, 2026-11-12: also on the lab's server), never copied into results
  or repositories; LARMORX's ports need none (decision log 2026-10-19).

## 3.0 Preparation

1. **The reference runs, every step recorded.** `recon-all` runs each program as `$fs_time <program>
   <args>`. A stand-in for `fs_time` (`fs_time_recorder.py`, with `run_recon.sh` and
   `install_recorder.sh`; written in `<workspace>/tools/freesurfer-oracle/`, committed under
   `validation/investigations/freesurfer-oracle/` by the preparation part) records, around each step,
   its command line, working directory, thread settings, times and exit status, the subject
   directory's state before it, what it wrote and deleted, and what changed between steps (commands
   recon-all runs without `fs_time`). Every file's content is kept once by SHA-256, so the subject
   directory before any step can be rebuilt. Runs use `-openmp 4 -itkthreads 4` and never
   `-parallel` (steps one after the other, so each change belongs to one step); each head is run in
   both streams (the default, v8, and `-no-v8`). Without the recording switch the stand-in only runs
   FreeSurfer's own `fs_time`.
2. **Where they run** (the user, 2026-11-12: "the server for now should serve us for complementary
   information"):
   - **The main machine, the critical path:** six heads chosen for their variety, everything the
     agents need: ds000005 sub-01 (a healthy adult, 3 T), ds002422 sub-02 (an older adult, 1.5 T),
     ds000228 sub-pixar008 (a 3.5-year-old), ds005454 sub-16 (7 T, MP2RAGE, `acq-denoised`),
     ds005003 sub-RS001 (a glioma), ds004173 sub-603309 (an older adult, `acq-headmotion2`).
   - **The lab's server, extras only** (64 threads, the user's rule; nothing on the development path
     waits for it): the rest of the data panel, one-thread runs of every head, repeated runs (FreeSurfer's
     own variation between thread counts and between machines), fMRIPrep 25.2.6 with recon-all (its
     FreeSurfer 7.3.2) for the final comparisons, later the heavy random-testing cases and the final
     validation phase's batches. Whatever a record ends up using is copied to the main machine first.
     How jobs run there: the workspace's private notes (`ORCHESTRATOR-STATE.md` names them); never its
     name, address or account in a repository.
3. **The inventory** (one replica-side agent, which may read FreeSurfer's source): every program the
   two streams and fMRIPrep run, from the script and the recordings, with the options used; per
   program its source files, the shared library functions it calls (the call graph) and their size;
   every third-party file in that code and its licence (CLAUDE.md rule 3: audit before porting); every
   data file each program reads (atlases, colour tables, `fsaverage`), its licence, source, size and
   whether FreeSurfer serves it on its own; the programs whose results depend on the thread count or a
   random seed; the waves and their order; one **porting card** per program (inputs, outputs, the
   options recon-all and fMRIPrep use, every option, source files, dependencies, an estimate of its
   size). **Done** (2026-11-12, nine relays): `docs/freesurfer/` (its README is the index: the programs,
   the code, the third-party audit with the placement table, the data files, variation, the waves and
   crates, the queue `docs/freesurfer/queue.md` with stable item numbers, one card per program in
   `cards/`, and their machine-readable tables), made by the tools in
   `validation/investigations/freesurfer-inventory/`; all of it closed to FreeSurfer clean-room
   implementers, whose specs carry what they need.
4. **The replay tool**, one for every program: it rebuilds the subject directory before a step from
   a recording, runs LARMORX's program (replica or clean room) there, and compares its files with the
   recorded ones and with FreeSurfer's same step run again on the main machine at one thread (through
   the oracle-output cache), with comparisons that understand MGZ volumes, surfaces, labels,
   annotations, transforms and the text files. Each program then has real-data tests on every
   recorded head with no new full runs. It replaces "the templates" part's replay harness (the user,
   2026-11-12). **Done** (2026-11-12): the recorder in `validation/investigations/freesurfer-oracle/`, the
   replay tool `larmorx_validation.freesurfer` (`python -m larmorx_validation freesurfer-replay
   {steps,rebuild,run,compare}`, FreeSurfer's runs through the oracle-output cache, one thread by default);
   proved on 68 recorded steps of both streams (38 v8 steps of one head, 15 classic steps, 7 long v8
   steps, 4 of a second head, 3 at one thread, `mri_ca_register`'s warp): every one reproduced, one where
   FreeSurfer itself varies (`defect2seg`'s colour table) and one with a message that varies
   (`docs/validation/freesurfer-replay.md`). A FreeSurfer clean-room implementer runs it and never reads
   it (the clean-room audit).
5. **The data files:** the four rules and the list of every data file ("Data files", below).

## The code's layout

- **Replicas:** a workspace `crates-freesurfer/` (FreeSurfer's licence, the `larmorx-freesurfer` wheel
  and program), several crates so agents work in parallel without touching the same files: the core
  (file formats, the image and surface structures, FreeSurfer's maths and random numbers), the atlas
  code (GCA, GCA morph), the surface library, and the programs in groups. It builds the Apache crates
  by path, never `crates-gpl` or `crates-nc`. CLAUDE.md's rule 3 names it (the user, 2026-11-12). Its
  crates (`docs/freesurfer/waves.md` §4; the names decided 2026-11-12, W2): the libraries in layers,
  `larmorx-freesurfer-core`, `-io`, `-volume`, `-filter`, `-surf`, `-deform`, `-defect`, `-gca`,
  `-robust`; one program crate per wave, `-volumes` (3.2), `-atlas` (3.3), `-topology` (3.4), `-cortex`
  (3.5), `-classic` (3.6), `-networks` (N), `-recon` (3.7); and `larmorx-freesurfer-cli` (the program and
  wheel).
- **Two borrowed pieces' replicas in the other companions** (the user, "No waiting for permissions"):
  `crates-gpl/larmorx-gpl-gsl` (GSL's generators, alone in `larmorx-gpl`; no FreeSurfer program links it:
  `mri_coreg`'s and `mris_expand`'s replicas use the clean-room generator) and
  `crates-nc/larmorx-nc-tkregister2` (`tkregister2_cmdl`, its own program in `larmorx-nc`, no FSL code;
  `bbregister`'s replica runs it when `larmorx-nc` is installed, the clean room otherwise).
- **Permissive pieces** FreeSurfer bundles or relies on (VXL's routines, the readers of GIFTI, MINC's
  `.xfm`, NIfTI and DICOM, nlohmann's JSON, ITK's filters, the networks' inference core, the x87 80-bit
  arithmetic of its `long double`, ...) are ported from their own upstreams into the Apache crates
  (`larmorx-core`, `larmorx-io`, `larmorx-image`, a new `larmorx-nn`); their clean rooms come with the
  roadmap's step 6 (queue CP-01 to CP-13).
- **Clean rooms:** crates of the main package, `larmorx-fs-core`, `larmorx-fs-gca`, `larmorx-fs-surf` and
  one program crate per wave (`larmorx-fs-volumes`, ..., `larmorx-fs-recon`), with clean-room files in
  `larmorx-io` (FreeSurfer's formats) and `larmorx-core` (the random generator and distributions, the
  cubic spline); created, empty, by the first FreeSurfer clean-room relay from a clean brief (queue
  C3.1-00), never by an agent that read FreeSurfer's material; each added to the clean-room audit's
  lists in the part that creates it (the porting guide; a test fails otherwise). The main package's
  plumbing (`larmorx-freesurfer` in `replica.rs`, the `freesurfer` family of the `larmorx` program,
  `lx.freesurfer`) is queue item 3.2-41, best written by such a clean agent too.
- MGH/MGZ, ANALYZE and `.mat` reading moves from `larmorx_tedana::formats` into `larmorx-io` with the
  foundations (t2smap's follow-up).

## The waves

| Wave | What (the programs per wave: `docs/freesurfer/waves.md` and the queue, replica side) | Queue items (replica / clean room) |
|---|---|---|
| 3.1 Foundations | the file formats, the image and surface structures, FreeSurfer's maths, random numbers and messages; x87 80-bit arithmetic; the borrowed pieces of the placement table (GSL's generators alone in `larmorx-gpl`, Burkardt's spline, and the clean-room random generator and spline the replicas use); the VNL routines and C-library behaviours the replicas need; the unit oracle (done) | 52 / 8 |
| 3.2 Volumes | the volume programs both streams run (`mri_convert`, `mri_info`, `mri_nu_correct.mni` with ITK's N4 `AntsN4BiasFieldCorrectionFs`, `AntsDenoiseImageFs`, `mri_normalize`, `mri_segment`, `mri_fill`, `mri_vol2vol`, ...), and fMRIPrep's `mri_coreg`, `mri_robust_template` and `mri_robust_register`, early, with EPFL's B-splines and the robust-registration library as items of their own; the main package's plumbing (3.2-41) | 41 / 30 |
| 3.3 Atlas | the atlas programs both streams run (`mri_em_register`, `mri_ca_normalize`, `mri_ca_register`) | 17 / 4 |
| 3.4 First surfaces | tessellation to the fixed, remeshed surface (with `mris_topo_fixer`, `mris_remesh`, `defect2seg` and the programs it runs) | 33 / 16 |
| 3.5 Final surfaces and statistics | placement, registration, parcellation, statistics (`mri_relabel_hypointensities` among them); fMRIPrep's surface programs and `bbregister` (with `mri_segreg` and `tkregister2_cmdl`: its replica in `larmorx-nc`) | 37 / 29 |
| 3.6 The classic-only programs | `mri_watershed`, the Talairach chain (`talairach_avi`, `mpr2mni305` and the four 4dfp programs: a clean room only, its atlases fetched), `talairach_afd`, `mri_ca_label` | 21 / 7 |
| N The v8 networks (a parallel track) | the inference core, the four network programs (`mri_synthstrip`, `mri_synthseg`, `mri_synthmorph`, `mri_sclimbic_seg` with its three models), their scripts and what only those scripts run | 15 / 14 |
| 3.7 `recon-all` | the script's behaviour (its steps, flags, checks, logs, the resume logic sMRIPrep uses) | 13 / 8 |
| later | `nu_correct` (MNI's N3, only with `-no-ants-n4`), MINC's `talairach` (after two failed Talairach checks), MARS | - |

The permissive pieces' clean rooms (13 items, CP-01 to CP-13) come with the roadmap's step 6. In all: 229
replica items and 129 clean-room items, about 309,000 lines of function bodies to follow (static upper
bounds, 90,000 of them the foundations); `mri_segment_hypointensities` is not run by 8.2.0.

**Roles.** Each wave's replica authors also write its specs (`specs/<program>.md`; one reading of the
source, two products; their briefs name them spec writers). The clean rooms follow one wave behind,
written by other agents from the specs and black-box runs only, so a wave's replicas and the previous
wave's clean rooms run side by side, on separate files.

**Each program's validation** (the testing depth of 2026-10-28): the replay on every recorded head
(the six, and the server's when they exist), synthetic edge cases for every option and failure,
golden cases (volumes up to 32³, small synthetic surfaces), its defects decided by rules A to D, its
record, memory near the original's, its reference page and evidence page from the page templates
(the documentation plan). The extensive evidence waits for the final validation phase.

## Working modes

- **A burst** (a fresh weekly usage window the user opens, e.g. Friday and Saturday): about six agents
  at once on separate files (FreeSurfer two to three, the confounds one, the documentation two), merged
  in batches about twice a day through the merge checklist and the clean-room audit.
- **A normal week:** one agent at a time, the cheaper way, on the queue `docs/freesurfer/queue.md`
  (written by the inventory, kept by the orchestrator): the next wave's replicas and the previous
  wave's clean rooms, in priority order.
- Honest timing: the whole of `recon-all`, replica and clean room, is several weeks (the inventory measured
  about 309,000 lines reached, an upper bound); the first burst should bring the preparation and a good part of the
  foundations (wave 3.1 alone is about 50 queue items).

## Data files

**Borrowed code inside FreeSurfer** (the user, 2026-11-12; decision log): every third-party piece gets a
replica, in the package of its own licence family (GSL's GPL code in `larmorx-gpl`, CorTechs' non-commercial
`tkregister2_cmdl` in `larmorx-nc` as its own program, Burkardt's spline in `larmorx-freesurfer`, EPFL's B-splines
(`mriBSpline.cpp`, carrying MGH's notice) in `larmorx-freesurfer` crediting EPFL), and an improved clean room in
`larmorx`; `larmorx` keeps full freedom, and users install a restricted replica only if they want it. Where a
piece's licence and FreeSurfer's cannot be combined in one program, the replica uses the clean-room piece there
(the placement table, `docs/freesurfer/third-party.md`). MARS later; ITK 4.13.2 where it differs from 5.4.5.
No waiting for permissions (the user, 2026-11-12): the replicas are built now and published like the rest
(Burkardt's 2006 routines included, with his credit).
**Washington University's Talairach tools get a clean room only** (the user, 2026-11-12): their source forbids
reproduction without written permission, so no replica unless that permission comes.

The four rules (the user, 2026-11-12; decision log):

1. **The main package ships only small data anyone may use freely** (public domain, CC0, CC-BY with
   its credit in NOTICE, MIT, BSD, Apache-2.0, MNI's permissive notice).
2. **Data with restrictions** ships in its family's companion (FSL's in `larmorx-nc`; FreeSurfer's in
   `larmorx-freesurfer` where its terms allow redistribution), or is **fetched on first use** from its
   owner (`larmorx fetch`, checked by SHA-256, cached in `LARMORX_DATA_DIR` or the user cache folder;
   decision log 2026-11-05): the user gets it from the owner under the owner's terms; LARMORX never
   redistributes it. A local installation can serve it instead (`FREESURFER_HOME`, as
   `LARMORX_FSL_DATA_PACKAGE` does for FSL).
3. **Big files are always fetched** (PyPI limits a file to 100 MB by default; FreeSurfer's atlases are
   about 650 MB).
4. **One list of every data file** (name, source, version, SHA-256, licence, size, the programs that
   need it, where it lives), checked by a test and shown on the site, so users see which terms they
   accept. Where a free source gives the same file (an MNI template from MNI's own release rather than
   FSL's copy), LARMORX prefers it.

LARMORX's code stays Apache-2.0 everywhere; data keeps its own licence. Reading a file at run time does
not change the code's licence; shipping it would make its terms apply to the package.
**The files, measured** (the preparation ran every recorded step again under `strace`:
`docs/freesurfer/data-files.md` and its table `data-files.toml`, replica side): the default paths of both
streams and fMRIPrep's calls read 143 files of the package (4,195,460,712 bytes): 125 of them FreeSurfer's
(atlases, colour tables, `fsaverage` with its labels and surfaces, the seven network weight files, priors,
tables and four configuration files), 8 MINC models under MNI's notice (only for the classic stream's last
Talairach fallback), 4 MNI-derived templates and Washington University's 6 Talairach atlas files (classic
stream only). The classic stream's atlases are
`RB_all_2020-01-02.gca` and `RB_all_withskull_2020_01_02.gca` (`RB_all_2019_10_25.talxfm.mni305.gca`
only when `mri_em_register` is turned off; `wmsa_new_eesmith.gca` on no default path). Every large file
has a stable SHA-256 in FreeSurfer's public git-annex remote. **Decided** (the orchestrator, 2026-11-12,
within the rules): the annex remote is the first-use fetch address (the owner's own source; it
rate-limits hard, so a fetch goes slowly and backs off), a local installation (`FREESURFER_HOME`) the
alternative; the MNI-derived templates follow rule 2, as FreeSurfer's derivation; FreeSurfer's files up to
2 MB may ship in `larmorx-freesurfer`, larger ones are fetched. The list's test and its site page come
with the foundations.
