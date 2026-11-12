# FreeSurfer's preparation (step 3.0): final report (relay 9, replica-side, agent a1d07ea63edaa7ce4)

The part's final report as its last relay gave it (2026-11-12 and 13). Nine relays, all replica-side (they read
FreeSurfer's source; nothing they wrote is meant for a FreeSurfer clean-room implementer).

The part is ready to merge. Branch `worktree-agent-a2dbae2a2c45ae423`: relays 1 to 7 are on main already (the
intermediate merge f9dece03 and relay 7's commits to 7e504f9f); the branch adds relay 8's three commits (22c522a1,
df3d89ff, 7ada9813) and relay 9's 11212b1d (the final commit, clean): 17 files and 79 cards, none a file main changed
since. The handoff: `HANDOFF.toml` (25 operations, 9 shared documents), `HANDOFF-audit.toml` (9 operations: the
restricted paths as code for `restrictions.py` and its tests), the reasons in `HANDOFF.md`; both pass
`apply_handoff.py --check` on main 0f310788 and were applied in a trial (the doc and audit tests: 287 passed, and
217 with the audit's 19 new cases).

**The programs** (programs.md, programs.toml). `recon-all -all` 8.2.0 runs 83 stages; its default is the v8 stream
(`FS_V8_XOPTS=1`: SynthStrip, SynthSeg, SynthMorph, `mri_sclimbic_seg` with three models), `-no-v8` the classic one.
110 programs listed, 102 on a default path: 66 compiled C++ programs, 4 Washington University 4dfp tools, 4 Python
network programs, 24 tcsh/csh scripts, 3 Python helpers, one awk script. By stream: v8 84, classic 80, fMRIPrep's
and sMRIPrep's calls 14 (10 programs outside recon-all, `bbregister`'s chain included); 16 only in v8, 13 only in
classic. The plan's corrections: N4 (`AntsN4BiasFieldCorrectionFs`) is the default intensity correction in both
streams, not MNI's N3; `AntsDenoiseImageFs` and `mris_remesh` run in both; v8 keeps `mri_em_register` and
`mri_ca_normalize`; a fourth network program; `mri_segment_hypointensities` is not run; the default atlases are the
2020 GCAs. Checked against the recordings (ds000005, ds002422, both streams: order, arguments, timed and bare
steps); the v8 `T1PialSurf` quirk confirmed on all six v8 runs.

**The code** (code.md, code.toml). A CMake interpreter of FreeSurfer's build as the official one ran it (Ubuntu
22.04, GCC 8.4.0, reference BLAS) and a call graph from each `main`: every C++ program links `utils` (189 files,
360,356 lines) and 12 bundled packages; one program's core reach is 47,800 to 89,200 lines; the 72 compiled
default-path programs reach 194,957 lines of library functions (132,037 shared by two or more, 49,645 by 65 or
more: the base layer) plus 158,929 lines of their own files; 62 `utils` files (44,120 lines) are reached by none.
fspython 3.8 with TensorFlow 2.13.1, PyTorch 2.1.2, surfa 0.6.3.

**The third-party audit and the placement table** (third-party.md, third-party.toml, placement.toml). 543 files
(736,506 lines): portable 339, portable with a notice 175, clean room only 26 (Washington University's Talairach
code), the rest in the decided classes; 16 findings (TP-01 to TP-16): TP-03 `numerics.cpp`'s "GSL replacement" is
GSL 1.8's RANLUX and distributions transcribed (GPL), run by `mri_coreg` by default; TP-04 `mri_normalize`'s spline
is Burkardt's (its 2006 form); TP-05 `mriBSpline.cpp` follows EPFL's paper and example code (MGH's notice); TP-10
`tkregister2.cpp` is under CorTechs' non-commercial terms; TP-11 the official programs link ITK 4.13.2; OpenRan1 is
VXL's `vnl_random`, not Numerical Recipes'. The placement table (18 pieces) at the user's decisions of 2026-11-12:
GSL's generators alone in `larmorx-gpl` (`crates-gpl/larmorx-gpl-gsl`; FreeSurfer's replicas use the clean-room
generator), Burkardt's and EPFL's code in `larmorx-freesurfer` with their credit, `tkregister2_cmdl` its own
program in `larmorx-nc` (`crates-nc/larmorx-nc-tkregister2`), Washington University's Talairach tools
(`talairach_avi`, `mpr2mni305`, the four 4dfp programs) a clean room only, MARS later; a clean room of every piece
in `larmorx`. ITK 4.13.2 against 5.4.5 (796 files, 545 classes): no per-element arithmetic change in what
FreeSurfer's default paths compute with.

**The data files** (data-files.md, data-files.toml). Measured by 104 `strace` jobs (each recorded step run again)
and the recordings' command lines: 143 files of the package, 4,195,460,712 bytes: 125 FreeSurfer's (the seven
network weight files among them; SynthMorph's deformable model alone 3.5 GB, about 18 GiB at inference), 8 MINC
models (MNI's notice), 4 MNI-derived templates, 6 Washington University atlas files. FreeSurfer's public git-annex
remote serves every large file under its SHA-256 (all 32 needed objects and the 4 tarballs answered; it
rate-limits hard). D1 to D4 decided (the decision log's new entry).

**Variation** (variation.md). The build: GCC 8.4.0 everywhere (the 4dfp tools' GNU Fortran too), `-O3`, SSE2, no
fast-math; GCC 8.4.0 keeps N4's float roundings (GCC bug 110182 measured: 0 of 120 probe cases, no lossy chain);
`long double` is the x87's 80-bit format in 12 reached functions (hence `larmorx_core::x87`); `vnl_random::drand64`
compiled left to right in all 65 programs that have it. Threads: FreeSurfer's ROMP-guarded loops run serially in
the official build; of 129 accumulations, 24 parallel floating-point ones, real effects only in the Gaussian
smoothing's long-double sums, `mri_robust_template`'s centroid (an unbiased template, four or more inputs) and a
brain-volume sum for non-integer voxels; measured: `mri_robust_register`, `mri_coreg` and `mri_robust_template` as
fMRIPrep and sMRIPrep call them give the same files at 1, 2 and 4 threads. Seeds: recon-all's fixed seeds (1234,
53); the clock seeds measured from FreeSurfer's own warning in the logs: `mri_label2vol --defects` (both streams, 2
per run) and `mri_edit_wm_with_aseg` (v8, 3 per run) in all 12 reference runs. The 8.2.0 update archive changes
only expert-option handling.

**The waves, the layout and the queue** (waves.md, waves.toml, queue.md, queue-ids.toml, cards/). The user's order:
the shared programs, then the classic-only ones (a new wave 3.6), the v8 networks as a parallel track N,
`recon-all` as 3.7. 229 replica items and 129 clean-room items (13 of them the permissive pieces' clean rooms, with
step 6), about 309,000 lines (static upper bounds; 3.1 alone 52 items, 89,900 lines); stable item numbers. The
crates: `larmorx-freesurfer-*` (9 libraries, 7 program crates, the CLI), the `larmorx-fs-*` clean rooms (created by
queue C3.1-00 from a clean brief), `larmorx-nn`, the plumbing 3.2-41; W1 to W6 decided. 110 porting cards (streams,
wave, crates, items, calls, measured inputs and outputs, data files, the options each parser compares, reach,
borrowed pieces, variation and, since relay 9, the findings for the defect register in 11 cards).

**The replay tool and its proof** (validation/investigations/freesurfer-oracle/, larmorx_validation.freesurfer).
The orchestrator's recorder committed byte-identical with a README; the replay tool rebuilds a subject directory
before any recorded step and runs FreeSurfer's program there through the oracle-output cache (a launcher per
program naming the whole installation; one thread by default), comparing MGZ volumes and surfaces record by record
with FreeSurfer's provenance left out. Second proof pass: 68 steps of both streams, 66 reproduced, 1 where
FreeSurfer itself varies (`defect2seg`'s colour table, never cached), 1 with a varying message (`mri_convert
--conform`'s unset `valid` field); both are register candidates in the cards (the brief gave no ID range).

**The restricted paths.** Already in the audit: the clone and tags, the unpacked package's code, the unit oracle's
build, the runs' recon-all copies, `crates-freesurfer/`, `docs/freesurfer/`, the findings, the inventory's and the
unit oracle's tools, the replay tool (run-only), the borrowed pieces' sources, the scratch folders. Proposed
(HANDOFF-audit.toml, 19 test cases): (1) the recordings' two configuration blobs by SHA-256 (`recording_source.py`:
no other blob is FreeSurfer source); (2) recon-all's copies in any subject directory (the replays' work folders, a
stopped run, a scratch `--dest`); (3) `crates-gpl/larmorx-gpl-gsl` and (4) `crates-nc/larmorx-nc-tkregister2` in
FreeSurfer's family, before their workspaces' anchors; (5) the recorder, run-only.

**Commits and gates.** 38 commits from a86a05f9 (relay 1) to 11212b1d (relay 9); every finding's tool committed
under `validation/investigations/freesurfer-inventory/` (20 scripts and `gcc84/`) or `freesurfer-oracle/`. Gates of the last
commits (FSLDIR unset): ruff check and format, `check_spdx.py`, `tables --check`, the doc and audit tests.

**Open questions and follow-ups** (in next-steps through the handoff): which stream larmorprepx runs by default
(with a recorded run of sMRIPrep's own calls on 8.2.0, on the server); FreeSurfer's own variation (the networks at
1 and 4 threads; the 1-thread runs against the 4-thread ones; the smoothing's and the unbiased template's thread
effects; the time-of-day seeds of `bbregister`, `mri_vol2vol`, `mri_surf2surf`; `mri_edit_wm_with_aseg`'s clock
seed on the other heads); the classic stream's later steps and the large steps replayed, the replays' messages
against `recon-all.log`; the proof page's tables (`proof_table.py`); the reference runs' environment (FreeSurfer
6.0.0's variables from the login profile, harmless); new recordings' blobs into the audit; W6's coverage build; a
flaky cache test; `tags/freesurfer-8.2.0/.upstream-sha256` in `sha256sum`'s form, not the bootstrap's bare hash.

