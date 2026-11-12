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
   size). Output: `docs/freesurfer/inventory.md` and `docs/freesurfer/cards/`.
4. **The replay tool**, one for every program: it rebuilds the subject directory before a step from
   a recording, runs LARMORX's program (replica or clean room) there, and compares its files with the
   recorded ones and with FreeSurfer's same step run again on the main machine at one thread (through
   the oracle-output cache), with comparisons that understand MGZ volumes, surfaces, labels,
   annotations, transforms and the text files. Each program then has real-data tests on every
   recorded head with no new full runs. It replaces "the templates" part's replay harness (the user,
   2026-11-12).
5. **The data files:** the four rules and the list of every data file ("Data files", below).

## The code's layout

- **Replicas:** a workspace `crates-freesurfer/` (FreeSurfer's licence, the `larmorx-freesurfer` wheel
  and program), several crates so agents work in parallel without touching the same files: the core
  (file formats, the image and surface structures, FreeSurfer's maths and random numbers), the atlas
  code (GCA, GCA morph), the surface library, and the programs in groups. It builds the Apache crates
  by path, never `crates-gpl` or `crates-nc`. CLAUDE.md's rule 3 names it (the user, 2026-11-12).
- **Clean rooms:** crates of the main package, named by the inventory; each added to the clean-room
  audit's lists in the part that creates it (the porting guide; a test fails otherwise).
- MGH/MGZ, ANALYZE and `.mat` reading moves from `larmorx_tedana::formats` into `larmorx-io` with the
  foundations (t2smap's follow-up).

## The waves

| Wave | What (the programs per wave: `docs/freesurfer/waves.md` and the queue, replica side) |
|---|---|
| 3.1 Foundations | the file formats, the image and surface structures, FreeSurfer's maths, random numbers and messages; x87 80-bit arithmetic; the third-party pieces (the placement table) |
| 3.2 Volumes | the volume programs both streams run (`mri_convert`, `mri_nu_correct.mni` with ITK's N4, `AntsDenoiseImageFs`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), and fMRIPrep's `mri_coreg`, `mri_robust_template` (with EPFL's B-splines) and `mri_robust_register`, early |
| 3.3 Atlas | the atlas programs both streams run (`mri_em_register`, `mri_ca_normalize`, `mri_ca_register`) |
| 3.4 First surfaces | tessellation to the fixed, remeshed surface |
| 3.5 Final surfaces and statistics | placement, registration, parcellation, statistics; fMRIPrep's surface programs and `bbregister` (with `tkregister2_cmdl`) |
| 3.6 The classic-only programs | `mri_watershed`, the Talairach chain (`talairach_avi`: a clean room only, its atlases fetched), `talairach_afd`, `mri_ca_label` |
| N The v8 networks (a parallel track) | the inference core, the four network programs, their scripts |
| 3.7 `recon-all` | the script's behaviour (its steps, flags, checks, logs, the resume logic sMRIPrep uses) |

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
No waiting for permissions (the user, 2026-11-12): the replicas are built now. **Publishing is the user's call**:
until the user says whether a push to the public repository counts as publishing, the replicas of pieces that carry
no licence at all (Burkardt's 2006 routines) stay on a local branch, never pushed, outside every wheel.
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
recon-all's classic stream reads about 25 data files: the GCA atlases (`RB_all_2019_10_25.talxfm.mni305.gca`,
the with-skull one, `wmsa_new_eesmith.gca`), the cortical parcellation atlases (`*.gcs`), the folding
atlas (`folding.atlas.acfb40.noaparc.i12.2016-08-02.tif`), `mni305.cor.mgz`, colour tables,
`fsaverage`'s labels and the Talairach atlas; the v8 stream also reads the three networks' weights. The
inventory lists each one's licence and source.
