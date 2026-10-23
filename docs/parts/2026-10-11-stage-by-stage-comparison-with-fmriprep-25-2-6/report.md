# Final report: Stage-by-stage comparison with fMRIPrep 25.2.6

Agent `a443c5a76b071f29f`, 2026-10-11. The agent's report to the orchestrator, verbatim.

I've built the stage-by-stage comparison and recorded it. Against fMRIPrep 25.2.6 as it ran on ds000005 sub-01, 61 of the 92 steps larmorx can run are bit-identical. The other 31 differ for explained reasons, apart from 14 where the cause is a known larmorx gap. Nothing is pushed.

The command is `python -m larmorx_validation pipeline-stages --oracle <run dir> --out docs/validation`. It reads only nipype's `command.txt` and `_report/report.rst`, never the pickles. It runs larmorx on fMRIPrep's own inputs and parameters for each step. Where an output differs, it re-runs the step through a chain where each link changes one thing:
- fMRIPrep's program in its own image at one thread;
- the same with CORE-MATH preloaded (the image has the same glibc 2.35 as the host);
- ANTs 2.6.5 from the repository's oracle, also with CORE-MATH;
- larmorx.

The pytest wrapper skips cleanly without the oracle. The final run (32 min) was on a quiet machine (load 0.83 at the start, no other jobs) from a clean commit.

## Results by stage

| Stage | Result vs fMRIPrep as it ran | Why they differ | §11.3 | fMRIPrep re-run → larmorx, same threads |
|---|---|---|---|---|
| A01 DenoiseImage | NCC 1 − 1.1e-4, max diff 271 | fMRIPrep ran 8 threads, which don't reproduce run to run; glibc maths | pass | 55 s → 3.2 s |
| A03, A10 truncate; A05 Laplacian; A07 ThresholdImage | bit-identical (5 nodes) | – | pass | e.g. Laplacian 9.7 s → 3.0 s |
| A08 brain-extraction morphology (26 nodes) | bit-identical | – | pass | 31 s → 4.4 s |
| A04, A09 brain-extraction N4s | NCC ≥ 1 − 1.6e-11 | threads, glibc maths, the ANTs build | pass | 13.1 s → 3.0 s; 7.3 s → 1.2 s |
| A06 template priors (Gaussian, inverse SyN warp) | max diff 7e-16 | glibc maths | pass | 15.8 s → 20.5 s (larmorx slower) |
| A11 anatomical outputs to MNI (8 nodes) | 3 bit-identical; probability maps max diff 0.146 | larmorx's `--float 1` gap (below) | **fail** in ≤ 6 of 1,082,035 voxels | 32.7 s → 33.1 s |
| F01 head motion (3 runs) | median diff ≤ 0.0025 mm / 0.0036°, RMS 95th pct ≤ 0.023 mm | clean-room algorithm; mcflirt reproduces itself exactly | pass | mcflirt 84 s vs larmorx 85 s on one thread (74 s matrices only, 15 s on 12 threads) |
| F03 BOLD-reference N4 | NCC ≥ 1 − 6.8e-8 | glibc maths, the ANTs build | pass | 3.0 s → 0.4 s |
| F04, F11 (linear, MultiLabel `.h5`) | bit-identical | – | pass | |
| F05–F07 one-shot resampling, all 9 | **bit-identical**, node outputs and derivatives | – | pass | MNI: 215 s → 44 s, writing included |
| F08, F09, F12 boldrefs and masks | masks bit-identical; boldrefs max relative diff ≤ 9.5e-6 | larmorx's `--float 1` gap | pass | |
| F10 confound masks | max diff 2e-16 | glibc maths | pass | |

FD correlation is 0.89–0.93 against fMRIPrep's confounds. That threshold only applies when the largest FD reaches 0.5 mm; here it is at most 0.33 mm, and mcflirt's own band is 0.79. Slice timing and the pre-template N4 did not run in this oracle: there is no `SliceTiming`, and there is only one T1w.

## The one larmorx difference not explained by threads, maths or version
- **`antsApplyTransforms --float 1` (14 nodes):** ANTs runs this whole pipeline in single precision; larmorx computes it in double. This was already documented in A1. The harness proves it: ANTs run in double precision and rounded to float32 equals larmorx bit for bit in all 14. Through it, three probability maps miss the 1e-4 interpolation threshold in at most 6 of 1,082,035 voxels each, where ANTs' own float result departs from its double one. Fixing it means a single-precision transform pipeline, which is not small, so I recorded it and did not fix it.
- **Fixed (small):** `larmorx ants` command lines read only `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`. nipype passes the thread count as `NSLOTS`, so under nipype larmorx would have used every CPU (results are the same either way). It now reads the variables as ITK does, with a unit test.
- **Performance:** antsApplyTransforms through fMRIPrep's warps is no faster than ANTs on one thread; the template priors (Gaussian, through the SyN warp) take 20.5 s against 15.8 s.

## Findings about fMRIPrep 25.2.6
These are in `docs/findings/fmriprep-oracle.md`.
- **Wrong ANTs version assumed:** fMRIPrep's image ships ANTs 2.6.2 (conda-forge) on ITK 5.4.4, not 2.6.5. Its mcflirt is the same release (2111.0) on newer libraries, yet it gives the same 240 matrices per run as the repository's FSL 6.0.7.
- **N4 differs by build, not version:** fMRIPrep's N4 binary follows the C++ source. The repository's GCC 11 oracle, and so larmorx, keep two `float` roundings in double precision. I tested a scratch larmorx build with the source's roundings; with CORE-MATH preloaded into fMRIPrep's binary, it matched all six N4 outputs of the run bit for bit. The scratch change is reverted and not in the repository.
- **fMRIPrep's `ResampleSeries` corrupts data with multi-threaded OpenBLAS:** with `OMP_NUM_THREADS=8`, one MNI resampling had 22,635 wrong voxels in 12 of 240 volumes, by up to 846. A second run hit 13 different volumes, so it is a race. Only the image's `ENV OMP_NUM_THREADS=1` prevents it, so installs outside the image, or Apptainer without `--cleanenv`, are exposed. The oracle run is not affected.
- **Threads, seeds and nondeterminism:** nipype passes threads as `NSLOTS`. DenoiseImage and the anatomical N4s ran on 8 threads; the BOLD N4, ImageMath and antsApplyTransforms on 1. `--random-seed 0` becomes `ANTS_RANDOM_SEED=50495`. DenoiseImage at 8 threads did not reproduce itself (two re-runs differed from fMRIPrep's output in 163,367 and 346,866 voxels); the 8-thread N4s did reproduce exactly.
- **Differences from `docs/analysis/`:** the BOLD reference's antsAI uses `Affine[0.1]`, not `Rigid`. The BOLD N4's `shrink_factor=2` is dropped by nipype (confirmed), so ANTs' default 4 applies. The BOLD premask registration stops after 25 of 200 iterations.
- **Headers:** niworkflows rewrites antsApplyTransforms output headers after ANTs writes them. Masks are float64 in the working directory and uint8 as derivatives, and the DataSink's log message about the dtype is wrong.

## Roadmap: tools larmorx cannot run yet
fMRIPrep's own time on this run (8 jobs at once, shared machine):

| Tool | fMRIPrep time | Milestone |
|---|---|---|
| antsRegistration (MNI SyN alone: 2,118 s) | 2,639 s | A4 / A5 |
| 3dUnifize | 169 s | AFNI |
| FLIRT-BBR | 139 s | FSL |
| antsAI | 117 s | A4 |
| confounds | 53 s | larmorprepx |
| mri_coreg | 48 s | FreeSurfer |
| FAST | 40 s | FSL |
| 3dvolreg (time of the whole averaging node) | 32 s | AFNI |
| Atropos | 12 s | A6 |
| BET, 3dAutomask, fslmaths, lta_convert | under 6 s together | FSL, AFNI, FreeSurfer |

The same table is in the record and in `docs/overview.md` ("What is still needed").

Decision for you: should larmorx's N4 follow the C++ source (which matches fMRIPrep's actual binary) or the GCC 11 oracle it replicates today? Switching means re-recording the N4 validation and golden checksums against a source-faithful reference.

## Commits
In the worktree, my identity, no AI attribution:
- `38099d5` ANTs command lines read the thread count as ITK does, `NSLOTS` included
- `7d3b1cb`, `20e076d` WIP checkpoints for the harness and findings
- `83caad2` merge of `main` (which had moved by one docs commit)
- `f1ad074` the validation record, findings, overview and CLAUDE.md status
- `0e43f8b` findings index entry

I also corrected the `NSLOTS` claim in `ants-n4.md` and `ants-cli.md`, updated the API option tables, and added a cross-reference in `fmriprep-resampling.md`. All outputs and reference runs are in `oracles/compare/fmriprep-25.2.6/ds000005-sub-01-noreconall/`, outside git.

Files are in `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a443c5a76b071f29f`:
- docs/validation/fmriprep-stages.md
- docs/validation/fmriprep-stages.json
- docs/findings/fmriprep-oracle.md
- validation/src/larmorx_validation/stages/
- tests/parity/test_pipeline_stages.py
