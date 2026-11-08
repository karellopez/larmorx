## The field maps' part A, sdcflows' port: final report (relays 1 to 11, 2026-11-08)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: the port author (sdcflows, niworkflows, nitransforms, nibabel, scikit-learn, SciPy and
numpy read; no clean-room crate, spec, replica crate, `crates-gpl/` or `crates-nc/` touched). Branch of 52 commits on
2b453c1, last eca11c3a. The part's agents: ad5d6c85d74dc98ba, a52fb97af12fd7a98, ab2c4c87b023ea94e,
aed354163dc359170, add71b54eddc33c55, a12f04869a7af7743, abf1e3da6428896bb, a3f6fcb919202dd47, a7b32ff0ee5bd279b,
a1349b2efb577f2fe, a60e3346e4d4eda3d.

### The port
`crates/larmorx-sdcflows` and `lx.sdcflows`: an exact port of sdcflows 2.17.1, the newest release (2.15.1, fMRIPrep
25.2.6's, is byte-identical on these paths): the B-spline grid, weights and collocation matrix; scikit-learn's `Ridge`
with `sparse_cg` and SciPy's `cg`; `BSplineApprox` complete (`resample_to_zooms`, `rescale_affine`,
`deoblique_and_zooms`); `B0FieldTransform` (fit, apply, to_displacements) and `ApplyCoeffsField`;
`TOPUPCoeffReorient`, `TransformCoefficients`, `reorient_pedir`; the field map and displacement conversions and
`DisplacementsField2Fieldmap`; the phase-difference preparation (`au2rads`, `subtract_phases`, `delta_te`,
`phdiff2fmap`, `check_b0_units`, `check_register`) and `phdiff_workflow` with `lx.fsl.prelude`. In `lx.io`: nibabel's
scaled writer and header arithmetic (`save_nibabel`, `load_nibabel`, `nibabel_affine`), with numpy's `svd`, `eigh` and
`det` replicated for LAPACK 3.12.0 and 3.12.1 (`larmorx_transform::numpy_linalg`). Not done, by decision:
`check_register`'s two-magnitude branch (FreeSurfer's `mri_robust_register`: a `NotImplementedError` until then);
sdcflows' command line (the end of part B; these interfaces have none, as the resampler's).

- Record `sdcflows-fieldmap` (standard tier, newest environment, from clean commit 3c9c10af): 37 cases, 37 pass, 821
  checks; the step from the shared environment: 33 cases identical, 4 changed, all named `LAPACK-3.12.1` (numpy 2.3.5
  to 2.5.4: nibabel's own quaternions and one orientation tie).
- fMRIPrep's real calls bit for bit in both environments: the seven phase-difference estimators (ds005454, ds002603,
  ds002422 twice, ds001600 three times), their seven `bs_filter` fits, fMRIPrep's own `coeff000`, `_extra` and
  `_fixed` files (TOPUPCoeffReorient on ds001600 and ds005040); the workflow with PRELUDE gives fMRIPrep's field map in
  every voxel.

### Defects (SDC-01 to SDC-11; no NP-, SCI- or SKL- defect)
Corrected by default (rule A, after measuring where rule D applied), `reproduce` sdcflows' bits: SDC-01 (weights beyond
the knot grid), SDC-02 (the rotated sampling grid: 3.65 Hz RMS in the brain on a real ApplyCoeffsField call), SDC-05
(an integer or float32 readout time crashes), SDC-06 (a 4D field map of one volume), SDC-07 (TOPUP's negation for BIDS
`i`), SDC-08 (a positive-axes series from a file), SDC-10 (the approximation ignores the data-to-field-map transform:
7.7 to 8.5 Hz RMS on ds002603's oblique references), SDC-11 (a resize onto a single voxel); a clear error (rule B):
SDC-04 (a constant phase map gives NaN), SDC-09 (a missing EchoTime); SDC-03 (CheckRegister's signed translation
test, rule A) with FreeSurfer. Kept and documented (rule C): CheckB0Units' re-quantisation, header zooms against the
affine, PIPE-14 (head motion ignored by `apply`, with sdcflows' own warning).

### Golden, memory, random testing, speed
Golden: 16 new cases (190 outputs, about 1.9 s together), recorded from 14ff7709; no other checksum changed. Memory at
one thread: fit ds002422 83 against 366 MiB, fit ds005454 468 against 1,473, ApplyCoeffsField on 20 volumes 828
against 3,740. Random testing: a half-hour trial of 335,471 cases (335,456 agree, 1 notice, 14 differ: 10 SDC-11, 4
larmorx mistakes fixed); 28 permanent cases. Speed, every output identical: the fit 6.5 to 7.8 times sdcflows' at 14
threads; at one thread 1.09 to 1.14 times on four of the seven calls (ds001600's three 0.86 times in the benchmark's
run); ApplyCoeffsField 1.7 times at 1 and 14 threads. The collocation matrix is stored at one thread only (2 to 3
times slower than the on-the-fly weights at 14 threads; the orchestrator's decision confirmed after relay 8).

### The site
A new page for sdcflows' field-map estimation: the three phase-difference estimators, the coefficients, the
unwarping with SDC-02, SDC-10 and SDC-06 on real inputs, the library step, speed, memory, random testing and every
case. Build 0 problems, numbers 158/158, every view draws (relay 10 found three views that drew nothing while the
site's viewer check passed: fixed, with a `drawn` check in `validation/investigations/sdcflows-fieldmap/site_checks.sh`),
every other page's HTML unchanged, screenshots at 1440 and 390 in both themes. `evidence-blobs` 96bca689 (1,275; 39
added, none replaced).

### Open
Commit 1c48bc82 is labelled "lint" but also holds the unwarp binding, the `larmorx-interp` dependency and `pub mod
unwarp`. The two oracle-cache disputes of relay 6 (keys `fac9bd9a…`, `ba61bf75…`, nipype's timestamps) left in the
cache's findings. Three tests fail only under pytest-xdist (two oracle-cache tests at `-n 8`, test_difftest's
memory-limit night at `-n 12`). The site's own viewer check should check that every viewer drew.
`lx.transforms.ensure_positive_cosines` still decides axes with vnl's SVD (NP-02; measure on the resampler's records
first). The one-thread fit slows in a process that already ran sdcflows (ds001600: 2.5 s fresh, 3.3 to 5.7 s after):
find the cause before larmorprepx runs fits in one long-lived process. The stored matrix's transpose is used only by
tests. When sdcflows' clean-room original starts, the audit's restriction table needs an sdcflows family.
