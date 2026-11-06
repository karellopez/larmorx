# FLIRT replica: investigation helpers

- `compare_images.py` compares NIfTI images the way the parity suite `fsl-flirt-replica` does (the
  decompressed bytes with the header's `descrip` blanked) and, when they differ, prints the
  largest data difference and the number of differing voxels. Used while the replica was brought
  to bit-identity (2026-11-01) to compare FSL's and the replica's outputs of one case by hand.
- `finding_repro.py` reproduces one finding of random testing (`difftest`, tool `fsl-flirt` or
  `fsl-flirt-programs`) by hand: the shrunk recipe's inputs (or the drawn one's, `--drawn`) in two
  directories, FSL's program and the replica (`reproduce`) on each, their standard output and
  error kept with the first line where they differ, the output images compared voxel by voxel. It
  showed the two differences part 1 fixed (the blur of `-applyxfm` beyond the image's edges; the
  sign of BBR's NaN cost) and part 2's missing quaternion message (trial-flirt-programs-00219).
- `memory_findings.py` measures the peak memory and time of FSL and of the replica on the findings
  that ran over the trial's memory budget (`/usr/bin/time -v`, a timeout; `--replica-only` measures
  a new build again against the earlier FSL run). Part 2: FSL's own peak is 8 GiB to above 39 GiB
  on those inputs.
- `rsstrace.py` samples both programs' resident memory every millisecond with their printed lines
  placed in time, and `gdb_allocations.py` (a gdb script) lists every allocation of an image's size
  with the call that made it and the memory at that moment: together they located the replica's
  extra copies on large references (part 2: 262 MiB against FSL's 192, now 187).
- `fsl67_measure.py` measures FSL-67 (BBR's normals from the segmentation's voxel sizes) on
  fMRIPrep's three BBR calls and an anisotropic synthetic case with a known motion; its results
  are `fsl67-results.json` (rule C: no gain above the noise).
- `recheck_findings.py` re-runs every finding of a random-testing run (shrunk and drawn recipes)
  through the suite's own runner with the program in `LARMORX_NC_BIN` and says which still
  differ: what a fix leaves open.
- `crash_probe.py` runs FSL's `flirt` and the replica with the default options on small and
  constant images, to narrow down the inputs on which FSL corrupts its heap (part 2).
- `lapack_eig_svd_probe.py` (part 3) calls FSL's own `liblapack.so.3` (OpenBLAS 0.3.33, Haswell
  kernels) through ctypes with Armadillo's work sizes and compares every output word with the
  replica's translation (`crates-nc/larmorx-nc-core/tools/lapack-probe`): the BLAS kernels
  (`ddot`, `drot`, `dnrm2`, `dgemv` both ways, `dgemm`), `dsyevd` on 3 × 3 matrices of every family
  and `dgesdd('S')` on 3 × 3, 12 × 12 and 12 × n: 400 records, every word identical. It found
  `dgemv_t`'s FMA-contracted tails, `dnrm2`'s x87 rounding and `drot`'s FMA.
- Part 3's pre-check and trial: `finding_repro.py`, `recheck_findings.py` and `memory_findings.py`
  serve `fsl-flirt-part3` too (the double template of `applyxfm4D`, its integer splines, the
  scripts' messages; finding probe-part3b-01970 needs 9 GiB in FSL as in the replica).

```bash
python -P compare_images.py <fsl/out.nii.gz> <replica/out.nii.gz> [more pairs]
python -P finding_repro.py <workspace>/oracles/difftest/2026-11-01-fsl-flirt/findings/fsl-flirt/trial-flirt-00016-replica.json <scratch>/r16
LARMORX_NC_BIN=<larmorx-nc> python -P recheck_findings.py <run>/findings/fsl-flirt all 16
python -P crash_probe.py <larmorx-nc>
LARMORX_NC_BIN=<larmorx-nc> python -P memory_findings.py <out> 1800 <finding.json>... [--replica-only]
LARMORX_NC_BIN=<larmorx-nc> python -P rsstrace.py <out> -in @epi.nii -ref @t1full.nii.gz -omat out.mat -v
gdb -batch -x gdb_allocations.py --args <larmorx-nc> fsl flirt -in epi.nii -ref t1full.nii.gz -omat out.mat
python -P fsl67_measure.py <out>
python -P lapack_eig_svd_probe.py --tool <crates-nc>/target/release/lapack-probe --out <dir>
```

Run them with `PYTHONPATH=<checkout>/python:<checkout>/validation/src` and `FSLDIR` unset or
FSL 6.0.7.23's. The record itself is made with `python -m larmorx_validation parity
fsl-flirt-replica`; the probes beside it are in `validation/probes/fsl-flirt-costs` and
`validation/probes/fsl-flirt-pinv`.
- `normcorr_row_totals.py` (PRELUDE part's spec writer, 2026-11-06): builds a probe of the
  replica whose normalised correlation adds each row's total weight to the count's running total
  at the row's end (FSL adds every point's weight as it is visited): on `real/weights-normcorr`
  the probe gives the clean-room original's `-verbose 7` evaluations and final matrix, so that
  order is the cause of the original's record failure (`specs/flirt.md` §6.4, `specs/mcflirt.md`
  §6.4). The source is restored after the build.
