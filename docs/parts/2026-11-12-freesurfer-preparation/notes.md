# FreeSurfer's preparation (step 3.0): relay notes

Worktree (all relays work here): /home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a2dbae2a2c45ae423
Branch: worktree-agent-a2dbae2a2c45ae423 (from main ba876f7f)
Role: replica-side (may read FreeSurfer's source; nothing written here may reach a FreeSurfer
clean-room implementer). Brief: BRIEF.md in this folder.

Worktree rule seen by relay 1: the harness refuses shell commands with `$VAR` paths, `source`,
heredocs and `cd` into other checkouts. Use literal absolute paths; put Python in a scratch file and
run `python3 <file>`; run tools as `/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`
(and `.../.venv/bin/ruff`) instead of sourcing env.sh.

## Done

### Relay 1 (2026-11-12): sub-task 1, the programs — DONE; sub-task 2 started (2a, first pass)
Commits: a86a05f9 (extractor), b3b4f9bd (programs.md, README.md), e54abeb8 (programs.toml, counts,
tool README), fce1ccfd (cmake_targets.py, code.md §1), b89b3791 (checked against the first
recordings; rca-talairach's thread count corrected). Working tree clean at b89b3791.
Checked against recordings (end of relay 1): ds000005-sub-01-v8 steps 0001-0008 and -classic
0001-0012 match programs.md §3 exactly (order, arguments; N4's AntsN4BiasFieldCorrectionFs -i
orig.mgz -o ... --dtype uchar in mri_nu_correct.mni.log). My source reading "rca-talairach runs
at one thread" was WRONG (its --threads case resets OMP_NUM_THREADS, lines 165-170): recorded
--threads 4 / mri_synthmorph -j 4; corrected in both files. Recorder observation for sub-task 7:
the nested fs_time calls (inside rca-talairach / fs-synthmorph-reg) have no step file of their own
so far (steps 0007, 0008 only); check how the recorder treats nesting before building the replay.
Files:
- docs/freesurfer/README.md: the folder's index and the closed notice.
- docs/freesurfer/programs.md: how recon-all 8.2.0 decides what runs (v8 switch, the v8 options
  file, recon-config.yaml defaults, check_params, threads, seeds, timed vs bare), the 83 stages of
  `recon-all -all` in order with exact commands per stream (v8/classic), the 15 wrapper scripts and
  what they call, fMRIPrep 25.2.6/sMRIPrep 0.19.2's recon-all calls and its 10 programs outside
  recon-all (+ bbregister's chain), counts, the plan's corrections, open questions.
- docs/freesurfer/programs.toml: 110 programs (102 on a default path), fields name, kind, source,
  streams, called_by, run (timed/bare/nested/mixed/conditional), stages, uses, plan_wave, notes.
  Validated: parses, every `source` exists under tags/freesurfer-8.2.0.
- validation/investigations/freesurfer-inventory/extract_commands.py (+README): joins tcsh
  continuations, prints `set cmd` statements and run lines ([fs_time]/[bare]).
- The image's fMRIPrep/sMRIPrep/niworkflows packages were copied out of
  oracles/images/fmriprep-25.2.6.sif into <scratch>/fmriprep-image-src/ (Apache-2.0; read-only use).
- <scratch>/nipype_argstr.py: prints nipype 1.12.0's FreeSurfer argstrs (used for §5).
Gates run: ruff check/format (the tool), check_spdx.py (exit 0), tables --check (exit 0),
validation/tests/test_docs_shell_blocks.py + test_cleanroom_audit.py (131 passed).
No recording existed during relay 1 (oracles/freesurfer-8.2.0/runs/ absent; the .deb at 5.8 of
9.6 GB). Nothing was run with FreeSurfer.

Key findings (all in programs.md §7, to go into the handoff for freesurfer-plan.md):
1. 8.2.0's default intensity correction is N4 (AntsN4BiasFieldCorrectionFs --dtype uchar), both
   streams (recon-config.yaml DoAntsN4: True); MNI's nu_correct only with -no-ants-n4. The plan says
   the opposite.
2. Both streams also run AntsDenoiseImageFs (before mri_segment) and mris_remesh (after the fix).
3. v8 keeps mri_em_register and mri_ca_normalize; runs mri_ca_register -invert-and-save inside
   fs-synthmorph-reg. Wave 3.3 calls them classic-only.
4. A fourth network program: mri_sclimbic_seg (3 models: mca-dura, vsinus, entowm) via the v8
   scripts mri_mcadura_seg, mri_vsinus_seg, mri_entowm_seg; rca-talairach (SynthMorph affine) makes
   v8's talairach.xfm.
5. mri_cc only via seg2cc; mri_segment_hypointensities, mris_make_surfaces, mri_aparc2aseg,
   mris_thickness not run by 8.2.0.
6. fMRIPrep's mri_surf2surf comes only through bbregister (its RMS check); fsaverage sampling is
   mri_vol2surf --trgsubject. bbregister also brings mri_segreg, tkregister2_cmdl, mri_coreg.
7. Default GCA RB_all_2020-01-02.gca (+ withskull 2020_01_02); the talxfm.mni305 GCA only with
   mri_em_register off. v8 reads mni305.cor.stripped.mgz, MNI152 2009c reg-targets, mca-dura and
   vsinus priors (for sub-task 4).
Script quirks to confirm on recordings (then findings + rules A-D when ported): v8 key T1PialSurf
never matches PlaceT1PialSurf (mrisps.pial.mgz never written: check a finished v8 run's surf/ and
the pial step's argv); the v8 mri_ca_label options apply in no stream; mri_vsinus_seg gets the
previous step's $xopts (recon-all 2241; harmless without expert options).

### Relay 2 (2026-11-12): sub-task 2, the code each program needs — DONE
Commits: cc48a96c (cmake_targets.py interprets CMake as the official build ran it), 0c225bd5
(callgraph.py + report_code.py), 5bfd850b (code.md, code.toml, precision fixes, READMEs),
8c685539 (programs.md checked against ds002422). Working tree clean after 8c685539.
Gates: ruff check/format (the four tools), check_spdx.py 0, tables --check 0,
test_docs_shell_blocks + test_cleanroom_audit 131 passed.
- cmake_targets.py: a CMake-subset interpreter (scopes, option, if/elseif/else, foreach,
  FreeSurfer's functions, include, find_library, file(GLOB), flags per target). ASSUMED = the
  official build: Ubuntu22, GCC 8.4.0 (measured: every one of the 74 binaries' .comment is
  "GCC: (Ubuntu 8.4.0-3ubuntu2) 8.4.0"), BLAS = reference libblas.a (no OpenBLAS string in
  mri_robust_register). Both gaps closed: dcm2niixfs resolves; mri_robust_* found.
- callgraph.py: source-level index + preprocessor (PREDEFINED/UNDEFINED), resolution heuristics,
  BFS from main; views full / --core (UNUSED_FORMATS cut) / --calls-only; --why paths;
  --toml adds foundations. ~90 s per view at one core. Precision: definition counts match a
  crude count (mri.cpp 351 vs 348 ...); 2 of 949 files unbalanced (romp_for_{begin,end}.h,
  fragments by design); three false-edge sources found and fixed (local var named like a class;
  std container method names vs nlohmann json; template parameter '=' parsed as a table).
- report_code.py: code.md's tables + docs/freesurfer/code.toml (74 programs: build, flags, core
  reach per library, files reached >= 50 lines).
- Intermediate JSONs (not committed, 10-22 MB each): <scratch>/cmake_targets3.json,
  cg-all.json (full), cg-core.json, cg-core-calls.json, cg-calls.json; tables.md. Regenerate per
  code.md §6 (the next relay can reuse them while the tools are unchanged).
Key numbers (code.md): every C++ program links utils (189 files, 360,356 lines) + 12 bundled
packages; core reach per program 47,800 (converters) to 89,200 lines (mris_place_surface);
static reach with every format 130,000-174,000; union over the 72 default-path programs:
194,957 lines of library functions (132,037 shared by 2+, 49,645 by 65+ = the base layer: volume
I/O incl. GIFTI/expat, the DICOM probe, MINC's .xfm reader, MRI struct, matrices, transforms,
colour tables, chklc), plus 158,929 lines of the programs' own files; 62 utils files (44,120
lines) reached by none. Python: fspython 3.8, numpy 1.24.3, SciPy 1.10.1, TF 2.13.1, Keras
2.13.1, torch 2.1.2+cpu, surfa 0.6.3, nibabel 5.2.1, h5py 3.11.0, neurite/voxelmorph 0.2.
Recordings checked (programs.md header): ds002422 v8 steps 1-48 (stages 1-55), classic 1-14
(stages 1-27) match.

Sub-task 3 STARTED by relay 2 (commit 9cb9f8e1): licences.py (543 files, 736,506 lines: the
full view's reached files of the 72 default-path programs + target sources incl. 4dfp Fortran +
scripts/Python programs) -> <scratch>/licences.json; docs/freesurfer/third-party.md = first pass
(notices by area table; origins inside FreeSurfer files: NR replaced by OpenSvdcmp/OpenRan1
"mimic ran1" (numerics.cpp 1248-1256; setRandomSeed/randomNumber use it, utils.cpp 92-95, 153),
quickselect "based on NR's description, Devillard's public-domain code" (utils.cpp 1382,
RobustGaussian.cpp 34), fs_powell/fs_lbfgs from VNL, DICOMRead.cpp 5590 Joerg-R. Hill
copyright, tkregister2 CorTechs, MARS (Yeo/Sabuncu MIT CSAIL) files, utilsmath.h Geometric
Tools (Boost 1.0), json.h nlohmann (MIT, parts from ranges-v3), ITK ANLM filter (Apache),
neurite copied with permission in synthseg/sclimbic, packages/dicom = Washington University's
CTN, packages/gifti no licence file; 3 questions). Final gates after 9cb9f8e1: ruff check/format
(5 tools), check_spdx 0, the two docs/audit tests 131 passed; tables --check was 0 at 5bfd850b
(no tools.toml change since).

Findings for later sub-tasks (not yet written anywhere else):
- sub-task 3 (third-party): include/json.h is nlohmann/json (header-only); mri_synthseg and
  mri_sclimbic_seg carry neurite's layers "Copied with permission from github.com/adalca/neurite";
  MGZ's talairach.xfm is read through MINC's volume_io (packages/minc, MNI's code) — in the base
  layer; GIFTI goes through expat; the DICOM probe (packages/dicom) is in every volume reader's
  static reach. The audit's file list = the union reach: cg-all.json's per-program "reached"
  keys (file:line:name) -> files.
- sub-task 5 (variation): GCC 8.4.0 at -O3 for every binary (check whether GCC bug 110182 or a
  similar float-rounding miscompilation applies to 8.4; CLAUDE.md names it for 11.4/12.3); C++
  is built with -fno-builtin (libm calls stay calls), C without; no -ffast-math/-march; the
  robust tools' LAPACK/BLAS are reference Netlib (static), not OpenBLAS.
- For the orchestrator: ds000228-sub-pixar008-v8 stopped at step 6: mri_synthseg "Command
  terminated by signal 9" with maximum resident 24,056,836 KB (23 GiB) in its FSTIME line
  (recon-all.log line ~325) — a memory box too small for SynthSeg on that head? The classic runs
  of all three heads were at step 14 (mri_ca_normalize; mri_ca_register running) when relay 2
  looked.

Plan corrections for the handoff (relay 2; add to relay 1's):
- docs/freesurfer-plan.md: the 4dfp tools are compiled with GNU Fortran (GCC 8.4.0), not Intel
  (no -ax=CORE-AVX512/svml in the official binaries): the classic Talairach numbers come from
  the source as GCC 8.4 compiles it.
- CLAUDE.md "Facts worth knowing", Sizes: "FreeSurfer's recon-all path ≈ 200–250k lines (surface
  library 82k, GCA + GCA morph 44k)" -> measured (code.md §3): the 72 compiled programs on a
  default path reach about 195,000 lines of library functions (50,000 a base nearly all share)
  plus 159,000 lines of their own files; GCA + GCA morph 28,000 lines in reach (of 39,500 lines
  of functions); the surface library's files (mrisurf*) about 50,000 in reach.

### Relay 3 (2026-11-12): sub-task 3, the third-party audit — DONE
Commit: 5b79982e (third-party.md complete, third-party.toml, licences.py extended, both READMEs).
Gates: ruff check/format (the inventory tools), check_spdx 0, tables --check 0,
test_docs_shell_blocks + test_cleanroom_audit 131 passed.
- licences.py: licence family + porting class per file (FAMILY_BY_PACKAGE / FAMILY_BY_FILE /
  NOTICE_FIXES), a second origin-pattern set, FINDINGS (TP-01..TP-16) with programs computed from
  the core and full views (scripts/Python by programs.toml `source`), --toml-out, --rng (reached
  functions calling a random source; <scratch>/r3/rng.txt, 97 rows).
- Classes: portable 339 files / 527,907 lines; portable-with-notice 175 / 197,083;
  clean-room-only 26 / 5,635 (talairach_avi, Washington University); question 3 / 5,881
  (tkregister2.cpp, the two MARS files) + 3 findings inside portable files (TP-03/04/05).
- Answers to the first pass's three questions: (1) LICENSE.txt covers any software/data from
  FreeSurfer, Part B licenses MGH's rights: no-notice files are Part B as far as MGH holds the
  rights, third-party code in them is not (B.3); listed by name in third-party.md §6. (2) gifti is
  public domain (distribution/docs/license.gifti.txt). (3) OpenRan1 is VXL's vnl_random
  (Marsaglia-Zaman SWB, LCG seeding, 1000 warm-ups; drand64 cast to float), NOT NR code; identical
  in ITK 4.13.2 and 5.4.5 except the default constructor's init (diffed: ITK 4.13.2's two files
  fetched from GitHub's v4.13.2 tag into <scratch>/r3/dl-itk4132/). OpenSvdcmp = vnl_svd<float>.
- Main findings: TP-03 numerics.cpp's "GSL library replacement" (sc_rng_*, sc_ran_*, sc_gamma_*)
  is a statement-by-statement transcription of GSL 1.8's ranlux.c + randist files (GPL-2.0+),
  run by mri_coreg by DEFAULT (dithering on, seed 53; fMRIPrep's bbregister --init-coreg).
  TP-04 mri_normalize's spline = Burkardt's spline library (MIT on his page today; terms of the
  ~2006 copy unknown). TP-05 mriBSpline.cpp derived from EPFL's coeff.c (no licence statement) +
  Unser's NIH pyramid filters; runs in mri_robust_template's default final resampling (several
  T1w). TP-10 tkregister2.cpp is under the non-commercial CorTechs licence (bbregister's
  tkregister2_cmdl). TP-11 the official binaries link ITK 4.13.2 (CMakeLists.txt 690-691;
  build paths in the binaries), not 5.4.5. TP-14 MARS runs on no default path. TP-15 drand48
  (POSIX) and glibc rand/srandom behaviours. Dead/not compiled: DICOMRead's Hill scandir (SunOS),
  Moller-Trumbore (commented out; callgraph.py lists it for mris_volmask: a false edge to fix).
- Downloads (small, read-only, in <scratch>/r3/dl-*/, not in the repo): ITK 4.13.2 vnl_random,
  GSL ranlux.c + 7 randist files (GitHub mirror ampl/gsl) and GSL 1.8's gamma.c/tdist.c
  (savannah), Burkardt's spline.html, EPFL's interpolation page + coeff.c, GitHub licence JSONs.
  SHA-256s in third-party.md §8.
- Sub-task 4 STARTED (commit 363563c5): data_refs.py (`refs`: lines naming data locations in the
  default-path scripts, the recon-all config files and the reached C/C++ files; `package`: every
  file of the package's data folders with size + SHA-256). Outputs (not committed, regenerate per
  the inventory README): <scratch>/r3/data-refs.txt (143 files, 2,134 lines; noisy on purpose:
  .dat/.lta/.label/talairach match; most recon-all paths go through variables $GCADIR/$GCA/$GCS
  and the Python programs' --model defaults: resolve by reading), <scratch>/r3/package-files.tsv
  (7,624 files, 12,914,205,992 bytes, 4 workers ~1 min; log oracles/logs/fs-prep-relay3-package-sha.log),
  <scratch>/r3/annex-keys.tsv (the tag's 321 git-annex symlinks, 15.5 GB by their keys, with the
  package files of the same SHA-256). FINDING for data-files.md: FreeSurfer's git repository
  stores its large data with git-annex SHA256E keys, and the release package's files carry the
  same SHA-256 (105 of the tag's annexed files, 6.39 GB, are in the package's data folders, e.g.
  RB_all_2020-01-02.gca, synthseg_2.0.h5, synthmorph.deform.3.h5 3,508,630,424 bytes): a stable
  checksum for each data file without downloading; whether FreeSurfer serves annex objects at a
  stable public address is relay 4's check (HEAD requests only).
- For sub-task 5 (variation), from this audit: drand64's two lrand32() calls are operands of '+'
  (unspecified order in C++): measure the official binary's order; drand64 can exceed 1 by
  <= 2^-32; OpenRan1 exits off thread 0 (so no thread dependence); setRandomSeed seeds srand/
  srandom too; time-based seeds when none is set (FREESURFER_SEED env otherwise).

### Relay 4 (2026-11-12): sub-task 4, the data files — DONE; sub-task 5 first pass
Commits: cee0532c (data_trace.py, jobs, data_files.py), 62376593 (drand64 probe), b3fdea48
(variation.md first pass), af205f3b (annex_check.py, READMEs), 51dd0a91 (data-files.md and .toml,
parser fix), df37fb02 (variation.md seeds, tool fixes), 7a5d59d7 (annex checks complete, --markdown
splice). Working tree clean after 7a5d59d7. (<scratch>/r4/data-files.prose*.md and assemble.py are
superseded: the prose lives in data-files.md itself.) Gates at the end: ruff check/format (inventory tools), check_spdx 0,
tables --check 0, test_docs_shell_blocks + test_cleanroom_audit 131 passed.
- Measurement: data_trace.py ran 104 jobs (boxed 12 GiB, 4 threads, strace -f; results and kept
  logs in <scratch>/r4/trace/, log oracles/logs/fs-prep-relay4-data-trace.log; vsinus re-run with
  --force). Two parser bugs found and fixed by `reparse` (no re-run needed): strace pads short
  return values (`)          = 0`), so split execve/open lines were dropped (children's opens fell
  to the parent: bbregister's mri_coreg/tkregister2_cmdl were missing); interpreters' data
  arguments were taken as program names. After the fix: 101 programs seen, 55 read the licence.
- data_files.py -> docs/freesurfer/data-files.toml (143 files, 4,195,460,712 bytes) and, with
  --markdown, the tables of data-files.md between its `<!-- data_files.py: ... -->` markers (the
  prose around them is hand-written in the file; re-running is idempotent). annex_links() reads both key kinds (SHA256E and the older SHA256) from the tag;
  right-hemisphere twins added for lh-only jobs; recorded command lines (10 runs) as a second
  source; fsaverage via $SUBJECTS_DIR/fsaverage mapped to subjects/fsaverage.
- Annex remote checks: annex_check.py (60 s spacing, 300 s back-off on 429, stops on refused);
  table <scratch>/r4/annex-urls-retry.tsv; logs oracles/logs/fs-prep-relay4-annex-check*.log.
  RESULT: every needed annexed object (32, 4,148,046,098 bytes) and the 4 tarballs answered 200.
  The server rate-limits hard (a burst of ~100 HEADs -> 429 then refused for ~1 h; at 20 s
  spacing 429 after nine). The first pass's 403s were burst artefacts (lh.folding tif: 403 then
  200).
- Findings for the handoff (data-files.md §8): mri_watershed reads
  average/rigidly_aligned_brain_template.tif implicitly; talairach_afd reads fsafd/*.adf; v8
  `recon-all -avgcurv` also ran mri_mcadura_seg and mri_entowm_seg (not gated by step flags: for
  programs.md §1 and wave 3.6, read the script); fsaverage's sphere.reg lh == rh and fsaverage5/6's
  spheres are annexed alone (average/surf/lh.sphere.ico{5,6}.reg); the git-annex keys give every
  large file's SHA-256 without download; 8.2.0 POST-RELEASE UPDATES (fs820_updates.sh, six
  archives, newest 22 MB not downloaded; the first changes recon-all/rca-surfreg expert options
  only and leaves recon-all's FS_RECON_VERSION unconfigured); the sclimbic-family models are
  regular git files (GitHub raw), not annexed; synthstrip.1.pt is a pickled state dict (a Rust
  reader must not run pickle code); synthmorph.deform.3.h5 3.5 GB, 877 M parameters, ~18 GiB at
  inference.
- Questions D1-D4 (data-files.md §8): the annex remote as fetch address; MNI-derived templates
  rule 1 or 2; download the newest update archive; the size limit for shipping small files.
- Plan correction for the handoff (freesurfer-plan.md "Data files", last paragraph): "about 25 data
  files" -> 143 files (125 FreeSurfer-licensed, 10 MNI, 6 Washington University, 4 configuration ...
  see data-files.md); the classic stream reads RB_all_2020-01-02.gca and RB_all_withskull_2020_01_02.gca
  (not RB_all_2019_10_25.talxfm.mni305.gca: relay 1 found it used only with mri_em_register off),
  and no wmsa_new_eesmith.gca is read on a default path (measured); the v8 stream reads seven
  network weight files.

Sub-task 5 groundwork (relay 4, while the trace ran; not yet written into a document):
- utils/romp_support.cpp 53-55: `romp_level = ROMP_level_assume_reproducible`, never reassigned (the
  only other writes, 140 and 394, are the nesting guard): every `if_ROMP(fast)` and
  `if_ROMP(experimental)` loop runs SERIALLY in the official binaries (ROMP_if_parallel1: level >=
  romp_level); only assume_reproducible / shown_reproducible and unconditioned `omp parallel for`
  loops run in parallel. Loops with romp_for_begin.h (5 files) use ROMP_Distributor's fixed
  partitions for sum reductions (thread-count independent by design).
- <scratch>/r4/omp_scan.py over licences.json's 543 files: 50 files with OpenMP pragmas, 269 pragmas,
  68 reduction clauses (list <scratch>/r4/omp-reductions.txt), 38 critical, 12 atomic. Plain
  floating-point `reduction(+:d)` with schedule(static) and no ROMP guard: mri_robust_register's
  CostFunctions.cpp (10) and MyMRI.cpp (mean, std) -> thread-count dependent (fMRIPrep's
  mri_robust_register/mri_robust_template); utils/surfgrad.cpp 1362 (cost), mri2.cpp 7429/7492/7956
  (csum, lxsum ... sums), mrifilter.cpp 6713 (scale, vmf), dmatrix.cpp 773 (len),
  mrisurf_metricProperties_faster.cpp 308 (total_area), cma.cpp 1181 (volumes) -> check each
  variable's type (integer counts are exact) and whether a default-path call reaches it.
- MEASURED (commit 62376593, validation/investigations/freesurfer-inventory/drand64_order.sh +
  find_consts.py): vnl_random::drand64 (ITK 4.13.2 vnl_random.cxx 220: `(double(lrand32())/0xffffffff
  + double(lrand32())/(double(0xffffffff)*double(0xffffffff)))*(upper-lower)+lower`) is compiled
  LEFT TO RIGHT in every official binary that has it: the first lrand32 call's result (kept in %rbx)
  is divided by 4294967295.0, the second (%rax) by 4294967295.0^2 (folded to 0x43efffffffc00000);
  then (upper-lower) * sum + lower. 65 of the 72 compiled default-path programs contain it, all the
  same; the 7 without: mri_matrix_multiply fs_temp_file fs_temp_dir analyzeto4dfp gauss_4dfp
  imgreg_4dfp compute_vox2vox. Output <scratch>/r4/drand64-all.txt.
- Every object in AntsN4BiasFieldCorrectionFs and mri_ca_register (ITK's static libraries included)
  carries the one .comment string "GCC: (Ubuntu 8.4.0-3ubuntu2) 8.4.0": ITK 4.13.2 was built by the
  same GCC 8.4. Open: does GCC 8.4 at -O3 drop the float roundings of N4's fit (GCC bug 110182, fixed
  only in 11.5/12.4/13.2; the faulty TYPE_PRECISION test in match.pd is old)? Measure with GCC 8.4.0
  itself (Ubuntu focal's gcc-8 8.4.0-3ubuntu2 .debs extracted user-level, ~30 MB: ask the orchestrator)
  running scripts/build_ants_oracle.sh's float_roundings probe, or inspect the N4 fit in the binary.

### Relay 5 (2026-11-12): sub-task 3b (the placement table) and sub-task 5's rest — DONE
Commits: 915227d5 (burkardt_compare.py, itk_diff.py, gcc84/), 2e31c609 (itk-diff.toml,
itk_functions.py), c6597fee (third-party.md §9 placement + §10 ITK, placement.toml, READMEs),
5d5c3053 (variation.md checkpoint, omp_reductions.py, thread_check.py, thread_jobs.toml), 78eb9789
(variation.md's measurements; working tree clean after it). Gates: ruff check/format (inventory tools),
check_spdx 0, test_docs_shell_blocks + test_cleanroom_audit 131 passed.
- Sub-task 3b (third-party.md §9, placement.toml, 18 pieces P-00..P-17): permissive pieces ->
  larmorx, combinable; GSL (P-08) -> larmorx-gpl, NOT combinable (GPL's "as a whole"/no further
  restrictions vs FreeSurfer's B.1(b)+B.5+Part C; A.6), in-process in mri_coreg -> the clean-room
  piece there; Burkardt (P-09): 2006 no licence, 2010-2022 LGPL, 2024+ MIT (Wayback, hashes in
  §9); FreeSurfer's SplineCubicSet = the 2006/2012 form, NOT in today's MIT release (rewritten
  2020); d3_np_fs identical everywhere -> depends: ask Burkardt to confirm MIT for 2006; EPFL
  (P-10): no terms, permission from EPFL (Swiss CopA Art. 17); ITK's Apache-2.0 B-spline code has
  the same EPFL origin; CorTechs (P-11): combinable with FreeSurfer's code (one program, non-
  commercial), NOT with GPL or FSL code; a process boundary already exists (bbregister runs
  tkregister2_cmdl); WashU (P-12): written permission, separate programs; MARS later; glibc,
  libstdc++, zlib rows.
- ITK 4.13.2 vs 5.4.5 (§10, itk-diff.toml): 796 files / 545 classes; no per-element arithmetic
  change in what FreeSurfer computes with except StatisticsImageFilter's sums (unused there);
  ITKAdaptiveDenoising: FreeSurfer's copy older than 24825c8, the denoiser's own file the same.
  ITK 4.13.2 tarball in <scratch>/r5/itk-4.13.2/ (SHA-256 d8760b27...4938c1).
- variation.md: §1 GCC 8.4.0 keeps N4's float roundings (probe 0/120; GIMPLE 0 lossy chains;
  official binary's fit/evaluation windows: no packed float op, divsd->cvtsd2ss); long double =
  x87 80-bit in 12 reached functions (MRIgaussianSmoothNI, dmatrix, surfgrad, ...: a replica
  needs software 80-bit arithmetic). §2: 129 accumulations, 24 parallel FP ones read; real
  effects: MRIgaussianSmoothNI (thread-count dependent at long-double level), mri_robust_template
  initialXforms' centroid in a critical block (run-to-run nondeterministic with >= 4 inputs and
  > 1 thread), ComputeBrainVolumeStats2 (exact at 1 mm); relay 4's "mri_robust_register
  reductions" are NOT on fMRIPrep's path (RegPowell only). Measured: mri_robust_register
  (sMRIPrep), mri_coreg (defaults) and mri_robust_template (MotionCor, 4 inputs, --fixtp) give the
  same outputs at 1/2/4/4 threads (with --fixtp the centroid is never used).
  ITK: both programs at one ITK thread. §4: the newest update archive (D3): 10 files for
  ubuntu22, on default paths only recon-all (line 4220) and rca-surfreg (--expert), no change
  without -expert (tcsh measured), no data file.
- GCC 8.4.0 installed (approved): <workspace>/oracles/toolchains/gcc-8.4, conda-forge via FSL's
  micromamba (no conda on this machine any more), 756 MB on disk (not 30 MB: the compiler with its
  sysroot); its package cache in <scratch>/r5/mamba-root.
- LTA files record "# created by <account> on <date>": the replay tool must compare LTAs without
  their comment lines (thread_check.py's lta_values does).

### Relay 6 (2026-11-12): the EPFL row; sub-task 6 (split 6a + 6b) — DONE
Commits: 272c7443 (the placement table's EPFL row per the orchestrator's last entry: P-10's replica
in larmorx-freesurfer crediting EPFL, the clean room in larmorx, the request a courtesy; the facts kept
in the row: the paper, the 2D example code's page and coeff.c, its routine names; TP-05 and Q4 answered;
placement.toml gains a `decided` field), acdee84f + 0a3fa6d7 (6a). Gates: ruff check/format (waves.py),
check_spdx 0.
- 6a, validation/investigations/freesurfer-inventory/waves.py (+README entry): MODULES (23 modules:
  8 permissive pieces -> larmorx-core / larmorx-io, 14 FreeSurfer modules -> crates-freesurfer's
  library crates, cr.4dfp clean room only), MODULE_WAVE, PLACE (all 110 programs: wave + reason),
  SPLIT (mri_segstats, mris_calc: a in 3.2, b in 3.5), SCRIPT_PARTS (recon-all in 6 parts), ORDER,
  EXTRA (8 hand items: workspace/plumbing, the unit oracle, x87 80-bit, random numbers, RANLUX P-08,
  Burkardt spline P-09, VNL gaps, libm/C library; N's inference core larmorx-nn). Ownership rule: a
  function goes to the program item when one program alone reaches it (and its wave is not earlier
  than the module's), else to its module's item; earlier programs' paths into a later module "wait"
  (completeness items). Pieces cut at 2,500 lines by function. Checks: user's order per group, no
  script before what it runs (both pass, exit 0). Inputs: cg-core.json (this folder), data-files.toml,
  the tag (script lines), the recordings (step counts).
- docs/freesurfer/waves.md (the order and the plan's corrections, per-program waves table, modules,
  the dependency map, the crates: replicas larmorx-freesurfer-{core,io,volume,filter,surf,deform,
  defect,gca,robust} + program crates -{volumes,atlas,topology,cortex,classic,networks,recon} + -cli;
  permissive in larmorx-core/-io/-image + new larmorx-nn; clean rooms larmorx-fs-{core,gca,surf} +
  program crates + clean-room files in larmorx-io and larmorx-core; questions W1-W6), waves.toml,
  queue.md (228 replica items, 117 clean-room items; EPFL's B-splines = 3.2-01; totals: 3.1 52 items
  89,708 lines, 3.2 40 / 65,895, 3.3 17 / 35,177, 3.4 33 / 50,885, 3.5 37 / 41,047, 3.6 21 / 22,586,
  N 15 / 3,528 (+ Python), 3.7 13 / 297 (+ recon-all's 10,128 tcsh lines); all ~309,000 lines static).
- Waves changed vs the plan: new 3.6 classic-only (mri_watershed, Talairach chain, talairach_afd,
  mri_ca_label), N track (networks + their scripts + mri_concatenate_lta/mri_warp_convert/mri_diff/
  mri_compute_seg_overlap + csvprint), recon-all 3.6 -> 3.7; mri_em_register/mri_ca_normalize shared;
  mri_relabel_hypointensities 3.3 -> 3.5; mri_vol2vol 3.5 -> 3.2; mri_label2label 3.5 -> 3.4;
  mri_segment_hypointensities out (not run); nu_correct and talairach later.
- 6b (commits ca68d225, 92438a52): validation/investigations/freesurfer-inventory/cards.py (+README
  entry) -> docs/freesurfer/cards/<program>.md (110) + cards/README.md (index by wave). Each card: kind,
  source, streams/group, wave, replica and clean-room crates, queue items, callers, what it runs, stages;
  recon-all's and fMRIPrep's calls; subject files read/written (relay 4's strace jobs <scratch>/r4/trace
  + the recordings' step outputs, subject paths as <subj>/...: no private paths, checked); data files;
  the option names its parser compares (C/C++ string comparisons and addArgument, tcsh case labels,
  Python add_argument; old parsers compare without the dash: candidates for the replica item to
  confirm; recon-all 440, mri_convert 265, mris_place_surface 123); code (own files, flags, libraries,
  files reached, reach per module, waits, the items); borrowed pieces (TP-xx and P-xx); variation (a
  VARIATION table in cards.py from variation.md); checks. Also: TP-05's porting class set to the
  decided portable-with-notice in licences.py's FINDINGS and third-party.toml (the one field, edited
  identically instead of a full regeneration), third-party.md §1's sentence.
  Gates after 92438a52: ruff check/format (inventory tools), check_spdx 0, tables --check 0,
  test_docs_shell_blocks + test_cleanroom_audit 131 passed. Working tree clean.
  Regenerating order: waves.py, then cards.py (both commands in the tools README).

### Relay 7 (2026-11-12): sub-task 7, the tools into the repository — 7a DONE (see the end state below)
Commits: fa7668ff (recorder tools byte-identical in validation/investigations/freesurfer-oracle/,
the replay tool validation/src/larmorx_validation/freesurfer/{recording,package,files,replay,cli}.py,
`python -m larmorx_validation freesurfer-replay {steps,rebuild,run,compare}`, oracle cache: env family
"freesurfer" (FAMILY_ONLY_ENV: no other key changes) + Policy.formats="freesurfer" (compare.same_file
dispatches to freesurfer.files.same_result; core/rerun pass it; rerun restores stored (regex, repl)
volatile pairs as tuples), tests validation/tests/test_freesurfer_replay.py), 951a71ba (README,
--compare-only), 6ee168a2 (the comparison after the first proof pass; launcher `exec -a <name>`;
ctime-based scan; defect2seg VARIES), 49dac0d0 (proof_table.py, docs/validation/freesurfer-replay.md
draft).
Written WITHOUT opening FreeSurfer's source or docs/freesurfer/ (formats from nibabel, the files,
black-box runs; this relay did read NOTES.md, which carries source-derived facts).
How it works (README in validation/investigations/freesurfer-oracle/ has it all): rebuild = copies
of `before`'s blobs, directories by birth time (find %B) before the step's start, the finished run's
links where content allows, subjects/fsaverage by birth time; argv/cwd/env remapped; env = the
recorded scripts/recon-all.env filtered (FreeSurfer, MNI, OMP/ITK, PATH, locale, shell; never
LARMORX_*) + the step's six variables; threads 1 by default (env + --threads/-threads/-nthreads/
-openmp/-j). Runs via oracle_cache.run with a launcher per program
(<workspace>/oracles/inputs/freesurfer-replay/launchers/<tree16>/<program>/{<program>,PACKAGE.json};
PACKAGE.json = build stamp + package SHA-256 + SHA-256 of a manifest of all 70,117 files, hashed once
in 18 s, memo manifest-*.json beside), inputs = every rebuilt file, outputs = the subjects dir.
Comparison: files.py (verdicts identical/provenance/log/varies/differs/missing/extra; MGH and
surfaces record by record: [int32 number][int64 length][bytes] as OBSERVED, number 3 = command line;
a colour table record (number 1) has no length: the rest of the file).
Proof pass 1 (batch A, 34 v8 steps, 4 threads): 29 reproduced; the 5 others were tool faults, fixed
(argv[0], inherited paths, mktemp names, PID temp dirs, comma decimals, ~ paths) except defect2seg:
FINDING (black-box): surface.defects.mgz's embedded colour table gives defect clusters colours that
change from run to run (two replays with the cache off differ from each other; voxels identical) ->
VARIES in replay.py, never cached; a candidate for the defect register (rule C or A at port time).
OBSERVATION for the orchestrator: the reference runs' environment (scripts/recon-all.env) carries
FreeSurfer 6.0.0's MNI_DIR, MINC_*, PERL5LIB, FSFAST_HOME, FUNCTIONALS_DIR (/usr/local/freesurfer/...)
and FS 6's fsfast/bin and mni/bin on PATH (the user's profile; FS_OVERRIDE=0 keeps them). No log of
the 10 runs names those paths (grep over every subject dir: only recon-all.env): no default step used
them (N4 is the default; nu_correct not run). Harmless for the recordings; the replay reproduces it.
Also recon-all.env holds the whole session environment (session tokens): recordings stay private.
Pre-existing flake (not mine, seen at the base commit too): validation/tests/test_oracle_cache.py::
test_above_one_thread_a_run_is_served_after_three_identical_runs fails ~1 in 2 under `-n 3`
(passes serially).
Batch B's v8 part (the final tool, 38 steps fresh): 38 of 38 reproduced (defect2seg "where the
original is deterministic").
FINDING 2 (black-box, a log only): mri_convert --like (classic mri_nu_correct.mni's last call) prints
"volume geometry: valid : <number>" with a number that changes from run to run (589411376 recorded,
1149350928 replayed; the volume written is identical): an unset field printed? Candidate for the
register (a message, rule C or A at port time; the replica must not print memory).
Proof pass 2 (batch B: the v8 list again with the final tool, 15 classic steps, 7 long v8 steps,
4 ds002422 steps, 3 steps at 1 thread, mri_ca_register's m3z): log oracles/logs/fs-prep-relay7-
replay-b.log, script <scratch>/r7/batch-b.sh, reports <scratch>/r7/replay-b-*.json.

**For the handoff (relay 7; docs/validation/oracle-cache.md is the cache's shared page, not edited
in the branch):**
- "The key" table, row "The environment", after the `python` family's sentence: add "FreeSurfer's
  programs (the family `freesurfer`, 2026-11-12: the replay tool): `FS*` (`FS_LICENSE` by its
  content), `FREESURFER*`, `SUBJECTS_DIR` (a place, by its role), `MNI_*`, `MINC_*`, `PERL5LIB`,
  `FUNCTIONALS_DIR`, `LOCAL_DIR`, `FIX_VERTEX_AREA`, `OMP_*`, `GOMP_*`, `ITK_*`, `PYTHON*` and `PATH`
  (FreeSurfer's tcsh and Python scripts find the programs they run by it), in the key of that
  family's runs only".
- "The runners" table, a row: `freesurfer-replay` | `larmorx_validation.freesurfer.replay.run_freesurfer`
  (a launcher per program, `oracles/inputs/freesurfer-replay/launchers/<tree>/<program>/`, beside a
  `PACKAGE.json` naming the whole installation: build stamp, package SHA-256, the SHA-256 of a
  manifest of its 70,117 files) | 1 by default (`--threads recorded`: the recording's 4, served after
  3 identical runs) | every file of the rebuilt subject directory an input; the subjects directory
  the place; `Policy.formats = "freesurfer"`.
- "Differences: findings, never overwritten", after the HDF5 sentence: "FreeSurfer's files by
  `larmorx_validation.freesurfer.files.same_result` where `Policy.formats` is `freesurfer`: its
  provenance records (command line, time, user, machine, a surface's stamp, the run's directories,
  the header lines of LTA and statistics files) left out (since 2026-11-12)."

### Relay 8 (2026-11-12): the coherence fixes (findings 1, 6, 7 queue side, 16) and the user's two later decisions — DONE
Branch fast-forwarded to main a1a26763 first (track B's waves.py folder name, the decision log's 2026-11-12 entries).
Commits: 22c522a1, df3d89ff, 7ada9813 (+ this NOTES). Gates: ruff check/format (inventory tools), check_spdx 0,
tables --check 0, test_docs_shell_blocks + test_cleanroom_{audit,verdicts,summary} 246 passed (FSLDIR unset).
- 22c522a1: stable item numbers: docs/freesurfer/queue-ids.toml (waves.py --ids; a key per item: extra:<name>,
  mod:<module>:<piece>, w3:<module>:<part>, prog:<program>[:<piece>], ...; a new item takes the next free number
  of its wave; one no longer generated is a PROBLEM until marked `retired = "why"` by hand; [[key]] in a text
  becomes the item's number). Only the reference runs counted (--run-pattern `-(classic|v8)$`; the laptop's
  -classic-1thread runs were changing the counts; cards.py too). Regenerated: unit-oracle folder, ds000228-v8's
  re-run steps. Found: the completeness lists' tie order depended on PYTHONHASHSEED (sets): now by name; the
  outputs are identical across hash seeds (checked).
- df3d89ff + 7ada9813, the decisions: W3 items by role (3.1-09 the XML parser under GIFTI, 3.1-10/11 gifticlib's
  reading ported, 3.1-12 its writer exact; 3.1-16 the DICOM probe; 3.1-19 JSON's reader, 3.1-20 its writer exact;
  3.1-13 and 3.1-17 RETIRED; 3.1-14 MINC's .xfm: W3 wording); GSL's sc_* functions a module `gpl.gsl` of their
  own (FUNCTION_MODULES + OWN_ITEM): item 3.1-53, crate larmorx-gpl-gsl in crates-gpl/, OUT OF fs.maths' 3.1-24
  (now 1,992 lines, was 2,273); Burkardt's SplineCubicSet/SplineCubicValue/d3_np_fs a module `fs.spline`
  (larmorx-freesurfer-core): item 3.1-54 (192 lines), out of mri_normalize's 3.2-25; 3.1-05/06 texts (spec
  writer then clean-room implementer; mri_coreg's and mris_expand's replicas use the clean RANLUX; the spline's
  replica is 3.1-54); 3.1-04 kind "permissive; spec + clean room (glibc's part)"; 3.1-01 = replica side only,
  its main-package side NEW 3.2-41 (plumbing (main package): replica.rs's LARMORX_FREESURFER, the freesurfer
  family, lx.freesurfer; best by a clean agent) and the clean-room crates NEW C3.1-00 (the first FreeSurfer
  clean-room relay); tkregister2_cmdl's replica crate `larmorx-nc-tkregister2` (crates-nc/, its own program in the
  larmorx-nc wheel; NAME PROPOSED); talairach_avi (3.6-18) and mpr2mni305 (3.6-19) now "clean room only" with
  the four 4dfp programs (3.6-14..17) by the user's decision naming talairach_avi (C3.6-06 and C3.6-07 RETIRED);
  the permissive pieces' clean rooms CP-01..CP-13 (crate "with step 6"); mris_expand's note (-tmap_std uses the
  clean RANLUX). placement.toml: P-08, P-09, P-11, P-12, P-13 decided (fields `decided`, `queue`; options
  emptied; permission empty: no requests before). licences.py: classes gpl-alone (TP-03), noncommercial-program
  (TP-10, tkregister2.cpp), later (TP-14, MARS files), TP-04 portable-with-notice; third-party.toml regenerated
  (identical but those 7 fields; licences.py reproduces it with <scratch>/cmake_targets3.json). third-party.md
  §1 classes table, the TP paragraphs' "Porting:", §7 Q1-Q6 answered (+ the Talairach tools), §9 table and the
  five paragraphs "Decided: ...". waves.md: §1 (3.1 row, 3.5 tkregister2, 3.6 Talairach clean room, later MARS),
  §3 (FUNCTION_MODULES, OWN_ITEM, W3), §4 (the replicas in the other companions; the permissive pieces' clean
  rooms with step 6 and a crate proposal; C3.1-00; the main package's plumbing; names W2 accepted), §6 W1-W6
  DECIDED, §7 (--ids, the PROBLEM cases, --run-pattern). queue.md "How to take an item": stable numbers, the kinds
  (W3 parser, permissive (exact writer), gpl, clean room only, plumbing (main package)), who may take what, the CP
  table with step 6. The cards regenerated (84 changed). docs/freesurfer/README.md and the tools' README updated.
  Checked: the recorded programs of the 12 reference runs are all in programs.toml (52 names; only the recorder's
  `--larmorx-final` marker besides).

### Relay 9 (2026-11-12 and 13): the handoff, the restricted paths, the final report — DONE
Agent a1d07ea63edaa7ce4. Commit 11212b1d: clock_seeds.py (randomNumber's clock seed: mri_label2vol --defects both
streams 2/run, mri_edit_wm_with_aseg v8 only 3/run; variation.md §3 and §5), recording_source.py (19 recordings:
two recon-config.yaml blobs 6930b943 classic / 7d560ebd v8; no other blob is a package source file), cards.py
REGISTER_CANDIDATES (11 cards changed), programs.md (T1PialSurf confirmed on the six v8 runs), data-files.md (D1,
D2, D4 decided), docs/freesurfer/README.md complete. Then the handoff: <scratch>/HANDOFF.toml (25 ops, 9 shared
documents), HANDOFF-audit.toml (9 ops: restrictions.py and its tests, the restricted paths 1-5), HANDOFF.md (the
reasons, the proposals with regexes, rules, families and test lines, the register candidates); --check passes on a
fresh copy of main 0f310788; applied in the trial <scratch>/r9/trial (main 0f310788 + the branch's files): doc and
audit tests 287 passed, audit tests 217 passed with the 19 new cases, tables --check 0, SPDX 0. The harness refuses a
REPORT.md file from a subagent: the final report is the section "The part's final report" below. Scratch helpers:
r9/yaml_blobs.py, r9/source_blobs.py (superseded by recording_source.py), r9/report_section.md.

## For the orchestrator (relay 1)

**Restricted paths for the clean-room audit's table** (restrictions.py today covers workspace
`freesurfer/`, `tags/freesurfer*`, and `/usr/local/freesurfer/{bin,python,lib}` content): add
- `<workspace>/oracles/freesurfer-8.2.0/root/` (the unpacked 8.2.0 package, prefix
  `root/usr/local/freesurfer/8.2.0/`: its `bin/` scripts, `python/`, `lib/` are FreeSurfer's
  source; running its programs is a black-box run, reading them is not; also each run's own logs
  that scripts write, e.g. `mri/transforms/synthmorph.mni305/log/`, are run output, not source);
- inside every recording: `oracles/freesurfer-8.2.0/runs/*/subjects/*/scripts/recon-all.local-copy`
  (recon-all copies itself there, recon-all line 891) and `scripts/recon-config.yaml` (FreeSurfer's
  YAML), and the same files' blobs in `record/blobs/` (by SHA-256: the replay tool or the audit must
  know which blobs these are);
- `crates-freesurfer/` (the replicas, when created), `docs/freesurfer/` (all of it),
  `docs/findings/freesurfer-*.md`, `validation/investigations/freesurfer-inventory/`;
- probably `validation/investigations/freesurfer-oracle/` only if the replay tool learns formats
  from FreeSurfer's source (decide when sub-task 7 writes it; nibabel-based reading is not
  restricted).
- `<scratch>/fmriprep-image-src/` is fMRIPrep's Python (Apache-2.0), not restricted.

**Questions (relay 1):**
1. sMRIPrep's recon-all calls on 8.2.0 would run the v8 stream (no -no-v8). Should larmorprepx run
   recon-all's v8 stream or -no-v8 (fMRIPrep 25.x's FreeSurfer 7.3.2 behaviour)? With v8,
   `-noskullstrip` skips the v8 `mri_mask T1 synthstrip brainmask` step, so the injected mask stays
   (from the script; worth one recorded run with sMRIPrep's flags on the server).
2. The plan's line "mri_nu_correct.mni runs MNI's N3 ... --ants-n4 off by default" is wrong for
   8.2.0 (finding 1): the handoff will correct it; nothing to decide unless the user wants N3.
3. mri_sclimbic_seg (a fourth network, three models) is not in wave 3.3's list: add it there with
   the three v8 scripts (the handoff proposes this).

**Questions (relay 3, the third-party audit; third-party.md §7 has the reasons):**
Q1 (TP-10) tkregister2_cmdl (fMRIPrep's bbregister) is under the non-commercial
   FreeSurfer/CorTechs licence: replica in larmorx-freesurfer with those terms (one program with
   non-commercial terms in the wheel), or a clean room only (its bbregister uses: --regheader,
   --ltaout, --fslregout; lta_convert, FreeSurfer-licensed, does the same conversions)?
Q2 (TP-03) mri_coreg's default dithering uses a GSL-derived RANLUX389 + distributions (GPL
   origin): may that code enter larmorx-freesurfer, or are these functions written from the
   published descriptions (Luescher 1994, James 1994, Knuth, Marsaglia) by an agent who has read
   neither GSL nor FreeSurfer's transcription, from a spec of the exact expressions? (Relay 3 read
   GSL's files; GSL's source should then join the restriction table for that agent.)
Q3 (TP-04) mri_normalize's spline: port Burkardt's code under his present MIT terms, or write it
   from the published description?
Q4 (TP-05) EPFL-derived cubic B-splines (no licence statement upstream): port as part of
   FreeSurfer's file, or write from the published method (LARMORX already ports the same
   recursion from ITK and SciPy)?
Q5 (TP-14) MARS runs on no default path: leave it out until a program needs it?
Q6 (TP-11) the official binaries link ITK 4.13.2: do the replicas of Ants*Fs and of the VNL calls
   follow 4.13.2 (pin tags/ITK-v4.13.2, diff each routine used against 5.4.5) or 5.4.5 with the
   differences measured as findings?

**Restricted material to add (relay 3):** GSL's source (if Q2 chooses a clean room for those
functions); `docs/freesurfer/third-party.toml` is under `docs/freesurfer/` (already listed).

**Questions (relay 4, the data files; data-files.md §8 has the reasons):**
D1 FreeSurfer's git-annex remote (pub/dist/freesurfer/repo/annex.git, public, objects by key, the
   SHA-256 in the key) as the first-use fetch address: use it slowly and verified, ask FreeSurfer
   first, or fetch only the 9 GB package? Its rate limit is strict (429 after nine HEADs at 20 s).
D2 The MNI-derived templates (mni305.cor*.mgz, the ICBM 2009c registration targets, 12.4 MB): MNI's
   terms (rule 1) or FreeSurfer's derivation (rule 2)?
D3 Download the newest 8.2.0 update archive (patch_data_20260625.tgz, 22 MB) to list what it
   replaces, and should the reference follow the updated files ("newest upstream versions")?
D4 The size limit for small FreeSurfer files in larmorx-freesurfer (the TOML uses 2,000,000 bytes).
**Restricted material (relay 4):** nothing new outside the paths already listed (docs/freesurfer/,
validation/investigations/freesurfer-inventory/); the trace logs in <scratch>/r4/trace/ hold
command lines and paths of FreeSurfer's scripts, not source text.

**Placement decisions for the user (relay 5; third-party.md §9 has the facts; nothing ported):**
- P1 (P-11, CorTechs' tkregister2_cmdl): which non-commercial package: (a) its own package and
  program (e.g. larmorx-freesurfer-nc, building crates-freesurfer by path); (b) a second program in
  the larmorx-nc wheel linking no FSL code (docs/licensing.md's one-family-per-package rule would
  need an exception); (c) inside larmorx-freesurfer. CorTechs 2(f) forbids more restrictive terms,
  so no FSL code in the same program; bbregister already runs it as a separate program.
- P2 (P-09, Burkardt): ask Burkardt to confirm his MIT terms cover the 2006 spline routines (clean
  room meanwhile)? Or port under the LGPL into larmorx-gpl (mri_normalize's replica cannot use it)?
- P3 (P-10, EPFL): the request to EPFL's Biomedical Imaging Group (the decision log: the orchestrator
  drafts it); meanwhile the clean room, or ITK's Apache-2.0 B-spline code if it gives FreeSurfer's
  bits (to measure in the replica part).
- P4 (P-12, Washington University): ask for written permission for a replica, or the clean room
  only (the settled default)?
- P5 (P-08, GSL): the clean-room RANLUX389 + distributions in larmorx used by larmorx-freesurfer's
  mri_coreg, the GSL replica in larmorx-gpl as its bit-exact reference (what the facts support), or
  the process-boundary option (FSF's caution). GSL's source joins the audit's restriction table
  for that clean room.
- ITK (Q6): the facts support reusing LARMORX's 5.4.5 ports in FreeSurfer's replicas, each confirmed
  bit-exact against the official program at FreeSurfer's settings, 4.13.2's source where a check
  fails (third-party.md §10, "What this means").

**Candidates for the defect register (relay 5; rules A-D at port time, no IDs assigned here):**
mri_robust_template initialXforms' centroid order (nondeterminism, unbiased templates only:
larmorx sums in input order);
MRIgaussianSmoothNI's thread-partitioned long-double sums and ComputeBrainVolumeStats2's for
non-integer voxel volumes (larmorx: one fixed order); recon-all 4220 `if($XOptsFile)` (fixed by the
June update: follow it); SplineCubicValue's end clamps (harmless, rule C).

**Restricted material (relay 5):** relay 5 read Burkardt's spline code (2006, 2012, 2018, 2022,
today's: <scratch>/r5/dl-wayback/, <scratch>/r5/dl-burkardt-now/) and ITK 4.13.2's source
(Apache-2.0, not restricted). If P-09's clean room is written, Burkardt's code joins that
implementer's restriction; likewise EPFL's coeff.c (<scratch>/r3/dl-epfl/) for P-10's and GSL's
for P-08's. New closed files, all under paths already listed: docs/freesurfer/{placement.toml,
itk-diff.toml}, validation/investigations/freesurfer-inventory/{burkardt_compare.py, itk_diff.py,
itk_functions.py, omp_reductions.py, thread_check.py, thread_jobs.toml, gcc84/}.

**For the orchestrator (relay 7): the replay tool and the restriction table.**
- Q7 (recommendation: OPEN) The clean rooms of FreeSurfer's programs will need the replay tool to
  check themselves against the recordings. Everything relay 7 wrote is free of FreeSurfer's source
  (nibabel's readers, the files, black-box runs): validation/src/larmorx_validation/freesurfer/,
  validation/investigations/freesurfer-oracle/ (the orchestrator's recorder scripts + README +
  proof_table.py), docs/validation/freesurfer-replay.md, the oracle-cache changes. Recommend: NOT in
  the restriction table (relay 1's "probably validation/investigations/freesurfer-oracle/ only if the
  replay tool learns formats from FreeSurfer's source": it does not). The relay read NOTES.md
  (source-derived facts) but no source; judge it in the audit if needed.
- Restricted patterns the replays add (FreeSurfer material a replay copies into its directories):
  `<workspace>/oracles/inputs/freesurfer-replay/work/**/scripts/recon-config.yaml` (recon-all's
  YAML, copied from the recordings' blob 7d560ebd1430a9050bec9c3f71ed4e350f472304f418bb07aa7435fccc1a9ef7
  and any other blob of that path), as relay 1's entry for the runs' own `scripts/recon-config.yaml`
  and `recon-all.local-copy` (the latter is never recorded, so never copied). The launchers'
  PACKAGE.json and the manifest-*.json (file names and hashes of the installation) are not source.

**Questions (relay 6; waves.md §6 has the reasons):** W1 fMRIPrep's programs outside recon-all placed
with the shared programs (3.2, 3.5): the user's order? W2 crate names larmorx-freesurfer-* (replicas) /
larmorx-fs-* (clean rooms) or another prefix? W3 the permissive parsers (expat, the DICOM probe,
nlohmann's parser): full ports or pure-Rust parsers with the same values (writers stay exact ports)?
W4 build the unit oracle (3.1-02: libutils with GCC 8.4.0)? W5 recon-all 3.6 -> 3.7 in the plan and
next-steps. W6 a coverage build to size the default paths exactly (the sizes are static upper bounds).
**For the handoff (relay 6):** freesurfer-plan.md's wave table from waves.md §1 (3.6 classic-only, N,
3.7 recon-all; the corrections per wave); its line "the first burst should bring the preparation, the
foundations and most of wave 3.2" is optimistic (3.1 is 52 items, about 90,000 lines static); CLAUDE.md
rule 3 already names crates-freesurfer/ (main has it).

**For the handoff (relay 5):** relay 4's first pass named mri_robust_register's reductions as
thread-dependent for fMRIPrep: wrong (variation.md §2 corrects it); CLAUDE.md rule 8's GCC 11 note
does not apply to FreeSurfer's official binaries (GCC 8.4.0 keeps N4's roundings, variation.md §1).

**For the orchestrator (relay 8): what the tracks must hear (their items changed meaning or content).**
- Track A: 3.1-24 (fs.maths 2 of 2) no longer holds numerics.cpp's GSL functions (sc_rng_*, sc_ran_*, sc_gamma_*,
  sc_uni*, sc_err_msg, sc_inc_status: 281 lines): they are 3.1-53 (`gpl.gsl`, crates-gpl/larmorx-gpl-gsl, a replica
  author there) and must never enter crates-freesurfer/; nor Burkardt's three routines (now 3.1-54, `fs.spline`,
  larmorx-freesurfer-core, crediting Burkardt). Item numbers are stable from now on (queue-ids.toml).
- Track C: 3.1-09..12, 3.1-16, 3.1-19..20 rewritten by W3 (3.1-13, 3.1-17 retired); 3.1-14's wording says W3 too.
- Track D: 3.1-05's and 3.1-06's texts name their replicas (3.1-53, 3.1-54) and roles; 3.1-04's kind names the
  glibc part as spec + clean room.
- Talairach: the decision log names talairach_avi; I made talairach_avi AND mpr2mni305 (Washington University's
  notice) "clean room only" with the four 4dfp programs (avi2talxfm, mri_matrix_multiply, fs_temp_dir and the QA
  awk stay replica + clean room: FreeSurfer's own). Revert in waves.py's CLEAN_ONLY if the orchestrator reads the
  decision more narrowly.
- Names proposed (no decision recorded): `larmorx-gpl-gsl`, `larmorx-nc-tkregister2`; the CP items' crates "with
  step 6" (proposal in waves.md §4: readers in larmorx-io's clean-room files, the rest in larmorx-core's, the
  inference core's beside larmorx-nn).

**The restricted paths (relay 9's groundwork, checked against restrictions.py on main a1a26763).** Already
covered: the tags and clone (R03), the unpacked package's bin/python/lib/etc/... (R03), the unit oracle's build
tree (R03), the runs' scripts/recon-all.local-copy and scripts/recon-config.yaml (R03), crates-freesurfer/ and
larmorx-freesurfer* copies (R10), docs/freesurfer/ (queue-ids.toml included), docs/findings/freesurfer-*.md,
validation/investigations/freesurfer-inventory/ and freesurfer-unit-oracle/, the replay tool and
freesurfer_unit.py (R11, run-only: Q7), the borrowed pieces' web sources (R19), other agents' scratch (R18: the
preparation's <scratch>/freesurfer-0/ with its call graphs and the GSL, Burkardt and EPFL downloads in r3/, r5/,
and the tracks' <scratch>/freesurfer-1/). To add (exact proposals for restrictions.py, through the handoff):
1. the recordings' blobs of scripts/recon-config.yaml (recon-all's YAML, FreeSurfer's): two blobs in every run
   so far, 6930b94343b8c1767ce6fdfa301cdb4fe381ee885ce4b20a99097e2184ca0ba6 and
   7d560ebd1430a9050bec9c3f71ed4e350f472304f418bb07aa7435fccc1a9ef7 (recon-all.local-copy is never recorded):
   `^oracles/freesurfer-[^/]+/runs/[^/]+/record/blobs/[0-9a-f]{2}/(6930b943...|7d560ebd...)$` -> R03; better,
   a rule by content (a blob whose recorded path is scripts/recon-config.yaml), since the server's runs may add
   variants;
2. the replays' copies: `^oracles/inputs/freesurfer-replay/work/.*/scripts/(recon-config\.yaml|recon-all\.local-copy)$`
   -> R03 (relay 7's entry);
3. GSL's replica crate, once it exists: anchors `/crates-gpl/larmorx-gpl-gsl(/|$)` and
   `/larmorx[-_]gpl[-_]gsl(/|$)` -> R10 family "FreeSurfer" (or "GSL"), BEFORE the generic `/crates-gpl(/|$)`
   anchor (family "AFNI/tedana", which would not stop the RANLUX clean room's implementer);
4. tkregister2_cmdl's replica crate: `/crates-nc/larmorx-nc-tkregister2(/|$)` and `/larmorx[-_]nc[-_]tkregister2`
   -> R10 family "FreeSurfer", before `/crates-nc(/|$)` (family "FSL");
5. the recorder `validation/investigations/freesurfer-oracle/` (not on the list today): recommend the replay
   tool's rule (R11, run-only), because its author (the orchestrator) had read the inventory and its README
   documents recon-all's step structure; docs/validation/freesurfer-replay.md can stay readable (its facts are
   black-box observations of the files: checked);
6. for step 6 later (not now): the CP items' implementers will need the permissive sources restricted per piece
   (tags/minc-1.5.1, tags/nifticlib-2.0.0, tags/ITK-v4.13.2, ...: today "workspace"/permissive), as the ANTs and
   ITK clean rooms will;
7. an option, not needed now: the unpacked package's python/ prefix also restricts its bundled TensorFlow,
   PyTorch, neurite, voxelmorph (permissive) for every FreeSurfer clean-room implementer; the inference core's
   clean room (CP-13) would read those upstreams from their own sources instead.

**For the handoff (relay 8):** next-steps step 6's list: add "FreeSurfer's permissive pieces (queue CP-01 to CP-13:
the bundled readers, the small pieces, x87 arithmetic, the random numbers, VNL and netlib, ITK 4.13.2's filters, the
networks' inference core)"; freesurfer-plan.md "The code's layout": the replicas outside crates-freesurfer/
(larmorx-gpl-gsl alone in larmorx-gpl; larmorx-nc-tkregister2 as its own program in larmorx-nc) and "the clean-room
crates are created by the first FreeSurfer clean-room relay (queue C3.1-00), never by a replica-side relay; the main
package's plumbing is 3.2-41"; its 3.6 row: "the Talairach chain (talairach_avi, mpr2mni305 and the 4dfp tools: a
clean room only, its atlases fetched)"; the decision log needs nothing new (W1-W6 are the orchestrator's, now in
waves.md §6: add one line to the log if the orchestrator wants them there).

## The part's final report (relay 9; for docs/parts/<date>-freesurfer-preparation/report.md)

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

## Next

**Nothing: the part is complete** (relay 9). The orchestrator merges the branch, applies HANDOFF.toml and
HANDOFF-audit.toml (reasons and proposals in HANDOFF.md), and copies BRIEF.md, NOTES.md, HANDOFF.* and the final
report above into docs/parts/.

## FROM THE ORCHESTRATOR
- 2026-11-12, after relay 2 (the orchestrator): accepted (cc48a96c..9cb9f8e1). The laptop's
  ds000228-sub-pixar008-v8 run stopped at `mri_synthseg` (killed at a 23 GiB peak: six runs shared a 64 GiB box);
  the box is now 84 GiB, and the failed run is moved to `oracles/freesurfer-8.2.0/failed/` and run again after the
  queue (2 at a time, 4 threads). The orchestrator will add the restricted paths of NOTES' list to the audit before any
  FreeSurfer clean-room implementer starts. Next: **relay 3**, the third-party audit (sub-task 3) as "Next" says, then
  sub-task 4 (the data files) if time allows.
- 2026-11-12, after relay 3 (the orchestrator): accepted (5b79982e, 363563c5). Q1-Q6 go to the user in one batch with
  the orchestrator's recommendations; nothing is ported until they answer. If the GSL-derived functions go to a clean
  room, the orchestrator adds GSL's source to the audit's restriction table (this part read it). Next: **relay 4**,
  sub-task 4 (the data files) as "Next" says: resolve the path variables per program, which files FreeSurfer serves on
  its own (stable address and checksum) and which only inside its package, each file's licence; then sub-task 5
  (variation) if time allows.
- 2026-11-12, the user's decision (decision log "FreeSurfer's borrowed pieces and its two streams"): every third-party
  piece gets a replica (in its own licence family's package) and an improved clean room (in larmorx). **Add to this
  part (sub-task 3b, before the handoff): the placement table** for every piece of third-party.md: its licence, the
  package its replica would live in, whether that licence and FreeSurfer's licence may be combined in one program (quote
  both licences' relevant terms; where a piece is a separate program, the process boundary), where they may not, that
  the replica uses the clean-room piece for that part; for code with no licence (EPFL), what permission is needed and
  from whom. Also ITK 4.13.2 against 5.4.5 per ITK class FreeSurfer's default paths use (identical or not). Streams:
  shared programs first, then classic-only, the v8 networks in parallel (the queue and the waves follow this).
- 2026-11-12, after relay 4 (the orchestrator): accepted (to 7a5d59d7). D1-D4 decided within the user's rules:
  **D1** yes: FreeSurfer's public annex remote is the first-use fetch address (the owner's official source; SHA-256 from
  the annex keys), slow and rate-limited (back off on 429), a local FreeSurfer installation (`FREESURFER_HOME`) as the
  alternative; **D2** the MNI-derived templates follow rule 2 (FreeSurfer's derivation: the cautious reading); **D3**
  yes, download the newest 8.2.0 update archive (about 22 MB) and record what it replaces (the reference runs stay
  unpatched; say whether any patched file is a program on a default path); **D4** files up to 2 MB may ship in
  `larmorx-freesurfer`. **GCC 8.4.0** (about 30 MB, conda-forge, user-level, under `<workspace>/oracles/toolchains/`):
  yes, to test bug 110182 on N4's fit. Next: **relay 5**: the placement table (sub-task 3b, the entry above), then
  sub-task 5's rest (variation.md §4: the plain floating-point reductions, mri_robust_register first; GCC 8.4.0 and N4).
- 2026-11-12, the user: EPFL's B-splines get a replica and a clean room both; the authors are asked afterwards. The
  replica of that piece is built but kept off the public repositories (a local branch, never pushed, outside every
  wheel) until their permission arrives; say so in the placement table.
- 2026-11-12, correction (the orchestrator, from TP-05's facts): `mriBSpline.cpp` carries MGH's notice and FreeSurfer
  distributes it under its licence, so its replica goes into `larmorx-freesurfer` like FreeSurfer's other files,
  crediting EPFL (paper and 2D example code); no private branch. The placement table says so.
- 2026-11-12, the user: "please make sure EPFL's B-splines is ported". `utils/mriBSpline.cpp` (and the pyramid filters
  TP-05 names) must appear as their own item in the queue and in the cards (sub-task 6): a replica in
  `larmorx-freesurfer` (FreeSurfer's licence, EPFL credited) and a clean room in `larmorx`, in wave 3.2 with
  `mri_robust_template` (its final resampling), used too by the robust tools' pyramid (`Registration.cpp`) and by
  `mri_convert` / `mri_vol2vol` with cubic interpolation.
- 2026-11-12, after relay 5 (the orchestrator): accepted (915227d5..78eb9789). **Correct the placement table's EPFL row
  to the user's decision** (the entries above, written while relay 5 ran): `mriBSpline.cpp` carries MGH's notice and
  FreeSurfer distributes it under its licence, so its replica goes into `larmorx-freesurfer` like FreeSurfer's other files,
  crediting EPFL; the request to the authors is a courtesy; keep the facts you found (routine names, the 2D code's page)
  in the row. P1-P5 go to the user now with the orchestrator's recommendations; their answers come in a later entry.
  GCC 8.4.0's 756 MB (not 30 MB): accepted, kept under oracles/toolchains/. The x87 80-bit `long double` functions: a
  foundation item (an exact 80-bit arithmetic for the replicas) in the waves. Next: **relay 6**, sub-task 6 (the waves
  checked and corrected with the user's order: shared programs first, then classic-only, the v8 networks in parallel; the
  crates of `crates-freesurfer/` and of the clean rooms; the queue `docs/freesurfer/queue.md`; one card per program;
  EPFL's B-splines their own item in wave 3.2).
- 2026-11-12, after relay 6 (the orchestrator): accepted (272c7443..92438a52). W1-W6 decided by the agreed plan:
  **W1** fMRIPrep's programs outside recon-all sit with the shared programs, early (the plan's 3.2: "fMRIPrep runs them
  without recon-all: early value"); **W2** the names `larmorx-freesurfer-*` (replicas) and `larmorx-fs-*` (clean rooms),
  `larmorx-nn` for the networks: accepted; **W3** pure-Rust parsers giving the same values and the same failure messages
  on every path recon-all and fMRIPrep can reach, proved by differential tests (no full ports of expat, the DICOM probe or
  nlohmann's parser); **W4** the unit oracle: yes, a queue item in 3.1; **W5** recon-all as 3.7 in the plan and
  next-steps through the handoff; **W6** a coverage build later, as a lab-server extra (not blocking). The user hears the
  size (about 309,000 lines, an upper bound). P1-P5 are with the user. Next: **relay 7**, sub-task 7 (the recorder tools
  into `validation/investigations/freesurfer-oracle/` with a README; the replay tool in `larmorx_validation`, proved on
  recorded steps).
- 2026-11-12, the user on P1-P5 (decision log "No waiting for permissions"): build every borrowed piece's replica now in
  its licence family's package (P1 `tkregister2_cmdl` in `larmorx-nc` as its own program; P5 GSL's generators alone in
  `larmorx-gpl`, `mri_coreg`'s replica on the clean-room generator; Burkardt, EPFL and Washington University's 4dfp tools
  with FreeSurfer's code in `larmorx-freesurfer`), an optimised clean room of every piece in `larmorx`; no emails, no
  waiting; publishing is the user's call. Update the placement table and the queue accordingly (the 4dfp tools get
  replica items too).
- 2026-11-12, the coherence review (`<scratchpad>/coherence-review-2026-11-12.md`; the orchestrator): **the next relay
  (after relay 7's replay tool)** fixes findings 1, 6, 7 (queue side) and 16 on the branch: the placement table, placement.toml,
  the queue, waves.md and the cards to "No waiting for permissions" (every borrowed piece a replica in its family's package:
  tkregister2_cmdl in larmorx-nc, GSL alone in larmorx-gpl with a queue item, Burkardt and the 4dfp tools in
  larmorx-freesurfer with replica items, no "until confirmed"); W1-W6 recorded in waves.md §6 as decided; the parser items
  rewritten to W3 (pure-Rust parsers with differential tests; the writers' number formatting exact); 3.1-01 split (no
  clean-room crates); a place and items for the permissive pieces' clean rooms (finding 16); then the restricted paths and
  the handoff. The foundations' tracks follow the corrections in `<scratchpad>/freesurfer-1/BRIEF.md` meanwhile.

- 2026-11-12, the user: **Washington University's Talairach tools (4dfp, talairach_avi) get a clean room only**, no replica
  (their source forbids reproduction without written permission); drop their replica items; the clean room checks against
  the recorded heads. Burkardt's 2006 routines keep a replica (publishing to be confirmed by the user).

- 2026-11-12, the user: **Burkardt's 2006 routines: a replica and a clean room, published normally** (no local-only branch;
  the earlier "build them in a module of their own ... keep them off the public repository" no longer applies).

- 2026-11-12 ~22:40, after relay 7 (the orchestrator): accepted (to 7e504f9f: the recorder tools, the replay tool, 38/38 v8
  steps + batch B reproduced, the two black-box findings defect2seg's colours and mri_convert --like's "valid :" number).
  **Q7 decided:** the replay tool and freesurfer_unit.py stay in the restriction table as they are on main (R11, 254346b7):
  a FreeSurfer clean-room implementer RUNS them as a black box but never reads them (it was written by an agent that read
  NOTES.md's source-derived facts; restrictions.py's comment says so). Relay 7's restricted patterns for the replays' work
  folders (`oracles/inputs/freesurfer-replay/work/**/scripts/recon-config.yaml`) go in with relay 9's restricted paths.
  **Relay 7b: not now.** Its open items (the networks' threads at 1 vs 4, MRIgaussianSmoothNI's wide kernel,
  mri_robust_template with six inputs, the random sources, the classic stream's later steps, the streams against
  recon-all.log) become follow-ups in the handoff's next-steps text (the lab server's v8 1-thread runs and the laptop's
  classic 1-thread runs, running now, answer the threads' part). **Relay 8 (next):** the coherence fixes listed above
  (findings 1, 6, 7 queue side, 16) and the user's two later decisions (the 4dfp tools / talairach_avi: a clean room only,
  their replica items dropped; Burkardt's 2006 routines: a replica in larmorx-freesurfer and a clean room, published
  normally): the placement table, placement.toml, the queue, waves.md (W1-W6 decided), the cards regenerated (waves.py then
  cards.py), the parser items rewritten to W3, 3.1-01 split (no clean-room crates), the permissive pieces' clean rooms
  placed (finding 16). Then, if the relay has room, relay 9's restricted paths; else relay 9 does them with the handoff.
- 2026-11-12 ~23:20, after relay 8 (the orchestrator): accepted (to 7ada9813). **Decided:** Talairach scope as you made it
  (talairach_avi and mpr2mni305, which carries Washington University's notice, clean room only with the four 4dfp
  programs: the user's decision covers WashU's code); the crate names `larmorx-gpl-gsl` and `larmorx-nc-tkregister2`
  (the existing pattern: larmorx-gpl-tedana, larmorx-nc-*); the CP items' crates as waves.md §4 proposes, settled at step
  6. Tracks A, C and D are told their items' changes. **Relay 9 (the last):** the handoff (HANDOFF.toml ops + HANDOFF.md
  reasons) for every shared document: freesurfer-plan.md's wave table from waves.md §1 and the corrections, relay 7's
  oracle-cache.md text, upstream.tsv's FreeSurfer 8.2.0 line, W1-W6 and the two crate names as decided (decision-log
  text), relay 7b's open items as next-steps follow-ups, the black-box findings (defect2seg's colours, mri_convert
  --like's "valid :" number) as register candidates with the part's IDs if the brief gives a range (else as findings for
  their programs' items), relay 6's and 5's handoff notes; then the part's final report (docs/parts/ as earlier parts).
  **The restricted paths:** write your list 1-5 as exact proposals in HANDOFF.md (regexes, rules, families, and a
  classification test line for each); the orchestrator puts them into restrictions.py with the tests at the merge (it
  has edited the FreeSurfer rules before). Run the gates; stop with the report.
