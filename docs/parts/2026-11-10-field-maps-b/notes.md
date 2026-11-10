# NOTES: field maps part B (the rest of sdcflows' estimation and correction, niworkflows' helpers, fMRIPrep's own field-map steps; last, sdcflows' command line)

Worktree (relay 2 onwards): <workspace>/larmorx/.claude/worktrees/agent-a33e6cc1a7edea6ca
(harness worktree of relay 2, from main f5e89ff4; branch `worktree-agent-a33e6cc1a7edea6ca`). Relay 1's harness
worktree `agent-a4cf64edc320dd324` held no commit.
Build: `CARGO_TARGET_DIR=<scratch>/target` (`<scratch>/r2/cargo.sh <args>`: env.sh + 14 jobs; `<scratch>/r2/wheel.sh`:
maturin wheel into `<scratch>/wheel`, its `_core.abi3.so` copied into the worktree's `python/larmorx/`). Never rebuild
the wheel while a check runs on it. Checks: `<scratch>/r2/run_check.sh <shared|newest> <script> [args]` (PYTHONPATH,
LARMORX_WORKSPACE, Haswell); heavier ones inside `scripts/orchestration/boxed.sh`.
Scratch: `<scratch>` = <scratch>/
Brief: `<scratch>/BRIEF.md`. Part A's documents: `docs/parts/2026-11-08-field-maps-a/` (notes.md: its pins,
inventory, design, the BLAS/numpy exactness rules, the sandbox notes; report.md).

Part's agents (ids for the audit's archive step): relay 1 = a4cf64edc320dd324 (the harness worktree's
name; an earlier attempt of relay 1 stopped after a few minutes left `scripts/` and `versions.py` here: its id
is the orchestrator's to add: aaea7f61392fc0d27); relay 2 = a33e6cc1a7edea6ca (the harness worktree's name);
relay 3 = ac1175bc60550e332 (it worked in relay 2's worktree; scratch helpers in `<scratch>/r3/`); relay 4 =
a1fd06ee6416ffe30 (relay 2's worktree; scratch helpers in `<scratch>/r4/`); relay 5 = a58deb7daf1547dd5 (relay
2's worktree; scratch helpers in `<scratch>/r5/`); relay 6 = ac5079c785f056c44 (relay 2's worktree; scratch
helpers in `<scratch>/r6/`); relay 7 = a2d2e605fadc15ca8 (relay 2's worktree; scratch helpers
in `<scratch>/r7/`); relay 8 = a15a4cf4128e2669f (relay 2's worktree; scratch helpers in `<scratch>/r8/`); relay 9 = ae78b46850d0dea1a (relay 2's
worktree; scratch helpers in `<scratch>/r9/`); relay 10 = a5033e389fa9c9e19 (relay 2's worktree; scratch
helpers in `<scratch>/r10/`); relay 11 = a1a6fcf4ee03fcfb5 (relay 2's worktree; scratch helpers in
`<scratch>/r11/`); relay 12 = ab43113819ad79ad9 (relay 2's worktree; scratch helpers in
`<scratch>/r12/`); relay 13 = a9b207f19eb5645ce (relay 2's worktree; scratch helpers in `<scratch>/r13/`); relay 14 = a7ededd34215e0b82 (relay 2's worktree; scratch helpers in `<scratch>/r14/`); relay 15 = a1daa4575f3643c99 (relay 2's worktree; scratch helpers in `<scratch>/r15/`); relay 16 = a19e3c20cbb8f7312 (relay 2's worktree; scratch helpers in `<scratch>/r16/`); relay 17 = ae878eb5ad8d31a90 (relay 2's worktree; scratch helpers in `<scratch>/r17/`, the handoff drafts in `<scratch>/handoff/`); relay 18 = a7ba42bb360567da2 (relay 2's worktree; scratch helpers in `<scratch>/r18/`, its handoff items in `<scratch>/handoff/relay18.md`); relay 19 = a902bb371637afc9b (relay 2's worktree; scratch helpers in `<scratch>/r19/`, its handoff items in `<scratch>/handoff/relay19.md`); relay 20 = aa4f59554aad847a1 (relay 2's worktree; scratch helpers in `<scratch>/r20/`, its handoff items in `<scratch>/handoff/relay20.md`); relay 21 = af0974b3dbdc43289 (relay 2's worktree; scratch helpers in `<scratch>/r21/`, its handoff items in `<scratch>/handoff/relay21.md`); relay 22 = a694a281b3b4a0f88 (relay 2's worktree; scratch helpers in `<scratch>/r22/`, its handoff items in `<scratch>/handoff/relay22.md`); relay 23 = a8db1ba28a0b88624 (relay 2's worktree; scratch helpers in `<scratch>/r23/`, its handoff items in `<scratch>/handoff/relay23.md`); relay 24 = a3f4ba1c1e9c59f40 (relay 2's worktree; scratch helpers in `<scratch>/r24/`, its handoff items in `<scratch>/handoff/relay24.md`); relay 25 = aabe6c62889486dea (relay 2's worktree; scratch helpers in `<scratch>/r25/`, its handoff items in `<scratch>/handoff/relay25.md`); relay 26 = acc44b330546a5099 (relay 2's worktree; scratch helpers in `<scratch>/r26/`, its handoff items in `<scratch>/handoff/relay26.md`); relay 27 = aff910a486c582678 (relay 2's worktree; scratch helpers in `<scratch>/r27/`, its handoff items in `<scratch>/handoff/relay27.md`); relay 28 = aa113697e2f6b45b7 (relay 2's worktree; scratch helpers in `<scratch>/r28/`, its handoff items in `<scratch>/handoff/relay28.md`); relay 29 = ac9c94451e2b8426e (relay 2's worktree; scratch helpers in `<scratch>/r29/`, its handoff items in `<scratch>/handoff/relay29.md`); relay 30 = a0e0bb7a68ebd3989 (relay 2's worktree; scratch helpers in `<scratch>/r30/`, its handoff items in `<scratch>/handoff/relay30.md`); relay 31 = a56b27a2845fe52d5 (relay 2's worktree; scratch helpers in `<scratch>/r31/`; its work is the handoff itself, `<scratch>/HANDOFF.toml` and `<scratch>/HANDOFF.md`); relay 32 = a5c1cdfa1e9ad900e (relay 2's worktree; scratch helpers in `<scratch>/r32/`).

## Status by relay
- **Relay 1 (done, no commit by design):** pins fetched into `<workspace>/tags/` (below), the inventory, the
  paths fMRIPrep runs and the oracle runs that reach them, five measurements, the defect candidates, the
  design, the decisions for the orchestrator, the relay plan. Helper scripts in `<scratch>/scripts/` (list at
  the end; relay 2 commits the ones behind findings under `validation/investigations/sdcflows-fieldmap-b/`).
- **Relay 2 (done, 2026-11-08): commits 1b8a2643, bf4684d4, e98c05f4** (on main f5e89ff4).
  - `svd3` signed zero (1b8a2643): `dbdsqr_upper` takes `clear_negative_zero` (3.12.1's "avoid -ZERO" only),
    `dbdsdc_upper` takes the release; unit case `singular_value_zero_sign_follows_the_release` (numpy 2.3.5's
    values); `check_numpy_linalg.py` gained the family `singular` (3,000): all 15,048 matrices identical in both
    envs. **MERGE NOTE for the orchestrator:** the t2smap branch rewrote `dbdsqr_upper` as a wrapper of
    `dbdsqr_upper_ncc(..., clear_negative_zero)` passing `true`; at the merge keep its `_ncc` function, give its
    wrapper the parameter `clear_negative_zero: bool` and pass it through (dbdsdc_upper calls
    `dbdsqr_upper(n, d, e, vt, u, release == LapackRelease::V3_12_1)`); the label-160 line is the same text in both.
  - `larmorx_core::numpy` (bf4684d4): `NumpyRelease {V2_3, V2_5}` (+ `from_version`), `quantile::
    percentile_linear_weak` (Python-number q; 2.3: q/f32(100), vi and gamma in f32; 2.5: f64, gamma rounded to f32
    in `_lerp`; gamma subtracts the INTEGER index: a -0 vi gives a -0 weight), `percentile_linear` (list q, f64),
    `median`, `median_axis` ((outer, n, inner) of memory: F-ordered series along t = (1, t, xyz)), `PercentileError
    {Range, Empty}`; `umath::exp_f32` (= `math::expf_numpy`, registered with the FMA dispatch like logf_numpy) and
    `umath::clip`. Checked: `check_numpy.py` all identical in numpy 2.3.5 and 2.5.4 (819 percentile cases, 43 list,
    43 median, 40 median-axis, clip, exp; `--expf`: all 2^32 float32 inputs, default AVX-512 path and AVX-512 off).
  - **new crate `crates/larmorx-ndimage`** (D4: filters, morphology, measurements; interpolation stays in
    larmorx-interp): `Layout` (shape + element strides, any order), `NdElement` (u8=bool, i8..u64, f32, f64; x86
    casts `cvtt_i32/i64/u64`, SAFE output cast, SSE NaN), `Footprint`, `Extend`; `min_or_max_filter<T, O>` (running
    result a DOUBLE as SciPy's `tmp`: first value + ss[0] in double, others `x + (T)ss` in T, compared as double),
    `min_or_max_filter1d<T, O>` (NI_ExtendLine with cval as double, Harter), `rank_filter<T, O>` (NI_Select),
    `rank_filter_1d` (Ashelly's Mediator, MIT notice in filters.rs; SciPy 1.18's 1-D dispatch:
    `lim2 = size - ((fp.size-1)//2 - origin)`, the footprint's SIZE as window), `binary_erosion` (one pass; cit &&
    !brute_force: propagation from the changed list with the int8 mask; else whole passes; 2-cycle -> error),
    `dilation_arguments`, `center_is_true` (numpy indexing; centre before the structure -> `Origin` error where
    SciPy segfaults), `generate_binary_structure`, `iterate_structure`, `label` (union-find, numbered by first
    element in C index order; int32/int64), `sum_labels_bincount`. Outputs C-ordered; parallel over output lines.
  - Python: `python/larmorx/_ndimage_filters.py` (SciPy's wrappers and messages: minimum/maximum_filter(1d),
    rank/median/percentile_filter, grey_*, binary_* incl. propagation, fill_holes, hit_or_miss,
    generate_binary_structure, iterate_structure, label, sum_labels/sum), exported by `lx.ndimage`; bindings
    `crates/larmorx-py/src/ndfilters.rs` (`ndimage_*` with an `output` dtype name: same type -> <T,T>, else <T,f64>
    then SciPy's cast in Rust; private `numpy_percentile_weak`, `numpy_percentile`, `numpy_median`, `numpy_exp_f32`,
    `numpy_clip_f32`); stubs in `_core.pyi`; API page `docs/api/ndimage-filters.md`; findings
    `docs/findings/scipy-ndimage-filters.md`; PROVENANCE; investigations `validation/investigations/sdcflows-fieldmap-b/`
    (README; `check_ndimage.py` 39,436 cases identical in BOTH envs; `check_numpy.py`; `check_masker_steps.py`:
    IntensityClip from lx pieces = niworkflows' own 18/18 per env and (numpy 2.3 percentile) fMRIPrep's own
    clipped.nii.gz 18/18; numpy 2.5's percentile changes 6 of those 18 (THE STEP: record it); brain_masker up to the
    random walker identical 9/9 per step per env; `probe_origin_crash.py`, `probe_int_casts.py`; `relay1/` (relay 1's
    scripts, machine paths replaced by LARMORX_WORKSPACE/OUT/FSLDIR); `relay2/` probes).
  - Golden: 4 cases registered in `tests/golden/registry.py` (`py.ndimage.filters`, `.grey`, `.binary`, `.label`;
    they run) but NOT recorded: `tests/golden` fails on them until R10 records from a clean commit.
  - Gates run: cargo fmt --check (root), clippy -D warnings on larmorx-ndimage/-core/-transform/-py, cargo test of
    larmorx-core/-transform/-ndimage, the three Cargo.lock files `--locked` OK, ruff check + format (repo), check_spdx,
    pytest tests/python tests/golden validation/tests (only the 4 unrecorded golden cases fail). Not run: the full
    workspace tests, crates-gpl/crates-nc gates, the smoke tier (R10).
  - Speed: brain_masker's ndimage steps 5.5 s at 4 threads vs SciPy 5.4-5.8 s (one thread): NOT faster yet;
    profile in R3 (likely the ball(5) binary passes and the ball(3) median; e.g. erosion/dilation with early exits
    in footprint order are already there; consider a run-length/decomposed structure path that keeps the bits).
- **Defect candidates found in R2 (IDs at R9; SciPy's family next free SCI-06 in main: CHECK the t2smap part's use
  first; details in docs/findings/scipy-ndimage-filters.md):** (1) binary erosion/dilation with an origin putting the
  centre before the structure: SciPy segfaults/garbage -> larmorx ValueError('invalid origin') (B); (2) nd `reflect`
  maps 2m·2n before an axis to index -1 (OOB read) -> larmorx 0 (A); (3) `_rank_filter_1d` reads outside a short input
  -> larmorx the mode's extension (A); (4) `_rank_filter_1d` uses footprint.size (holes ignored) -> larmorx = SciPy
  now; decide A (fix via the nd filter, reproduce switch) in R9; (5) propagation reads the mask as int8 -> larmorx =
  SciPy now; decide A/C in R9; (6) iterations<1 with a non-centred structure can loop for ever -> larmorx error on a
  repeated state (B); (7) structure added in the element type (C, keep). Step SciPy 1.15 -> 1.18: 1-D rank on an input
  shorter than half the footprint returned zeros in 1.15 (gh-23293; fixed upstream, like SCI-03). numpy: the weak-q
  percentile step 2.3 -> 2.5 (not a defect: a step; the record shows it per case).
- **For the orchestrator (handoff items so far):** NOTICE: add "SciPy 1.18.1 (scipy/ndimage: filters, morphology,
  label, sum_labels; crates/larmorx-ndimage)" to the SciPy entry and ashelly's MIT notice for `_rank_filter_1d.cpp`
  (copied in crates-gpl/ and crates-nc/ NOTICE copies; not edited in the branch); the shared
  `nanpercentile_linear_axis0` (t2smap's) subtracts the float floor in gamma (differs from numpy only for q = -0.0):
  align when that module is next touched; `validation/tools.toml` entry for `ndimage-filters` at R9/R10.
- **Relay 3 (done, 2026-11-08): commits 7676de9f, dd1a4f59, 4dfdc97a** (on 1b8a2643..e98c05f4). R3 split in
  two: relay 3 did the library ports and the masks; **relay 4 = R3b** (below) does the workflows.
  - `larmorx_core::numpy`: `dot` (moved from sdcflows' `blas.rs`, which re-exports it; + Haswell `ddot`, its tail
    UNFUSED: measured; FMA-dispatched kernels, audited `unsafe` module), `histograms` (`linspace`, `histogram`
    equal bins, `min_max`), `NpFloat::{div, np_exp, np_dot}`, `reduce::SumOrder {Pairwise (numpy >= 2.3),
    Buffered (numpy <= 2.2: whole-array sums in 8192-element chunks)}`, `add_reduce_ordered`, `std_ordered`;
    `nanpercentile_linear_axis0` keeps a -0 weight for q = -0.0 (the orchestrator's item; unit case; numpy 2.3.5 and
    2.5.4 both give -0).
  - `larmorx-ndimage::skimage`: `ball`, `threshold_otsu` (float, `_int`, `_hist`), `random_walker` (`preprocess`,
    `RandomWalker {std: Option, spacing F32|F64, mode Cg|CgJ, order Scipy115|Scipy118, numbering Mask|Edges}`,
    ELL rows, per-label CG in parallel, `RwOutput::{full_prob, labels}`, `numbering_differs` = SKI-01).
  - `larmorx-sdcflows::masker::brain_masker` (MaskerOptions {padding, numpy, sum_order, scipy, reproduce}); sdcflows
    now depends on larmorx-ndimage (the three Cargo.lock files OK).
  - Python: `lx.skimage` (new family: ball, threshold_otsu, random_walker with every argument, scikit-image's
    checks/warnings/early returns, `img_as_float`; `mode='bf'` NotImplementedError, `cg_mg` -> cg_j with skimage's
    warning, `numpy_version` for std); `lx.sdcflows.{brain_masker, brain_extraction, binary_dilation, union,
    BrainMaskerResult}` (`python/larmorx/sdcflows/masks.py`); `lx.niworkflows` (new family:
    `intensity_clip`, `IntensityClipResult`, `python/larmorx/niworkflows/nibabel.py`); bindings
    `crates/larmorx-py/src/skimage.rs`; stubs; `lx.skimage`/`lx.niworkflows` registered submodules and golden
    namespaces.
  - Checks (`validation/investigations/sdcflows-fieldmap-b/check_skimage.py`, sections ball,dot,hist,otsu,rw,
    masker,clip; scratch runners `<scratch>/r3/synth.sh`, `masker.sh`): ALL identical in both envs: ball 24, dot/norm
    1,218, histogram 128, otsu 52, random_walker 10 (+14 float64 cases compared with numpy's AVX-512 dispatch off in
    the shared env: 24/24; SKI-01's 2 cases differ by default and agree with reproduce), warnings identical;
    brain_masker vs sdcflows' own files (data + header bytes) 27/27 per env; vs fMRIPrep's own masker files 27/27
    (numpy "2.2.6" -> Buffered sums + 2.3 percentile, SciPy "1.15.2" order); IntensityClip vs niworkflows' own
    files and fMRIPrep's clipped.nii.gz 18/18 + 18/18 per env.
  - **FINDING (for the orchestrator and the t2smap part):** numpy 2.2.6 (fMRIPrep's image) sums a whole contiguous
    array in buffered chunks of 8192 elements; numpy 2.3+ pairwise over the whole array (ds002603: 2125824.0 vs
    2125823.8; reached brain_masker through random_walker's `data.std()`: 5 of 9 probsegs differed until
    `SumOrder`). Any port compared with fMRIPrep's image that takes a whole-array sum/mean/var/std of > 8192
    elements needs it (tedana's t2smap in fMRIPrep's runs). `docs/findings/sdcflows-estimation.md`; probes
    `relay3/`.
  - Defects: **SKI-01** used in the warning (rule A: a masked Laplacian numbered by the edges' ends; corrected by
    default, reproduce bit for bit); NOT yet in `larmorx_core::defects` nor the register (R9). Candidates in the
    findings file: SDC-h (no component: error by default), no brain seed after erosion (error by default), SDC-g,
    random_walker's renumbered output labels, return_full_prob concatenation without zeros, PIPE-a.
  - Golden: `py.skimage`, `py.sdcflows.masks`, `py.niworkflows.intensity_clip` registered (they run; NOT recorded:
    R10), plus relay 2's four `py.ndimage.*`.
  - Docs: `docs/api/skimage.md` (option mapping), `docs/findings/sdcflows-estimation.md` (new), PROVENANCE of
    larmorx-core, larmorx-ndimage (skimage section), larmorx-sdcflows (part B section); README of the
    investigations folder.
  - Gates run: cargo fmt; clippy --workspace --all-targets --locked -D warnings (root) clean; cargo test of
    larmorx-core/-ndimage/-sdcflows; Cargo.lock x3 OK; ruff check + format (repo) clean; check_spdx; pytest
    tests/golden coverage passes (the 7 unrecorded cases fail as expected). Not run: full workspace tests,
    crates-gpl/nc gates, pytest tests/python in full (relay 2's ran), the smoke tier.
  - Speed and memory (ds002603, 128^3 -> 138^3 padded, newest env): 16.2 s at 1 thread (sdcflows 8.2 s: **2x
    slower**), 5.2 s at 4, 3.5 s at 14; peak RSS 404-442 MiB (sdcflows 1,438). The nine nodes: 10.5 s at 4 threads
    vs sdcflows 15.7 s single.
  - Not done in relay 3 (-> relay 4): IntraModalMerge, the workflows, the speed profile; `lx.skimage.random_walker`
    with `copy=False` does not yet write `-1` into the caller's labels in the masked (not all-isolated) path, as
    `_preprocess` does in place (small; do it with R9's switches); `mode='bf'` (SciPy's spsolve/SuperLU) not ported
    (say so in R9's API page; decide whether to port).
- **Relay 4 = R3b (done, 2026-11-08): commits b4fb6e26, 06430d10, f4a024e7, 18be6867, 5c3b3599** (on 4dfdc97a).
  - **`lx.niworkflows.intra_modal_merge`** (`python/larmorx/niworkflows/images.py`, with `reorient`,
    `as_closest_canonical` (nibabel's, `dim_info` moved), `IntraModalMergeResult`): every option (hmc through
    `larmorx fsl mcflirt` with nipype's MCFLIRT arguments, `hmc_implementation`; to_ras; zero_based_avg;
    grand_mean_scaling). The header goes through nibabel's constructors (`_constructed`: `set_data_shape` only when
    the shape changes, then later dims 1 and pixdims 1.0: a scanner header loses its TR) -- writing with the first
    file's header failed 6 of fMRIPrep's 7 magmrg files. The axis-3 mean is numpy's SEQUENTIAL sum from +0 (the
    summed axis outermost in memory) in numpy 2.2.6, 2.3.5 and 2.5.4 (`relay4/probe_merge_mean.py`, image too).
    **PIPE-18** (grand_mean_scaling's `mask[..., np.newaxis]` index: numpy IndexError for every series of >= 2
    volumes; rule A: per-volume median in the mask, warning; reproduce = numpy's IndexError) and **PIPE-19** (6/7-D
    images pass the 5D check; rule B: niworkflows' error; reproduce = numpy's mean) used in code (Python notices),
    NOT yet in `larmorx_core::defects` nor the register (R9). Check `check_merge.py`: 62 merges, 5 errors, PIPE-19,
    16 `_ras` files + 2 to_ras errors, PIPE-18 x2, hmc vs FSL 6.0.7.23 mcflirt through the oracle cache with the
    replica (`LARMORX_NC_BIN=<main checkout>/crates-nc/target/release/larmorx-nc`; descrip apart: FSL writes its
    version/time), fMRIPrep's 7 magmrg files: ALL identical in both envs.
  - **`lx.sdcflows.brainextraction_workflow`, `magnitude_workflow`, `fmap_workflow`** (`python/larmorx/sdcflows/
    workflows.py`; results `BrainExtractionWorkflowResult`, `MagnitudeWorkflowResult`, `FmapWorkflowResult`): node
    folders as nipype's under out_dir (temporary folder otherwise, paths stripped to None); N4 node =
    `lx.ants.n4_bias_field_correction(-d 3, 5x50, 1e-07, shrink 4, --bspline-fitting [%g])` + nipype's
    `copy_header(keep_dtype=False)` (ANTs' float32 values stored in the clipped int16 header with nibabel's scaling);
    `fmap_workflow` both modes (mapped: `KeyError('Units')` as sdcflows' `_get_units`). Part A's `phdiff_workflow`
    FIXED: PRELUDE's output named `<phase>_unwrapped.nii.gz` (nipype's FSL output type) also for `.nii` phases
    (ds002422). Check `check_workflows.py` (stand-ins `N4BiasFieldCorrection`/`prelude` on PATH run ANTs one thread
    + CORE-MATH and FSL's prelude through the oracle cache, recursion guard): brain-extraction 54/54, phasediff
    108/108 node files + 7/7 outputs, mapped (ds005752 Hz, rad/s int16 and float32, two maps) 51/51 + 4/4, in BOTH
    envs; `image` section (fMRIPrep's libraries): magmrg 7/7 and clipper_pre 7/7 = fMRIPrep's files; N4 (8 threads
    there) 36-66 % voxels identical, max 0.018; clipper_post <= 1 count on 0-0.16 % voxels; mask 100 %; probseg
    5-13 % identical (max 0.063); **the final field maps (bs_filter `_extra`) = fMRIPrep's 7 of 7**.
  - **Speed (brain_masker at one thread; the port is no longer slower):** relay 3's 16.9 s on ds002603 -> 6.2-6.7 s
    (sdcflows 7.6-8.0 s, paired, newest env); nine nodes 12.1 s vs 15.6 s; memory 422 MiB (1 thread) / 432 MiB (14)
    vs 1,439 MiB. Changes (all exact): `larmorx-ndimage` binary erosion pass prepares each line (elements inside on
    the other axes: `Neighbourhood::line_elements`) and for u8/bool inputs with a contiguous last axis and no mask
    decides 8 positions per 64-bit word (`NdElement::as_bytes`, SWAR nonzero test; change lists sorted per line);
    dilation ball(5) 0.20 s vs SciPy 0.44 s (1.38 s before); min/max and rank filters read interior elements at
    base + relative offsets; `random_walker`'s CG: all labels in lockstep (`cg_lockstep`, `matvec2` reads the
    matrix once for two labels), fast IEEE paths (`Plain` trait) with every NaN result recomputed with the exact
    NpFloat rules (row sums, blocked vector updates). Remaining one-thread gaps vs SciPy: median_filter ball(3)
    0.97 vs 0.81 s, grey_closing ball(1) 0.23 vs 0.08 s (min/max filter), CG ~4 s; `bspline_approx` (part A) takes
    1.3 s at one thread on a 30x28x26 golden field (684 CG iterations of 324 unknowns) -- part A's to look at.
  - Re-checked after the speed work: `check_ndimage.py` all 39,436 cases, `check_skimage.py` rw/masker/clip ALL OK in
    both envs (masker 27/27 sdcflows' + 27/27 fMRIPrep's).
  - Golden: `py.niworkflows.intra_modal_merge` (0.06 s) and `py.sdcflows.workflows` (1.4 s at one thread; 1 mm
    voxels and smooth fields keep the fits short) registered, NOT recorded (R11 = old R10); coverage test passes.
  - Docs: `docs/api/niworkflows-helpers.md` (new: IntensityClip, IntraModalMerge option tables, PIPE-18/19);
    `docs/api/sdcflows-fieldmap.md` (the workflows' section); `docs/findings/sdcflows-estimation.md` (the header
    chain, the sequential mean, PIPE-18/19, the workflows, PRELUDE's name, N4's copy_header, the speed work);
    investigations README rows (`check_merge.py`, `check_workflows.py`, `relay4/`). No new `_core` function (no
    `.pyi` change); no PROVENANCE change needed beyond R9's (niworkflows' images.py + nibabel funcs: add to the
    larmorx-py/python provenance at R9 -- the Python files carry SPDX headers).
  - Gates run: cargo fmt --check (root), clippy -D warnings (larmorx-ndimage, -sdcflows, -py), cargo test of
    larmorx-core/-ndimage/-sdcflows, ruff check + format (repo), check_spdx, pytest tests/python + validation/tests
    (843 passed; `test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on` failed only inside a 12 GiB
    box with -n 8 and passes alone: the harness test, not this relay's code). Not run: the full workspace tests,
    crates-gpl/nc gates, the smoke tier (R11).
  - For the orchestrator / handoff: the oracle runs the orchestrator is making now (ds006736, ds004958-pepolar,
    ds002603-epi, ds005752-direct) land in `oracles/fmriprep-25.2.6/`; the checks now skip unfinished nodes
    (result pklz) and the phase-difference collector skips direct maps. When ds005752-sub-ON01016-noreconall-direct
    finishes, its fmapmrg/units/bs_filter nodes are the direct mode's real fMRIPrep calls (relay 5 or the records
    relay: add a `direct` section to check_workflows.py comparing fMRIPrep's node files, as `image` does).
- **Relay 5 = R4 PEPOLAR (done, 2026-11-08): commits e5b593e4..89030087** (10 commits on 5c3b3599). Scratch helpers in `<scratch>/r5/` (`check.sh <shared|newest> <sections> [runs] [threads]`
  runs `check_pepolar.py` with LARMORX_NC_BIN; `run_golden_new.py <case ids>` runs unrecorded golden cases once).
  - **`python/larmorx/sdcflows/pepolar.py`** (new; exported by `lx.sdcflows`): `get_trt`, `get_readout_time`
    (`GetReadoutTimeResult`), `select_pe_volumes` (2.17.1), `sort_pe_blips`, `uniform_grid` (nitransforms' cubic
    `Affine.apply` via part A's `_core.sdcflows_affine_coordinates` + `lx.ndimage.map_coordinates`; 3D only on the
    resampling path, NotImplementedError for 4D: documented), `pad_slices`, `reorient_image_and_metadata` (**SDC-12**,
    rule A, used in code: the upper-cased code; NOT yet in `larmorx_core::defects` nor the register), `topup_config_file`
    (D9 in ONE function: sdcflows' own `b02b0.cnf`, `b02b0_quick.cnf`, `b02b0_{1,2,4}_quick.cnf` shipped in
    `python/larmorx/sdcflows/data/flirtsch/`; FSL's `b02b0_{1,2,4}.cnf` from `fsldir/etc/flirtsch`, else
    `<larmorx-nc prefix>/share/larmorx-nc/fsl-data/etc/flirtsch/` (beside its program, or sys.prefix), else
    FileNotFoundError naming both), `select_topup_config`, `topup_node` (nipype's TOPUP command line and encfile, run
    in the node dir through the replica (`_replica.run`, cwd) or the original (`fsl.topup._run_original`),
    `FSLOUTPUTTYPE=NIFTI_GZ`), `topup_mats_to_world`, `topup_workflow(wiring="2.17"|"2.15", debug, sloppy,
    topup_config, max_vols_per_pe, ...)` → `TopupWorkflowResult` (node results in `.nodes`).
  - **`lx.niworkflows`**: `robust_average` (`RobustAverageResult`; nipype's `Volreg` argv `-twopass -Fourier -1Dfile
    <s>.1D -1Dmatrix_save <s>.aff12.1D -prefix <s>_volreg.nii.gz -zpad 4 -maxdisp1D <s>_md.1D <in>` through the
    3dvolreg replica (built-in) or the original; mcflirt `-sinc_final` for `mc_method="FSL"`; the drift mean in numpy's
    order per release: new binding `_core.numpy_add_reduce(a, numpy_version)` = `add_reduce_ordered`), `merge_series`,
    `reorient_file` (ReorientImage), `reorient_image`, `as_reoriented`, `axcodes2ornt`, `aff2axcodes`,
    `ornt_transform`, `squeeze_image`, `four_to_three` (relay 4's `_concat` gained `check_affines`).
  - **Checks** (`validation/investigations/sdcflows-fieldmap-b/check_pepolar.py`, sections units, ravg, wf, image; 3dvolreg
    (AFNI 26.3.05), topup (FSL 6.0.7.23) and N4 stand-ins through the oracle cache; sdcflows 2.15.1's `pepolar.py`
    loaded from `tags/` against the installed interfaces):
    - units (both envs): get_trt 192, GetReadoutTime 10, SortPEBlips 3, TOPUP encfile + cmdline 3+3 (nipype's
      "thelength" message too), drift mean 4, UniformGrid 12 outputs / 8 files (ds001600's resampled run, synthetic
      equal/near/shifted/rotated, float32/int16), SelectPEVolumes 12-18 / 72-84, MergeSeries, PadSlices (odd, scaled
      int16, uint8, float32), ReorientImageAndMetadata (LAS/RAS/LPI/PIR/las), ReorientImage, _select_topup_config,
      _topup_mats_to_world: ALL identical.
    - ravg (both envs): RobustAverage every option, 3D, one volume, errors: 24-35 outputs, 51 files identical (14 of
      them 3dvolreg outputs equal but for AFNI's history extension + vox_offset: counted apart).
    - image (numpy 2.2.6 arithmetic, shared env): ds001600 + ds005040 + ds006736 (D8 run, finished 18:46, exit 0) =
      fMRIPrep's own files node by node: runwise_avg 4, regrid 2, concat 3, setwise_avg 6, fix_coeff 3, from_las 3,
      from_las_fmap 3, ref_average 3, topup command line 3; the topup replica = FSL 6.0.7.23's own topup (oracle cache)
      on fMRIPrep's 3 inputs (24 images, 6 matrices); fMRIPrep's image topup (fsl-topup 2203.5, its own build, old
      laptop) = FSL 6.0.7.23 on ds001600/ds005040 but NOT on ds006736 (0.15 % of field voxels identical, max 0.58 Hz;
      matrices identical) -> INFO, cause to measure (build/libraries vs CPU kernels: run the image's topup on the main
      machine through the oracle cache) in R10. ds006736's regrid: data identical, quatern_b -1.4631e-16 vs fMRIPrep's
      -1.5087e-16 (zero up to rounding): fMRIPrep's image reproduces its own value without OPENBLAS_CORETYPE on the
      main machine, ours with Haswell (`relay5/probe_quaternion.py`): counted apart as "quaternion rounding only";
      R10 to settle. Only brainextraction's N4 differs otherwise (8 threads there: relay 4).
    - wf (sdcflows' own graphs, 2.15.1 from tags/ and 2.17.1 installed; 3dvolreg/topup/N4 through the oracle cache):
      EVERY node file identical in both envs: shared ds001600+ds005040 2.15 61, 2.17 73, debug 39, sloppy 38 (+ ds006736
      25/24/25/24); newest ds005040+ds006736 55/59/36/35; configs b02b0_4 (80x80x60, 64x64x40), b02b0_2 (74x64x58);
      ds001600's and ds006736's PA runs are resampled by UniformGrid (their voxel sizes differ from the AP runs', so
      2.17.1's warning fires for them, in sdcflows and larmorx alike). Times (4 threads, programs cached on the sdcflows side): sdcflows 734 s vs larmorx
      673 s (shared), 419 vs 364 s (newest): topup dominates both; a benchmark with fresh FSL runs is R10's.
  - Golden: `py.niworkflows.robust_average` (0.4 s) and `py.sdcflows.pepolar` (1.0 s: both wirings, topup's
    clean-room original, 2.17 with `topup_config=b02b0_2_quick.cnf` + debug) registered, NOT recorded (R11).
  - Docs: API sections (`docs/api/sdcflows-fieldmap.md` "The PEPOLAR estimation", SDC-12 row;
    `docs/api/niworkflows-helpers.md` RobustAverage, MergeSeries/ReorientImage tables), findings
    (`docs/findings/sdcflows-estimation.md` "The PEPOLAR estimation"), PROVENANCE (larmorx-sdcflows part B PEPOLAR
    section), investigations README row.
  - Gates run: cargo fmt --check (root), clippy -D warnings on larmorx-py, ruff check + format (changed files),
    check_spdx, pytest tests/python + tests/golden (all pass but the 11 unrecorded golden cases: relays 2-5's).
    Not run: the full workspace tests, crates-gpl/nc gates, validation/tests, the smoke tier (R11).
  - Handoff items from relay 5: the register and `larmorx_core::defects`: SDC-12 (sdcflows, rule A); larmorx-nc: add
    FSL's `b02b0_1.cnf`, `b02b0_2.cnf`, `b02b0_4.cnf` (FSL 6.0.7.23's `etc/flirtsch/`, byte-identical to sdcflows'
    copies) under `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/etc/flirtsch/` with the manifest and the
    licence files (as BET's standard files; D9 for now); NOTICE: sdcflows' configuration files shipped in
    `python/larmorx/sdcflows/data/flirtsch/` (Apache-2.0); `docs/validation/golden.md`: the two new cases;
    `validation/tools.toml`: the PEPOLAR workflow under `sdcflows-estimation`, RobustAverage/MergeSeries/ReorientImage
    under `niworkflows-helpers`.
- **Relay 6 = R5a (done, 2026-11-08): commits 6039a064, 799b3c03, 7df61ec3, ba66d32a, 2b7ae0b1, b401e22b** (on 89030087). **R5 split in
  two** (it was too large for one relay): relay 6 = R5a (nipype's command lines, the header helpers, the
  registration and skull-stripping workflows); **relay 7 = R5b** (fMRIPrep's own steps: below). Scratch helpers in
  `<scratch>/r6/` (`check.sh <shared|newest> <script> [args]`: a check of `validation/investigations/
  sdcflows-fieldmap-b/` with LARMORX_NC_BIN; `probe_reg.py`, `dump_specs.py` (committed under `relay6/`)).
  - **`python/larmorx/_nipype.py`** (new, private): nipype 1.12.0's command lines as one module: a generic
    `_Interface` (nipype's `_parse_inputs`: positions, then by name, negative last; `_format_arg` with `shlex.quote`
    for single paths only, lists unquoted as nipype; `_filename_from_source` + `_overload_extension`;
    `_list_outputs`) and the interfaces `registration(from_file=JSON or dict, **inputs)` (FixHeaderRegistration;
    nipype's traits' coercions: float stage settings `6` -> `6.0`, `transform_parameters`' Either order
    F/FFF/FIII/FIFFFF/FFFI/FIIII), `apply_transforms`, `n4_bias_field_correction` (save_bias, bspline_order),
    `image_math` (`no_copy_header`), `denoise_image` (save_noise), `bet` (relpaths), `binary_maths`,
    `multi_image_maths`, `topup` (encoding file in `Command.files`), `automask`, `volreg`; `Command(program, args,
    cwd, outputs, environ, files, run_args)`: `.cmdline` = command.txt, `.argv()` = the shell's words,
    `.run_argv()` = output names absolute (in-process ANTs); `run(cmd, implementation, n_threads,
    upstream_defects, ants_random_seed)`: ANTs (Registration, N4, ImageMath, DenoiseImage) through
    `_core.ants_program` in-process, FSL bet/fslmaths/topup through replica or `_core.fsl_run_original` in the node
    dir (FSLOUTPUTTYPE from the command), AFNI 3dAutomask/3dvolreg through the built-in replica (original:
    NotImplementedError: typed API only). **antsApplyTransforms is formatted but NOT runnable** (`_core.ants_program`
    has no antsApplyTransforms; relay 8 (SyN) adds it: a `main_with`-based binding in `crates/larmorx-py/src/
    ants_filters/mod.rs` or `lx.ants.apply_transforms` typed). Also `split_filename`, `fname_presuffix` (nipype's
    full signature), `ensure_list`, `load_settings`. RobustAverage's 3dvolreg (`niworkflows/images._volreg_nipype`)
    and sdcflows' `topup_node` now take their lines from it (check_pepolar units + ravg re-run: all identical).
    Part A's private `_split_filename`/`_fname_presuffix` in `sdcflows/approx.py` are left as they are (callers
    unchanged; they can delegate later).
  - **`lx.niworkflows`**: `copy_xform` (`_copyxform`: q/sform with codes, a code-0 form sets the code only,
    `descrip` padded), `copy_header` (CopyHeader `_fixhdr`), `apply_mask` (ApplyMask), `key_select` (KeySelect),
    `skullstrip_bold_workflow` (init_skullstrip_bold_wf: BET -f 0.20 -m, 3dAutomask -dilate 1 NIFTI_GZ, BinaryMaths
    mul, ApplyMask; the inputs linked into the node folders as nipype links them; `SkullstripBoldResult`)
    (`python/larmorx/niworkflows/header.py`). **`lx.sdcflows`**: `coeff2epi_workflow` (init_coeff2epi_wf: two
    BinaryDilation(5), FixHeaderRegistration with `fmap-any_registration[_testing].json`, debug's warped images +
    `copy_xform` with `niworkflows_version`, `write_coeff` warning, `ants_random_seed`), `registration_settings`
    (sdcflows 2.17.1's six ANTs JSON files shipped in `python/larmorx/sdcflows/data/`), `Coeff2EpiResult`
    (`python/larmorx/sdcflows/registration.py`).
  - **Checks** (`validation/investigations/sdcflows-fieldmap-b/`): `check_nipype.py`: every command.txt of the
    oracle runs for the ported interfaces = ours character for character: antsRegistration 77, antsApplyTransforms
    663, N4 95 (63 nipype + 32 FixN4), ImageMath 577, DenoiseImage 29, bet 46, fslmaths (BinaryMaths) 46,
    3dAutomask 46, topup 4 (+ 4 encoding files vs nipype's own): **1,583/1,583 in both envs**; nipype's own cmdline
    + `_list_outputs` on 14 synthetic option sets: 14/14 (RobustMNINormalization `norm` 21 nodes: another
    interface). `check_bold_fmap.py`: `nodes` (all 14 finished BOLD runs with a field map incl. the D8 runs
    ds006736, ds004958-pepolar and the three SyN runs): dilated masks 28/28 = fMRIPrep's, skullstrip_bold_wf
    70/70 files = fMRIPrep's own (BET/fslmaths replicas FSL 6.0.7.23, 3dAutomask replica), coregister vs
    fMRIPrep's own (6 threads, its build) within 9.9e-12 per parameter (INFO; the panel's thread band); `wf`
    (ds002422 phasediff, ds005040 PEPOLAR, ds000228 SyN): sdcflows' init_coeff2epi_wf graph with ANTs 2.6.5 one
    thread + CORE-MATH through the oracle cache: node files 6/6, transforms 3/3, debug 4/4 + 1/1; niworkflows'
    init_skullstrip_bold_wf graph: 18/18 (6 AFNI-history-only): **ALL OK in both envs**.
  - **FINDING: fMRIPrep seeds ANTs** (`ANTS_RANDOM_SEED`, 50495 for `--random-seed 0`, in each node's report);
    unseeded, ANTs seeds Random sampling from the clock (a different transform per run; relay6/probe_reg.py:
    0.37 mm apart). The check passes the run's seed (`ants_random_seed`, given as `--random-seed`, which ANTs
    prefers to the variable). Any later port of fMRIPrep's ANTs calls (SyN's epi2anat/syn, the templates) needs the
    seed the same way. In docs/findings/sdcflows-estimation.md ("nipype's command lines ... relay 6").
  - Golden: `py.niworkflows.skullstrip` (0.3 s; BET/fslmaths clean-room originals, 3dAutomask replica) and
    `py.sdcflows.coeff2epi` (0.1 s; sloppy + debug + seed 42) registered, NOT recorded (R11, with relays 2-5's 11:
    13 unrecorded cases fail tests/golden until then); coverage test passes.
  - Docs: `docs/api/niworkflows-helpers.md` (header fixes, ApplyMask, KeySelect, skullstrip workflow, nipype's
    command lines), `docs/api/sdcflows-fieldmap.md` ("The field map's registration to the EPI"), findings,
    `crates/larmorx-sdcflows/PROVENANCE.md` (part B relay 6 section), investigations README rows. No `_core` change
    (no `.pyi` change).
  - Gates run: ruff check + format (repo: 942 files), check_spdx, pytest tests/python + tests/golden (720 passed, the
    13 unrecorded golden cases fail as expected). Not run: cargo gates (no Rust change), validation/tests, smoke tier.
  - D8 small checks: `ds004958-sub-fsm64ps-noreconall-pepolar` (exit 0 at 19:40): check_pepolar wf,image (shared)
    and wf (newest): **ALL OK in both** (sdcflows' graphs node for node: 2.15 30, 2.17 35, debug 36, sloppy 35 files;
    fMRIPrep's own runwise/set averages, merge, fix_coeff, from_las(_fmap), ref_average, topup line identical;
    topup's replica = FSL 6.0.7.23 on fMRIPrep's input 8+2; fMRIPrep's image's topup differs from FSL 6.0.7.23 again
    (0.03 % voxels identical, max 0.197 Hz, matrices identical: relay 5's ds006736 note, now two runs; for R10).
    Commit ba66d32a (findings). `ds002603-sub-01-noreconall-epi` (exit 0 at 20:32): **ALL OK in both** (two EPI+BOLD estimators: 2.15 60,
    2.17 70, debug 36, sloppy 35 node files; fMRIPrep's own nodes identical; topup replica = FSL 6.0.7.23 16+4;
    check_bold_fmap nodes: its 2 BOLD runs' masks and skull-stripping = fMRIPrep's, both envs); fMRIPrep's image's
    topup differs from FSL 6.0.7.23 by up to 5.35 and 17.8 Hz there (matrices identical): **3 of 5 runs now:
    R10 must measure the cause** (the image's topup on the main machine through the oracle cache). Commit 2b7ae0b1.
    `ds005752-sub-ON01016-noreconall-direct` (exit 0 at 21:11; FMAP-QUEUE-END): relay 6 wrote the `direct` section
    of check_workflows.py (commit b401e22b): magmrg, fmapmrg, clipper_pre = fMRIPrep's; mask 100 % identical; N4
    (8 threads there) and probseg reported; **the final field differs from fMRIPrep's by <= 0.067 Hz everywhere**;
    its 2 BOLD runs' dilations + skull-stripping = fMRIPrep's, coregister within 1.8e-13 (both envs).
  - **FINDING (for the orchestrator and R10): the D8 runs ran on the main machine (AVX-512, `hostname` in their
    reports) WITHOUT `OPENBLAS_CORETYPE=Haswell`**, so fMRIPrep's image's OpenBLAS (and FSL's) took AVX-512 kernels:
    the likely cause of all three "fMRIPrep's image differs" notes: topup on ds006736/ds004958/ds002603 (the two
    old-laptop runs ds001600/ds005040 match FSL 6.0.7.23), relay 5's ds006736 quaternion, and ds005752's B-spline fit
    (the seven old-laptop phasediff fits are bit-identical). R10 confirms by re-running the image's topup and
    BSplineApprox with/without `OPENBLAS_CORETYPE=Haswell` on one of them (oracle cache). Suggestion for the
    orchestrator: note it in those runs' RUN_INFO / oracle-runs.md, and set `OPENBLAS_CORETYPE=Haswell` for future
    fMRIPrep oracle runs on the main machine (or decide which kernel the reference is). In
    docs/findings/sdcflows-estimation.md ("The likely cause: where the runs ran").
  - For the orchestrator / handoff: NOTICE: sdcflows' ANTs settings files shipped (`python/larmorx/sdcflows/data/
    *.json`, Apache-2.0) and nipype 1.12.0's interfaces ported in `python/larmorx/_nipype.py` (Apache-2.0);
    `validation/tools.toml`: `coeff2epi_workflow` under `sdcflows-estimation`, `skullstrip_bold_workflow`,
    `copy_xform`, `copy_header`, `apply_mask`, `key_select` under `niworkflows-helpers`; `docs/validation/golden.md`:
    the two new cases; next-steps: "fMRIPrep seeds ANTs" for the later ports (SyN, templates, larmorprepx).
- **Relay 7 = R5b (done, 2026-11-08): commits 7011a19e, a4e07b9d** (on b401e22b). R5b split in two as relay 6
  proposed: relay 7 = `distortion_parameters` + `concatenate_xfms` + `reconstruct_fieldmap` (+ the `unwarp_boldref`
  re-check); **relay 8 = R5c** (the BOLD fit's stage 3 as one function: below). Scratch helpers in `<scratch>/r7/`
  (`check.sh <shared|newest> [args]`: `check_bold_steps.py` boxed 8 GiB; `probe_golden.py`).
  - **`python/larmorx/transforms/fieldmap.py`** (new; exported by `lx.transforms`): `reconstruct_fieldmap(coefficients,
    fmap_reference, target, transforms=None, inverse=False, *, n_threads, upstream_defects)` -> Image (fMRIPrep 25.2.7's
    `reconstruct_fieldmap` = 25.2.6's: `as_affine` (lx Affine; chains combined as nitransforms' `asaffine`, numpy's
    products), `aligned`, the direct path (part A's `_core.sdcflows_collocation_apply` on the projected target made
    positive, fresh nibabel header) and the resampled path (the field on the fmap reference made positive, `as_closest_
    canonical` hack, `NOT_ALIGNED_REFERENCE` ValueError; then nitransforms' `apply`: `_core.nitransforms_ndcoords` of the
    target, `_core.nitransforms_map` through the chain + the field's ras2vox, `lx.ndimage.map_coordinates` cubic constant
    0; the target's header), intent/cal fields; `reconstruct_fieldmap_node(in_coeffs, target_ref_file, fmap_ref_file,
    transforms, inverse=(False,), *, out_dir)` -> `ReconstructFieldmapResult(fieldmap, out_file)` (`<coeff>rec`);
    `distortion_parameters(metadata, in_file=None, fallback=None)` -> `DistortionParametersResult(readout_time,
    pe_direction)` (get_trt as fMRIPrep calls it, the traits' checks). `chain.py`: `_itk_linear_text` (nitransforms'
    ITK text writer, `"%g"`). **`python/larmorx/niworkflows/nitransforms.py`** (new; `lx.niworkflows.concatenate_xfms`,
    `ConcatenateXfmsResult`, `XFM_FMT`): itk only; `fs`/`.lta` NotImplementedError (FreeSurfer's part); a series file
    NotImplementedError. No Rust change (no `.pyi` change; the .so of relay 5 is current).
  - **Defects (used in code; NOT yet in `larmorx_core::defects` nor the register: R9):** **PIPE-20** (rule C, kept:
    DistortionParameters swallows KeyError/ValueError -> None outputs -> ResampleSeries applies no field map; fMRIPrep's
    documented design; larmorx warns (UserWarning) in fix mode; the stage-3/larmorprepx level may stop with fMRIPrep's
    "Missing readout time information" (relay 8 decides by the rules)); **PIPE-21** (rule A, corrected: the direct path
    returns the field on the projected, positive-cosine grid: for a LAS target the voxels are reversed along x while
    ResampleSeries reads it voxel for voxel on the target grid = a mirrored field, 159 Hz on the synthetic case; fix = the
    same values in the target's voxel order with the target's affine, fresh header; UpstreamDefectWarning in both modes);
    **PIPE-22** (rule C, kept: nitransforms' `"%g"` six digits; measured <= 7.9e-05 mm on the 7 transforms).
  - **Checks** (`validation/investigations/sdcflows-fieldmap-b/check_bold_steps.py`, sections dparams, concat, recon,
    unwarp, synth; only the EARLIER runs (old laptop): ds002422, ds002603, ds005040, ds005454): **ALL OK in both envs**:
    dparams 28/28 = fMRIPrep's result pklz and = DistortionParameters here, 13 synthetic branches, 2 errors, PIPE-20's
    warning; concat 7/7 out_fwd.tfm byte for byte = fMRIPrep's and = ConcatenateXFMs here (fwd and inv), 7/7 = the
    desc-fmap_xfm.txt derivatives, 4 synthetic combinations (.txt/.mat/.tfm); recon 28/28 (14 boldref, 7 T1w ref, 7 MNI
    2 mm through the .h5 composite) = fMRIPrep's own files (data + header bytes) and = ReconstructFieldmap here, fix =
    reproduce on all (every node takes the RESAMPLED path: `aligned` is in mm^2 and fails for any registration
    rotation); unwarp 7/7 (`lx.transforms.resample_series` with the reconstructed field; ds005040 with the Jacobian) =
    fMRIPrep's files and ResampleSeries here, fix = reproduce; synth 8 path cases = ReconstructFieldmap (direct RAS
    identity/shift/2 levels/beyond the grid, resampled LAS/RAS int16/rotated target, canonical hack), the rotated
    reference's ValueError, a DenseField chain = fMRIPrep's function with nitransforms' DenseFieldTransform, PIPE-21
    shown (fMRIPrep's direct field = the fix's reversed along x; fix = the resampled path's field, 0 Hz). Times on the 28
    nodes: ReconstructFieldmap 28.1-28.7 s, larmorx 12.3-13.1 s (4 threads). Peak memory (relay7/mem_recon.py, newest
    env, ds005454): MNI node 699 MiB vs 2,275 MiB, boldref node 419 vs 1,438 MiB.
  - Golden: `py.transforms.reconstruct_fieldmap` (both paths, PIPE-21 fix/reproduce, as_affine, aligned),
    `py.transforms.distortion_parameters`, `py.niworkflows.concatenate_xfms` registered (they run, 0.02 s), NOT recorded
    (R11): 16 unrecorded cases fail tests/golden until then.
  - Docs: `docs/api/fmriprep-fieldmap-steps.md` (new: option tables, the two paths, PIPE-20/21), `docs/api/niworkflows-
    helpers.md` (ConcatenateXFMs, PIPE-22), `docs/findings/sdcflows-estimation.md` ("fMRIPrep's own field-map steps ...
    (relay 7)"), `crates/larmorx-transform/PROVENANCE.md` (the Python ports), investigations README row, probes
    `relay7/probe_orient.py` (every target LAS on the earlier runs' boldref nodes), `relay7/mem_recon.py`.
  - Gates run: ruff check + format (repo, 948 files), check_spdx, pytest tests/python tests/golden validation/tests
    (1,107 passed; 17 failed = the 16 unrecorded golden cases + the harness test that fails only in a box with -n 8, as in
    relay 4). Not run: cargo gates (no Rust change), the smoke tier (R11).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: PIPE-20 (fMRIPrep DistortionParameters,
    rule C), PIPE-21 (fMRIPrep ReconstructFieldmap, rule A), PIPE-22 (nitransforms ITK writer, rule C); NOTICE: fMRIPrep
    25.2.7's `interfaces/resampling.py` (ReconstructFieldmap, DistortionParameters) and niworkflows' `interfaces/
    nitransforms.py` in `python/larmorx/transforms/fieldmap.py` / `python/larmorx/niworkflows/nitransforms.py`
    (Apache-2.0), nitransforms 25.1.0's ITK writer and `asaffine` (MIT); `validation/tools.toml`: `reconstruct_fieldmap(
    _node)`, `distortion_parameters` (a `fmriprep-fieldmap-steps` entry, or under `sdcflows-estimation`), and
    `concatenate_xfms` under `niworkflows-helpers`; `docs/validation/golden.md`: the three new cases; next-steps: the
    `.lta` path of ConcatenateXFMs (fMRIPrep's `itk2lta` with `out_fmt='fs'`, registration's `concat_xfm`) with
    FreeSurfer's part.
  - **To re-run on the re-made D8 runs** (the orchestrator's message 22:25/22:10: the seven 2026-11-08 field-map runs are
    re-made with Haswell kernels; usable once `oracles/logs/fmap-queue-haswell.log` has `<run> exit 0`):
    `check_bold_steps.py --runs <full run names>` (all sections; give the full names, e.g.
    `ds002603-sub-01-noreconall-epi,ds000228-sub-pixar008-noreconall-syn,...`), and look whether any `boldref_fmap`
    takes the DIRECT path there (the SyN runs are the candidates: `INFO recon paths`); relay 5's `check_pepolar.py`
    image sections (topup, regrid quaternion) and relay 6's `check_bold_fmap.py nodes` and `check_workflows.py direct`
    (ds005752's B-spline fit, 0.067 Hz) on those runs, now expected identical; relay 6's coregister note (within 9.9e-12)
    is ANTs' threads, unaffected.
- **Relay 8 = R5c (done, 2026-11-08): commits c575e12b, 729bbc57, dda0646a, f7fa6d5a, 70fc8dae** (on a4e07b9d). Scratch helpers in
  `<scratch>/r8/` (`check.sh <shared|newest> [args]`: `check_bold_stage.py` boxed 12 GiB with LARMORX_NC_BIN;
  `recheck.sh <full run> [pepolar|direct|syn]`: relays 5-8's checks on one re-made run, boxed, both envs; `probe_golden.py`,
  `smoke.py`). No Rust change (the .so of relay 5 is current; no `.pyi` change).
  - **`python/larmorx/pipelines/larmorprepx/fieldmaps.py`** (new; the first larmorprepx module, D4):
    `bold_fit_fieldmap(bold_file, metadata, fieldmap_id, estimates, hmc_boldref, coreg_ref, coreg_mask, *, out_dir,
    sbref_files, boldref2fmap_xfm, jacobian, fallback_total_readout_time, sloppy, debug, ants_random_seed, sink,
    fsl_implementation, n_threads, upstream_defects)` -> `BoldFitFieldmapResult` (init_bold_fit_wf's stage 3 + stage 4's
    field-map branch: fmapref_buffer `select_ref`, fmap_select `key_select`, fmapreg_wf `coeff2epi_workflow` (skipped when
    `boldref2fmap_xfm` is given: "Found fieldmap transform"), itk_mat2txt `concatenate_xfms`, boldref_fmap
    `reconstruct_fieldmap_node(inverse=[True])`, distortion_params, unwarp_boldref `resample_series` written as
    `<coreg_ref>resampled`, skullstrip_bold_wf; node folders as fMRIPrep's); `resampling_fieldmap(fieldmap_id, estimates,
    target_ref_file, boldref2fmap_xfm, metadata, in_file, boldref2target, *, fallback_total_readout_time, out_dir, ...)`
    -> `ResamplingFieldmapResult` (bold_native_wf with boldref2target None; init_bold_volumetric_resample_wf: fmap2target
    = nipype Merge, inverses = `gen_inverses`); `select_ref`, `gen_inverses`, `fieldmap_jacobian(method, force, ignore)`
    (fMRIPrep's base.py rule, sdcflows' EstimatorType names); `FieldmapEstimates` (fMRIPrep's fmap buffers),
    `DerivativeRequest(role, in_file, source_file, entities, sources)` + `sink` callable (D7: fMRIPrep's later nodes read
    the derivatives `desc-fmap_xfm.txt`, `desc-coreg_boldref`, `desc-brain_mask`; the entities as fMRIPrep's
    `init_ds_*_wf` set them; without a sink the later nodes read the node files: same data). `out_dir` required.
    `FieldmapMetadataError(ValueError)`, `MISSING_READOUT_TIME`. The steps before (estimators, hmc boldref,
    `init_enhance_and_skullstrip_bold_wf`: D6) are the caller's inputs. Multi-echo runs get no field map in the
    resampling workflows (fMRIPrep's `fieldmap_id if not multiecho`): the caller's rule, documented.
  - **PIPE-20 decided (the orchestrator asked): rule B at the pipeline level** (the interface keeps rule C): a BOLD run
    with a field map whose metadata give no readout time (or no PE direction) cannot be corrected as asked; fMRIPrep
    silently resamples it uncorrected while its report names the correction and the sidecars list the field map;
    not a design pipelines rely on (fMRIPrep itself stops with "Missing readout time information. See documentation
    for `--fallback-total-readout-time`." when an estimator lacks it), and larmorx cannot complete the input -> a clear
    error (`FieldmapMetadataError`, fMRIPrep's message + the run and the estimator); `reproduce` = fMRIPrep's (no field
    map applied, UpstreamDefectWarning PIPE-20). In the findings, the API page's defects table, fieldmap.py's docstring.
  - **Checks** (`validation/investigations/sdcflows-fieldmap-b/check_bold_stage.py`, sections fit, resampling, pipe20;
    earlier runs ds002422, ds002603, ds005040, ds005454 by default): **ALL OK in both envs** (logs
    `oracles/logs/fmapb-r8-stage-{shared,newest}.log`): fit 7 BOLD fits x 2 branches (registering; fMRIPrep's transform
    given) with fMRIPrep's own `DerivativesDataSink` as the sink: fmapref_buffer 14/14, fmap_select 14/14, dilated masks
    7+7, itk_mat2txt out_fwd.tfm 7/7, desc-fmap_xfm derivative (name, bytes) 7/7, boldref_fmap 14/14 (+names),
    distortion_params 14/14, unwarp_boldref 14/14 (+names), desc-coreg_boldref 14/14, desc-brain_mask 14/14 (descrip
    only: fMRIPrep's FSL), skullstrip_bold_wf 70/70 files (56 descrip/extension only: the image's FSL/AFNI builds, as
    relay 6); the registration differs from fMRIPrep's 8-thread ANTs by 1.3e-13..6.7e-13 (reported) and "%g" writes
    the same text on all 7, so everything after it is fMRIPrep's file; fix = reproduce 7/7; resampling 21 workflows
    (7 bold_native, 7 bold_anat, 7 bold_std MNI 2 mm): fmap_select 21/21, inverses 14/14, fmap2target 14/14, native
    transforms/inverse 7/7, distortion_params 21/21, field maps 21/21 (+names); pipe20 3/3 (fix's error message;
    reproduce warns; reproduce = fMRIPrep's ResampleSeries without ro_time on the same files). About 1 s per
    `bold_fit_fieldmap` call (21 calls in 20.5 s, 6 threads). Unit tests `tests/python/test_larmorprepx_fieldmaps.py` (4).
  - **The re-made (Haswell) fMRIPrep runs, re-checked** (`<scratch>/r8/recheck.sh`; log
    `oracles/logs/fmapb-r8-recheck-<run>.log`): **ds006736-sub-004-noreconall (exit 0 at 22:56): ALL OK everywhere**:
    check_bold_steps (both envs: dparams 4/4, concat 1/1, recon 4/4 = fMRIPrep's own files, unwarp 1/1; `INFO recon
    paths: direct 0, resampled 4`), check_bold_stage (both envs, all identical; registration within 4.3e-12),
    check_bold_fmap nodes (all identical), **check_pepolar image: fMRIPrep's image's topup = FSL 6.0.7.23 now (7/7
    images descrip apart, 2/2 matrices, the field identical in every voxel) and regrid = fMRIPrep's file**: relay 6's
    cause (AVX-512 kernels) confirmed on this run, relay 5's two image notes settled there.
    **ds004958-sub-fsm64ps-noreconall-pepolar (exit 0 at 23:49; committed `relay8/recheck.sh`): ALL OK everywhere** (log
    `oracles/logs/fmapb-r8-recheck-ds004958-pepolar.log`): check_bold_steps both envs (recon 4/4 resampled, direct 0),
    check_bold_stage both envs (registration within 1.2e-12, all files identical), check_bold_fmap nodes, check_pepolar
    image: the image's topup = FSL 6.0.7.23 (7/7 images, 2/2 matrices, field identical), runwise/setwise averages,
    concat, fix_coeff, from_las(_fmap), ref_average = fMRIPrep's. RECHECK_LATER below for the others.
  - Golden: `py.larmorprepx.fieldmaps` registered (0.75 s; repeatable; a sink copying the derivatives, sbref chosen,
    sloppy seeded registration (the direct path: PIPE-21 fixed), the precomputed branch, a resampling workflow through
    one more affine (the resampled path), PIPE-20 reproduced and its error), NOT recorded (R11): 17 unrecorded cases
    fail tests/golden until then. `tests/golden/registry.py`: NAMESPACES + `lx.pipelines.larmorprepx.fieldmaps` (the
    module imported at the top so `lx.pipelines` resolves); coverage test passes.
  - Docs: `docs/api/fmriprep-fieldmap-steps.md` ("larmorprepx: the field-map nodes of a BOLD run": both functions with
    node and option tables; PIPE-20's row: C for the interface, B for the pipeline; status line), `docs/findings/
    sdcflows-estimation.md` ("fMRIPrep's field-map nodes of a BOLD run, composed (relay 8)"; the "likely cause"
    paragraph: confirmed on ds006736), `crates/larmorx-transform/PROVENANCE.md` (the fieldmaps.py entry), the
    investigations README row.
  - Gates run (boxed, `oracles/logs/fmapb-r8-gates-python.log`): `gates-python.sh --no-build`: ruff check + format (951
    files), check_spdx, collection without larmorx_validation, pytest tests/python tests/golden validation/tests 1,112
    passed, 17 failed = exactly the 17 unrecorded golden cases (also without FSLDIR/LARMORX_NC_BIN: 724 passed, the same
    17); `tables --check` OK. Not run: cargo gates (no Rust change), the smoke tier (R11).
  - **Findings for later relays:** niworkflows' `DerivativesDataSink` logs "Changing <file> dtype from float32 to
    float32" when it converts the mask to uint8 (`LOGGER.warning(..., orig_dtype, orig_dtype)`: the message names the
    original type twice; the file is right): a PIPE candidate for the DataSink relay (rule A for the message,
    `reproduce` prints niworkflows'); the mask derivative is rewritten as uint8 (`np.rint`, DEFAULT_DTYPES['mask']),
    the coreg boldref derivative is a plain copy (its header needs no fix) on all 7 runs.
  - For the orchestrator / handoff: the register's PIPE-20 entry (the two levels: C interface, B pipeline; reason as
    above); `validation/tools.toml`: `larmorx.pipelines.larmorprepx.fieldmaps` (bold_fit_fieldmap, resampling_fieldmap,
    select_ref, gen_inverses, fieldmap_jacobian) with the `fmriprep-fieldmap-steps` entry (or a larmorprepx entry);
    `docs/validation/golden.md`: the new case and the new namespace; NOTICE: fMRIPrep 25.2.7's `workflows/bold/fit.py`,
    `apply.py`, `outputs.py`, `base.py` (Apache-2.0) ported in `python/larmorx/pipelines/larmorprepx/fieldmaps.py`;
    next-steps: the first larmorprepx module exists (fMRIPrep's field-map nodes of a BOLD run), its callers (the
    estimators, the BOLD reference workflows, DerivativesDataSink) come with their relays and with larmorprepx's part.
  - **RECHECK_LATER (the re-made runs not yet finished when relay 8 stopped, 2026-11-08 ~23:58; queue order
    ds002603-epi (started ~23:50), ds005752-direct, then the three SyN runs (about 50 min each); use a run only once
    `oracles/logs/fmap-queue-haswell.log` has `<run> exit 0`):** the committed
    `validation/investigations/sdcflows-fieldmap-b/relay8/recheck.sh` (= `<scratch>/r8/recheck.sh`; needs
    `LARMORX_WORKSPACE` and `LARMORX_NC_BIN=<main checkout>/crates-nc/target/release/larmorx-nc`), `... <full run> pepolar` for
    `ds002603-sub-01-noreconall-epi`, `... direct` for
    `ds005752-sub-ON01016-noreconall-direct` (check_workflows `direct`: the B-spline fit's 0.067 Hz expected to vanish),
    `... syn` for the three `-syn` runs (check_bold_steps: look for `INFO recon paths` with direct > 0: PIPE-21's real
    case; check_bold_stage; check_bold_fmap nodes). Each takes about 5-10 min (boxed 12 GiB, 6 threads). Write the
    results here and in the findings ("likely cause" paragraph and relay 8's section).
- **Relay 9 = R6a (done, 2026-11-09): commits e5bf4d4e, 9aa3fa02, 0d741029, 3202f89a, c92115d1, fe03b706, e5991832, 80a8ef72, ca0ac3c8, 6d8c2501** (on 70fc8dae). Scratch helpers
  in `<scratch>/r9/` (`check.sh <shared|newest> [args]`: `check_syn_preprocessing.py` boxed 12 GiB with the atlas's
  local copy; `probe_golden.py <case ids>` (twice: time, repeatable), `probe_bounds.py`, `probe_inv.py`,
  `probe_validate_qform0*.py`, `probe_val_ds004884.py`, `probe_obliquity.py`, `fetch_nilearn*.sh`).
  - **Rust (cargo gates of the root workspace run: fmt, clippy -D warnings, tests: OK; crates-gpl/crates-nc not run: only additive public
    functions in larmorx-ants and larmorx-interp, which they build by path):** `larmorx_interp::ndimage::{AffineMap,
    affine_transform3}` (SciPy's `affine_transform` for 3-D: `NI_GeometricTransform`'s matrix path, the offset first
    from SciPy 1.16, last in 1.15 (`offset_last`); `NI_ZoomShift`'s `(o + offset/zoom) * zoom`; the same sampling and
    output casts as `map_coordinates`; 2 unit tests); `larmorx_ants::cli::apply_transforms::main_with_threads` (an
    explicit thread count; `main_with` calls it with `None`); `larmorx-py`: `_core.ndimage_affine_transform3(...,
    scipy_version)` and `_core.ants_file_program("antsApplyTransforms", args, n_threads, upstream_defects)`; stubs.
    The wheel in `<scratch>/wheel` and the worktree's `.so` are current (rebuilt after the last Rust change).
  - **Python:** `lx.ndimage.affine_transform` (SciPy's checks and messages; 3-D only, `NotImplementedError` else);
    `larmorx._nipype.run` runs `antsApplyTransforms` (files); `larmorx.io.save_nibabel(scale=False)` (nibabel's
    `ArrayWriter(check_scaling=False)` path for a header whose scl fields are set, as nilearn 0.13+'s
    `new_img_like` sets them to 0); **`lx.niworkflows`**: `validate_image` (+`ValidateImageResult`; the truth table,
    the report's text with transforms3d's `decompose44` exactly and numpy's `eig`; the new qform read as stored,
    float32), `nonsteady_states_detector` (+`NonsteadyStatesResult`) and nipype's `is_outlier` (`bold.py`),
    `binarize` (+`BinarizeResult`), `generate_sampling_reference`, `resample_img_nearest`, `BoundingBoxError`
    (`sampling.py`: nilearn 0.14.1 by default, `nilearn_version="0.11.1"` = fMRIPrep's image: fresh header, the
    whole-voxel copy path; the bounds' 4x8 product = one exact FMA per term (Python `Fraction`); numpy's inverse in
    place of `scipy.linalg.inv`: see the findings), `epi_reference_workflow` (+`EpiReferenceResult`, `refmap.py`:
    one input only; several -> NotImplementedError naming mri_robust_template, raised before any node);
    **`lx.sdcflows`** (`syn.py`): `syn_preprocessing_workflow` (+`SynPreprocessingResult`; `debug=True` ->
    NotImplementedError naming RegridToZooms; `prior_file` a larmorx addition), `deoblique`, `reoblique`,
    `set_dtype`, `merge_meta`, `obliquity`, `fmap_atlas` (LARMORX_SDCFLOWS_ATLAS, else the data cache, fetched from
    sdcflows 2.17.1's GitHub tag with the pinned SHA-256 72b8d2cc...), `fmap_atlas_affine` (shipped:
    `python/larmorx/sdcflows/data/fmap_atlas_2_MNI152NLin2009cAsym_affine.mat`), `OBLIQUE_THRESHOLD_DEG`,
    `FMAP_ATLAS_SHA256`. Pins fetched (sdist SHA-256 checked against PyPI): `tags/nilearn-0.14.1`
    (6f7ec639be2c9e4a3625ebc4f25c3de31ae3af21331c8d03e27201c482609672), `tags/nilearn-0.11.1`
    (a01df08fc6c8ded3cd6fb7a211634603ad46a6df780504b6d05222c2e7a972fe); both sdists contain AGENTS.md/CLAUDE.md
    files of nilearn's own (data, ignored).
  - **Defects (used in code; NOT yet in `larmorx_core::defects` nor the register: R9 of the plan):** **PIPE-23**
    (nipype `is_outlier`, rule A: MAD = 0 makes the values at the median 0/0 = NaN outliers, so NonsteadyStatesDetector
    calls every leading volume non-steady; fix: z = 0 at the median; UpstreamDefectWarning when it changes the
    count; reproduce = nipype's, with numpy's RuntimeWarnings); **PIPE-24** (niworkflows ValidateImage, rule A: the
    report's unit lookup raises KeyError for spatial unit codes 4-7; fix: mm; reproduce: the KeyError);
    **SDC-13** (sdcflows Deoblique/Reoblique, rule D, UNDER MEASUREMENT: the 0.5 "degrees" threshold compared with
    radians, the plumb grid centred in voxels and the axcodes compared with RAS by position; both modes = sdcflows'
    until relay 10 measures it with the SyN registration; the runs' SyN EPIs are oblique by up to 20.7 degrees:
    pixar008 12.2/20.7/18.8, ds004884 2.5/6.3/5.8, ds005003 plumb).
  - **Checks** (`validation/investigations/sdcflows-fieldmap-b/check_syn_preprocessing.py`, sections nodes,
    workflow, library; `relay9/check_affine_transform.py`; `relay9/probe_bounds.py`):
    affine_transform 600 seeded cases per env bit for bit (SciPy 1.15.3: 26 differ only where map_coordinates
    differs the same way: SCI-03's reflect boundaries); probe_bounds 3,000 affines (box product = FMA chain, both
    envs; numpy inv + matmul = numpy); library section **ALL OK in both envs** (GenerateSamplingReference 25 cases
    against nilearn 0.14.1 with the env's SciPy: oblique, plumb, LPS, NaN/inf, 4D x (default, LPS moving, fov_mask,
    target_resolution, xform_code 4); ValidateImage 9 truth-table cases incl. det-0 sform, wrong-zoom sform and
    impossible quaternions (file + report); NonsteadyStatesDetector 5 + PIPE-23; is_outlier 40; Binarize 3 dtypes;
    _set_dtype 3; Deoblique 3 (plumb, 35.5 degrees RAS and LPS); Reoblique). **fMRIPrep's runs (the archived
    AVX-512 SyN runs, `archive/2026-11-08-fmriprep-runs-avx512-kernels`, 5 estimators: pixar008, ds004884 x 2,
    ds005003 x 2; log `oracles/logs/fmapb-r9-syn-avx512-shared.log`): nodes ALL IDENTICAL, 27 checks x 5** (every
    node fed fMRIPrep's inputs = fMRIPrep's files with header bytes as written, incl. sampling_ref with nilearn
    0.11.1's path and the 5 command lines; n4_avgs, ref_anat and epi2anat = ANTs 2.6.5 at one thread with CORE-MATH
    and the run's seed; fMRIPrep's own (4 threads, glibc): N4 40-45 % of voxels equal, DenoiseImage 97.8-99.0 %,
    registration 4.5e-13..2.6e-11, and 0.081 on ds005003's second estimator (affine.json's Random 0.4 sampling vs
    threads)); the brain mask = fMRIPrep's on all 5 despite the AVX-512 runs (probseg not kept by nipype).
    **workflow (shared env) = sdcflows' own graph** (nipype, stand-ins: ANTs one thread CORE-MATH, AFNI's
    3dvolreg): 105 files, 100 identical byte for byte, 5 = RobustAverage's `_sliced_volreg` series with only
    `vox_offset` apart (AFNI's history extension; re-run with that classified: ALL OK, `oracles/logs/fmapb-r9-syn-workflow-shared.log`);
    newest env (SciPy 1.18.1's random walker, numpy 2.5.4, nilearn 0.14.1): ALL OK, 105/105 (the same 5 vox_offset),
    `oracles/logs/fmapb-r9-syn-workflow-newest.log`. Times: sdcflows' graph 118-155 s one thread, larmorx 30-39 s on 6 threads; node checks
    58-165 s.
  - **The re-made (Haswell) runs re-checked:** **ds002603-sub-01-noreconall-epi (exit 0 at 00:39): ALL OK everywhere**
    (`oracles/logs/fmapb-r9-recheck-ds002603-epi.log`, relay8/recheck.sh pepolar): check_bold_steps and
    check_bold_stage both envs (recon 8/8 resampled, direct 0), check_bold_fmap nodes, check_pepolar image: the
    image's topup = FSL 6.0.7.23 on both estimators (7/7 images, 2/2 matrices, fields identical; 17.8 Hz apart on the
    AVX-512 run). **ds005752-sub-ON01016-noreconall-direct (exit 0 at 01:17): ALL OK everywhere** (`oracles/logs/fmapb-r9-recheck-ds005752-direct.log`, recheck.sh direct): check_bold_steps/stage both envs (recon 8/8 resampled), check_bold_fmap nodes, check_workflows direct both envs: the B-spline fit's field identical in every voxel (0.067 Hz on the AVX-512 run), merges, clipper_pre and mask fMRIPrep's. The three SyN runs follow in the queue (about 50 min each).
  - Golden: `py.ndimage.affine_transform`, `py.sdcflows.syn_preprocessing` (0.7 s; prior_file = a synthetic prior),
    `py.niworkflows.epi_reference` (0.7 s) registered (repeatable), NOT recorded (R11): 20 unrecorded cases fail
    tests/golden until then (17 + 3). Coverage test passes.
  - Unit tests `tests/python/test_sdcflows_syn.py` (5).
  - Docs: `docs/api/resample-series.md` (`lx.ndimage` table: affine_transform), `docs/api/niworkflows-helpers.md`
    ("The EPI reference and the steps under it", "Upstream defects of these steps": PIPE-23, PIPE-24; the
    antsApplyTransforms run), `docs/api/sdcflows-fieldmap.md` ("The fieldmap-less (SyN) preprocessing", SDC-13's row:
    under measurement), `docs/findings/sdcflows-estimation.md` ("The fieldmap-less (SyN) preprocessing (relay 9)";
    the likely-cause paragraph: ds002603 confirmed), PROVENANCE (`crates/larmorx-interp`, `crates/larmorx-sdcflows`).
  - Gates run: `gates-rust.sh <worktree> root` OK (fmt, clippy, tests; with CARGO_TARGET_DIR=<scratch>/target, 8 jobs); `gates-python.sh --no-build` (log `oracles/logs/fmapb-r9-gates-python.log`): ruff check + format (958 files), check_spdx, collection without larmorx_validation OK; pytest 1,117 passed, 21 failed = the 20 unrecorded golden cases + `test_the_replica_itself` (an environment issue with LARMORX_NC_BIN set, below); CI's environment (no FSLDIR/LARMORX_NC_BIN) 729 passed, the same 20; `tables --check` OK. Not run: crates-gpl/crates-nc gates, the smoke tier (R11).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: PIPE-23 (nipype is_outlier, rule A),
    PIPE-24 (niworkflows ValidateImage, rule A), SDC-13 (sdcflows Deoblique, rule D pending: relay 10);
    `upstream.tsv`: `nilearn-0.14.1` (https://files.pythonhosted.org/packages/ac/10/fb546467307bb411c0d8be39a0c17bff2e6d454d7cc5e912b50abf60ab52/nilearn-0.14.1.tar.gz,
    BSD-3-Clause, tags) and `nilearn-0.11.1` (https://files.pythonhosted.org/packages/bf/da/99933b75f89288bc356e8ad7096af3d1ae7e59db6891ba7fb8e9c71ee994/nilearn-0.11.1.tar.gz,
    BSD-3-Clause, tags: fMRIPrep 25.2.6's image's, the step); NOTICE: nilearn 0.14.1/0.11.1 (BSD-3-Clause:
    `python/larmorx/niworkflows/sampling.py`), transforms3d 0.4.2 (BSD-2-Clause: `decompose44`, `mat2axangle` in
    `python/larmorx/niworkflows/header.py`), SciPy 1.18.1/1.15.3's `NI_GeometricTransform`/`NI_ZoomShift` coordinates
    in `crates/larmorx-interp` (BSD-3, already noticed for map_coordinates), nipype 1.12.0's `is_outlier` and
    `copy_header` (Apache-2.0), sdcflows' `fmap_atlas_2_MNI152NLin2009cAsym_affine.mat` (Apache-2.0, shipped);
    `validation/tools.toml`: `lx.ndimage.affine_transform` (the ndimage entry), `lx.niworkflows.{validate_image,
    nonsteady_states_detector, is_outlier, binarize, generate_sampling_reference, resample_img_nearest,
    epi_reference_workflow}` (niworkflows-helpers), `lx.sdcflows.syn_preprocessing_workflow` and helpers
    (sdcflows-estimation); `docs/validation/golden.md`: the three new cases; next-steps: SciPy's 4x4 `inv`
    (getrf/getri with SciPy's OpenBLAS; 1.18's structure detection) for nilearn's sampling points (follow-up: no
    voxel changed on the checked grids); niworkflows' `RegridToZooms` (sdcflows' `debug`/sloppy SyN; needs SciPy's
    `gaussian_filter`); the identity LTA of StructuralReference with FreeSurfer's part; the stage catalogue's
    "SyN field-map-less estimation" roadmap entry (main) -> stages once relay 10 ports init_syn_sdc_wf.
  - **Follow-up found (not this part's file):** `tests/python/test_mri_replica.py::test_the_replica_itself` fails
    when `LARMORX_NC_BIN` is set and the checkout has no `crates-nc/target/release/larmorx-nc` (the `no_replica`
    fixture clears the variable, then `_built_replica()` inside the test returns `None` and the test sets
    `LARMORX_NC_BIN="None"`); its `skipif` saw the variable. Passes without the variable (CI). For next-steps.
- **Relay 10 = R6b (done, 2026-11-09): commits 3f7936bb, 5d03e49c, fa88b935, eb032b63, 20d0460d, 9b72f971, 7c4c7505, a42676a5, 1411e088** (on 6d8c2501).
  Agent id: the orchestrator's to add (relay 10). Scratch helpers in `<scratch>/r10/` (`check.sh <shared|newest> <GiB>
  [args]`: check_syn_sdc.py boxed; `run_check.sh` (r2's with a 4 h limit); `sdc13.sh <syn_preprocessing folder> <out>
  <log>`; `recheck_syn.sh <run>` (waits for the run's `exit 0` in the Haswell queue's log, then relay8/recheck.sh <run>
  syn; `WAIT_FOR=<log>` also waits for an SDC-13 end line); `probe_syn_time.{py,sh}`, `probe_cmd.py` (committed as
  relay10/probe_syn_cmdline.py)). No Rust change (the `.so` of relay 9 is current; no `.pyi` change).
  - **Port (`python/larmorx/sdcflows/syn.py`, exported from `lx.sdcflows`):** `syn_sdc_workflow(epi_ref, epi_mask,
    anat_ref, anat_mask, sd_prior, *, out_dir, name="syn_sdc_wf", use_metadata_estimates, fallback_total_readout_time,
    sloppy, debug, omp_nthreads, laplacian_weight, ants_random_seed, niworkflows_version, numpy_version,
    upstream_defects, n_threads)` -> `SynSdcResult(fmap, fmap_ref, fmap_coeff, fmap_mask, out_warp=None, method,
    readout_time, pe_direction, zooms, files, commands)` (sdcflows 2.17.1 `init_syn_sdc_wf`, = 2.15.1; every node in
    nipype's folders `out_dir/<name>/<node>`; fMRIPrep names it `wf_<estimator>`); helpers `warp_dir` (SDC-16),
    `mm2vox`, `adjust_zooms` (numpy 2's float32 scalar arithmetic: the value's type as sdcflows'), `norm_lap` (SDC-14;
    `_core.numpy_median`/`numpy_percentile`), `INPUT_FIELDS`, `MAX_LAPLACIAN_WEIGHT`, `SYN_METHOD`; `_laplacian`
    (ImageMath + nipype's copy_header), `_copy_header_keep_dtype`; `_apply_transforms(invert=None, args=...)`.
    `zooms_epi` (RegridToZooms, a dead node) is NOT computed (SDC-15); `files["syn_warped_image"]` with debug.
  - **`larmorx._nipype.registration` fix:** the interface's inputs now win over the settings file (nipype's
    `BaseInterface.__init__` sets them again after `load_inputs_from_json`; connections later): sdcflows' SyN
    `transform_parameters` (`_mm2vox`) and `metric_weight` replace sd_syn.json's. Found by check_syn_sdc.py on
    ds005003 (10.2354, its anatomy 0.977 mm along PE); no earlier caller passed a key its settings file holds; check_nipype
    antsRegistration still ALL OK. Unit test `test_registration_inputs_over_the_settings_file`.
  - **Defects (in code and docs; NOT yet in `larmorx_core::defects` nor the register: the defects relay):**
    **SDC-13 decided: rule C** (measured with `relay10/probe_sdc13.py` on two of fMRIPrep's SyN estimators, larmorx's preprocessing + estimation three times: A sdcflows' pass-through, B `Deoblique` as meant (0.5 degrees, centre kept in mm, axes on their closest world axes), C A with another seed: pixar008 (20.7 deg) field A-B p95 8.19 Hz vs seed 1.70 Hz, NMI of the unwarped EPI with the anatomy B -0.0008 vs A (seed 0.0001); ds004884 first (6.3 deg) p95 6.12 vs 2.40 Hz, NMI -0.0025 (seed 0.0003); the change is 3-5x the seed's spread and brings no gain: sdcflows' behaviour kept in both modes; logs oracles/logs/fmapb-r10-sdc13-{pixar008,ds004884}.log); **SDC-14** (`_norm_lap`
    of an image without positive or negative values: numpy's IndexError; rule B, a ValueError; reproduce the
    IndexError); **SDC-15** (`zooms_epi` dead node; rule C, not computed); **SDC-16** (`_warp_dir` indexes the fixed
    image's voxel axes, ANTs' `--restrict-deformation` weighs physical axes (relay10/probe_restrict_axes.py: larmorx's
    port and ANTs 2.6.5 identical, weights 1x0x0 move only physical x on an S,R,A grid); rule A, the PE's world axis;
    reproduce sdcflows' position; UpstreamDefectWarning when they differ; fMRIPrep's axial EPIs unchanged).
    `_warp_dir`'s `Logger.warn`: not a defect (the alias still works).
  - **Checks (`validation/investigations/sdcflows-fieldmap-b/check_syn_sdc.py`):** nodes (25 checks per estimator,
    ~6 s each, both envs): archived AVX-512 runs 5 estimators: all 5/5 but bs_filter 0/5 (the AVX-512 kernels in the
    ridge fit); re-made Haswell runs: all 25/25 on ds000228's estimator and ds004884's two, both envs, bs_filter included. syn (ds004884/wf_auto_00001, archived inputs): larmorx 373 s (8
    threads) = ANTs 2.6.5 one thread 1,054 s, forward and inverse warps byte for byte; fMRIPrep's 4-thread warp up to
    0.74 mm away. workflow: shared env 18/18 files of sdcflows' graph (1,036 s one thread; larmorx 333 s on 8);
    newest env 18/18 too (sdcflows 1,115 s; larmorx 518 s on 3 threads). Logs: oracles/logs/fmapb-r10-{nodes-avx512-shared,syn-workflow-shared,workflow-newest,
    sdc13-pixar008,sdc13-ds004884,restrict-axes}.log.
  - **The re-made (Haswell) SyN runs re-checked** (relay8/recheck.sh syn, now with check_syn_preprocessing nodes and
    check_syn_sdc nodes in both envs): **ds000228-sub-pixar008-noreconall-syn (exit 0 at 02:04): ALL OK everywhere**
    (check_bold_steps/stage both envs: recon paths direct 0, resampled 4 (no PIPE-21 real case); check_bold_fmap nodes;
    check_syn_preprocessing nodes (28 checks; n4_avgs/ref_anat/epi2anat = ANTs one thread; fMRIPrep's 4-thread epi2anat
    0.0077 apart); check_syn_sdc nodes 25/25 in both envs, bs_filter identical now). **ds004884-sub-M2079-noreconall-syn (exit 0 at 03:18): ALL OK everywhere** (log oracles/logs/fmapb-r10-recheck-ds004884-sub-M2079-noreconall-syn.log; recon paths direct 0, resampled 8; registrations within 6.9e-12; check_syn_preprocessing nodes 28 checks; check_syn_sdc nodes 2 estimators both envs, bs_filter 2/2). **RECHECK_LATER: ds005003-sub-RS001-noreconall-syn** (the queue's last run, started 03:18, about 50 min; final line FMAP-QUEUE-HASWELL-END): `bash <scratch>/r10/recheck_syn.sh ds005003-sub-RS001-noreconall-syn` (waits for its `exit 0`, then relay8/recheck.sh syn into oracles/logs/fmapb-r10-recheck-ds005003-sub-RS001-noreconall-syn.log, about 10 min boxed); its two estimators are the ones with the 0.977 mm anatomy (transform_parameters 10.2354) and the 0.081 affine difference of relay 9.
  - Golden: `py.sdcflows.syn_sdc` registered (2.5 s, repeatable; sloppy + debug + laplacian weights (clipped), SDC-14
    and SDC-16 values), NOT recorded (R11): 21 unrecorded cases fail tests/golden until then (20 + 1). Coverage passes.
  - Unit tests `tests/python/test_sdcflows_syn.py` (+4: helpers, norm_lap SDC-14, warp_dir SDC-16, the precedence).
  - Docs: `docs/api/sdcflows-fieldmap.md` ("The fieldmap-less (SyN) estimation (part B)" with the option and helper
    tables; SDC-13 decided, SDC-14, SDC-15, SDC-16 rows), `docs/findings/sdcflows-estimation.md` ("The fieldmap-less
    (SyN) estimation (relay 10)"), `crates/larmorx-sdcflows/PROVENANCE.md` (relay 10 entry), investigations README rows.
  - Gates (boxed, oracles/logs/fmapb-r10-gates-python.log): `gates-python.sh --no-build`: ruff check + format (967 files), check_spdx, collection without larmorx_validation OK; pytest 1,121 passed, 22 failed = the 21 unrecorded golden cases + `test_the_replica_itself` (the LARMORX_NC_BIN follow-up); CI's environment 733 passed, the same 21; `tables --check` OK. Not run: cargo gates (no Rust change), the smoke tier (R11).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: SDC-13 (rule C, measured), SDC-14 (rule
    B), SDC-15 (rule C), SDC-16 (rule A); `validation/tools.toml`: `lx.sdcflows.syn_sdc_workflow` and helpers
    (sdcflows-estimation); `docs/validation/golden.md`: `py.sdcflows.syn_sdc`; next-steps: niworkflows' `RegridToZooms`
    (`resample_by_spacing`: numpy's float32 `inv` of a float32 card (OpenBLAS's `sgesv`/`strsm` with the reciprocal
    diagonal), `np.dot` products, SciPy's `map_coordinates` order 3 with prefilter on int16, clip, `set_zooms`/qform/
    sform products; `smooth=True` needs SciPy's `gaussian_filter`) for `zooms_epi` and `syn_preprocessing_workflow
    (debug=True)`; larmorx's antsRegistration SyN uses ~3.4 of 12 cores (465 s on 12 threads for pixar008's 10 M-voxel
    estimator, 373 s on 8 for ds004884's): a follow-up for the ANTs port's threading; the oracle cache did not share the
    one-thread SyN run between a direct call and nipype's stand-in (nipype sets `NSLOTS` in the stand-in's environment,
    part of the key): give direct calls nipype's environment when a check wants one run; the stage catalogue's "SyN
    field-map-less estimation" roadmap entry (main): its nodes are ported (lx.sdcflows.syn_preprocessing_workflow and
    syn_sdc_workflow), to become stages with the other field-map entries (part A's B-spline entry is still a roadmap
    entry too).
- **Relay 11 = R7a (done, 2026-11-09): commits 9cad18ea, 4abe7b7f, 7c038b44, 8bdea199, 253f6473, 9c8de055** (on 1411e088).
  Agent id: the orchestrator's to add (relay 11). Scratch helpers in `<scratch>/r11/` (`run_est.sh <shared|newest>
  <sections> <log>`: check_estimators.py boxed 8 GiB; `golden_try.py` (the three new golden cases run twice, repeatable);
  `probe_order.py`, `probe_hdr.py`, `smoke.py`, `dbg_deriv.py` (first probes); `fetch_pybids020.sh` (committed in
  relay11/); `NOTES.before-relay11.md`). No Rust change (the `.so` of relay 9 is current; no `.pyi` change: pure Python).
  - **First: ds005003-syn's re-check: ds005003-sub-RS001-noreconall-syn (exit 0 at 04:27): ALL OK everywhere**
    (oracles/logs/fmapb-r10-recheck-ds005003-sub-RS001-noreconall-syn.log, relay8/recheck.sh syn via r10/recheck_syn.sh):
    check_bold_steps/stage both envs (recon paths direct 0, resampled 8), check_bold_fmap nodes, check_syn_preprocessing
    nodes (epi2anat = ANTs one thread; fMRIPrep's 4-thread epi2anat 6.7e-10 and 0.0076 apart: the second estimator's
    affine.json Random sampling, 0.081 on the AVX-512 run), check_syn_sdc nodes in both envs (bs_filter 2/2). **All
    seven Haswell re-runs are now re-checked** (the queue's log ends with FMAP-QUEUE-HASWELL-END; the me-queue's
    ds000210 is being re-made now: not this part's).
  - **Port (pure Python):**
    - **`larmorx.bids`** (new family `lx.bids`, `python/larmorx/bids/layout.py`, Apache-2.0 AND MIT; configs
      `python/larmorx/bids/config/{bids,derivatives}.json` (pybids 0.22.0, unchanged) and `nipreps.json` (niworkflows 1.15.4)):
      pybids 0.22.0's `BIDSLayout` without a database (D3): `BIDSLayoutIndexer` (ignore/force patterns, `layout_config.json`,
      derivatives never walked, dot files, links: scandir's order as fsspec's), entities searched in the whole path,
      metadata by pybids' inheritance (reverse natural order within a folder: the less specific sidecar wins; `null`
      dropped; `BIDSConflictingValuesError`; the `IntendedFor` association checks: pybids' `KeyError`/0.22's warning),
      `get` (all return types; `None`/`Query.NONE`/`REQUIRED`/`OPTIONAL`/`[]`; `regex_search` = case-insensitive search of
      every filter; integer filters as SQLite's CAST; `invalid_filters`; `scope`; extension dot), `get_<entity>s`
      (`__getattr__` with English plurals for inflect), `get_file`, `get_metadata` (keys by name), `get_entities`,
      `parse_file_entities`, `add_derivatives`, `BIDSFile` family (`get_entities` bids_sort'ed: bidsschematools 2.0.0's
      order; `pybids_version="0.20.0"`: unsorted), `PaddedInt` values, pybids' errors and messages. Not ported (nothing on
      the paths): associations, `build_path`/`write`, `to_df`, `get_collections`, database files, `bids-schema` config,
      `BIDSLayoutIndexer(validate=True)` (NotImplementedError: the BIDS validator). Windows: `/` in ignore matching.
    - **`lx.sdcflows`** (`estimators.py`, `wrangler.py`): `FieldmapFile` (`sdcflows_version="2.17.1"|"2.15"`: the broken-link
      check), `FieldmapEstimation` (attrs' setters, registration), `EstimatorType`, `MODALITIES`, `MetadataError`,
      `ImageFileError` (nibabel's errors for an unreadable header in the readout time: `_get_trt`), `bidict`,
      `EstimatorRegistry`, `FieldmapRegistry` + `REGISTRY` (sdcflows' globals as one object; every function takes
      `registry=None`), `get_identifier`, `clear_registry`, `relative_to_root`, `create_logger`; `find_estimators`
      (`upstream_defects`: fix = 2.15.2's search, reproduce = 2.17.1's), `find_anatomical_estimators`. `IntendedFor`
      written with `/` on every platform. `FieldmapEstimation.get_workflow` NOT ported yet: R7b (with fmap_preproc_workflow).
    - **`larmorx.pipelines.larmorprepx.estimators`**: `fmriprep_layout` (config.py's indexer), `map_fieldmap_estimation`,
      `get_estimator`, `collect_fieldmaps` (+ `FMAP_SPEC` = fmap_spec.json), `split_precomputed` (init_single_subject_wf's
      precomputed split and its messages), `setting_up_message`, `check_pepolar_scheme`.
    - Pin fetched: `tags/pybids-0.20.0` (sdist SHA-256 a68b577426942e50e6ac37e011463361d00a378d19b75df817619450357ebbb6
      = PyPI's; its layout/, utils.py, exceptions.py, configs = the image's copies).
  - **Defects (in code and docs; NOT yet in `larmorx_core::defects` nor the register: the defects relay):**
    **SDC-17** (rule A, upstream's own fix: 2.17.1 lacks 2.15.2's three wrangler fixes; fix = 2.15.2; warning computed from
    the read-only alternative queries), **SDC-18** (rule C: `bidict.add` takes the last free auto_ index), **SDC-19**
    (nondeterminism: step 1 iterates the B0FieldIdentifiers as a set (hash order per process); with aliases it decides
    which name survives; larmorx natural order in both modes; the check fixes sdcflows' order by
    `estimators_dump.natural_reduce`), **SDC-20** (rule A, message: get_identifier's literal `'{by}'`), **SDC-21** (rule
    A: the listed-B0FieldIdentifier query is a case-insensitive regex of every filter: subject 01 matches 010, part mag
    matches imag, pe matches "PE"; fix = exact filters + list membership; measured on the sdc21 tree), **PIPE-25** (rule A,
    fMRIPrep: a BOLD's B0FieldSource naming an alias of the estimator's sources gets no field map; fix = the alias reaches
    its estimator). Also documented (not defects): FieldmapFile reads only the direct sidecar; pybids' sidecar order within
    a folder; fMRIPrep's `^\.` ignore never matches; Windows separators in pybids and sdcflows.
  - **Checks (`validation/investigations/sdcflows-fieldmap-b/check_estimators.py` + `relay11/estimators_dump.py`), newest env
    (pybids 0.22.0 and sdcflows 2.17.1 are the same in the shared env; the image is the other library set):**
    index 306/306 layouts identical (19 oracle datasets, dsA/dsB/dsC, 13 generated skeletons (sdcflows' test specs incl.
    2.15.2's), 4 edge trees (sdc21, edges, conflict, badjson); fMRIPrep's, sdcflows app's, sdcflows tests' and pybids'
    default indexers; derivatives once); wrangler 2,790/2,790 (sdcflows 2.17.1 = reproduce with every layout call replayed; 2.15.2's wrangler = the default; sdcflows' identifier order fixed to the natural one, SDC-19; the 14 sdc21 cases SDC-21 corrects counted apart); units 577/577 (FieldmapFile, FieldmapEstimation 11 source sets x 2,
    registry, get_identifier, bidict 22 ops, EstimatorRegistry, SDC-21's correction); fmriprep 2,567/2,567 (fMRIPrep
    25.2.7's map_fieldmap_estimation + get_estimator extracted by ast; 24 option combos x every dataset, log messages,
    collect_fieldmaps on sdcflows' 8 runs, fmap_spec.json); runs 92/92 (the 23 oracle runs' "Setting up fieldmap" (19
    estimators), "Several fieldmaps" (ds002603: 2), "estimated with the following" lines and fmap_preproc_wf/wf_* folders,
    per session as fMRIPrep 25.2 runs them, pybids 0.20.0 + sdcflows 2.15 + reproduce); apps 8/8 (sdcflows 2.17.1's own runs'
    fmapids); image 612/612 (pybids 0.20.0, sdcflows 2.15.1, fMRIPrep 25.2.6 in the image: index, wrangler, fmriprep).
    **Final runs on the committed code (8bdea199/253f6473): newest env ALL OK** (index 306, wrangler 2,790 (+14 SDC-21
    expected), units 577, fmriprep 2,567: oracles/logs/fmapb-r11-est-final-newest.log; runs 88 (ds000210 skipped: the
    orchestrator's me-queue is re-making it, its RUN_INFO unfinished; it has no estimators), apps 8, image 595:
    fmapb-r11-est-final-newest2.log); **shared env ALL OK** (index 306, units 577, fmriprep 2,567, runs 92, apps 8:
    fmapb-r11-est-final-shared.log; the wrangler and image sections need no second run: pybids 0.22.0 and sdcflows 2.17.1 are
    the same there). The comparison sorts what the originals write in a set's order (SDC-19's identifier order via
    natural_reduce; the "Multiple common", "Incompatible suffices" and "Valid targets" lists; ties of get_<entity>s between
    strings such as ('PE',)/('pe',); with derivatives, the layouts' entity and configuration order): a first full run passed
    unsorted by chance and the next showed these per-process orders.
  - Golden: `py.bids.layout`, `py.sdcflows.estimators`, `py.larmorprepx.estimators` registered (NAMESPACES += `lx.bids`,
    `lx.pipelines.larmorprepx.estimators`; repeatable, paths with a blank and `~`), NOT recorded (R10): 24 unrecorded cases
    fail tests/golden until then (21 + 3). Coverage passes.
  - Unit tests `tests/python/test_bids_estimators.py` (5).
  - Docs: `docs/api/bids.md` (new: option table, kept details, differences, validation), `docs/api/sdcflows-fieldmap.md`
    ("The estimators of a BIDS dataset (part B)"; SDC-17..21 rows), `docs/api/fmriprep-fieldmap-steps.md` ("larmorprepx:
    the field-map estimators of a subject"; PIPE-25), `docs/findings/sdcflows-estimation.md` ("The estimators of a BIDS
    dataset and fMRIPrep's choice (relay 11)"), `crates/larmorx-sdcflows/PROVENANCE.md` (relay 11 entry), investigations
    README rows.
  - Gates (boxed, oracles/logs/fmapb-r11-gates-python.log, at 7c038b44; later commits: docs and the check's normalization
    only): `gates-python.sh --no-build`: ruff check + format, check_spdx, collection without larmorx_validation OK; pytest
    1,126 passed, 24 failed = the 24 unrecorded golden cases (21 + 3; `test_the_replica_itself` passed this time); CI's
    environment 738 passed, the same 24; `tables --check` OK. Not run: cargo gates (no Rust change), the smoke tier (R10).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: SDC-17 (A), SDC-18 (C), SDC-19
    (nondeterminism, not reproduced), SDC-20 (A), SDC-21 (A), PIPE-25 (A); `validation/tools.toml`: a new family `bids`
    (`lx.bids.BIDSLayout` and its queries; record: inside `sdcflows-estimation` or its own `bids-index`: the records relay
    decides), `lx.sdcflows.find_estimators` and the estimator classes (sdcflows-estimation), larmorprepx's estimators;
    `docs/validation/golden.md`: the three new cases; NOTICE: pybids 0.22.0 (MIT; `python/larmorx/bids/layout.py`, the two
    shipped configurations) and niworkflows' `nipreps.json` (Apache-2.0, shipped); `upstream.tsv`: `pybids-0.20.0`
    (https://files.pythonhosted.org/packages/8b/7a/595b5608054f73c0ed09817bfb6365d0931c27d929c0c0e70f8e1018c24f/pybids-0.20.0.tar.gz,
    sha256:a68b5774...7ebbb6, MIT, tags: fMRIPrep 25.2.6's, the step); `pybids-0.22.0`, `sdcflows-2.15.2`, `fmriprep-25.2.7`
    are relay 1's rows already; README's tool table (generated from tools.toml): `lx.bids`. Follow-ups for next-steps:
    pybids' BIDS validator (`validate=True` indexing) and the file associations (`get_fieldmap`, ...) if a later pipeline
    needs them; PIPE-25 and SDC-21 as upstream reports (drafts) for sdcflows/fMRIPrep.
- **Relay 12 = R7b (done, 2026-11-09): commits cf0dbcae, 0ece5b08, e3746556** (on 9c8de055; + this NOTES/gates
  commit if any). Agent id: the orchestrator's to add (relay 12). Scratch helpers in `<scratch>/r12/`
  (`probe_ds_nodes.py` (first probe of the ds_ nodes), `preproc.sh <shared|newest> <log> <sdcflows runs...>`
  (check_derivatives --sections preproc, boxed 16 GiB, LARMORX_NC_BIN = the main checkout's crates-nc build),
  `golden_try.py` (the two new golden cases twice: repeatable), `NOTES.before-relay12.md`). No Rust change (the
  `.so` of relay 9 is current; no `.pyi` change: pure Python).
  - **Port (pure Python):**
    - **`lx.bids.build_path`** (`python/larmorx/bids/writing.py`): pybids 0.22.0's `layout/writing.build_path`
      (= 0.20.0), `_expand_options`, `_expand_entities`.
    - **`lx.niworkflows.derivatives_data_sink`** (`python/larmorx/niworkflows/bids.py`) -> `DerivativesDataSinkResult
      (out_file, out_meta, compression, fixed_hdr; .out)`: niworkflows 1.15.4's `DerivativesDataSink` (= 1.15.2 =
      1.15.3), every input (`base_directory` (None = cwd), `out_path_base` (class attribute: 'niworkflows';
      sdcflows 'sdcflows'; fMRIPrep ''), `check_hdr`, `compress`, `data_dtype` (incl. 'source', DEFAULT_DTYPES),
      `dismiss_entities`, `meta_dict`, `allowed_entities`, `entities` (entity inputs), `metadata` (metadata inputs:
      RepetitionTime sets the 4th zoom), `init_metadata` (constructor's)), `STANDARD_SPACES` (TemplateFlow 25.1.2's
      30 templates), `template_metadata`/`templateflow_version` for a standard space's resolution
      (NotImplementedError without: the templates part), `get_tf_resolution`, `copy_any` (`_copy_any`: gzip level 9,
      no name, mtime 0; plain: nipype's `copyfile` = hard link else copy, with related .mat/.img/.HEAD, in
      `larmorx._nipype.copyfile`/`get_related_files`). nibabel's class choice (NIfTI-1 by magic ni1/n+1, NIfTI-2 by
      size, CIFTI-2 by intent 3000-3099 -> not NIfTI -> copied), load-time fixes (`nifti_read_header(fix=True)`),
      magic normalised, header setters (set_qform/sform via `lx.io.nibabel_header.set_xform`, units, zooms, dtype,
      slope/inter), `write_to` (header block, extension flag, extensions padded to 16, zeros to vox_offset),
      stored data copied raw (or coerced). Writes Python's gzip -> **gzip bytes identical too**.
    - **`lx.sdcflows`** (`python/larmorx/sdcflows/outputs.py`, `base.py`): `fmap_derivatives_workflow` (+
      `FmapDerivativesResult`, `gendesc`, `getname`, `getsourcetype`, `selectintent`, `SDCFLOWS_OUT_PATH_BASE`);
      `estimator_workflow` / `FieldmapEstimation.get_workflow` (new slot `_wf`, the cache) -> `EstimatorWorkflow(name,
      method, function, options, inputs; .run(out_dir, **inputs), .input_fields)`, `workflow_options()` (each
      workflow's keyword arguments by signature), `check_readout`, `EPI_LIKE_SUFFIXES`; `fmap_preproc_workflow`
      (`inputs={sanitized_id: {...}}` = the in_<id> node; PEPOLAR without inputs: the sources; ANAT needs
      syn_preprocessing's 5 outputs; `work_dir/<name>/wf_<id>`, `fmap_derivatives_wf_<id>`) -> `FmapPreprocResult`.
      Reports (`init_fmap_reports_wf`) not written: larmorprepx's reports part (D7), said in the API page.
  - **Defects (in code and docs; NOT yet in `larmorx_core::defects` nor the register: the defects relay):**
    **PIPE-26** (nondeterminism: two or more list-valued entity inputs combined in a set's order; larmorx sorted,
    warning), **PIPE-27** (rule A, message: "Changing ... dtype from X to X"), **PIPE-28** (rule B: compress shorter
    than the files -> IndexError; fix ValueError), **PIPE-29** (undefined float->int casts in the dtype coercion:
    clamp + warning; reproduce numpy's cast), **PIPE-30** (rule A: RepetitionTime input on a non-4-D image ->
    nibabel's HeaderDataError; fix: 4th zoom only on images with a 4th axis), **SDC-22** (rule B: a keyword
    argument no estimator workflow takes is ignored by sdcflows; fix TypeError). Kept (documented, no ID):
    get_workflow's cache returns the first workflow whatever the later arguments.
  - **Checks (`validation/investigations/sdcflows-fieldmap-b/check_derivatives.py`), both envs ALL OK**
    (oracles/logs/fmapb-r12-derivatives-newest.log, fmapb-r12-derivatives-shared.log): `nodes` every field-map ds_ node given its own `_inputs.pklz`: fMRIPrep runs 95 files (76 NIfTI, 19
    JSON) of 19 estimators, sdcflows app runs 75 (60 + 15) of 15 estimators: names, uncompressed bytes, **gzip
    bytes** and sidecars identical (every sink fixed the header; fMRIPrep's image needs `use_lapack("3.12.0")`:
    ds006736's coefficients' quatern_d last bit otherwise); `workflow` the 34 estimators' derivatives through
    `fmap_derivatives_workflow` with larmorx's estimators (fMRIPrep's choice / the app's search) and the runs'
    estimation outputs: all files identical (RawSources compared with the host root mapped to /data); `synthetic`
    38 edge cases vs niworkflows' own sink (33 with files incl. the related .mat, 5 errors), defects' cases with
    reproduce; `preproc` sdcflows 2.17.1's own `init_fmap_preproc_wf` in nipype (shims: N4 one thread CORE-MATH,
    prelude, topup, 3dvolreg through the oracle cache) vs `fmap_preproc_workflow` (replicas, reproduce):
    ds005752-direct 5/5 files, ds001600 (3 phase-difference + 1 PEPOLAR on two grids, 2.17 wiring) 20/20 files,
    gzip bytes included (logs oracles/logs/fmapb-r12-preproc-{direct,ds001600}-shared.log; 227 s sdcflows one
    thread, 198 s larmorx on 8: mostly TOPUP's replica).
  - Golden: `py.niworkflows.derivatives_data_sink` (26 arrays) and `py.sdcflows.fmap_preproc` (16; a MAPPED
    estimator run end to end on a 30x28x26 synthetic dataset, a PEPOLAR one built) registered, repeatable, ~2 s,
    NOT recorded (R10 = relay 17): 26 unrecorded cases fail tests/golden until then (24 + 2). Coverage passes.
  - Unit tests `tests/python/test_sdcflows_derivatives.py` (5).
  - Docs: `docs/api/niworkflows-helpers.md` ("`derivatives_data_sink`: `DerivativesDataSink`" with the option
    table; PIPE-26..30 rows), `docs/api/sdcflows-fieldmap.md` ("The estimators' workflows and their derivatives
    (part B)"; SDC-22 row; get_workflow's row), `docs/api/bids.md` (`build_path`), `docs/findings/
    sdcflows-estimation.md` ("The derivatives and `init_fmap_preproc_wf` (relay 12)"), PROVENANCE (relay 12 entry),
    investigations README row.
  - Gates (boxed, oracles/logs/fmapb-r12-gates-python.log, at e3746556): `gates-python.sh --no-build`: ruff check +
    format (980 files), check_spdx, collection without larmorx_validation OK; pytest 1,131 passed, 26 failed = the
    26 unrecorded golden cases (24 + 2); CI's environment 743 passed, the same 26; `tables --check` OK. Shared-env
    derivatives log: oracles/logs/fmapb-r12-derivatives-shared.log (ALL OK). Not run: cargo gates (no Rust
    change), the smoke tier (R10).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: PIPE-26 (nondeterminism), PIPE-27
    (A), PIPE-28 (B), PIPE-29 (undefined conversion), PIPE-30 (A), SDC-22 (B); `validation/tools.toml`:
    `lx.niworkflows.derivatives_data_sink` (+ copy_any, get_tf_resolution) in niworkflows-helpers,
    `lx.sdcflows.fmap_preproc_workflow`, `estimator_workflow`, `fmap_derivatives_workflow` (+ helpers) in
    sdcflows-estimation, `lx.bids.build_path` in the bids family; `docs/validation/golden.md`: the two new cases;
    NOTICE: pybids 0.22.0's `writing.py` (MIT, already noticed for layout.py: add the file), TemplateFlow 25.1.2's
    template list (a list of names; Apache-2.0 client) in `python/larmorx/niworkflows/bids.py`; `upstream.tsv`:
    nothing new (niworkflows 1.15.4, nipype 1.12.0, pybids 0.22.0/0.20.0 listed; templateflow 25.1.2 is only the
    source of a name list: the orchestrator decides whether it needs a row). Follow-ups for next-steps: the data
    sink's TemplateFlow resolution (`template_metadata`) with the templates part; the reports (`FieldmapReportlet`)
    with larmorprepx's reports part (D7, already planned).
- **Relay 13 = R7c (done, 2026-11-09): commits 04fb1068, 98e68a88** (on e3746556; + this NOTES commit-free
  update). Agent id: the orchestrator's to add (relay 13). Scratch helpers in `<scratch>/r13/` (`stage.sh <shared|newest>
  <log> <sections...> [--datasets .. --reference ..]`: check_fmap_stage.py boxed 12 GiB with FMRIPREP_IMAGE_SRC and
  LARMORX_NC_BIN; `golden_try.py` (the three cases twice: repeatable), `golden_peek.py`, `opts.py`, `golden_new.py.txt`
  (the cases' text), `NOTES.before-relay13.md`). No Rust change (the `.so` of relay 9 is current; no `.pyi` change).
  **The split was used: relay 13 = R7c = the stage + 3dQwarp's Python steps; relay 14 = R7d (below).**
  - **Port (pure Python):**
    - **larmorprepx's field-map estimation stage** (`python/larmorx/pipelines/larmorprepx/estimators.py`; fMRIPrep
      25.2.7 `workflows/base.py` lines `fmap_cache = {}` .. the BOLD workflows, byte-identical to 25.2.6's image):
      `precomputed_fieldmaps(derivatives, subject)` (fmap_cache + its debug line); `plan_fieldmap_estimation(all_estimators,
      fmap_cache, *, output_dir, subject, omp_nthreads, fallback_total_readout_time, debug, sloppy,
      exit_on_missing_readout=True, syn_options, logger, upstream_defects, n_threads, **workflow_options)` ->
      `FieldmapEstimationPlan` (all_estimators, estimator_types, precomputed, precomputed_buffers (in1), estimators,
      preproc_options (out_path_base ''), inputs (PEPOLAR in_data/metadata), syn_preprocessing (`SynPreprocessingCall`:
      name syn_preprocessing_<id>, bold/sbref sources+meta, omp_nthreads, debug=sloppy, auto_bold_nss True,
      t1w_inversion False), desc (the boilerplate), workflows; `.needs_anat`); the graph built first through the new
      **`lx.sdcflows.build_workflows`** (fmap_preproc_workflow's build loop moved out unchanged: readout checks +
      estimator_workflow, cached on the estimators, so the run uses the same workflows); RuntimeError -> critical
      "Missing readout time information. ..." with exc_info + `SystemExit(65)` (`EX_DATAERR`; `exit_on_missing_readout=False`
      re-raises); per estimator setting_up_message, check_pepolar_scheme; `FieldmapEstimationPlan.run(*, work_dir,
      t1w_preproc, t1w_mask, std2anat_xfm, templates)` -> `FieldmapEstimationResult(estimates (FieldmapEstimates),
      estimator_types, std2anat_xfm, syn_preprocessing, preproc)`: fmap_select_std = `key_select(templates,
      'MNI152NLin2009cAsym')`, syn_preprocessing_workflow in work_dir/<name> (new **`name=` parameter** of
      `syn_preprocessing_workflow`, default 'syn_preprocessing_wf'), fmap_preproc_workflow in work_dir/fmap_preproc_wf,
      `merge_buffers(precomputed_buffers, preproc)` (nipype's Merge(2) through fieldmaps._merge); `estimate_fieldmaps` =
      plan + run. Constants BUFFER_FIELDS, EX_DATAERR, SYN_PRIOR_TEMPLATE, FMAP_PREPROC_DESC.
    - **3dQwarp's Python steps** (`python/larmorx/sdcflows/qwarp.py`, sdcflows 2.17.1 = 2.15.x): `flatten` (Flatten/
      _flatten: nibabel's slicer + four_to_three, `<stem>_idx-NNN.nii.gz`), `sorted_pe` (_sorted_pe, `QWARP_ARGS`),
      `qwarp2ants` (ConvertWarp/_qwarp2ants: float32, intent vector, X x Y x Z x 1 x 3; **named `_warpfield<in>`**:
      sdcflows passes '_warpfield' as fname_presuffix's prefix, kept), `qwarp_workflow` (NotImplementedError naming
      3dQwarp, mri_robust_template, init_enhance_and_skullstrip_bold_wf, before any work).
  - **Defects (in code and docs; NOT yet in `larmorx_core::defects` nor the register: the defects relay):** **PIPE-31**
    (rule B: a precomputed field map lacking its fieldmap/magnitude/coeffs file -> fMRIPrep's bare KeyError while it
    builds the graph; fix ValueError naming the id, the missing and found files; reproduce KeyError; the check is in
    `split_precomputed`, which got `upstream_defects=`), **SDC-23** (rule B when ported: init_3dQwarp_wf reads
    `inputnode.in_reference`, absent: fmap_ref never computed; documented, the workflow is NotImplementedError),
    **SDC-24** (rule A: `_qwarp2ants`' np.squeeze drops a spatial axis of length one: a one-slice warp becomes 4-D
    X x Y x 1 x 3; fix keeps the three spatial axes, warning only when it changes the shape). Kept (documented, no ID):
    the `_warpfield` prefix name; nipype's OutputMultiObject unwrapping a one-item list (larmorx returns tuples).
  - **Checks, ALL OK:** `validation/investigations/sdcflows-fieldmap-b/check_fmap_stage.py`: `graph` fMRIPrep 25.2.7's own
    stage lines executed (under `if True:`, stand-in config/anat_fit_wf/map_fieldmap_estimation, sdcflows' estimators
    made from larmorx's) to build the real nipype graph: 85 cases (12 fMRIPrep oracle runs with field maps x {base,
    'estimated', 0.05, debug, sloppy, own derivatives} + one-of-two precomputed + PIPE-31 on 4 runs + 5 synthetic:
    missing readout = SystemExit 65, sophisticated PEPOLAR, 'estimated', fallbacks): log lines (+ the critical line's
    exception), errors, init_fmap_preproc_wf kwargs, in_<id> inputs, syn kwargs + inputs, buffers' in1, ds_ out_path_base,
    every edge, desc, estimator types identical; PIPE-31 fixed = ValueError 4/4; the stage's source = the image's
    (FMRIPREP_IMAGE_SRC); `nodes` the 12 runs' work folders: fmap_select_std/syn_preprocessing_*/wf_* folders, PEPOLAR
    in_data+metadata (6 estimators), syn in_epis/in_meta/std2anat_xfm (= key_select of fmap_select_std's inputs)/anat
    inputs (5), the six *_merge outputs through merge_buffers from out_merge_* (12 runs), the log's setting-up and
    estimated lines: identical. Both envs (oracles/logs/fmapb-r13-stage-shared.log, fmapb-r13-stage-newest.log).
    `run` (shared, oracles/logs/fmapb-r13-run-shared.log): estimate_fieldmaps end to end on ds005752-direct (1 s, 8
    threads) and ds001600 (4 estimators, 197 s) = sdcflows 2.17.1's own init_fmap_preproc_wf (relay 12's
    `<scratch>/r12/p-shared/preproc` reference): 25 files, uncompressed + gzip + JSON bytes identical; buffers named
    as the runs' (6 x 2). **Information / open question for the records relay:** against fMRIPrep's own out/: brain
    masks and coefficients identical for every field-map estimator, ds005752's preproc fieldmap identical; magnitudes
    differ (fMRIPrep's N4 at 4 threads, glibc); **ds001600's three phase-difference preproc fieldmaps differ although
    their coefficients are identical** (look at it with fMRIPrep's library versions: numpy 2.2.6/SciPy 1.15.2 and the
    data sink's LAPACK 3.12.0?); its PEPOLAR differs (the run used 2.17 wiring, fMRIPrep 2.15: expected).
    `check_qwarp_steps.py` (both envs, fmapb-r13-qwarp-{shared,newest}.log): sdcflows' own Flatten (13 cases),
    _sorted_pe (9), ConvertWarp (8 warps incl. big-endian, scaled int16, oblique, no sform, one slice with reproduce):
    names, metadata, uncompressed bytes, errors identical; SDC-24 fix shape + warning; qwarp_workflow's error. (gzip
    bytes of files nibabel writes are not compared: zlib vs larmorx's deflate, as everywhere.)
  - Golden: `py.larmorprepx.fmap_estimation` (13 arrays: precomputed PEPOLAR + estimated MAPPED, plan with both,
    buffers, derivatives, merge_buffers) and `py.sdcflows.qwarp` (15) registered, repeatable (blank and `~` paths),
    ~0.5 s; `py.sdcflows.fmap_preproc`'s dataset moved into `_fmap_bids(ctx, name)` (same outputs, repeatable). NOT
    recorded (R10): 28 unrecorded cases fail tests/golden until then (26 + 2). Coverage passes.
  - Unit tests `tests/python/test_fmap_stage.py` (7).
  - Docs: `docs/api/fmriprep-fieldmap-steps.md` ("larmorprepx: the field-map estimation stage of a subject": option
    table, PIPE-31 row, validation), `docs/api/sdcflows-fieldmap.md` (build_workflows row, syn_preprocessing `name`
    row, "The PEPOLAR estimation with 3dQwarp: its Python steps (part B)", SDC-23/24 rows),
    `docs/findings/sdcflows-estimation.md` ("fMRIPrep's field-map estimation stage and 3dQwarp's Python steps (relay
    13)"), PROVENANCE (relay 13 entry), investigations README rows.
  - Gates (boxed, oracles/logs/fmapb-r13-gates-python.log, at 98e68a88): `gates-python.sh --no-build`: ruff check +
    format (985 files), check_spdx, collection OK; pytest 1,138 passed, 28 failed = the 28 unrecorded golden cases;
    CI's environment 750 passed, the same 28; `tables --check` OK; check_spdx OK. Not run: cargo gates (no Rust
    change), the smoke tier (R10).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: PIPE-31 (B), SDC-23 (B, documented),
    SDC-24 (A); `validation/tools.toml`: larmorprepx's `precomputed_fieldmaps`, `plan_fieldmap_estimation`,
    `estimate_fieldmaps`, `merge_buffers` (with the estimators entry), `lx.sdcflows.build_workflows`, `flatten`,
    `sorted_pe`, `qwarp2ants`, `qwarp_workflow` (sdcflows-estimation); `docs/validation/golden.md`: the two new cases;
    NOTICE / upstream.tsv: nothing new (sdcflows 2.17.1, fMRIPrep 25.2.7, nipype 1.12.0, nibabel 5.4.2 listed).
    Follow-up for next-steps: `qwarp_workflow` when AFNI's 3dQwarp, FreeSurfer's mri_robust_template and niworkflows'
    init_enhance_and_skullstrip_bold_wf are ported (SDC-23 then rule B).
- **Relay 14 = R7d (done, 2026-11-09): commits 97733ab9, 41bcdae9, 81f24a69** (on 98e68a88; + this NOTES commit-free
  update). Agent id: the orchestrator's to add (relay 14). Scratch helpers in `<scratch>/r14/` (`fmriprep_stage.sh
  <shared|newest> <log> <runs...>` (check_fmap_stage.py --sections fmriprep, boxed 16 GiB), `unwarp.sh <env> <log>`
  (check_unwarp.py, boxed 10 GiB), `match.sh <env> <log>` (check_match_histogram.py, boxed 8 GiB), `gates.sh`
  (gates-python --no-build + tables --check, boxed 16 GiB), `golden_try.py` (the two new cases twice), `probe_ds1600.py`,
  `probe_hdr.py`, `probe_tree.py`, `probe_ravg.py` (the open question's probes), `NOTES.before-relay14.md`). No Rust
  change (the `.so` of relay 9 is current; no `.pyi` change: pure Python).
  **The split was used: relay 14 = R7d = the open question + `unwarp_workflow` + `match_histogram`; relay 15 = R7e =
  `epi_mask` (below); every later relay one number on (R8 = 16, R9 = 17, R9b = 18, R10 = 19, R11 = 20, R12 = 21).**
  - **Relay 13's open question: CLOSED, no larmorx mistake.** `check_fmap_stage.py --sections fmriprep` (new):
    `estimate_fieldmaps` as fMRIPrep ran the stage (`wiring="2.15"`, numpy 2.2.6 / SciPy 1.15.2 as the image's,
    `use_lapack("3.12.0")`, the replicas, reproduce) on ds005752-direct, ds001600 (3 phdiff + PEPOLAR on two grids),
    ds005040 (PEPOLAR), ds002422 (2 phdiff), ds006736 (PEPOLAR): 9 estimators, 36 NIfTI + 9 JSON vs fMRIPrep's own
    `out/`: 13 identical (uncompressed + gzip), 9 JSON identical, 13 data identical with ONLY `descrip` = FSL's
    newimage build string (image fsl-newimage 2501.4 `2501.4 2025-04-22T09:56:22+01:00` vs FSL 6.0.7.23's 2601.0
    `2601.0 2025-12-08T15:20:57Z`, written by PRELUDE/TOPUP, copied on by Phasediff2Fieldmap/BSplineApprox/the
    sinks: **this is all of ds001600's three phase-difference field maps' difference**), 10 differ only through N4
    (at most 1 stored int16 value in 1..302 voxels: the magnitude refs, the PEPOLAR refs, ds005040's PEPOLAR mask 3
    voxels). The N4 cause separated: each of the 9 `n4` nodes re-run with the ANTs oracle (GCC 14.3) at the node's
    **8** threads (`num_threads`, not 4 as relay 13 wrote) with glibc = fMRIPrep's file 9/9; at one thread CR =
    larmorx's 9/9. ALL OK in both envs (oracles/logs/fmapb-r14-fmriprep-{shared,newest}.log). Also measured: with
    numpy 2.3.5 instead of 2.2.6 the 2.15 PEPOLAR estimates of the series EPIs (ds001600, ds005040) differ (runwise_avg's
    drift sum, pairwise in 2.3; TOPUP amplifies: fields up to 3.7 / 19.6 Hz, coefficients up to 11.9 / 46.0 apart);
    with 2.2.6 identical: a numpy step the records relay shows per case. Findings: `docs/findings/sdcflows-estimation.md`
    "The stage against fMRIPrep's own derivatives, every difference explained (relay 14)" (relay 13's paragraph
    rewritten).
  - **Port (pure Python):**
    - **`lx.sdcflows.unwarp_workflow`** (`python/larmorx/sdcflows/correction.py`; sdcflows 2.17.1 = 2.15.1 = 2.15.2
      `workflows/apply/correction.py`) -> `UnwarpWorkflowResult(fieldmap, fieldwarp (None), corrected, corrected_ref,
      corrected_mask, num_threads, nodes)`; nodes rotime (get_readout_time), resample (apply_coeffs_field), merge
      (merge_series), average (robust_average mc_method=None), brainextraction_wf (_brainextraction); `unwarp_num_threads`,
      `MEM_PER_THREAD`; a list target -> TypeError (nipype's File trait message). **SDC-25** (rule C, kept): `fieldwarp`
      connected to nothing -> None. **SDC-26** (rule B): both `fmap2data_xfm` and `data2fmap_xfm`: the graph sets both
      (no xor check through connections) and `_run_interface` uses data2fmap_xfm -> ValueError; reproduce uses
      data2fmap_xfm with a warning.
    - **`lx.sdcflows.match_histogram`** (`syn.py`; sdcflows' `fit/syn.match_histogram`, unused by any workflow) over
      **`lx.skimage.match_histograms`** (`skimage.py`; scikit-image 0.26.0 = 0.25.2 `exposure/histogram_matching.py`,
      numpy's unique/bincount/cumsum/interp called directly: the same bits in 2.2.6/2.3.5/2.5.4; channel_as_last_axis
      ported). **SKI-02** (rule A): uint image + a reference bincount refuses (float: TypeError, negatives: ValueError,
      uint64) -> unique path (identical wherever scikit-image works). **SDC-27** (D13 undefined casts): matched values
      out of the int type's range or NaN -> clamp (NaN 0) + warning; reproduce = numpy's cast. Truncation toward zero
      into int images kept (documented, no ID).
  - **Checks, ALL OK in both envs:** `check_unwarp.py` (oracles/logs/fmapb-r14-unwarp-{shared,newest}.log): sdcflows'
    own init_unwarp_wf in nipype (N4 shim) on ds005752-direct/ds006736/ds000228-syn coefficients + boldref + the run's
    boldref->fmap xfm, a 4-D target with hmc matrices, fmap2data_xfm without Jacobian, fallback 0.05 (+free_mem),
    EstimatedEffectiveEchoSpacing with use_metadata_estimates (+debug), both transforms (reproduce = sdcflows'; fix
    refuses), 3 errors: 81 node files identical, no other file, outputs at sdcflows' places, fieldwarp, hmc warning;
    48 thread settings. `check_match_histogram.py` (fmapb-r14-match-{shared,newest}.log): 7 real files (3 SyN runs'
    anat2epi onto clip_epi masked/unmasked, ds000228's raw T1w), 12 synthetic files (7 identical, 5 same errors, SKI-02
    x3 and SDC-27 x2 corrected with warnings), 10 array cases (6 values, 4 errors).
  - Golden: `py.sdcflows.unwarp` (26 arrays) and `py.sdcflows.match_histogram` (11) registered, repeatable (blank and
    `~` paths), <0.5 s, NOT recorded (R10 = relay 19): 30 unrecorded cases fail tests/golden until then (28 + 2).
  - Unit tests `tests/python/test_sdcflows_unwarp.py` (9).
  - Docs: `docs/api/sdcflows-fieldmap.md` ("Unwarping an EPI with the coefficients: `unwarp_workflow` (part B)" with
    the option table; `match_histogram`'s helper row; SDC-25/26/27 rows), `docs/api/skimage.md` (match_histograms row,
    SKI-02), `docs/findings/sdcflows-estimation.md` (the stage section, unwarp, match_histogram sections), PROVENANCE
    (`crates/larmorx-sdcflows/PROVENANCE.md` relay 14 entry, `crates/larmorx-ndimage/PROVENANCE.md` match_histograms),
    investigations README rows (check_unwarp.py, check_match_histogram.py, check_fmap_stage.py's `fmriprep`).
  - Gates (boxed, oracles/logs/fmapb-r14-gates-python.log, at 41bcdae9; 81f24a69 = the check's `--numpy-version`
    option, `relay14/probe_ravg.py` and README rows, ruff + SPDX checked): `gates-python.sh --no-build`: ruff check +
    format (989 files), check_spdx, collection without larmorx_validation OK; pytest 1,147 passed, 30 failed = the 30
    unrecorded golden cases; CI's environment 759 passed, the same 30; `tables --check` OK. Not run: cargo gates (no
    Rust change), the smoke tier (R10). `check_fmap_stage.py --sections fmriprep --numpy-version 2.3.5` reproduces the
    numpy step's numbers; `relay14/probe_ravg.py` the first node's (147,312 voxels).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: SDC-25 (C, documented), SDC-26 (B),
    SDC-27 (undefined conversion), SKI-02 (A; scikit-image family); `validation/tools.toml`: `lx.sdcflows.unwarp_workflow`
    (+ `unwarp_num_threads`) and `match_histogram` in sdcflows-estimation, `lx.skimage.match_histograms` in the skimage
    entry; `docs/validation/golden.md`: the two new cases; NOTICE / upstream.tsv: nothing new (sdcflows 2.17.1,
    scikit-image 0.26.0/0.25.2 listed). For the records relay: the stage's fMRIPrep comparison (the `fmriprep` section)
    and its numpy 2.2 -> 2.3 step per case.
- **Relay 15 = R7e (done, 2026-11-09): commits f6e2861b, b28c8b2a, dc9c981f** (on 81f24a69). Agent id: the orchestrator's to add (relay 15).
  Scratch helpers in `<scratch>/r15/` (`epi.sh <shared|newest> <log>` (check_epi_mask.py, boxed 16 GiB), `nd.sh
  <env> <log>` (check_ndimage.py --threads 4, boxed 16 GiB), `gates.sh` (gates-python --no-build + tables --check),
  `golden_try.py` (the three new cases twice), `golden_diff.py` (their fingerprints: distinct paths),
  `golden_search.py` (the epi_mask input whose two modes differ: seed 252), `profile_epi_mask.py` (each step at one
  thread against SciPy), `com_probe.py`, `probe_queue_only.py` (committed as `probe_watershed_queue.py`), `probe_ws.py`
  (SciPy's edge behaviours)). The `.so` was rebuilt (Rust changed): current = the committed source.
  - **Port:**
    - **`lx.ndimage.watershed_ift`** (Rust `crates/larmorx-ndimage/src/watershed.rs`: SciPy 1.18.1 `ni_measure.c`
      `NI_WatershedIFT`, byte-identical in 1.15.3; binding `ndimage_watershed_ift` in `crates/larmorx-py/src/
      ndfilters.rs`, `_core.pyi`; wrapper in `python/larmorx/_ndimage_filters.py` with `_measurements.py`'s checks
      and messages). SciPy's per-level doubly linked queues kept exactly (`faithful`); a corrected run (neighbours
      inside the array, an element always taken out of its queue, 64-bit markers); both runs made, the chosen one
      returned with SciPy's run's events and the number of differing elements; `upstream_defects` (default fix),
      a warning per defect whenever the labels differ. Labels stored in the output's type both modes (int8: 300 ->
      44); a non-array `output` ignored (SciPy's wrapper); 0-d keeps its marker.
    - **`lx.ndimage.center_of_mass`** (Python: `_measurements.py`'s `center_of_mass` and `_stats`' single group;
      `numpy_version` keyword: whole-array/selected float32/float64 sums of C- or F-contiguous arrays through
      `_core.numpy_add_reduce` in memory order; other layouts/types the installed numpy's sum; a sequence index via
      `sum_labels`' bincount).
    - **`lx.sdcflows.epi_mask`** (`python/larmorx/sdcflows/masks.py`, `EpiMaskResult(mask, centre, out_file)`;
      sdcflows 2.17.1 = 2.15.1 `utils/epimanip.py`): `numpy_version` (percentiles, the centre's sums),
      `upstream_defects`, `write`, `n_threads`; numpy's x86-64 casts made explicit (`_x86_int64` for the centre:
      NaN -> -2**63 -> numpy's IndexError; `_uint16_levels`: **SDC-28**, D13's clamp + warning (NaN 0, inf 65535),
      reproduce = x86-64's 0). Exported from `lx.sdcflows`.
    - **Speed of the grey filters (exact):** `min_or_max_filter` got NaN-free fast paths (`NdElement::add_plain`,
      `to_f64_plain`, `finite`, `any_nan`): plain arithmetic with the structure cast once at interior positions
      (`extreme_plain`), nested per-axis tables at 3-D border positions (`extreme_plain_3d`, `Neighbourhood::axes3`),
      and a contiguous last axis's interior positions all at once, element by element in footprint order
      (`lines_plain`, `Neighbourhood::line_interior`). One thread, ds004958's 104x104x72 reference: opening ball(3)
      0.29 s (SciPy 0.29), closing ball(2) 0.10 (0.10), erosion ball(5) of the Fortran-ordered image 0.78 (0.57:
      per-point path; was 2.08); epi_mask 0.98 s vs sdcflows 1.05 s there, 0.23-0.29 vs 0.18-0.25 s on the 64^2
      references (the first erosion reads nibabel's F-ordered data). Tried and dropped (slower): run-wise iterators,
      comparisons in the element type.
  - **Checks, ALL OK in both envs** (at the final `.so`): `check_epi_mask.py` (oracles/logs/fmapb-r15-epi-{shared,
    newest}.log): watershed 600 seeded cases reproduce = SciPy (labels + type), fix = an independent Python
    corrected IFT 600/600, warning iff the labels differ 600/600 (differ in 209: W1 among the causes in 205, W2 in
    169, W3 in 17), 3 edge cases + 13 errors identical; center_of_mass 44/44 bit for bit; synthetic 4 files
    identical (reproduce) + 2 errors identical (4-D: SciPy's `footprint.ndim (3) must match len(axes) (4)`, flat:
    numpy's IndexError); real (desc-hmc_boldref of ds005040, ds000228, ds004958, ds003592) 4/4 uncompressed bytes =
    sdcflows' with reproduce; corrected masks: ds000228 131,072 (the whole image) -> 47,081, ds003592 12,843 ->
    51,162, ds005040 15,882 -> 16,106, ds004958 143 = 143. `check_ndimage.py` (fmapb-r15-ndimage-{shared,newest}.log)
    all 39,436 cases ALL OK after the speed work. `probe_watershed_queue.py`: W2 alone (1-D) changes 37 of 20,000.
  - **Defects:** SDC-28 (D13 undefined conversion, clamp + warning). SciPy's watershed candidates 8-11 in
    `docs/findings/scipy-ndimage-filters.md`, coded with **provisional IDs SCI-W1..SCI-W4** (`_WATERSHED_DEFECTS` in
    `python/larmorx/_ndimage_filters.py`, the unit tests and the API pages): W1 neighbours across edges (rule A), W2
    an element alone in its queue (rule A), W3 32-bit markers (rule A), W4 int byte offsets above 2 GiB (out of
    bounds: never reproduced; 64-bit in both modes, a warning). **FOR THE ORCHESTRATOR: four SciPy IDs are needed
    beyond relay 2's seven candidates (SCI-06..12 is this part's range; SCI-13 on are t2smap's): name the range; the
    defects relay replaces SCI-W1..W4 (grep `SCI-W`).**
  - Golden: `py.ndimage.watershed_ift` (8 arrays: both modes of a uint8 image, uint16 + full structure + int8
    output, the 1-D queue case, int64 beyond 32 bits), `py.ndimage.center_of_mass` (10: numpy 2.2/2.3/2.5 x C/F on
    26,970 elements, labels, indexes, int16) and `py.sdcflows.epi_mask` (12: a noisy head both modes (7 voxels
    differ) and numpy 2.3, a scaled int16 image touching the edges) registered, repeatable (blank and `~`), <0.2 s,
    NOT recorded (R10 = relay 19): 33 unrecorded cases fail tests/golden until then (30 + 3). Coverage passes.
  - Unit tests: Rust `tests.rs` (4 watershed tests: SciPy's values hand-checked + corrected), `tests/python/
    test_epi_mask.py` (7).
  - Docs: `docs/api/ndimage-filters.md` (title, rows, the watershed's exactness, the speed paths, "Differences":
    the four candidates, Rust API), `docs/api/sdcflows-fieldmap.md` ("A quick EPI mask: `epi_mask`" with the option
    table and the four runs' numbers; SDC-28 row), `docs/findings/sdcflows-estimation.md` ("`epi_mask` and SciPy's
    watershed (relay 15)", the candidates' list updated), `docs/findings/scipy-ndimage-filters.md` (candidates 8-11,
    the speed work), PROVENANCE (`crates/larmorx-ndimage`: watershed row, center_of_mass, the fast paths;
    `crates/larmorx-sdcflows`: relay 15 entry), investigations README rows (check_epi_mask.py,
    probe_watershed_queue.py).
  - Gates: (boxed; oracles/logs/fmapb-r15-gates-rust.log, fmapb-r15-gates-python.log, at the final source):
    `gates-rust.sh <worktree> root` (fmt, clippy -D warnings, tests) OK; `gates-python.sh --no-build`: ruff check +
    format (993 files), check_spdx, collection without larmorx_validation OK; pytest 1,154 passed, 33 failed = the
    33 unrecorded golden cases; CI's environment 766 passed, the same 33; `tables --check` OK. crates-gpl/crates-nc:
    they do not use `larmorx-ndimage` (no check needed; no dependency change). Not run: the smoke tier (R10).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: SDC-28 and the four watershed
    candidates (IDs above); `validation/tools.toml`: `lx.ndimage.watershed_ift` and `center_of_mass` in the ndimage
    entry, `lx.sdcflows.epi_mask` in sdcflows-estimation (or sdcflows-masks); `docs/validation/golden.md`: the three
    new cases; NOTICE: the SciPy entry gains `ni_measure.c` (`NI_WatershedIFT`) and `_measurements.py`
    (`watershed_ift`, `center_of_mass`); upstream.tsv: nothing new. Follow-ups for next-steps: the grey filters'
    per-point path for inputs whose last axis is not contiguous (nibabel's Fortran-ordered data: 1.35x SciPy at one
    thread; a line pass along the contiguous axis needs a scatter into the C-ordered output), the median/rank
    filters' one-thread gap (relay 4's 0.97 vs 0.81 s).
- **Relay 16 = R8 (done, 2026-11-09): commits 5abde0d5, 9017ed3f** (on dc9c981f). Agent id: the orchestrator's to add (relay 16).
  Scratch helpers in `<scratch>/r16/` (`app.sh <shared|newest> <log> <check_app.py args>` (boxed 16 GiB; `CHECK_SCRATCH`
  chooses the folder), `gates-rust.sh` (root workspace, boxed 16 GiB, the part's target), `gates.sh` (gates-python
  --no-build + tables --check), `golden_try.py` (the new case twice), `dup/mk.py` (the two-participant tree of SDC-36),
  `NOTES.before-relay16.md`). The `.so` was rebuilt (Rust changed: the CLI's family): current = the committed source.
  - **Port:**
    - **`python/larmorx/sdcflows/app.py`** (SPDX Apache-2.0 AND MIT): sdcflows 2.17.1 `cli/parser.py` (`build_parser`:
      every option, help, default and converter on Python's own argparse, `prog` `sdcflows`; `parse_args`: from_dict,
      the layout with the app's ignore patterns, the plugin and filter files, output = bids / work in bids errors, the
      folders, the participants), `cli/main.py` + `cli/workflow.py` (`run_app(argv, *, upstream_defects, env)` -> exit
      status: the settings file `.sdcflows.<8>.toml`, the analysis level's ValueError, `--dry-run`'s listing, "Running
      SDCFlows" (the cli logger, sdcflows' format and level names, standard output), "Workflow did not generate any
      jobs..." on standard error (Python's last resort: sdcflows' parent has no handler) + 64, the threads' warning,
      "Finished all workload"; uncaught exceptions as the traceback's first + last lines with the original's module
      (`bids.exceptions.BIDSValidationError`); `--pdb` = niworkflows' hook), `config.py` (`AppSettings` with
      `EnvironmentSettings`/`ExecutionSettings`/`WorkflowSettings`/`NipypeSettings` in sdcflows' key order, `.get()`,
      `.dumps()`; one object per run), `workflows/fit/base.py` (`sdcflows_workflow(...)` -> `SdcflowsWorkflow(subjects,
      estimators, workflows, ...)`, `.run()`: each estimator's workflow + `fmap_derivatives_<id>`, out_path_base
      'sdcflows', all sources incl. T1w as source_files); toml 0.10.2's `encoder.py` (`toml_dumps`, MIT; tag
      `<workspace>/tags/toml-0.10.2`, sdist sha256 b3bda1d108d5dd99f4a20d24d9c348e91c4db7ab1b749200bded2f839ccbe68f,
      encoder.py = the environments'). larmorx's knobs as environment variables: `LARMORX_UPSTREAM_DEFECTS`,
      `LARMORX_IMPLEMENTATION` (prelude/topup/volreg), `LARMORX_SDCFLOWS_LIBRARIES` (newest|shared), `FSLDIR` (D9).
      Python warnings hidden as sdcflows' `_warnings` hides them, except larmorx's UpstreamDefectWarning notices.
    - **`lx.niworkflows.collect_participants`, `BIDSError`, `BIDSWarning`** (`python/larmorx/niworkflows/
      participants.py`, niworkflows 1.15.4 `utils/bids.py`).
    - **The command line:** `larmorx.cli.run` sends `larmorx sdcflows sdcflows ...` to `run_app` before Rust parses
      (also `python -m larmorx`); Rust `crates/larmorx-cli/src/lib.rs`: family `("sdcflows", PYTHON_TOOLS)` in
      `FAMILIES` (the usage lists it), `run_tool` writes "error: 'sdcflows sdcflows' (sdcflows' BIDS app) is a Python
      program: install the larmorx Python package ..." and returns `EXIT_NOT_AVAILABLE` = 127; unit test
      `python_programs_say_they_need_the_python_package`.
  - **Defects (decided here; in code and docs, NOT yet in `larmorx_core::defects` nor the register):** **SDC-29** (rule
    B; rule A would need an anatomical preprocessing in the app) fieldmap-less estimators built without inputs fail at
    run time -> a clear error before any work (exit 1, `FieldmaplessError`), reproduce = run in order, fail on the first;
    **SDC-30** (A) `--session-label` unused -> the sessions given; **SDC-31** (A) `--bids-filter-file` read, unused -> its
    `fmap` entry as find_estimators' bids_filters (fMRIPrep's convention for the same file); **SDC-32** (A) `--sloppy`,
    `--debug` unused -> passed to the workflows; **SDC-33** (B) `--mem-gb` units `_to_gb` cannot look up (KeyError), a
    missing or non-JSON `--bids-filter-file`, a missing `--use-plugin` file -> argparse's error (exit 2); **SDC-34**
    (nondeterminism, never reproduced) the `functools.partial` repr (memory address) in argparse's message, the set
    order of missing labels and of the settings' analysis_level -> `_min_one`, sorted; **SDC-35** (A) `--use-plugin`
    ALWAYS fails (`yaml.load` without the Loader PyYAML 6 requires: TypeError) -> accepted (must exist), no effect;
    **SDC-36** (A) participants sharing a B0FieldIdentifier (BIDS: unique only within a participant) stop the app
    (KeyError "'<id>' is already  in mapping", one registry for the run) -> a registry per participant seeded with the
    earlier participants' `auto_` entries (names = sdcflows' wherever sdcflows runs), a warning naming the participants,
    their work folders under `sdcflows_wf/sub_<label>/`.
    **Candidates for the defects relay (no ID yet):** fMRIPrep's `map_fieldmap_estimation` has SDC-36's defect too
    (one registry for a multi-participant run: a PIPE- ID in larmorprepx's `estimators.py`); sdcflows builds pybids'
    database before its checks, so `output_dir == bids_dir` writes `.bids_db` into the dataset before refusing (larmorx
    writes no database: nothing to switch); sdcflows' config fails at import without `HOME` (Windows; a crash, never
    reproduced); `init_sdcflows_wf`'s reports named `desc-EstimatorType.PEPOLAR_fieldmap.svg` (for the reports part, D7).
  - **Checks (`validation/investigations/sdcflows-fieldmap-b/check_app.py`, sdcflows' own app in its own process per run
    via `relay16/app_original.py`; newest env; ALL OK; logs oracles/logs/fmapb-r16-app-{parser,settings,dryrun,
    dryrun-exits,exits,run,run2}.log):** parser 58/58 command lines `reproduce` = sdcflows
    (exit status, stdout, stderr, parsed values), default 53/53 + SDC-33's 5 `--mem-gb` units corrected; toml 7/7 texts
    (edge values + two real settings) = toml 0.10.2's `dumps`; settings 16/16 runs `reproduce` (errors, messages,
    folders, 11 settings files' [execution]/[workflow]/[nipype] + [environment] keys; the run uuid and the database path
    apart, the `.bids_db` folders sdcflows makes apart), default 2/2 + SDC-33 x3 + SDC-35 x1 corrected; dryrun 57/57
    listings `reproduce` = sdcflows 2.17.1 (the 8 D8(d) datasets, ds000228's view, dsA/dsB/dsC, relay 11's generated
    trees; with and without --no-fmapless), default 52/52 = sdcflows with 2.15.2's search (`--wrangler 2.15.2`; SDC-17's
    4 notices on stderr stripped), tree-sdc21 x2 = SDC-21 + SDC-36 corrected (2.15.2 fails with ValueError, larmorx
    lists), 3/3 with sessions/filter file passed to sdcflows' own search (`--pass-filters`: SDC-30/31); exits 7/7 (no
    estimator x3: 64 + the stderr line; group: 1 + the ValueError; fieldmap-less: reproduce both 1 (sdcflows: nipype's
    crash; larmorx: the ValueError naming the 5 inputs), default SDC-29 before any work, no file; missing readout: both
    1, the same ValueError); run (one thread; sdcflows `--pdb` = nipype Linear, its N4/prelude/topup/3dvolreg through the
    oracle cache via `--shims`; larmorx replicas, reproduce): ds005752-direct 5/5 and ds001600 (3 phdiff + PEPOLAR, 2.17
    wiring) 20/20 derivative files identical (uncompressed, gzip, sidecars), settings files identical, the cli lines
    identical (ds001600's first run differed only by the run's own folder in "Output folder" before the check rebased
    it; re-run on ds005752 after the fix: identical); sdcflows 26 s / 242 s, larmorx 4 s / 208 s. Not run: the shared
    environment (the parser and listing do not depend on numpy/SciPy; the whole runs' libraries are the records
    relay's).
  - Golden: `cli.sdcflows.sdcflows` (13 arrays: the dry run's listing and code through `larmorx.cli.run`, a direct field
    map run end to end through `run_app` (5 derivative files), `sdcflows_workflow` built; covers `cli:sdcflows sdcflows`,
    `lx.sdcflows.run_app`, `sdcflows_workflow`, `SdcflowsWorkflow.run`, `lx.niworkflows.collect_participants`),
    repeatable (blank and `~`), 0.5 s, NOT recorded (R10): 34 unrecorded cases fail tests/golden until then (33 + 1).
  - Unit tests `tests/python/test_sdcflows_app.py` (9).
  - Docs: `docs/api/sdcflows-fieldmap.md` ("Command line: sdcflows' BIDS app, `larmorx sdcflows sdcflows` (part B)":
    the option table, exit statuses, the Python API, what is not reproduced; SDC-29..36 rows), `docs/findings/
    sdcflows-estimation.md` ("sdcflows' BIDS app: `larmorx sdcflows sdcflows` (relay 16)"), PROVENANCE
    (`crates/larmorx-sdcflows/PROVENANCE.md` relay 16 entry, toml's copyright lines), investigations README rows
    (check_app.py, relay16/app_original.py).
  - Gates: `gates-rust.sh <worktree> root` (fmt, clippy -D warnings, tests) OK (oracles/logs/fmapb-r16-gates-rust.log);
    `gates-python.sh --no-build` (oracles/logs/fmapb-r16-gates-python.log): ruff check + format (998 files), check_spdx,
    collection without larmorx_validation OK; pytest 1,163 passed, 34 failed = the 34 unrecorded golden cases (33 + 1);
    CI's environment 775 passed, the same 34; `tables --check` OK. crates-gpl/crates-nc do not build larmorx-cli (no dependency change; no check needed). Not run: the smoke
    tier (R10).
  - For the orchestrator / handoff: the register and `larmorx_core::defects`: SDC-29 to SDC-36 (above);
    `validation/tools.toml`: `lx.sdcflows.run_app`, `sdcflows_workflow` (+ `SdcflowsWorkflow.run`) and the command
    `larmorx sdcflows sdcflows` in sdcflows-estimation, `lx.niworkflows.collect_participants` in niworkflows-helpers;
    `docs/validation/golden.md`: the new case; NOTICE: toml 0.10.2 (MIT, its seven copyright lines) for
    `python/larmorx/sdcflows/app.py`'s `toml_dumps`; `upstream.tsv`: `toml-0.10.2  https://files.pythonhosted.org/
    packages/be/ba/1f744cdc819428fc6b5084ec34d9b30660f6f9daaf70eead706e3203ec3c/toml-0.10.2.tar.gz  sha256:b3bda1d1
    08d5dd99f4a20d24d9c348e91c4db7ab1b749200bded2f839ccbe68f  MIT  tags`; README's tool table (generated): the
    command line's new family `sdcflows`. Follow-ups for next-steps: none new (the app's whole runs in the shared
    environment and the remaining D8(d) datasets with the records relay, R10).
- **Relay 17 = R9a (done, 2026-11-09): commits 60e498d5, 476968d5, 419c6e5f, 250ecb08, d5d0b845, 953206b6** (on 9017ed3f).
  Agent id: the orchestrator's to add (relay 17). Scratch helpers in `<scratch>/r17/` (`cargo-nc.sh` (crates-nc with its
  own target `<scratch>/target-nc`, seeded from main's), `gates.sh` (root + nc gates, a crates-gpl check with
  `<scratch>/target-gpl`, Python gates + tables), `wsrep.py` (whitespace-tolerant edits), `acqp/` (FSL's own topup run with
  test_acqp), `dl/` + `ex/` (FSL's fsl-topup conda package and its extracted files), `watershed_masks.json`,
  `NOTES.before-relay17.md`). The `.so` was rebuilt (Rust changed: defects.rs, masker.rs): current = the committed source.
  **The split (FOR THE ORCHESTRATOR to agree):** R9 = relay 17 (R9a: D9's settings, the IDs, the switches, the
  measurements, the register's text) + **R9c (the parity suites, D12, and the part's audit of `.pyi`/API option
  tables/PROVENANCE/SPDX)**, proposed after R9b so the suites cover its code: relay 18 = R9b (SciPy's 4 x 4 `inv`,
  `RegridToZooms`, `gaussian_filter`), relay 19 = R9c, relay 20 = R10 (records), 21 = R11 (site), 22 = R12 (handoff).
  - **D9's replica and spec (the orchestrator's (a) to (d)):**
    (a) FSL 6.0.7.23's six topup settings files (`b02b0.cnf` = `b02b0_2.cnf` bytes, `_1`, `_4`, `b02b0_7T`, `test_acqp`;
    6,166 bytes) byte for byte in `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/etc/flirtsch/`, checked against
    FSL's conda package `fsl-topup-2203.5-ha905fa9_6.conda` (downloaded, SHA-256 24381a8a... = the installation's
    conda-meta record; its six files extracted = the installation's = the record's per-file SHA-256s; sdcflows 2.17.1's
    sdist's `_1/_2/_4` the same bytes): MANIFEST rows (package fsl-topup 2203.5), README (title, table, sizes, origin,
    lookup, updating), the data folder's NOTICE, `crates-nc/NOTICE` ("Data included"), `fsldata::BUNDLED_SETTINGS`;
    the check is `larmorx-nc-cli/tests/fsl_data.rs` (not notices.rs: the README said so wrongly, corrected): the manifest
    per package, the copy's files, the NOTICE's package checksum, and a new program test
    `topup_finds_its_settings_in_the_copy` (an installation's layout without FSLDIR: FSL-121's message without the copy;
    with it `--config=test_acqp` read from the copy, exit 0, the log's levels, `field_test` written). **The replica's
    `--config` search now looks in the copy after FSL's own places** (`existing_conf_file_in`, unit test of the order;
    FSL's messages unchanged when neither has it): the decision's "so `--config=b02b0` works without FSL"; PROVENANCE
    (topup, core), `docs/api/fsl-topup.md` ("FSL's settings files without FSL").
    (b) `specs/topup.md` §2.5 (spec writer): what each file sets, option by option and level by level (two tables: the
    nine-level four with `--subsamp`/`--lambda` per file; the 7 T file's twelve levels; `test_acqp`'s two levels and
    its `--fout=field_test --iout=iout_test`, **observed** with FSL's own topup: `field_test`, `iout_test`,
    `iout_test_mean`), each option's meaning in the spec's words, the suffix rule, and sdcflows' choice (2.17.1:
    divisible by 4 / even / else from the merged LAS series' sizes, `_quick` with sloppy; 2.15: its own `b02b0.cnf`);
    §12 gains the check of the main package's own settings against larmorx-nc's files (bit for bit).
    (c) `lx.sdcflows.topup_config_file`: order unchanged (`fsldir`, larmorx-nc's copy, the clear error); its message,
    docstrings, module docstring and API row name larmorx-nc's copy and the clean-room settings to come with t2smap's
    clean-room part; unit test `test_topup_configurations_fsldir_then_larmorx_ncs_copy`.
    (d) the handoff's lines: `<scratch>/handoff/relay17.md` §2 (licensing.md's bullet; no root NOTICE change).
    `lx.fsl.topup(config="b02b0")` without `fsldir` still raises (its wrapper is also the clean-room original's: not
    edited here; a follow-up).
  - **The watershed measurement (the orchestrator's 07:27):** `measure_watershed_masks.py` (committed; log
    `oracles/logs/fmapb-r17-watershed-masks.log`): both `epi_mask` masks against fMRIPrep's own BOLD brain mask and a head
    mask on the four references: corrected better on 3 (Dice ds000228 0.460 -> 0.906, ds003592 0.464 -> 0.904,
    ds005040 0.469 -> 0.474), the same on ds004958 (143 voxels both), worse on none; outside the head 79,304 -> 1,662,
    0 -> 179, 0 -> 0. **Rule A stands** (findings, register text).
  - **IDs (all part B's, in code, docs and `larmorx_core::defects`):** SCI-W1..W4 -> **SCI-13..16**; SDC-h -> **SDC-37**,
    no brain seed -> **SDC-38** (rule B, switch); **SDC-39** (brain_masker's non-flat closing: rule D measured with
    sdcflows' own masker, `relay17/probe_sdc39.py`: masks unchanged on 14 of 14 inputs, probabilities <= 1.7e-06 ->
    rule C, kept); relay 16's candidates: **PIPE-32** (fMRIPrep's one registry for every participant: VERIFIED with
    fMRIPrep 25.2.7's own map_fieldmap_estimation, `relay17/probe_shared_identifier.py`: KeyError at the second
    participant; rule A: larmorprepx's `map_fieldmap_estimation` searches each participant in its own registry seeded
    with the earlier `auto_` names and merges into the shared one where names are free, warning on a shared one;
    `reproduce` the KeyError), **SDC-40** (pybids' database written before the checks: e, no switch), **SDC-41** (config
    import without HOME: a, never reproduced), **SDC-42** (reports' figures named after the enum: c, switch with the
    reports, D7); the lettered: **SDC-43** (get_trt's `'0'` message: f, kept), **SDC-44** (`Logger.warn`: f, `warning`),
    **PIPE-33** (IntensityClip of a constant image: rule B, ValueError; `reproduce` x86-64's NaN 0xFFC00000 and casts 0
    on every platform; relay 1's "-32768" was wrong: x86-64 gives 0), **PIPE-34** (RobustAverage of a zero-mean volume:
    rule B), **SKI-03** (random_walker's renumbered labels: rule A), **SKI-04** (full_prob's boolean concatenation: rule
    A); relay 2's SciPy candidates: **SCI-06** (origin crash, a), **SCI-07** (reflect OOB, a), **SCI-08** (1-D rank OOB,
    a), **SCI-09** (1-D rank's footprint holes: rule A, NEW switch on rank/median/percentile_filter), **SCI-10** (the
    propagation's int8 mask: rule A, NEW switch on binary_erosion/dilation/opening/closing/propagation), **SCI-11**
    (endless iterations, a), **SCI-12** (non-flat structure in the element type: C), **SCI-17** (SciPy 1.15's short 1-D
    rank: fixed in 1.18). NP-05..08, NIB-05..08 unused; SCI-18..20 and SDC-45 free. intensity_clip's six callers pass
    `upstream_defects` (syn.py x2, workflows.py x2, refmap.py x2).
  - **`larmorx_core::defects`:** 46 new entries (PIPE-18..21, 23..34; SCI-09, 10, 13..16; SDC-12, 14, 16, 17, 20, 21,
    22, 24, 26..33, 35..38; SKI-01..04): every part B ID with a switch or a notice. `tests/python/test_fmap_defects.py`
    (7 tests): every ID a Python notice of the package names is registered (it failed on 20 IDs before), PIPE-32,
    PIPE-33, PIPE-34, SKI-03/04, SCI-09, SCI-10 both modes.
  - **The register's text:** `<scratch>/handoff/register_part_b.py` -> `register-part-b.md` (66 summary rows, 66 sections,
    the new SKI family's header, the 46 rows of docs/api/upstream-defects.md; insertion points named); the other
    handoff items in `<scratch>/handoff/relay17.md` (licensing.md's bullet, the audit note (relay 17 opened FSL's six
    .cnf files and the topup replica's options.rs; edited no clean-room file), tools.toml mentions, follow-ups:
    lx.fsl.topup's name lookup, random_walker(copy=False)'s in-place -1, a PIPE-32 upstream report).
  - Docs: findings `sdcflows-estimation.md` ("The register's entries of part B (relay 17)": every family, the watershed
    table, SDC-39's numbers; relay 16's candidates named), `scipy-ndimage-filters.md` (IDs and rules of candidates 1-11,
    SCI-17), API pages (sdcflows-fieldmap: SDC-37..44 rows; niworkflows-helpers: PIPE-33/34, intensity_clip's signature;
    fmriprep-fieldmap-steps: PIPE-32; skimage: SKI-03/04; ndimage-filters: SCI-06..17 named, the option table's
    switches), PROVENANCE (sdcflows: SDC-37/38), investigations README (3 rows).
  - **Checks after the new defaults** (both environments, boxed): `check_ndimage.py` ALL OK (SciPy's results with
    `reproduce`; SCI-09 corrected = SciPy's n-dimensional filter along a second axis 125/125; SCI-10 corrected = SciPy
    with the mask's truth 471 + 471), `check_skimage.py --sections rw` ALL OK (SKI-01 x2, SKI-03 = the labels mapped
    back, SKI-04 = the seeds' probabilities; a `reproduce` variant of the non-consecutive case), logs
    `oracles/logs/fmapb-r17-{ndimage,skimage-rw}-{shared,newest}.log`; `check_estimators.py --sections
    runs,apps,fmriprep` (newest) after PIPE-32: runs 92, apps 8, fmriprep 2,567 identical, ALL OK
    (`fmapb-r17-estimators.log`); `measure_watershed_masks.py`, `relay17/probe_sdc39.py`, `probe_shared_identifier.py`
    as above. Not re-run: check_fmap_stage/check_derivatives (default-mode callers of map_fieldmap_estimation with one
    participant per call: the runs section covers that path), the smoke tier (R10).
  - **Gates** (`<scratch>/r17/gates.sh`, boxed 16 GiB, log `oracles/logs/fmapb-r17-gates.log`, at 419c6e5f..d5d0b845's
    source; 953206b6 changed only two check scripts, ruff-clean): `gates-rust.sh root` OK, `gates-rust.sh nc` OK (fmt,
    clippy -D warnings, tests incl. the new fsl_data and options tests), crates-gpl `cargo check --workspace
    --all-targets --locked` OK (no dependency change: no Cargo.lock refresh); `gates-python.sh --no-build`: ruff,
    format, check_spdx OK; pytest 1,171 passed, 34 failed = the 34 unrecorded golden cases (relay 16's 1,163 + 8 new);
    CI's environment 783 passed, the same 34; `tables --check` OK.
- **Relay 18 = R9b (done, 2026-11-09): commits f663f5ca, e9a740e2, 3d869ab9, 5d57d9b6, 11fa35cb** (on 953206b6).
  Agent id: the orchestrator's to add (relay 18). Scratch helpers in `<scratch>/r18/` (`syn_checks.sh`, `syn_checks2.sh`
  (the SyN checks boxed), `gates.sh`, `gates_root.sh`, `golden_try.py` (the two new golden cases twice: repeatable),
  `images.txt` (the 513 oracle images whose affines check_scipy_inv.py reads), `NOTES.before-relay18.md`). The `.so` was
  rebuilt (Rust changed) at the last commit (11fa35cb): current = the committed source.
  - **SciPy's 4 x 4 `inv`** (`crates/larmorx-transform/src/scipy_linalg.rs`, `inv4(m, ScipyRelease::{V1_15, V1_18})`;
    binding `_core.linalg_scipy_inv4`; Python `larmorx.transforms._linalg.scipy_inv(a, *, scipy_version="1.18.1",
    upstream_defects)` + `LinAlgWarning`): 1.15 = OpenBLAS's `getf2` (numpy's, `openblas::getf2` now `pub(crate)`) +
    reference `dgetri` unblocked (`trti2_U` with `trmv_NUN`'s daxpy tail and dscal; `dgemv_n` 4x2 Haswell kernel (two
    products summed, one FMA by alpha) and SSE2 4x1 (x*alpha first); column swaps); 1.18 = structure detection on the
    column-major copy: diagonal (`1/a`, the warning exact), upper/lower triangular (`trti2_U`/`trti2_L`, other triangle
    +0), general (getrf/getri; gecon only for the warning), symmetric NOT PORTED (NotImplementedError); 1.18's warning for
    non-diagonal matrices needs gecon/trcon (not ported): the bound `rcond >= 1/(‖A‖ ‖A⁻¹‖₁)` decides "no warning" at
    >= 1000 eps, else NotImplementedError. SciPy's checks and messages (non-finite, non-square, 1-D, singular per
    version). SciPy wheels' OpenBLAS: 1.15.3 = 0.3.28, 1.18.1 = 0.3.31.dev (Haswell kernels = 0.3.30's: validated).
    `resample_img_nearest`'s second inverse now `scipy_inv` (nilearn's `linalg.inv(affine)`); its first stays numpy's.
    **check_scipy_inv.py ALL OK both envs**: 13 families x 20,000 + 513 image affines identical (1.18 refusals: scaled
    2^+-300 20,000 (values identical), +-1/+-2 79 (70 values identical, 9 symmetric), integers 47 (values identical)).
    Logs `<scratch>/r18/scipy-inv-{shared,newest}.log`. After it: check_syn_preprocessing nodes+library ALL OK both envs
    (`oracles/logs/fmapb-r18-syn-preprocessing-{shared,newest}.log`).
  - **SCI-18** (rule A; in `larmorx_core::defects`): 1.18 returns the LU factors when the norm overflows (gecon info
    -5 skips getri); fix = getri's inverse + notice; reproduce = the LU. Candidates (lettered, for whoever ports the
    condition estimates): 1.18's norm1_ is the infinity norm (combined with gecon's 1-norm estimate); the diagonal
    warning prints the condition number as "rcond" (rule C proposed).
  - **SciPy's Gaussian filter** (`larmorx_ndimage::correlate1d` = `NI_Correlate1D`; bindings `_core.ndimage_correlate1d`,
    `_core.numpy_exp_f64`; Python `lx.ndimage.correlate1d`, `gaussian_filter1d`, `gaussian_filter` (order 0; derivatives
    and complex: NotImplementedError; `numpy_version` keyword for the kernel's sum)): the kernel's exp = numpy's on its
    path without AVX-512 (glibc's: `larmorx_core::numpy::umath::exp_f64`); check_regrid.py re-runs itself with
    `NPY_DISABLE_CPU_FEATURES` (AVX-512 off) so SciPy's numpy takes glibc's exp too. **filters ALL OK both envs**:
    correlate1d 6,912 + output types 72 + errors 5, gaussian_filter 160, gaussian_filter1d 24, axes/radius 4, errors 1.
  - **niworkflows' RegridToZooms** (`python/larmorx/niworkflows/regrid.py`: `regrid_to_zooms(in_file, zooms, *, order,
    clip, smooth, out_dir, numpy_version, n_threads)` -> `RegridToZoomsResult(out_file)`, `resample_by_spacing(...)` ->
    Image; exported from `lx.niworkflows`; `larmorx.transforms._linalg.affine_points` + `_core.linalg_affine_points` =
    `openblas::affine_points` (numpy's dgemm of homogeneous points: point_row; dgemv for one)): **numpy's float32 inverse
    is its float64 inverse cast** (`_commonType`): a first port with float32 reciprocals differed on 4 real res_target
    nodes (88.00001 vs 88.0). **check_regrid.py regrid+runs ALL OK both envs**: 576 synthetic files (data + 348 header
    bytes), edge voxel sizes 4 (zero/negative fixed by the loader as nibabel's; subnormal/NaN: HeaderDataError "Could not
    decompose affine:" first line; the printed matrix not niworkflows'), **fMRIPrep's own nodes: res_tmpl 23/23,
    res_target 23/23, zooms_epi 5/5** (logs `<scratch>/r18/regrid-{shared,newest}.log`, `filters-newest.log`).
  - **PIPE-35** (proposed rule D, both modes niworkflows'; no defects.rs entry): smooth=True's sigma `max(0, (old/new -
    1)/2)` is positive only for a finer grid: fMRIPrep's brain extraction's 1 mm -> 4 mm regrids are never smoothed;
    measure with the anatomical brain extraction's port. `relay18/probe_regrid_edges.py`.
  - **syn.py:** `syn_sdc_workflow` computes `zooms_epi` (`files["zooms_epi"]`; SDC-15 stays rule C, now computed);
    `syn_preprocessing_workflow(debug=True)` works: `regrid_anat` (RegridToZooms 2 mm smooth=True, `files["regrid_anat"]`)
    feeds sampling_ref's moving image, epi2anat gets `--write-interval-volumes 5` (sdcflows' args; first run of the
    debug check failed only on the missing interval volumes). check_syn_sdc.py nodes compare zooms_epi's file: **5/5
    both envs** (`oracles/logs/fmapb-r18-syn-sdc-nodes-{shared,newest}.log`, ALL OK). **check_syn_sdc.py workflow (shared, ds004884/wf_auto_00001): 19/19 files of sdcflows' graph, zooms_epi's included, ALL OK** (sdcflows 956 s one thread, larmorx 315 s on 8; `fmapb-r18-syn-sdc-workflow-shared.log`). **check_syn_preprocessing.py --sections workflow --debug (newest, ds004884's two estimators): 112/112 files of sdcflows' graph with debug=True (regrid_anat, the interval volumes, the warped images), ALL OK** (`fmapb-r18-syn-preprocessing-debug2-newest.log`; the first run, without `--write-interval-volumes 5`, is `-debug-newest.log`).
  - Golden: `py.ndimage.gaussian`, `py.niworkflows.regrid` registered (0.01 s each, repeatable), NOT recorded: 36
    unrecorded golden cases fail tests/golden until the records relay (34 + 2).
  - Unit tests: `tests/python/test_regrid_scipy_inv.py` (5 tests: structures per version, errors/messages, the warning
    and SCI-18, the Gaussian filter, RegridToZooms); `test_sdcflows_syn.py::test_errors` lost its obsolete debug check.
  - Docs: `docs/api/ndimage-filters.md` (rows correlate1d, gaussian_*; status), `docs/api/niworkflows-helpers.md`
    (regrid_to_zooms row; resample_img_nearest's inverses; PIPE-35 row), `docs/api/sdcflows-fieldmap.md` (debug row,
    zooms_epi paragraph, SDC-15 row), `docs/findings/sdcflows-estimation.md` ("SciPy's `linalg.inv`, niworkflows'
    `RegridToZooms` and SciPy's Gaussian filter (relay 18)"), PROVENANCE (transform: scipy_linalg + affine_points;
    ndimage: correlate1d + the Python wrappers; sdcflows: regrid.py, syn.py's nodes), investigations README (3 rows),
    `_core.pyi` (4 stubs).
  - Gates: `gates-rust.sh root` OK after `cargo fmt` and two clippy fixes (`oracles/logs/fmapb-r18-gates-root.log`: fmt, clippy -D warnings, tests); crates-nc and crates-gpl `cargo check --workspace --all-targets --locked` OK (no dependency change: no Cargo.lock refresh); `gates-python.sh --no-build` (`fmapb-r18-gates.log`): ruff, format, check_spdx, collection OK; pytest 37 failed = the 36 unrecorded golden cases + `test_sdcflows_syn.py::test_errors` (its obsolete debug check, removed after: `tests/python validation/tests` 913 passed); `tables --check` OK. Not run: the smoke tier (R10).
  - Handoff items: `<scratch>/handoff/relay18.md` (register: SCI-18, PIPE-35, candidates, free IDs; tools.toml; golden.md;
    NOTICE (none new); next-steps follow-ups: 1.18's symmetric inverse and condition estimates, Gaussian derivatives,
    PIPE-35's measurement, pinning SciPy's OpenBLAS 0.3.28/0.3.31.dev).
- **Relay 19 = R9b2 (done, 2026-11-09): commits c5198f81, 0de578fb** (on 11fa35cb). Agent id: the orchestrator's to add
  (relay 19). Scratch helpers in `<scratch>/r19/` (`probe_inv.py` (the first 8,500-case probe), `inv_both.sh` (the check
  in both environments, boxed), `expected_msgs.py` (SciPy's warning texts for the unit tests), `golden_mats.py`,
  `golden_try.py`, `gates.sh`, `syms.txt` + `dis_*.txt` (the disassembly of SciPy 1.18.1's `libscipy_openblas` kernels and
  LAPACK routines: FMA counts), `register_part_b.before-relay19.py`, `NOTES.before-relay19.md`). The `.so` was rebuilt at
  c5198f81's source (= the committed source).
  **The split (FOR THE ORCHESTRATOR to agree):** (a) filled this relay; **(b) moves to relay 20 = R9b3** and the
  numbering shifts by one (21 = R9c, 22 = R10 records, 23 = R11 site, 24 = R12 handoff). **(c) is closed without pins**:
  every kernel and routine (a) reached is identical bit for bit to SciPy 1.18.1's 0.3.31.dev library (below), the reason
  is in the findings.
  - **(a) SciPy 1.18.1's `inv` complete** (`crates/larmorx-transform/src/scipy_lapack.rs`, new, private module;
    `scipy_linalg.rs` now calls it; `openblas::iamax` and `scipy_linalg::trti2_upper` `pub(crate)`): the condition
    estimates `dgecon` (on getrf's factors; `kase` 1: dlatrs L-unit then U, 2: U^T then L^T-unit), `dtrcon` (on the
    INVERSE: SciPy calls trtri first; dlantr's 1-norm; `(1/anorm)/est`), `dpocon` (both dlatrs per step), `dsycon`
    (dsytrs, one rhs), all with `dlacn2` (5 states, ITMAX 5), `dlatrs` complete (column norms, tscal rescaling above
    2^970, dlange when a norm overflows, growth bounds, dtrsv or the careful solves, the uscal transposed path), `drscl`;
    the symmetric path: SciPy's `norm1_sym_herm('U')`, OpenBLAS's `potf2_U` (ddot, dgemv_t's strided tail, scal_k along
    the row), `dpocon`, LAPACK's `dpotri` (OpenBLAS's dtrtri + `lauu2_U`: scal_k, strided ddot, dgemv_n's tail); when a
    pivot is not positive the original matrix through `dsytf2` (= dsytrf here; alpha (1+sqrt17)/8, strided idamax,
    dsyr's small path, the 2x2 Fortran loop), `dsycon`, `dsytri` (dsymv_U below 16 rows, ddot, swaps); the lower
    triangle filled from the upper; zero pivots: LinAlgError. Kernels checked in the disassembly (`<scratch>/r19/dis_*`):
    FMA only in `dgemv_t_HASWELL` (its tails: fma(a0,x0,a1x1), fma(a2,x2,.), + y; one row fma(a,x,y); the strided-y
    loop the same). `Outcome::Inverse { inverse, rcond, ill_conditioned }` (no `Condition`, no `SymmetricNotPorted`);
    binding statuses "ok", "ill_conditioned", "singular", "norm_overflow", `rcond` always SciPy's value (0 for 1.15 and
    SCI-18); `scipy_inv`'s NotImplementedErrors for symmetric and "estimate needed" removed.
  - **check_scipy_inv.py ALL OK in both environments** (logs `oracles/logs/fmapb-r19-scipy-inv-{shared,newest}.log`): 27
    families x 20,000 (relay 18's 13 + condition 1e8..1e20, scaled 2^+-500, symmetric definite/indefinite each well and
    1e8..1e20, symmetric integers, zero diagonal, 2^+-1000, symmetric affines with a zero origin, triangles with a tiny
    diagonal, 2^+-500, overflowing column sums) + 513 image affines = **540,513 identical per environment**; with 1.18
    every non-diagonal rcond also = SciPy's own `scipy.linalg.lapack` dgecon/dtrcon/dpocon/dsycon on the same factors:
    **506,565/506,565**; 1.18 warned 141,272 times (the printed rcond in full). The singles (singular symmetric,
    ill-conditioned general) identical.
  - **Defects (decided by rules A-D):** relay 18's candidates: **SCI-19** (the general path's `norm1_` = the infinity
    norm passed to gecon as the 1-norm): class f, kept in both modes (a message; measured `probe_sci19_norm.py`: near the
    threshold SciPy warns 10,249 / 20,000, the 1-norm 10,234; ratio <= 3.83; log `fmapb-r19-sci19-norm.log`); **SCI-20**
    (the diagonal path prints the condition number as rcond): class f, kept. No new `larmorx_core::defects` entries (no
    switch). Nothing new on the symmetric path (pocon has no HUGEVAL check: an infinite norm gives rcond 0 and the
    warning, as SciPy). **The part's SciPy range SCI-06..20 is used up.** **PIPE-35's register row** (rule D agreed, the
    measurement named, gap "measure (anatomical brain extraction)"): in `<scratch>/handoff/register_part_b.py` with
    SCI-18..20 (relay 18's SCI-18 was not in the generator yet); `register-part-b.md` regenerated (70 entries; 47
    api rows). Handoff items: `<scratch>/handoff/relay19.md`.
  - Unit tests: `tests/python/test_regrid_scipy_inv.py` 12 (the symmetric paths with SciPy's bits, the five warning texts
    of every path = SciPy 1.18.1's, 1.15 never warns, a singular symmetric matrix); Rust `scipy_lapack::tests` (2),
    `scipy_linalg::tests` updated. Golden: `py.niworkflows.regrid` (registered, unrecorded) extended: a symmetric affine
    resampled with resample_img_nearest, the inverse and rcond on 7 paths (values = SciPy 1.18.1's), repeatable.
  - Docs: findings `sdcflows-estimation.md` (new section "SciPy 1.18.1's `inv` complete ... (relay 19)"; relay 18's "not
    ported" sentences marked as ported by relay 19), API `niworkflows-helpers.md` (resample_img_nearest's row),
    PROVENANCE (transform: relay 19's files), investigations README (check_scipy_inv row, probe_sci19_norm row), `_core.pyi`
    (linalg_scipy_inv4's statuses), `_linalg.py` docstring.
  - Gates (`<scratch>/r19/gates.sh`, boxed 16 GiB, log `oracles/logs/fmapb-r19-gates.log`, at c5198f81's source; 0de578fb
    changed only the investigations README): `gates-rust.sh root` OK (fmt, clippy -D warnings, tests), crates-nc and
    crates-gpl `cargo check --workspace --all-targets --locked` OK (no dependency change: no Cargo.lock refresh);
    `gates-python.sh --no-build`: ruff, format, check_spdx, collection OK; pytest 1,183 passed, 36 failed = the 36
    unrecorded golden cases (unchanged: `py.niworkflows.regrid` was already one); CI's environment 795 passed, the
    same 36; `tables --check` OK. Not run: the smoke tier (the records relay).
- **Relay 20 = R9b3 (done, 2026-11-09): commit 984942f6** (on 0de578fb). Agent id: the orchestrator's to add
  (relay 20). Scratch helpers in `<scratch>/r20/` (`probe1.py`, `probe2.py` (SciPy's complex and argument behaviours;
  relay 18's correlate1d against SciPy's errors), `probe_matvec.py` and `ref_values.py` (both committed under
  `validation/investigations/sdcflows-fieldmap-b/relay20/`), `golden_vs_scipy.py` (the seven new golden entries =
  SciPy's in both environments), `gates.sh`, `gates_root.sh`, `filters-*.log`, `NOTES.before-relay20.md`). The `.so`
  was rebuilt at 984942f6's source (= the committed source).
  - **numpy's matrix-vector product** (`larmorx_core::numpy::dot::matvec_f64`, binding `_core.numpy_matvec_f64`):
    `cblas_matrixproduct`'s shapes (one row: `dot_f64`; one column: `x0·a + 0`; else `cblas_dgemv` row-major) over
    OpenBLAS's Haswell `dgemv_t` (`dgemv_t_4.c` + `dgemv_t_microk_haswell-4.c`, same in 0.3.30/0.3.34): blocks of 2048
    rows, 4x4 assembly kernel (four FMA lanes from +0, `(l0+l2)+(l1+l3)`), SSE2 4x2 (even/odd lanes) and 4x1 (rows mod
    4, `(l0+l2)+(l1+l3)`) without FMA, `add_y`, the C tails' contractions (`numpy_linalg`'s). One thread (OpenBLAS:
    below 460,800 elements; above, a column's kernel depends on the thread split: documented, no register entry).
    **2,800 shapes in check_regrid + 5,898 in the probe identical, numpy 2.3.5 and 2.5.4.**
  - **SciPy's Gaussian derivatives** (`_gaussian_kernel1d`, `python/larmorx/_ndimage_filters.py`): SciPy's code line for
    line with numpy's elementwise ops, `_core.numpy_exp_f64`, `numpy_add_reduce` and the two dots through
    `numpy_matvec_f64` (radius 0: the `_row` path, a dot product); SciPy's errors arise the same way (order -1, a float
    order's `np.zeros(2.0)` TypeError, sigma 0's ZeroDivisionError). Not ported: an `np.longdouble` sigma (the kernel in
    long double): `NotImplementedError` kept.
  - **Complex inputs and weights**: `_complex_via_real_components` and `correlate1d`'s complex path verbatim (weights
    conjugated, complex128; parts combined by numpy's `-=`/`+=` in the output's part type), `_get_output` (with
    `complex_output`: promote + `UserWarning`, `str`/array refusals) replacing `_out_dtype` in correlate1d and the
    Gaussian filters; `gaussian_filter` passes `_OutputOf(target)` (SciPy's array-branch rules, a new array per pass).
  - **correlate1d's argument handling = SciPy's** (relay 18's port differed, found by the new checks): `NI_InitLineBuffer`'s
    messages ("array type dtype('float16') not supported", input then output; longlong views as int64), `_get_output`'s
    non-numeric `str` ("bool" included), the C parser `"O&O&iO&idn"` order (read-only output "output array is
    read-only.", `cval` by `math.ldexp(cval, 0)` = CPython's `PyFloat_AsDouble`: `numpy.str_` refused although it has a
    Python `__float__`; `origin` by `operator.index`), SciPy's own `normalize_axis_index`.
  - **check_regrid.py --sections filters ALL OK in both environments, 1 and 8 threads: 15,995 cases** (relay 18's 7,178 +
    matvec 2,800, `_gaussian_kernel1d` 677 (orders 0-9, radii 0-3,000), derivatives 384 + 96 + 10 edges, complex
    correlate1d 4,752 + outputs 33, complex Gaussian 28 + refusals 9, arguments 28; warnings compared too). Logs
    `oracles/logs/fmapb-r20-filters-{shared,newest}.log`.
  - Defects: none (the part's ranges unchanged: SCI-21..30 free).
  - Unit tests: `test_regrid_scipy_inv.py` +2 (`test_gaussian_derivatives` with SciPy's hex values, `test_complex_correlate`;
    the obsolete order=1 NotImplementedError check removed): 14 passed; Rust `numpy::dot::tests::matvec_follows_dgemv_t_haswell`.
    Golden `py.ndimage.gaussian` (registered, unrecorded) +7 entries (orders 2, 3, 5, radius 0, complex x3), repeatable,
    = SciPy's in both environments.
  - Docs: API `ndimage-filters.md` (status paragraph, correlate1d and Gaussian rows, the complex difference line),
    findings `sdcflows-estimation.md` ("SciPy's Gaussian derivatives and complex filters (relay 20)"), PROVENANCE
    (larmorx-core: matvec_f64; larmorx-ndimage: the Python ports), investigations README (check_regrid row), `_core.pyi`
    (numpy_matvec_f64), `numpy/mod.rs` and `dot.rs` module docs.
  - Gates (`<scratch>/r20/gates.sh` boxed 16 GiB, log `oracles/logs/fmapb-r20-gates.log`; root again after a clippy fix:
    `fmapb-r20-gates-root.log`): `gates-rust.sh root` OK (fmt, clippy -D warnings, tests), crates-nc and crates-gpl
    `cargo check --workspace --all-targets --locked` OK (no dependency change); `gates-python.sh --no-build`: ruff,
    format, check_spdx, collection OK; pytest 1,185 passed, 36 failed = the 36 unrecorded golden cases (unchanged); CI's
    environment 797 passed, the same 36; `tables --check` OK. Not run: the smoke tier (the records relay).
  - Handoff items: `<scratch>/handoff/relay20.md` (no register entry; tools.toml; golden.md; no pin/NOTICE; next-steps:
    close the derivatives/complex follow-up; NEW follow-up: the other filters' `_out_dtype` messages and `cval`/`origin`
    parsing, see next).
- **Relay 21 = R9c(a) (done, 2026-11-09): commits 7cb8250d, aa24049c, cf5d389a** (on 984942f6). Agent id: the orchestrator's to add
  (relay 21). Scratch helpers in `<scratch>/r21/` (`probe_args.py`, `probe_args2.py` (SciPy's edge cases), `probe_rank_out.py`
  (SCI-21 found: SciPy writes the input type into an output of another type; the process SIGSEGVs at exit),
  `probe_binary.py` (the next relay's start), `section.py` + `args_section.py` (the code written), `ndimage_both.sh`,
  `ld_only.py` (check_regrid's long-double section alone), `golden_try.py`, `gates.sh`, `register_part_b.before-relay21.py`,
  `NOTES.before-relay21.md`). The `.so` was rebuilt at 7cb8250d's source (defects.rs) = the committed source.
  **The split (FOR THE ORCHESTRATOR to agree):** R9c was three pieces of work; this relay did the argument handling, the
  long-double sigma and `random_walker(copy=False)` (they change the code the suites run). **Relay 22 = R9c(b)**: the binary
  morphology's, `label`'s, `sum_labels`' (and `center_of_mass`'s, `watershed_ift`'s) argument handling as SciPy's
  (`relay21/probe_binary.py`: 63 of 144 cases differ) + the part's `.pyi`/API/PROVENANCE/SPDX audit; **relay 23 = R9c(c)**:
  the D12 parity suites (design notes below); then 24 = R10 records, 25 = R11 site, 26 = R12 handoff.
  - **The minimum, maximum and rank filters and grey morphology take their arguments as SciPy 1.18.1**
    (`python/larmorx/_ndimage_filters.py`: `_min_or_max_filter1d` + `_min_or_max_1d_c`, `_min_or_max_filter`, `_rank_filter`
    + `_rank_1d`; helpers `_intp` (numpy's `PyArray_PyIntAsIntp`: bools refused "an integer is required"), `_nd_type` (the
    n-D switch's "array type not supported"), `_native`, `_c_output` ("output array is read-only."), `_empty_result`):
    SciPy's order (complex, `normalize_axis_index`, size, relay 20's `_get_output`, origin, mode, then the C parsers
    `"O&niO&idni"`, `"O&O&O&O&idO&i"`, `"O&iO&O&idO&"`: `__index__` for sizes/axes/1-D origins, `PyFloat_AsDouble` for
    cval, `PyArray_IntpConverter` for n-D origins; `NI_InitLineBuffer`'s "array type dtype('float16') not supported"; no
    type error for an empty n-D input; the separable path with no axis to filter = numpy's assignment of any type; outputs
    sharing the input's memory). The 1-D running rank's own dispatch (`_rank_filter_1d`: float16 via float32, bool and
    integers via int64, else RuntimeError "Unsupported array type"; `cval` in its type; `"OiiOiOi"`; cval by
    `PyLong_AsLongLong`/`PyFloat_AsDouble`; read-only output "WRITEBACKIFCOPY base is read-only"). Relay 2's `_input`,
    `_out_dtype`, `_result` stay for binary morphology, `label`, `sum_labels` (relay 22).
  - **Defects found (SCI-21..24; rules A-D; register text in `<scratch>/handoff/register_part_b.py`, regenerated: 82
    sections, 50 API rows; SCI-21..23 in `larmorx_core::defects`):** **SCI-21** (class a, rule A, switch): SciPy hands
    the 1-D C++ the output array itself for float32/float64/int64 inputs and the C++ writes the input's type: another
    output type or byte order gets the bytes reinterpreted, a smaller one past its end (SIGSEGV at exit; into a strided
    float32 array a hang); fix = the values cast (+ notice), reproduce = SciPy's bytes where they are the output's own
    (never past its end; the notice gives the count). **SCI-22** (class c, rule A, switch): `>f4`/`>f8` refused by dtype
    equality (RuntimeError), numpy's `longlong` by the C++'s type number (TypeError; on Windows longlong is int64 and
    passes): fix = the native copy's result (+ notice), reproduce = SciPy's errors. **SCI-23** (class f, rule A, switch):
    a non-contiguous output of the input's type: numpy's WRITEBACKIFCOPY RuntimeWarning (text per numpy release: 2.3
    "array_dealloc", 2.5 "clearing of array"): none by default (no notice: the result is the same), numpy's with
    reproduce. **SCI-24** (class f, fixed in 1.18.1, no switch, as SCI-17): SciPy 1.15.3's C++ returns with an error set
    (longlong, a `cval` its type cannot take): SystemError; larmorx gives 1.18.1's error. The part's SciPy range: SCI-25..30
    free.
  - **`np.longdouble` sigma** (the orchestrator's 11:34): `_gaussian_kernel1d` takes it as float64 (`dtype.char == "g"`),
    a `UserWarning` per kernel naming the platform's long double where it is wider than float64; = SciPy's kernel of
    `float(sigma)` bit for bit (192 kernels, both environments); measured against SciPy's 80-bit kernels: 184 of 192 differ,
    by at most 7.5e-16 of the largest weight, the order-0 weights by at most 12 ulp (findings, API page). Exact 80-bit
    arithmetic not attempted (not cheap: glibc's expl, long-double sums).
  - **`random_walker(copy=False)`**: `skimage._mark_isolated` (scikit-image's `_preprocess` marks on the caller's labels,
    with `lx.ndimage.binary_propagation`) for every kind but "nozero"; `check_skimage.py --sections rw`: the caller's labels
    afterwards 27/27 in both environments (logs `oracles/logs/fmapb-r21-skimage-rw-{newest,shared}.log`; the rest as relay 17's).
  - **Checks (both environments):** `check_ndimage.py` (every section, boxed 8 GiB, logs
    `oracles/logs/fmapb-r21-ndimage-{newest,shared}.log`): ALL OK, relay 17's 40,503 cases + **`--sections arguments` 964**
    (the filters' 15 call forms x inputs, outputs, cval, origins, modes, axes; sizes, ranks, percentiles, outputs that are
    the input; SCI-21 36 cases each SciPy call in its own process, SCI-22 9, SCI-23 9 x 2 modes; SCI-24 6 in the shared
    environment); `check_regrid.py`'s new `sec_longdouble` (part of `--sections filters`; run alone with `<scratch>/r21/ld_only.py`:
    200 checks + the INFO line); the probes before/after: 169 of 560 cases differed, then only SCI-22's 2.
  - Unit tests: `tests/python/test_ndimage_arguments.py` (new, 19 tests: SciPy's refusal texts, acceptances, SCI-21/22/23
    both modes, the long-double sigma); `test_fmap_defects.py` passes with the rebuilt `.so`. Golden: **new
    `py.ndimage.rank1d`** (registered, unrecorded: float16, int8, `>f8`, longlong inputs, float32/`>f8`/strided outputs) and
    `py.ndimage.gaussian` + `g.longdouble`; both repeatable and = SciPy's corrected forms in both environments
    (`<scratch>/r21/golden_try.py`): **37 registered, unrecorded cases** (36 + 1).
  - Docs: API `ndimage-filters.md` (status paragraph, the min/max/1-D/rank rows, the Gaussian row, "Differences": SCI-21
    to SCI-24 and the long-double sigma), `skimage.md` (copy=False), findings `scipy-ndimage-filters.md` ("The filters'
    argument handling and SciPy's 1-D running rank (relay 21)"), PROVENANCE (larmorx-ndimage), the module and
    `rank_filter` docstrings, investigations README (check_ndimage, check_regrid, check_skimage rows; `relay21/`).
  - Gates (`<scratch>/r21/gates.sh` boxed 16 GiB, log `oracles/logs/fmapb-r21-gates.log`, at aa24049c): `gates-rust.sh
    root` OK (fmt, clippy -D warnings, tests), crates-nc and crates-gpl `cargo check --workspace --all-targets --locked` OK
    (no dependency change: no Cargo.lock refresh); `gates-python.sh --no-build`: pytest 1,204 passed, 37 failed = the 37
    unrecorded golden cases (36 + `py.ndimage.rank1d`); CI's environment 816 passed, the same 37; ruff, format and
    check_spdx failed only on relay 21's two probes (no SPDX header, unformatted), fixed in cf5d389a and re-run: `ruff
    check .` and `ruff format --check .` OK, `check_spdx.py` exit 0; `tables --check` OK. Not run: the smoke tier (the
    records relay).
  - Handoff items: `<scratch>/handoff/relay21.md` (register SCI-21..24 + the free range; tools.toml; golden.md; next-steps:
    close relay 20's argument follow-up for these filters and keep it for binary morphology/labels, close copy=False and the
    long double; no NOTICE/licensing/upstream.tsv change; the audit note).
- **Relay 22 = R9c(b) (done, 2026-11-09): commits 15a77027, ed86a136, 8ad205dc, d42cdcfb** (on cf5d389a). Agent id: the
  orchestrator's to add (relay 22). Scratch helpers in `<scratch>/r22/` (`probe_morph.py`, `probe_out_mask.py`,
  `probe_label_order.py`, `probe_bcast.py`, `test_values.py`, `golden_try.py` (the first three and the last committed under
  `validation/investigations/sdcflows-fieldmap-b/relay22/`), `section_morph.py` (the code written), `ndimage_both.sh`,
  `gates.sh`, the logs, `_ndimage_filters.before-relay22.py`, `register_part_b.before-relay22.py`,
  `NOTES.before-relay22.md`). The `.so` was rebuilt at ed86a136's Rust source (defects.rs, morphology.rs, ndfilters.rs) =
  the committed source (sha256 3e85bed4d6b3093a...).
  - **Binary morphology, `label`, `sum_labels`/`sum`, `center_of_mass`, `watershed_ift` take their arguments as SciPy
    1.18.1** (`python/larmorx/_ndimage_filters.py`'s section rewritten): `_binary_erosion` check for check (`_center_is_true`
    ported: a float origin is numpy's IndexError; an `output` that is not an array ignored: a new bool array; then
    `Py_BinaryErosion`'s parser: `_c_output`, `_c_int(border_value)` (CPython's `"i"`: `__index__`, "Python int too large
    to convert to C long", "signed integer is greater than maximum"/"less than minimum"), `_intp` origins; `_morph_type` =
    `NI_BinaryErosion`'s switches, "data type not supported", mask then input then output, none for an empty input; the
    propagation strategy's `np.asarray(mask, int8)` (numpy's warnings with reproduce) and `_c_int(iterations - 1)` after
    the first pass wrote the output; 0-d inputs as one element; `longlong`); `binary_hit_or_miss`/`binary_fill_holes`
    into an array return None (SciPy's); `label` (output made from a dtype first, `_ni_label`'s symmetry assertion,
    `_label_type` = Cython's "No matching signature found"); `_stats` ported literally with numpy's `bincount`/`unique`/
    `searchsorted` (the Rust `ndimage_sum_labels` binding no longer used: kept, stub says so); `watershed_ift`'s read-only
    output refused by its parser.
  - **A larmorx mistake found and fixed:** `label` numbered in C order whatever its output; SciPy's lines run along the
    output's axis of the smallest stride (numpy's `PyArray_IterAllButAxis`, ported as `_iter_axis`): labelled with that
    axis moved last (428 of 1,800 probe cases differed before; 500 cases identical after, both environments).
  - **Defects SCI-25..30 (all six of the free range; rules A-D; register text regenerated: 88 sections, 56 API rows;
    `larmorx_core::defects` entries added):** SCI-25 (a, rule A, switch: an iterated erosion with a mask into a float
    output erodes outside the mask: `NI_BinaryErosion2`'s first-byte marking; reproduce through the new Rust
    `binary_erosion_unprotected`, binding parameter `protect_later`; 600 of 2,520 float results differ), SCI-26 (f, rule A:
    erosion2's unresolved write-back copy, numpy's warning with reproduce only), SCI-27 (c, rule A: `label` refuses
    big-endian arrays), SCI-28 (f, rule B: `label` writes into a read-only output; reproduce lifts numpy's flag where
    numpy allows), SCI-29 (a, rule A: `label` into a bool output writes label bytes; reproduce counts SciPy's working
    labels, `_provisional_labels`, for its retry beyond 255), SCI-30 (f, rule A: the sums fail on a 0-d index array and on
    an empty input; one notice per `center_of_mass` call). Kept as SciPy (rule C, documented): non-array outputs of
    binary morphology ignored; `None` from hit-or-miss/fill-holes into an array; Linux's C `long` message everywhere.
  - **Checks (both environments):** `relay22/morph_cases.py` (in `check_ndimage.py --sections arguments`): 1,655 cases,
    reproduce = SciPy in all, the default = SciPy (1,531) or a registered correction (SCI-10 26, SCI-13+14 29, SCI-26 13,
    SCI-27 5, SCI-28 2, SCI-29 37, SCI-30 12); the SCI-25 family 5,760 checks; label numbering 500.
    **`check_ndimage.py` (every section) ALL OK: 51,082 checks** (logs `oracles/logs/fmapb-r22-ndimage-{newest,shared}.log`).
  - Unit tests `tests/python/test_ndimage_morph_arguments.py` (new, 21: refusals, acceptances, label's numbering,
    SCI-25..30 both modes); Rust `erosion_into_a_float_output_protects_nothing_outside_the_mask`. Golden: `py.ndimage.binary`
    +4 entries, `py.ndimage.label` +4 (registered, unrecorded; repeatable, = SciPy's corrected forms in both environments:
    `relay22/golden_try.py`); still 37 unrecorded cases.
  - Docs: API `ndimage-filters.md` (status, option rows, labels' numbering, SCI-25..30 under Differences, Rust API),
    findings `scipy-ndimage-filters.md` ("Binary morphology's, `label`'s and the sums' argument handling (relay 22)"),
    PROVENANCE (larmorx-ndimage), `_core.pyi`, module docstring, the investigation's README.
  - **The part's contract audit** (`relay22/audit_contract.py`, seconds; 0 gaps at d42cdcfb): stubs (125 registered
    functions: added `ndimage_rank_filter_1d`, `output` of three filter stubs), API rows (170 public functions of
    lx.sdcflows/niworkflows/ndimage/skimage/bids and larmorprepx's field-map modules: 33 rows completed on six pages, a
    new mask-interfaces table on `sdcflows-fieldmap.md`, `select_ref`'s parameter name corrected), PROVENANCE (55 files:
    added numpy's float32 exp to larmorx-core, sdcflows' workflows + niworkflows' IntensityClip/IntraModalMerge to
    larmorx-sdcflows, part A's stale line corrected), SPDX (`check_spdx.py` exit 0).
  - Gates (`<scratch>/r22/gates.sh` boxed 16 GiB, log `oracles/logs/fmapb-r22-gates.log`, at d42cdcfb): `gates-rust.sh root`
    OK (fmt, clippy -D warnings, tests), crates-nc and crates-gpl `cargo check --workspace --all-targets --locked` OK (no
    dependency change); `gates-python.sh --no-build`: ruff, format OK; pytest 1,225 passed, 37 failed = the 37 unrecorded
    golden cases (unchanged); CI's environment 837 passed, the same 37; `check_spdx.py` exit 0; `tables --check` OK. Not
    run: the smoke tier (the records relay).
  - Handoff items: `<scratch>/handoff/relay22.md` (register SCI-25..30, the range now full; tools.toml; golden.md;
    next-steps: close the argument follow-up, the unused sum_labels binding, SCI-28's partial writes, upstream-report
    candidates SCI-25/SCI-29; no NOTICE/licensing/upstream.tsv change; the audit's summary; the clean-room note).
- **Relay 23 = R9c(c) (done, 2026-11-09): commits 909512f5, c4123d21, 29e235f7, 773aea02** (on d42cdcfb).
  Agent id: the orchestrator's to add (relay 23). Scratch helpers in `<scratch>/r23/` (`time_smoke.sh`, `suites.sh`,
  `suites2.sh` (OUTNAME), `step_diffs.sh`, the records `rec-smoke/`, `rec-standard/`, `rec-step/`, the step_diff outputs
  `sd-{ndimage,numpy,rw,inv}.txt` with their folders, `NOTES.before-relay23.md`). No Rust change: the `.so` is relay 22's.
  **The split (FOR THE ORCHESTRATOR to agree):** the suites and the step's machinery are done; the step's own
  measurement found 28 comparisons of `check_ndimage.py`'s argument sections whose SciPy/numpy outputs change between the
  releases without a reason written yet: **relay 24 = R9c(d)** (below), then 25 = R10 records, 26 = R11 site, 27 = R12.
  - **Three parity suites** (D12; `validation/src/larmorx_validation/parity/sdcflows_scripts.py` + one-line modules
    `ndimage_skimage.py`, `niworkflows_helpers.py`, `sdcflows_estimation.py`; registered in `__main__.PARITY_MODULES`,
    `python_versions.SUITES` (newest by default) and `STEP_SUITES`; pytest wrappers `tests/parity/test_{ndimage_skimage,
    niworkflows_helpers,sdcflows_estimation}.py`): **`ndimage-skimage`** 11 cases (check_ndimage quick/all, check_numpy,
    numpy-expf (full), check_skimage ball..otsu and rw, check_scipy_inv quick/20,000 + the oracle runs' 724 image affines,
    check_regrid filters, check_epi_mask watershed+com, check_match_histogram arrays), **`niworkflows-helpers`** 5
    (check_merge synth..gms and hmc+fmriprep, check_nipype, check_regrid regrid+runs, check_skimage clip),
    **`sdcflows-estimation`** 26 (masker, masker steps, epi_mask, match_histogram files, the contract audit
    (`relay22/audit_contract.py`, smoke), workflows, PEPOLAR units (standard) and workflow+image (full), qwarp steps,
    SyN preprocessing nodes+library (standard) and workflow (full), SyN nodes (standard) and syn+workflow (full),
    estimators without/with the image (full), derivatives nodes+workflow+synthetic and preproc (full), fmap stage
    graph+nodes, unwarp, app parser/toml/settings (smoke), dry runs+exits (standard), run (full), BOLD fmap/steps/stage).
    Tiers: smoke 12 cases (seconds each: 11 pytest tests pass in 69 s in the shared env, + contract audit), standard
    = + 27 (records), full = + 9 long workflow runs (not run by this relay: R10's choice, see below).
  - **Each case** = the script as a subprocess of the running environment's interpreter (`python -P`), larmorx's and
    the validation package's folders on PYTHONPATH, LARMORX_WORKSPACE, OPENBLAS_CORETYPE=Haswell, OPENBLAS_NUM_THREADS=
    OMP_NUM_THREADS=1 (as `r2/run_check.sh` ran every script), LARMORX_NC_BIN (`fsl_mcflirt_replica.replica_binary()`)
    where the script runs the FSL replicas, its own folder (`lxpb_<case>_*`, never a `-h` in it) as cwd, TMPDIR and
    `--scratch`; kept on a failure (a reported check names it), removed otherwise. Heavy cases run in the box the
    parity run runs in, else in a `boxed.sh` box of the README's size (a user scope started inside a box would leave it).
    One check per summary line (`<name>: N <unit>, identical K`; scipy_inv's families; `== <section>: K identical, M
    differ`; `<name>: same|refused`; `DIFF` lines; the contract audit's `N gaps`), INFO/NOTICES lines reported (times
    removed), EXPECTED lines reported as expected differences, the final ALL OK and the exit status checks, and **a run
    that compares nothing fails** (it caught check_app's `--sections parser,toml,settings` with commas: nargs="*").
  - **Found and fixed (a check-script mistake, not larmorx's):** `check_app.py`'s settings section compared larmorx's
    default with sdcflows 2.17.1's own search, so `--session-label` and `--bids-filter-file` (SDC-30, SDC-31 corrected
    by default) differed 2 of 4 (relay 16's log shows the same DIFFERENCES: 2); it now compares with sdcflows 2.15.2's
    search given the sessions and filters (`app_original.py --wrangler 2.15.2 --pass-filters`), as the dry runs do:
    `settings: run (fix)` 4/4 in both environments.
  - **Runs (newest, no step: `NOSTEP=1`):** smoke all pass (after the check_app fix); **standard: ndimage-skimage 10/10
    pass (55 s), niworkflows-helpers 5/5 (46 s), sdcflows-estimation 18/18 (2,209 s at `--jobs 1`; PEPOLAR units 1,172 s, workflows 286 s, estimators 187 s, SyN preprocessing 165 s, the rest under 90 s each; the contract audit, added after that run started, passes alone)** (log `oracles/logs/
    fmapb-r23-standard-{a,b}.log`, records in `<scratch>/r23/rec-standard/`). check_ndimage's SCI-21 gave 35 of 36 cases
    (one SciPy process crashed before printing: the heap SCI-21 corrupts; an INFO line says so): the record's counts may
    vary by one between runs there.
  - **The originals' own step for the library scripts** (`validation/investigations/sdcflows-fieldmap-b/steplines.py`:
    `oracle()` on SciPy's/numpy's/scikit-image's side of each comparison, `note(name)` in each script's Tally (or
    `steplines.Noted(cw.Tally())`), `lines()` before the summary: `STEP <sha256> <name>` when `LARMORX_STEP_LINES` is
    set; the suite sets it only for its step cases when the step runs (STEP_RECORD/STEP_FROM), and the step's shared
    passes skip the workflow cases (SKIPPED: "the originals' own step runs the library scripts only"); x86-64's
    long-double padding left out of the digest). `step_diff.py` runs one script in both environments with
    `LARMORX_STEP_DUMP` and prints each changed comparison's first case in both. Reasons are per case
    (`sdcflows_scripts.REASONS["<suite>/<case>"]`: name regex -> `CHANGES` key): **SCIPY-1.18-INV** (36 scipy_inv
    families: structure detection, LinAlgWarning, new messages, signed zeros), **NUMPY-2.5-PERCENTILE-RANGE** (49 of 819
    `percentile (Python q)` cases: q checked before dividing by 100), **SCIPY-1.16-ROW-ORDER** (random_walker 7 of 10).
    Step run (`fmapb-r23-step2.log`, `<scratch>/r23/rec-step/`): niworkflows-helpers 5/5 (regrid, merge, clip, nipype
    identical between the releases), ndimage-skimage 8/10: gaussian filters, watershed, center_of_mass (com section),
    match_histograms, ball/otsu/dot/hist all identical; **28 check_ndimage comparisons changed without a reason yet**.
  - **`_core.ndimage_sum_labels`** (the orchestrator's 13:03): its stub says the wrapper follows SciPy's `_stats` with
    numpy's `bincount` and the binding exposes the Rust kernel; `tests/python/test_ndimage_sum_labels_binding.py` (11
    tests: every input and label type the binding takes, C/Fortran/strided layouts, negative labels and labels beyond
    the size, absent labels) keeps the two equal bit for bit, in both environments.
  - Gates (`<scratch>/r23/gates.sh` boxed 16 GiB, log `oracles/logs/fmapb-r23-gates.log`, at 773aea02; no Rust change:
    the Rust gates not re-run): `gates-python.sh --no-build`: ruff, format OK; pytest 1,236 passed, 37 failed = the 37
    unrecorded golden cases (unchanged); CI's environment 848 passed, the same 37; `check_spdx.py` exit 0; `tables
    --check` OK. Not run: the parity smoke tier as a whole (the records relay), the full tier's 9 cases.
  - Handoff items: `<scratch>/handoff/relay23.md` (the three records' tools.toml entries, the suites in python_versions,
    next-steps: the check_app fix, SCI-21's count varying by one, `--jobs` for sdcflows-estimation).
- **Relay 23's plan for relay 24 = R9c(d), the step's remaining changes (DONE by relay 24: below)** (`<scratch>/r23/sd-ndimage.txt` has each one's first case;
  re-run `step_diff.py --work <f> --show 3 -- check_ndimage.py --quick` after each fix, about 2 minutes):
  (a) **not changes, the digest's noise, to remove** (in check_ndimage, before `steplines.oracle`, never in the
  comparisons): `_outcome`'s bytes of an object-dtype result are pointers ("arguments: sizes, ranks, outputs" case 23:
  digest the elements' reprs instead); morph_cases' `describe` hex of `<f16` scalars carries x86-64's 6 padding bytes
  (`center_of_mass`, `sum`, `sum_labels` (reproduce = SciPy), 3 cases each: '...0040000000000000' in shared,
  '...00406fe9627c0000' in newest; digest the 10 value bytes).
  (b) **real changes, a reason each in CHANGES/REASONS** (confirm each with step_diff, name the source change):
  numpy 2.5 raises TypeError "only 0-dimensional arrays can be converted to Python scalars" where 2.3 gave a
  DeprecationWarning (a size-1 array as a scalar argument: the filters' case 39 and their 1-D forms, 11 of 52);
  numpy's WRITEBACKIFCOPY warning text ("array_dealloc" in 2.3, "clearing of array" in 2.5: SCI-23's 9, the binary
  morphology's cases 138/139 and 35/36 = SCI-26, the SCI-25 family's one); SciPy 1.18's gh-23293 (1-D rank, short
  input: 10 cases in shared, 40 in newest); SCI-24 (1.15.3's SystemError: SCI-22's case set 6 vs 9, SCI-24's own
  name exists in shared only, so no step check); any other the re-run shows. Then the step run of the two library
  suites (`OUTNAME=step bash <scratch>/r23/suites2.sh standard 2 ndimage-skimage niworkflows-helpers`, boxed 8 GiB,
  about 5 minutes) must pass; the sdcflows-estimation step cases (epi-mask, match-histogram) too (`--case`).
  (c) Also for relay 24 or R10: decide whether R10's records run the full tier (the 9 long cases: about one to two hours by the README's times: PEPOLAR's workflows and fMRIPrep's runs, the SyN graphs, the estimators' image (Apptainer), sdcflows' preproc (7 min), the app's run (about 10 min), numpy's 2^32 float32 exp; never run as suite cases yet: each passed as a script in its relay)
  or the standard (verify-merge re-runs records at the standard tier: `run-suites.sh`).
- **Relay 24 = R9c(d) (done, 2026-11-09): commits 820e60fe, bbe0c1ad** (on 773aea02). Agent id: the orchestrator's to
  add (relay 24). Scratch helpers in `<scratch>/r24/` (`probe_sci21.py` + its outputs `probe-{shared,newest}-{inherit,fixed,
  fixed-noaslr}.txt`, `step_diff.sh` (`sd-ndimage-1/`), `suites.sh` (records `rec-step/`, `rec-step-est/`), `repeat.sh`
  (`repeat.log`), `gates.sh`, `NOTES.before-relay24.md`). No Rust change: the `.so` is relay 22's.
  - **(a) The digest's noise, removed (check_ndimage and relay22/morph_cases.py, the step form only; the comparisons
    unchanged):** `_outcome` returns an `_Outcome` (a tuple) whose `step` holds an object array's elements by repr
    (`_oracle(o)` digests `tuple(o)` or `o.step`: every other digest as before); morph_cases' `_oracle` digests
    `_step_form(x)`: a `<f16`/`<c32` (and `>`) value's hex in `describe` without x86-64's 6 padding bytes. Gone from
    the step's changes: `sizes, ranks, outputs` (no longer "varying in the shared releases"), `center_of_mass`, `sum`,
    `sum_labels` (reproduce = SciPy).
  - **(b) The real changes, a reason each** (`sdcflows_scripts.CHANGES`: **NUMPY-2.4-SCALAR** (numpy 2.4's expiry of
    1.25's deprecation: a one-element array with ndim > 0 as a scalar is a TypeError; common.c
    `check_is_convertible_to_scalar`, 2.4.0's notes; the filters' one-element cval: case 39, and its int8 form in the
    1-D lists), **NUMPY-2.5-WRITEBACK-TEXT** (arrayobject.c `_clear_array_attributes`: "in clearing of array" for 2.3.5's
    "in array_dealloc": SCI-21's strided case, SCI-23, the SCI-25 family, SCI-26's binary morphology cases),
    **SCIPY-1.16-SCI-17** (gh-23293: `1-D rank, short input`'s case set, 10 compared in 1.15.3, 40 in 1.18.1),
    **SCIPY-1.18-SCI-24** (1.15.3's SystemError counted under SCI-24's name: SCI-22's case set 6 vs 9, the 1-D median,
    rank and percentile lists' int8 one-element cval); `REASONS["ndimage-skimage/ndimage-quick"]` and `["...ndimage"]`
    = `_NDIMAGE_REASONS` (exact names; every changed name matched, no unchanged name matched). step_diff after the fixes:
    27 changed comparisons (26 in newest + SCI-24's shared-only name), each confirmed by its first case(s) in both
    (difflib alignment for the case-set changes: the 1-D lists' only edit is case 39 changed and case 40 inserted).
  - **SCI-21 deterministic (the orchestrator's 14:01):** `relay24/probe_sci21.py` measured each of the 36 calls 10 times
    per environment in three ways of starting the process (as the suites do; a fixed environment, hash seed and
    folder; and address-space randomisation off as well): 15 / 15 calls with more than one outcome as the suites start
    them (exit 0, crash after printing, crash before printing at 1-2 in 10, one hang in 360), 3 / 10 fixed, 1 / 0 with
    ASLR off (but there SciPy 1.18.1's float64-into-int16/uint8 percentile crashed before printing in all 10); a
    printed result never differed. So `_scipy_in_process` runs each call through `oracle_cache.run` (Policy
    `never="SCI-21: ... memory it never defines (D13) ..."`, timeout 30 s) in a new process until one prints a line
    ending in " END", up to `SCI21_ATTEMPTS` = 10; every call is one case of both SCI-21 comparisons whatever happened
    (a call none of whose processes printed: reproduce checked for type, size and SCI-21's notice alone, counted on
    `INFO SCI-21 calls whose SciPy processes all crashed or hung (10 each): 0`, printed on every run); the corrected
    comparison no longer skipped with it; SciPy's printed result and its in-process plain result now carry step
    digests. `relay24/repeat_counts.py --runs 5 --step -- check_ndimage.py --sections arguments`: **1 distinct output
    in 5 runs in each environment** (STEP lines included).
  - **Step runs (newest, with the step; records in `<scratch>/r24/rec-step*/`): ndimage-skimage 10/10 (287 step
    comparisons, 126 changed with a known reason, 0 varying), niworkflows-helpers 5/5, sdcflows-estimation's epi-mask +
    match-histogram 2/2 (6 step comparisons, identical)** (logs `oracles/logs/fmapb-r24-step{,-est}.log`; 149 s, 126 s,
    58 s).
  - Findings: `docs/findings/scipy-ndimage-filters.md` SCI-21 (the processes' outcomes); README row `relay24/`.
  - **(c) The full tier:** decided by the orchestrator (14:01): R10 runs the 9 full cases once, from its clean commit.
  - Gates (`<scratch>/r24/gates.sh` boxed 16 GiB, log `oracles/logs/fmapb-r24-gates.log`, at 820e60fe + the format fix
    bbe0c1ad): ruff check OK, ruff format OK after bbe0c1ad (the probe's one line); pytest 1,236 passed, 37 failed = the
    37 unrecorded golden cases (unchanged); CI's environment 848 passed, the same 37; `check_spdx.py` exit 0; `tables
    --check` OK; the three suites' pytest wrappers (smoke) 12 passed in 50 s. Rust gates not re-run (no Rust change).
  - Handoff items: `<scratch>/handoff/relay24.md` (relay 23's SCI-21 follow-up is done, the register's SCI-21 evidence
    line, the step's state for R10).
- **Relay 25 = R10a (records; done, 2026-11-09): commits 4c396799, ab5ffe99, e81cf26b, a9211041, e249fb8f, f67d05fe,
  3dfe21ad, 657b0f6f, c073ac6d, 241b3334, 95b2df2d, 0b3f472e** (on bbe0c1ad). Agent id: the orchestrator's to add (relay 25). Scratch helpers in `<scratch>/r25/`
  (`build-nc.sh` (the branch's larmorx-nc release in `<scratch>/target-nc`, the records' `LARMORX_NC_BIN`), `records.sh`
  (one parity record run into `r25/rec-<out>/`), `jobB.sh`, `app_try.sh`, `mem_try.sh`, `mem_test.sh`, `golden-wt.sh`
  (record_golden in the worktree), `golden.sh` + `clone_probe.sh` (the scratch clone that FAILS the smoke tier: see
  below), the records `rec-standard/`, `rec-full/`, `rec-app-shared/` (+ `rec-app-shared-first-before-the-fix/`),
  `plan-relay26.md`, `NOTES.before-relay25.md`). No Rust change: the `.so` is relay 22's (= the committed Rust source).
  **The split (FOR THE ORCHESTRATOR to agree):** R10 = this relay's records + memory calls (+ the golden attempt), then
  **relay 26 = R10b** (random testing, golden, benchmark, gates: the plan below), 27 = R11 site, 28 = R12 handoff.
  - **Records (all from clean commits; every case's code identical between the stamps; logs
    `oracles/logs/fmapb-r25-*.log`; in `docs/validation/`):**
    - `ndimage-skimage` (standard, newest + the step; 4c396799): **10/10 pass**; 333 comparison lines; the step 287
      comparisons, 126 changed with a known reason, 0 varying.
    - `niworkflows-helpers` (standard, newest + the step; e81cf26b, re-made after the reference text's fix, results
      identical to the first run by compare_records): **5/5 pass**.
    - `sdcflows-estimation` (standard, newest + the step; ab5ffe99): **19/19 pass** (1,472 s at `--jobs 2`; pepolar-units
      1,377 s, workflows 340, syn-preprocessing 256, bold-steps 255, estimators 229).
    - `ndimage-skimage-full` (the full tier's own case, numpy's float32 exp on all 2^32 inputs; ab5ffe99): **1/1**.
    - `sdcflows-estimation-app-shared` (the BIDS app run whole on each of sdcflows 2.17.1's eight own runs, shared
      environment, numpy 2.3.5 / SciPy 1.15.3; f67d05fe): **8/8 pass** (19 checks each: every derivative file's names,
      uncompressed and gzip bytes, sidecars, the cli lines, the settings) after the fix below.
    - `sdcflows-estimation-full` (the full tier's own 14 cases, newest, `--jobs 4` in a 64 GiB box, 4,319 s; 241b3334):
      **14/14 pass**: pepolar-workflow (wf, both wirings) 2,840 s, pepolar-image 965 s, syn-preprocessing-workflow 1,172 s,
      syn-sdc-workflow 4,317 s, estimators-image 51 s, derivatives-preproc 16 s, app-run-* x 8 (29 to 480 s). The
      first full run (12/13) failed pepolar-workflow's image section: the
      check-script mistake below; kept in `<scratch>/r25/rec-full/sdcflows-estimation.first.*.txt`.
  - **The suite's change (4c396799):** the BIDS app's whole run is one case per sdcflows 2.17.1 own run
    (`sdcflows_scripts.APP_RUNS`: `app-run-<dataset>` x 8, full tier; was one case on ds005752); `check_app.py`'s `run`
    gives larmorx's app the running environment's library set (`LARMORX_SDCFLOWS_LIBRARIES` from numpy's and SciPy's
    releases), so the runs work in both environments.
  - **A larmorx mistake found by the records and fixed (e249fb8f):** in the shared environment ds006736's coefficient
    file's `quatern_d` was one bit from sdcflows' (-1.4631351e-16 vs -1.463135e-16, float32 `0xa528b020`/`0xa528b01f`;
    the data and every other file identical): the app's `shared` set did not move nibabel's `set_qform` arithmetic to
    numpy 2.3.5's LAPACK 3.12.0 (the check scripts call `use_lapack` themselves). `app._run_workflow` now runs the
    workflows with the set's LAPACK (`_LAPACK = {"newest": "3.12.1", "shared": "3.12.0"}`, `use_lapack` in a copied
    context); unit test `test_library_sets_choose_the_writers_lapack`; API row (`docs/api/sdcflows-fieldmap.md`, the
    environment-variable table); findings `docs/findings/sdcflows-estimation.md` "The records (relay 25)". No register
    entry (larmorx's own mistake).
  - **Memory (ab5ffe99):** `test_fieldmap_steps_need_no_more_memory_than_the_originals` (4 calls; `memory_call.py` beside
    the scripts): brain_masker ds002603 larmorx 431 MiB vs sdcflows 1,438; IntensityClip 84 vs 143; RobustAverage's own
    steps (`mc_method=None`; the 3dvolreg is a program measured by its own calls) 108 vs 149; ReconstructFieldmap
    ds005454 onto the T1w grid 230 vs 1,438; files the originals' byte for byte in reproduce; part A's three re-measured
    with the branch: 89/366, 475/1,473, 881/3,740 MiB. "The topup workflow without topup" not made a call: its Python
    steps hold a few small EPI volumes; the heavy work is the programs (topup, 3dvolreg, N4), each with its own memory
    calls.
  - **A check-script mistake found and fixed (241b3334):** the first full run (4c396799 -> stamped later; 12 of 13)
    failed `pepolar-workflow` on `image: fix_coeff (fMRIPrep's topup) = fMRIPrep's` (ds006736, `quatern_d`): the image
    section compares larmorx with fMRIPrep's own files, written with fMRIPrep's image's numpy 2.2.6 (LAPACK 3.12.0), but
    used the running environment's LAPACK (3.12.1 in newest; the section had only run in shared before). It now sets
    `use_lapack("3.12.0")` for the section (as check_fmap_stage and check_derivatives); ds006736 then ALL OK in newest
    (regrid exact too, without the quaternion allowance). The suite's case split in two: `pepolar-workflow` (`wf`) and
    `pepolar-image` (`image`), both full tier; the full record re-made (below).
  - **The parity smoke tier (record_golden in the worktree, 16 workers, 3dfe21ad): 2,911 passed, 3 failed**:
    `test_pipeline_stages.py::test_every_program_node_is_classified` on the three `--use-syn-sdc` runs (their program
    nodes unclassified), which main fixed after the branch point (d1491604, "stages catalogue: a roadmap entry for
    sdcflows' SyN ..."; the runs became finished when they were re-made at 05:41). The serial part (memory peaks,
    thread pools) 235/235, part B's four memory calls included. So record_golden refused (parity failed).
  - **Golden NOT recorded:** `record_golden.py --skip-parity` then stopped in the case `py.sdcflows.estimators`:
    `ValueError: 'den' is not a recognized entity`: record_golden's temporary folder `larmorx-golden-<x>` holds `den-<x>`,
    sdcflows' defect below (SDC-45), which larmorx reproduces; `pytest tests/golden` (pytest's `tmp_path`) does not meet
    it. The 37 cases stay unrecorded (the next relay, after SDC-45's correction: the plan's (a) and (b)).
  - **sdcflows' defect found (candidate SDC-45, rule A proposed; c073ac6d, 657b0f6f):** `FieldmapFile` takes its
    entities from `parse_file_entities` on the file's ABSOLUTE path, and pybids 0.22.0's derivatives patterns `atlas-`,
    `roi-`, `label-`, `desc-`, `hemi-`, `seg-`, `res-`, `den-`, `model-`, `subset-` have no separator before them, so a
    dataset below a folder such as `golden-clone`, `andres-lab` or `garden-2024` gives every file that entity, and
    `find_anatomical_estimators` (fieldmap-less search; also inside `find_estimators`) queries the layout with it:
    `ValueError: 'den' is not a recognized entity`. sdcflows 2.17.1 and larmorx give the same error
    (`relay25/probe_root_entities.py`; two estimators each below `plain/`). fMRIPrep's `--use-syn-sdc` reaches it.
    Findings paragraph in `docs/findings/sdcflows-estimation.md`; not yet in the register draft nor in
    `larmorx_core::defects` (the next relay, with its correction).
  - **A scratch clone cannot run the smoke tier:** a clone of the branch in `<scratch>` failed tests whose headers carry
    the repository's path (`test_afni_automask_parity[options/default]`: "mask.nii: header bytes: differ"): record_golden
    runs in the worktree only.
  - **niworkflows' release:** both kept environments hold niworkflows 1.15.3 (not 1.15.4, the port's source); of their
    Python files only `interfaces/confounds.py` and `_version.py` differ, so the comparisons stand; the suites' text
    says so (e81cf26b).
  - **Records' reported values carry the cases' `/tmp/lxpb_*` folder names** in a few INFO/EXPECTED lines of
    `sdcflows-estimation.json` (5), as five older records do; no account name or machine path (`compare_records.py`
    strips them). A follow-up if the orchestrator wants them gone (`_reported` would replace the case's folder).
  - Gates at 0b3f472e: `ruff check .` and `ruff format --check .` OK, `check_spdx.py` exit 0, `tables --check` OK,
    `tests/python/test_sdcflows_app.py` 10 passed; the parity smoke tier as above (2,911 passed, 3 failed: main's
    d1491604). Not run: `gates-rust.sh` (no Rust change since relay 22), the whole `gates-python.sh` (relay 26, after its
    changes; tests/golden fails on the 37 unrecorded cases until then).
  - Handoff items: `<scratch>/handoff/relay25.md` (the six records' `[[record]]` entries, verify-merge's suites, the
    memory calls; no NOTICE/licensing/upstream.tsv change).
- **Relay 25's plan for relay 26 = R10b** (FOR THE ORCHESTRATOR to agree; then 27 = R11 site, 28 = R12 handoff; if it does
  not fit, split: 26 = (a) + (b), 27 = (c) + (d) + (e)):
  (a) **SDC-45 (sdcflows' defect found by this relay; rule A proposed, the orchestrator to agree):** `lx.sdcflows`'
  `FieldmapFile` takes its entities from the file's BIDS part (the path from its participant's `sub-<label>` folder on,
  or relative to the layout's root where known) by default, `reproduce` = sdcflows' `parse_file_entities` on the absolute
  path (pybids 0.22.0's unanchored derivatives patterns `atlas-`, `roi-`, `label-`, `desc-`, `hemi-`, `seg-`, `res-`,
  `den-`, `model-`, `subset-` match the folders above the dataset); look for every other place sdcflows, niworkflows or
  fMRIPrep parse entities from an absolute path (read by relay 25: niworkflows 1.15's `DerivativesDataSink` parses
  `relative_to_root(source_file)`, safe; fMRIPrep's `extract_entities` (`utils/bids.py` line 532, `workflows/bold/base.py`
  line 836) parses the absolute paths it is given; `lx.bids`' own index: pybids' `Entity.match_file` on `f.path`) and
  decide each by the rules; `larmorx_core::defects` entry, the register text (`<scratch>/handoff/register_part_b.py`), API rows,
  findings (relay 25's paragraph in `docs/findings/sdcflows-estimation.md` is the start), `check_estimators.py`: a
  section with datasets below `golden-x/`, `andres-lab/`, `workspace-2024/` (`space-` is anchored: a control), both
  modes against sdcflows; `relay25/probe_root_entities.py` is the reproducer.
  (b) **Golden** (37 unrecorded cases), IN THE WORKTREE (a scratch clone fails the smoke tier: tests compare headers that
  carry the repository's path): `record_golden.py` refused twice in relay 25: (1) the smoke tier (3dfe21ad, 16 workers:
  2,911 passed, 3 failed: `test_pipeline_stages.py::test_every_program_node_is_classified` on the three `--use-syn-sdc`
  runs, which main fixed after the branch point in d1491604: not part B's; the serial memory and thread tests 235/235,
  part B's four memory calls included); (2) `--skip-parity`: the golden case `py.sdcflows.estimators` raises SDC-45's
  `ValueError: 'den' is not a recognized entity` because record_golden's temporary folder is `larmorx-golden-<x>`
  (`den-<x>`), while `pytest tests/golden` (pytest's `tmp_path`) does not: the case's outputs depend on its folder's
  name, so a recorded checksum would differ from every platform's. Recommendation: (a) first (the default then reads
  the BIDS part only), the case's `reproduce` calls kept off the anatomical search, `record_golden.py` left alone; then
  `python -P scripts/record_golden.py --parity-workers 16` (boxed 32 GiB) if the orchestrator brings d1491604 into the
  branch (merging main, or its one file), else `--skip-parity` with the smoke tier's result named in NOTES
  (verify-merge re-records golden from the merge with `--golden`).
  (c) **Random testing**: difftest descriptors for part B's families (docs/validation/difftest.md; part A's
  `difftest/tools/sdcflows_fieldmap.py` the model; registered in `difftest/tools/__init__.py` and
  `python_versions.TOOLS`): `ndimage-skimage` (the filters, morphology, labels, sums, watershed, center_of_mass, `inv`,
  ball/otsu/random_walker/match_histograms, numpy's statistics on seeded arrays, SciPy's argument handling included),
  `niworkflows-helpers` (intensity_clip, robust_average with `mc_method=None`, merge_series, intra_modal_merge without
  HMC, regrid_to_zooms, nipype's command lines), `sdcflows-estimation` (brain_masker, get_trt/get_readout_time,
  select/sort PE volumes, uniform_grid, reorient, epi_mask, match_histogram, the estimators on generated BIDS trees,
  their roots' folder names included: SDC-45's kind); one short boxed trial (`difftest --tools <the three> --budget
  0.5`), every difference triaged into `validation/difftest/triage.toml` (a larmorx mistake fixed with a permanent
  case; an original's defect registered by rules A-D in the free IDs: PIPE-36..40, SCI-31..40, SKI-05 on; SDC-45 is
  (a)'s, so a new sdcflows defect needs a wider range from the orchestrator). If (c) changes larmorx's code, (b) comes
  after it.
  (d) **Benchmark** `docs/benchmarks/sdcflows-estimation.md` (part A's `docs/benchmarks/sdcflows-fieldmap.md` the model):
  the four memory calls at 1 and 14 threads against the originals (one thread here, informal: brain_masker 6.5 s vs
  11.3 s, IntensityClip 1.9 vs 2.0, RobustAverage's steps 0.4 vs 1.3, ReconstructFieldmap 0.5 vs 7.6), the BIDS app's
  whole runs (the records redact the INFO lines' seconds: re-measured, one thread, sdcflows' own app against `larmorx
  sdcflows sdcflows`), a script under `validation/investigations/sdcflows-fieldmap-b/` (bench_*.py), the machine shared
  or quiet stated.
  (e) The gates: `gates-rust.sh <worktree> all` (the three workspaces; crates-nc's release is built in
  `<scratch>/target-nc` at the branch's source: `<scratch>/r25/build-nc.sh`), `gates-python.sh --no-build`, `tables
  --check`, `check_spdx.py`; the records' `LARMORX_NC_BIN` = `<scratch>/target-nc/release/larmorx-nc`.
- **Relay 26 = R10b (done, 2026-11-09): commits 8e4ae409 (main's d1491604 cherry-picked, `-x`), adf07a34, 3e2340a8,
  bf4f04d7, 11a1f4dd** (on 0b3f472e). Agent id: the orchestrator's to add (relay 26). Scratch helpers in `<scratch>/r26/`
  (`wheel.sh`/`cargo.sh` (28 jobs), `run_check.sh`, `golden-wt.sh` (record_golden in the worktree), `records.sh` (a
  record from a checkout: the worktree's, `REC_CHECKOUT`; a scratch checkout does NOT work: the suites find the
  workspace from the repository's parents and skip every case), `gold_try.py`/`gold_all.py` (golden cases below two
  folder names; committed as `validation/investigations/sdcflows-fieldmap-b/relay26/golden_folders.py`, 11a1f4dd),
  `gates-rust.sh`, `plan-relay27.md`, `notes-section.md`, `NOTES.before-relay26.md`). The split: this relay = relay 25's plan (a) + (b) (+ the
  catalogue); (c), (d), (e) are relay 27's (plan below).
  - **SDC-45 (rule A, agreed) and PIPE-37 (rule A, new: pybids' index) corrected by default; PIPE-36 found (fMRIPrep, not
    ported) (adf07a34).** Every entity parse from a path in sdcflows 2.17.1, niworkflows 1.15.4, fMRIPrep 25.2.7 and
    pybids 0.22.0 read: (1) pybids' indexer matches every pattern against the absolute path (`Entity.match_file`), so
    below `func/` every file is datatype func, below `sub-pilot/` participant pilot (its own label lost), and with the
    derivatives configuration below `golden-x`/`andres-lab` every file gets den/res: `lx.bids.BIDSLayout(...,
    upstream_defects="fix")` matches the path below the root (`/`-rooted), warns (PIPE-37) when that changed an entity,
    passes the mode to its derivatives layouts; `parse_file_entities` (function and method) unchanged (an API: parses
    what it is given). (2) sdcflows' `FieldmapFile` parses the absolute path: larmorx takes the path from the nearest
    `sub-*` folder (niworkflows' `relative_to_root`; a file below none keeps sdcflows' parse), warns (SDC-45);
    `upstream_defects` threaded through `FieldmapFile`, `FieldmapEstimation`, `find_estimators`,
    `find_anatomical_estimators` (new parameter), the BIDS app's layout, larmorprepx's `fmriprep_layout`. (3) fMRIPrep's
    `extract_entities` parses absolute BOLD paths and `get_sbrefs` queries the layout with them: below `golden-x` every
    BOLD run stops `init_bold_fit_wf` (verified with fMRIPrep 25.2.7's own functions on pybids): PIPE-36, rule A when
    larmorprepx's BOLD stages are ported (a follow-up). (4) niworkflows' `DerivativesDataSink` and `BIDSInfo` with
    `bids_dir` parse relative paths (safe); `ReadSidecarJSON` and `BIDSInfo` without `bids_dir` (absolute) are not
    ported (decided when ported). `larmorx_core::defects`: PIPE_37, SDC_45 (wheel rebuilt). Evidence:
    `check_estimators.py --sections roots` (new; in the suite's `estimators` case): three sdcflows test skeletons and
    ds000228's BIDS input (linked) below plain, golden-x, andres-lab, workspace-2024, func, sub-pilot: 168 comparisons
    identical (reproduce = pybids' index and sdcflows 2.17.1's search, the same ValueError below golden-x and
    andres-lab; fix below every folder = pybids and sdcflows 2.15.2 below plain); the whole script at adf07a34: index
    306, wrangler 2790, units 577, fmriprep 2567, runs 92, apps 8, roots 168, ALL OK. Unit tests `test_sdc45_*`,
    `test_pipe37_*` (tests/python/test_fmap_defects.py); API rows (`docs/api/sdcflows-fieldmap.md`, `docs/api/bids.md`:
    the "details kept" bullet corrected, a D13 table); findings `docs/findings/sdcflows-estimation.md` "Entities from
    the folders above a dataset (relay 26)"; the register draft (`<scratch>/handoff/register_part_b.py`: SDC-45,
    PIPE-36, PIPE-37; 58 API rows).
  - **The catalogue (adf07a34, MERGE NOTE in `<scratch>/handoff/relay26.md` item 3):** d1491604's roadmap entry turned
    into stages S04 (ref_anat, DenoiseImage), S05 (epi_reference_wf/n4_avgs, N4), S06 (anat2epi, mask2epi, prior2epi,
    amask2epi, zooms_bmask, zooms_field: antsApplyTransforms), S07 (lap_anat, lap_epi: ImageMath); `epi_merge`
    (StructuralReference = FreeSurfer's mri_robust_template; `echo` for one run) moved to the mri_robust_template
    roadmap entry. Classified on every oracle run (32 tests); checked on ds000228-syn (`pipeline-stages --stage S04 S05
    S06 S07 --no-container --no-ants-oracle`, scratch, not a record): 8 of 10 nodes bit-identical to fMRIPrep's own
    outputs, S04 NCC 1 - 1e-13 and S05 NCC 1 - 5e-10 (multi-threaded ANTs; "unexplained" only because the references
    were not run: the stage records with references are the final comparisons').
  - **Golden:** **recorded** (3e2340a8): `record_golden.py --parity-workers 16` in the worktree from adf07a34,
    boxed 36 GiB, 1,582 s: the parity smoke tier passed whole (16 workers: 2,679 passed in 352 s; serial memory and
    thread tests 235 passed in 1,205 s; 2,914 in all, 0 failed: main's d1491604 fixed relay 25's three
    `test_every_program_node_is_classified`; marker `oracles/cache/markers/parity-smoke-adf07a34....json`); 299 cases
    (2,630 outputs), the 37 of part B added, none of the 262 recorded changed; `pytest tests/golden` 301 passed.
    Before it: the golden case `py.sdcflows.estimators` made independent of its folder (its reproduced searches leave
    out the four fieldmap-less cases and the files' entities, which hold the folders above the dataset: SDC-45; a new
    output `below_entity_folders`: the search below `golden-x/sub-pilot/` = the search below the case's folder),
    `py.bids.layout` parses `"/" + relpath` (the layout's `parse_file_entities` of an absolute path depends on the
    folders above it, as pybids'); `<scratch>/r26/gold_all.py`: all 37 cases give the same fingerprints below
    `plainrun/` and `larmorx-golden-abc/` (record_golden's `den-`), the 37 take about 10 s in all
  - **Record:** `sdcflows-estimation` (standard, newest + the step) re-made from 3e2340a8 in the worktree (bf4f04d7;
    `<scratch>/r26/records.sh`, `--jobs 3`, 40 GiB box, 1,364 s): **19/19 pass**; `compare_records.py` against relay
    25's: only the `estimators` case differs (it gains the `roots` section: 168/168 and its INFO lines; 54 checks
    against 9), every other case identical. No other record re-made (the index's default gives pybids' entities on
    every dataset not below an entity-named folder: the whole check_estimators at adf07a34 the same).
  - **Gates at 3e2340a8 (the code of the final commit; bf4f04d7 adds the record only):** `gates-rust.sh` all three
    workspaces OK (`<scratch>/r26/gates-rust.sh`: fmt, clippy -D warnings, tests; target folders `<scratch>/target`,
    `target-gpl`, `target-nc`; log `oracles/logs/fmapb-r26-gates-rust.log`); `gates-python.sh --no-build`: pytest
    1,276 passed, 1 failed = `test_mri_replica.py::test_the_replica_itself` (the known LARMORX_NC_BIN follow-up of
    relay 9: the test fails only when LARMORX_NC_BIN is set; without it 4 passed, 1 skipped), CI's environment 888
    passed, 13 skipped, the collection without larmorx_validation, ruff check and format (1,034 files), check_spdx OK;
    `tables --check` OK; the parity smoke tier passed whole at adf07a34 (2,914, above).
  - Handoff items: `<scratch>/handoff/relay26.md` (the register's three entries, next-steps follow-ups (PIPE-36 when
    porting larmorprepx's BOLD stages, ReadSidecarJSON/BIDSInfo, the `/tmp/lxpb_*` names, the SyN stage record, three
    more upstream drafts), the catalogue's MERGE NOTE, the record, golden.md).
- **Relay 26's plan for relay 27 = R10c (random testing; FOR THE ORCHESTRATOR to agree; then 28 = (d) benchmark + (e)
  gates (or with 27 if it fits), 29 = R11 site, 30 = R12 handoff):**
  (c) **Difftest descriptors** for part B's three families (docs/validation/difftest.md; part A's
  `difftest/tools/sdcflows_fieldmap.py` + `parity/sdcflows_fieldmap.py`'s `_random_case` the model: the descriptor
  draws a recipe, `case()` makes a `("random", spec)` parity case, `run(impl)` returns the family's parity module's
  `run_case`, which gains a `("random", spec)` branch running the original in-process and larmorx in `reproduce`
  bit for bit, then the D13 fix pass (default against `reproduce`: a defect named exactly when they differ)).
  Register each in `difftest/tools/__init__.py` (`TOOLS`) and give each its row in docs/validation/difftest.md's
  table (part A's `sdcflows-fieldmap` row the model; the page is not among CLAUDE.md's shared documents).
  The comparators to reuse (import the scripts in-process as part A's `_load`):
  - `ndimage-skimage` (`parity/ndimage_skimage.py`): `check_ndimage.py`'s `compare`, `values`, `layouts`,
    `random_footprint`, `_outcome` (gaussian_filter with orders/modes/truncate/radius, correlate1d/convolve,
    uniform/minimum/maximum/rank/median/percentile filters with size or footprint, origin, mode, cval, output
    dtype and byte order (SCI-21's crashing calls only through `_sci21`'s child process, `case_never`), grey and
    binary morphology (structure, iterations, mask, border_value, brute_force; SCI-10, SCI-25, SCI-26), label
    (structure, output types; SCI-27..29), sum_labels/center_of_mass index forms (SCI-30), watershed_ift (int
    marker types; SCI-13..16)); `check_skimage.py`'s `same` (ball, threshold_otsu, random_walker with labels and
    modes, match_histograms; SKI-01..04); `check_numpy.py`'s `same` (percentile, median, clip, float32 exp);
    `check_scipy_inv.py`'s `compare`/`outcome` (4x4 affines, near-singular, symmetric; SCI-18..20); both library
    environments (the originals' step as the suite's).
  - `niworkflows-helpers` (`parity/niworkflows_helpers.py`): intensity_clip (`check_skimage.sec_clip`'s
    comparison; PIPE-33 constant images), robust_average with `mc_method=None` (PIPE-34), merge_series,
    intra_modal_merge without HMC (`check_merge.py`'s `compare`, `synth_cases` as the draw's model), regrid_to_zooms
    (`check_regrid.py`'s `both`), nipype's command lines (`check_nipype.py`'s `synthetic_cases`/`check_synthetic`).
  - `sdcflows-estimation` (`parity/sdcflows_estimation.py`): brain_masker on small head phantoms (SDC-37, SDC-38),
    get_trt / readout times from drawn metadata (SDC-43), select/sort PE volumes, uniform_grid,
    reorient_image_and_metadata (SDC-12), epi_mask (SDC-28), match_histogram (SDC-27), and the estimators on drawn
    BIDS trees (niworkflows' `generate_bids_skeleton` specs: field-map kinds, sessions, IntendedFor,
    B0FieldIdentifier lists, echoes, sbrefs; the tree below a drawn parent folder, entity-like names included:
    `golden-x`, `andres-lab`, `func`, `sub-pilot`, `workspace-2024` (SDC-45, PIPE-37), with `check_estimators.py`'s
    `dump` serializer (`relay11/estimators_dump.py`) and the `roots` section's comparisons (both modes)).
  One short boxed trial (`difftest --tools ndimage-skimage,niworkflows-helpers,sdcflows-estimation --budget 0.5
  --jobs 12 --out oracles/difftest/<date>-fieldmaps-b`, `LARMORX_DIFFTEST_CACHE_MODE=use`), a one-minute check run
  first; every difference triaged into `validation/difftest/triage.toml` (a larmorx mistake fixed with a permanent
  case; an original's defect registered by rules A-D in the free IDs: PIPE-38..40, SCI-31..40, SKI-05 on,
  SDC-46..55). If (c) changes larmorx's code, the golden cases it touches are re-recorded (`record_golden.py`
  re-records only missing ones: delete the changed case's entry, or `--compare`; the smoke marker of the clean
  commit lets it skip the smoke tier only for the same commit and build).
  (d) **Benchmark** `docs/benchmarks/sdcflows-estimation.md` (part A's `docs/benchmarks/sdcflows-fieldmap.md` the
  model): the four memory calls (`memory_call.py`) at 1 and 14 threads against the originals (informal one-thread
  numbers in relay 25's notes), the BIDS app's whole runs (one thread; sdcflows' own app against `larmorx sdcflows
  sdcflows`), a script `validation/investigations/sdcflows-fieldmap-b/bench_estimation.py`, the machine's load
  stated (quiet: nothing else running).
  (e) **The gates** at the final commit: `gates-python.sh <worktree> --no-build` (with `LARMORX_NC_BIN` unset, or
  name `test_the_replica_itself`'s known failure), `tables --check`, `check_spdx.py`; `gates-rust.sh <worktree> all`
  again only if relay 27 changes Rust (relay 26 ran it at 3e2340a8: OK; `<scratch>/r26/gates-rust.sh`); the smoke
  tier again if larmorx's code changes (its marker of adf07a34 is in the oracle cache).
- **Next: relay 27 = R10c** (the plan above; the orchestrator agrees the numbering).
- **Relay 27 = R10c (done, 2026-11-09): commits 964f72a2, 95e2a38a, e88326b6, ad30e330, 27691ed4, 9cc48d9d** (on 11a1f4dd).
  Agent id: the orchestrator's to add (relay 27). Scratch helpers in `<scratch>/r27/` (`dt.sh` (boxed difftest
  from the worktree, `NOAVX512=1` = numpy's AVX-512 dispatch off), `try_cases.py` (drawn cases in-process,
  verdicts per call), `run_one.py` (TOOL:SEED:N through the runner), `one_side.py`, `show_case.py`,
  `rec_input.py`, `summ.py` (a night's verdicts per call), `wheel.sh` (the .so copied atomically: checks may run
  meanwhile), the probes' folders `dt-try1`, `dt-try2`, `dt-try2n`, `plan-relay28.md`, `triage-block.toml`,
  `NOTES.before-relay27.md`). The split agreed by the orchestrator's note: this relay = (c); (d) + (e) are relay
  28's (plan below), then 29 = R11 site, 30 = R12 handoff.
  - **The three difftest tools** (`validation/src/larmorx_validation/difftest/tools/fieldmaps_b.py`: NDIMAGE,
    NIWORKFLOWS, ESTIMATION; runner `parity/fieldmaps_b_random.py`, reached through `sdcflows_scripts.run_case`'s
    `("random", spec)` branch (skipped in the step's passes), registered in `difftest/tools/__init__.py`,
    `permanent.SUITES`, `python_versions.TOOLS` (newest by default); rows in `docs/validation/difftest.md`). One
    drawn call per case; each side builds its own arguments from the recipe (SCI-31 showed why); every output bit
    for bit plus the array arguments afterwards; larmorx in `reproduce`; the D13 fix pass (`defects.add_fix_check`).
    Calls with numpy's float64 `exp` (Gaussian, RegridToZooms, float64 random walker) compared only when numpy's
    exp is libm's (AVX-512 off), else skipped: hence the second, AVX-512-off trial. ndimage: 11 calls (gaussian,
    correlate1d, rank family, grey, binary, label, measure, watershed, skimage's 4, numpy's 4, inv); niworkflows:
    6 (clip, robust_average, merge_series, intra_modal_merge, regrid, nipype's 8 command lines); estimation: 10
    (get_trt, readout_time, select_pe, sort_pe, uniform_grid, reorient, brain_masker (weight 0.3), epi_mask,
    match_histogram, estimators on drawn BIDS trees below entity-named folders).
  - **Found and fixed or registered** (probes try1-6 in-process, difftest fmapb-try1, fmapb-try2, fmapb-try2n):
    SCI-31 (rule A, switch), SCI-32 (memory, no switch, never drawn), SDC-46 (rule A, switch), PIPE-38 (rule C),
    PIPE-39 (nondeterministic), PIPE-40 (rule A, switch); larmorx mistakes fixed: GetReadoutTime/DistortionParameters
    TraitError + enum checks, random_walker's mode warnings order, UniformGrid 4-D error, brain_masker IndexError
    (SdcError::Index), match_histogram's dtype in numpy's message, epi_mask's and opening/closing's notices only on a
    change, nibabel's io_orientation in ReorientImageAndMetadata and aff2axcodes, DenoiseImage's listed noise_image;
    harness: nipype's OutputMultiObject lists, SDC-45 silent_ok, valid nipype combinations. 13 permanent cases
    (`validation/investigations/sdcflows-fieldmap-b/relay27/make_permanent.py --check`: ALL OK in newest; in shared
    the reorient one differs: LAPACK-3.12.1, as expected). Unit tests `tests/python/test_fmap_random_findings.py`
    (9). Findings: `docs/findings/sdcflows-estimation.md` "Random testing of part B (relay 27)",
    `docs/findings/scipy-ndimage-filters.md` "Found by random testing"; API rows (ndimage-filters, niworkflows-helpers,
    sdcflows-fieldmap); register draft `<scratch>/handoff/register_part_b.py` (+6 entries); handoff items
    `<scratch>/handoff/relay27.md`.
  - **The trial (from clean commit e88326b6; two boxed half-hour runs at once, one thread per case, 3 GiB per case):**
    `oracles/difftest/2026-11-09-fieldmaps-b` (seed fmapb-trial, 10 jobs, box 48 GiB) and
    `oracles/difftest/2026-11-09-fieldmaps-b-noavx512` (seed fmapb-trial-noavx512, `NOAVX512=1`, 6 jobs, box 32
    GiB): **312,081 cases, 285,646 agree, 0 differences, 0 notices without a change, 0 crashes, 0 memory cases**;
    26,427 skipped in the first run (float64 exp calls), all compared in the second; **8 timeouts**, all
    brain_masker whose random walker does not converge (8 of 518 brain_masker cases > 900 s; larmorx 9x slower per
    CG iteration than SciPy there: follow-up). Per tool: ndimage-skimage 173,684 (159,396 agree, 14,288 skipped),
    niworkflows-helpers 115,691 (103,552 agree, 12,139 skipped), sdcflows-estimation 22,706 (22,698 agree, 8
    timeouts). triage.toml block "field maps part B (2026-11-09)": 17 entries.
    One timed-out case (`sdcflows-estimation:fmapb-trial:4563`) run to its end outside the limit
    (`<scratch>/r27/slow-trial-4563.txt`): 2 checks, 0 failed (726 s for sdcflows and larmorx's two modes).
  - **Gates** (code of e88326b6; ad30e330, 27691ed4, 9cc48d9d add the triage, findings, the difftest page and the
    investigation's drivers only): `gates-rust.sh` all
    three workspaces OK (`<scratch>/r27/gates-rust.sh`, 16 jobs, boxed 32 GiB; log
    `oracles/logs/fmapb-r27-gates-rust.log`); `gates-python.sh --no-build` with LARMORX_NC_BIN unset: exit 0 (pytest
    1,288 passed, 13 skipped; CI's environment 897 passed, 13 skipped; the collection without larmorx_validation;
    ruff check and format; check_spdx; log `oracles/logs/fmapb-r27-gates-python.log`); `tables --check` OK;
    `pytest tests/golden` 301 passed in both environments (no checksum changed). **Not run: the parity smoke tier**
    (larmorx's code changed since its marker of adf07a34; the 13 permanent cases now join the three suites' smoke
    tier) and golden coverage of the new paths: relay 28 (`record_golden.py` runs the smoke tier first).
- **Relay 27's plan for relay 28 = (d) benchmark + (e) golden and the smoke tier (FOR THE ORCHESTRATOR to agree; then 29 = R11 site,
  30 = R12 handoff):**
  (d) **Benchmark** `docs/benchmarks/sdcflows-estimation.md` (part A's `docs/benchmarks/sdcflows-fieldmap.md` the
  model): the four memory calls (`memory_call.py`) at 1 and 14 threads against the originals, the BIDS app's whole
  runs (one thread; sdcflows' own app against `larmorx sdcflows sdcflows`), a script
  `validation/investigations/sdcflows-fieldmap-b/bench_estimation.py`, the machine's load stated (quiet). Add the
  random walker's non-converging case (`relay27/slow_masker_*.py`, 209 s against 23 s at one thread) as a row,
  named a follow-up (relay 27's handoff item 4).
  (e) **Golden + the smoke tier** (relay 27 ran gates-rust and gates-python at e88326b6's code: OK): golden
  coverage of relay 27's new code paths (`py.ndimage.binary`: an int8 mask iterated in both modes, the mask
  afterwards (SCI-31); a `brain_masker` case with `padding=0` both modes (SDC-46); `intra_modal_merge` with `to_ras`,
  `grand_mean_scaling` and a mask (PIPE-40); one case per distinct code path, <= 32^3), then `record_golden.py`
  (delete the changed cases' entries or `--compare`; it runs the parity smoke tier first, which must pass whole:
  larmorx's code changed since its marker of adf07a34; the 13 permanent cases now run in the three suites' smoke
  tier), boxed, in the worktree. The gates again only if relay 28 changes code (`gates-rust.sh` if Rust;
  `gates-python.sh --no-build` with LARMORX_NC_BIN unset, `tables --check`, `check_spdx.py`). The three suites'
  records (relays 25 and 26) were made before the permanent cases existed: no re-record (each record shows what
  ran then), unless the orchestrator wants the permanent cases in them.
- **Next: relay 28 = (d) + (e) (smoke tier + golden)** (the plan above; the orchestrator agrees the numbering).
- **Relay 28 = (d) benchmark + (e) golden and the smoke tier (done, 2026-11-09): commits 2b07a2e7, 667a05e7, 91fb9e73,
  8828f241, 812853ef, a637d51f, e92dc283** (on 9cc48d9d). Agent id: the orchestrator's to add (relay 28). Scratch helpers
  in `<scratch>/r28/` (`slow_traced.py` (the slow case open to gdb: PR_SET_PTRACER_ANY), `samples*.txt` (the 40 gdb
  samples before and after the first change), `probe_case.py`, `probe_nan.py` (where the NaN comes from), `probe_ras.py`,
  `verify_masker.sh` + `cases-*.txt|json` + `verify-*.txt` (the replay; committed as `relay28/replay_masker_cases.py` and
  its results), `bench.sh`, `bench_nan.sh` (the benchmark's parts), `bench-*.json` (its runs; `*.dirty-stamp.json` the
  first NaN rows, made while a doc was being edited: redone), `so-before/` + `py-before/` (relay 27's build, for the
  before row), `golden-wt.sh`, `gates-rust.sh`, `wheel.sh`, `run_check.sh`, `NOTES.before-relay28.md`, `notes-section.md`).
  - **The random walker's slow non-converging case: profiled and fixed (contained, same bits; 2b07a2e7).** Where the time
    went (relay 27's build; perf is not allowed here (perf_event_paranoid 4), so 40 gdb samples one second apart of the
    running call): every sample in the conjugate gradients' NaN slow paths, `matvec2` 23/40, `x86_nan::nan_f32` (SSE's
    NaN rule, `#[cold]`, a call per element) 10/40, the vector updates the rest. The cause: the case's image (values 0 to
    3) has a grey closing of 0 everywhere, so sdcflows' `segtarget = 2 * closed / closed.max() - 1` is 0/0, every weight
    a NaN, the system NaN from its first iteration; SciPy runs to `maxiter` (2 x 162,120) at numpy's speed, larmorx
    recomputed every NaN element out of line. The change (`crates/larmorx-ndimage/src/skimage/random_walker.rs`): (1) the
    slow paths' SSE NaN rules inlined (`Plain::xadd/xsub/xmul`, unit test `the_inline_nan_rules_are_npfloats`: every
    pair of special values = `NpFloat`'s bits), a NaN row sum stops at its first NaN: 209 s -> 95 s; (2) once the
    residual holds a NaN and every element of the solution is one, nothing later can change the solution (SSE's `x + y`
    with a NaN `x` is `x`, already quiet) nor pass the test: larmorx stops with SciPy's solution and `info = maxiter`:
    -> 0.02 s. Verified bit for bit (both D13 modes): the trial's 139 brain_masker cases replayed (every one of the 59
    that timed out or took > 60 s, 40 others per night: all agree; the 8 timeouts of up to 2,592 s now 73 to 199 s,
    sdcflows' own maxiter runs; `relay28/replay_masker_cases.py`, `relay28/replay-*.txt`), the 13 permanent cases
    (both environments), the new golden case against scikit-image. Findings bullet, triage entry (status fixed),
    module documentation updated.
  - **(d) Benchmark** `docs/benchmarks/sdcflows-estimation.md` + `.json` (812853ef; `bench_estimation.py`, quiet
    machine, load 0.9 to 2.0; every file identical): the four memory calls (one thread: brain_masker 8.04 -> 6.93 s,
    IntensityClip 1.40 -> 1.63, RobustAverage 0.20 -> 0.23, ReconstructFieldmap 0.89 -> 0.30; 14 threads 2.47, 0.19,
    0.18, 0.14 s), the NaN random walker (relay 27's build 210.49 s, now 0.053 s, sdcflows 22 s), the BIDS app on five
    of sdcflows' own runs with the originals' programs run fresh (`LARMORX_ORACLE_CACHE_MODE=off`): ds005752 21 -> 4 s,
    ds005454 50 -> 23, ds002422 27 -> 7, ds005040 103 -> 87, ds006736 79 -> 56. Row in docs/benchmarks/README.md.
  - **(e) Golden:** four cases of the new code paths (2b07a2e7): `py.ndimage.binary.int8_mask` (SCI-31, both modes, the
    masks afterwards), `py.skimage.random_walker.nan` (the new stop; float32 both solvers + labels, float64),
    `py.sdcflows.brain_masker.no_padding` (SDC-46 both modes), `py.niworkflows.intra_modal_merge.ras_mask` (PIPE-40
    fixed; reproduce = niworkflows' IndexError message); `relay28/check_golden_paths.py`: their reproduce outputs =
    SciPy's, scikit-image's, sdcflows' and niworkflows' in-process, ALL OK. **Recorded** (e92dc283):
    `record_golden.py --parity-workers 16` in the worktree from a637d51f, boxed 40 GiB, 1,559 s: **the parity smoke tier
    passed whole** (2,692 on 16 workers in 323 s, the serial memory and thread tests 235 in 1,210 s: 2,927, 0 failed;
    marker `oracles/cache/markers/parity-smoke-a637d51f....json`), 303 cases (2,660 outputs: +4 cases, +30 outputs),
    no earlier checksum changed; `pytest tests/golden` 305 passed in the newest environment too. (The first
    `record_golden.py` from 812853ef refused: 2 smoke-tier failures, below; log `fmapb-r28-golden-1-refused.log`.)
  - **The smoke tier's first run (812853ef) found two check-script mistakes, not larmorx's (a637d51f):** (1)
    `relay22/morph_cases.py` (check_ndimage's arguments section, the smoke case `ndimage-quick`) did not know SCI-31
    and did not report the caller's mask afterwards: now both (1,655 cases, reproduce = SciPy with the masks, the
    default 5 SCI-31); (2) `parity.fieldmaps_b_random` ran larmorx's nibabel arithmetic on LAPACK 3.12.1 in the shared
    environment too, where nibabel uses 3.12.0: the permanent case `reorient-rotation-with-two-candidates` failed
    there (relay 27 had noted it as expected); the runner now follows the environment's release for each case (a
    copied context; newest unchanged). Also `relay27/make_permanent.py` rewrote every permanent case from its seed,
    `--check` included, and two seeds draw other calls since e88326b6: relay 28's first `--check` rewrote two case
    files (restored from git before any commit); fixed (91fb9e73): existing cases are never written again.
  - **Gates:** `gates-rust.sh` all three workspaces OK at 812853ef (the Rust of 2b07a2e7; log
    `oracles/logs/fmapb-r28-gates-rust.log`); `gates-python.sh --no-build` at e92dc283 with LARMORX_NC_BIN unset:
    exit 0 (pytest 1,292 passed, 13 skipped; CI's environment 901 passed, 13 skipped; the collection without
    larmorx_validation, ruff check and format (1,048 files)), `tables --check` OK, `check_spdx.py` OK (log
    `oracles/logs/fmapb-r28-gates-python.log`). The Rust changed only in 2b07a2e7 (812853ef's gates cover it); every
    later commit is Python checks, docs and the golden checksums. The worktree is clean.
  - Handoff items: `<scratch>/handoff/relay28.md` (next-steps: drop relay 27's random-walker follow-up (done), the two
    niworkflows steps' one-thread speed, brain_masker's draw weight; golden.md's four cases and the CI note on the NaN
    case; the benchmarks README; tools.toml's benchmark texts; the make_permanent mistake).
- **Relay 28's plan for relay 29 = R11 the site (FOR THE ORCHESTRATOR to agree; then 30 = R12 handoff; split 29 = R11a
  evidence module + exports, 29b = the pages + the site's checks, if it does not fit):** the brief's sub-task 4. A clone
  of `larmorx_documentation` in `<scratch>/site` from the site's `main` and `evidence-blobs` as they are when relay 29
  starts, branch `fmap-b-pages`, the no-reply identity set first (`git config user.email` as the larmorx repository's).
  `docs/figures.md` says how; part A's `validation/src/larmorx_validation/evidence/sdcflows_fieldmap.py` (+ `_run.py`)
  and `validation/investigations/sdcflows-fieldmap/site_checks.sh` (`drawn`) are the model, its page
  `content/tools/sdcflows-fieldmap.md` the style. A new evidence module (`evidence/sdcflows_estimation.py`) and page(s)
  for part B's paths, each with raw inputs, the originals' outputs, larmorx's outputs, the differences and their
  verdicts (`data/verdicts.yml`): the masks (`brain_masker` on ds002603's clipped magnitude: sdcflows' probabilities
  and mask beside larmorx's, identical; `epi_mask` with SCI-13/14's watershed correction where it changes the mask
  (the measured masks of relay 17: better on 3 of 4)), PEPOLAR (fMRIPrep's topup estimator's Python steps on ds005040
  or ds006736: the field and its coefficients = fMRIPrep's own files), SyN (`syn_sdc_workflow` on ds000228-syn: the
  field beside fMRIPrep's own), the direct field map (ds005752), the BIDS app (a table: every derivative file
  identical on the eight runs, the record `sdcflows-estimation-app-shared` and this benchmark's five), the records'
  tables (ndimage-skimage 10/10, niworkflows-helpers 5/5, sdcflows-estimation 19/19, the full tier, app-shared 8/8,
  the originals' step newest vs shared), random testing's trial (312,081 cases, 0 differences; the 17 triage entries),
  the defects of part B (SDC-12..46, PIPE-18..40, SCI-06..32, SKI-01..04, NP-05.., NIB-05..: the register draft
  `<scratch>/handoff/register_part_b.py`) with what larmorx does, the memory peaks (the four calls) and the benchmark
  (`docs/benchmarks/sdcflows-estimation.json`). Real images inside each step's mask, dilated (no face). The site's
  checks: `tools/build.py` 0 problems, number and viewer checks, every view drawn, the inventory after >= before on
  every page, screenshots at 1440 and 390, light and dark. Commit the evidence module and its exports' scripts in the
  larmorx branch; the site's pages on `fmap-b-pages`.
- **Relay 29 = R11a, the site's evidence module and its export (done, 2026-11-09): commits 26df46d1, 213d4d09,
  beb89b31, 2b399803, 682ca1f1** (on e92dc283). Agent id: the orchestrator's to add (relay 29). **The split was used**
  (the orchestrator's 22:57 numbering: 29 = R11a, 30 = R11b the page and the site's checks, 31 = R12 the handoff).
  Scratch helpers in `<scratch>/r29/` (`compute.sh` + `compute_entries.py` (the cache entries, named ones in order,
  boxed), `export.sh` (the export into `<scratch>/site`; give `--site` as an ABSOLUTE path: the script cds into the
  worktree, a relative path lands under `.claude/worktrees/`), `run_check.sh`, `probe_*.py` (shapes, work folders,
  fMRIPrep's derivatives against sdcflows', the catalog's licences), `register-orig.md`, `NOTES.before-relay29.md`).
  - **The evidence module** `validation/src/larmorx_validation/evidence/sdcflows_estimation.py` (TOOLS key
    `sdcflows-estimation`, page `/tools/sdcflows-estimation/`) and its run module `sdcflows_estimation_run.py` (tasks
    `app`: LARMORX's port of the BIDS app at N threads in a mode, through check_app's `run_mine`; `epi-mask` and
    `masker`: sdcflows' or LARMORX's call, `--libraries fmriprep` = numpy 2.2.6 / SciPy 1.15.2). **Cache entries**
    (`oracles/figures-cache/sdcflows-estimation/`, each from a clean commit): `app-ds006736` and `app-ds005752`
    (`check_app.py --sections run` = sdcflows 2.17.1's app at one thread (its programs through the oracle cache) and
    LARMORX's, ALL OK, kept folders; LARMORX at 14 threads in reproduce and in default), `stage-fmriprep`
    (`check_fmap_stage.py --sections fmriprep` on relay 14's five runs, ALL OK: 36 NIfTI = 13 identical, 13 data
    identical with FSL's build string in descrip, 10 after an N4 node), `masker-ds002603` (brain_masker on fMRIPrep's
    `clipper_post` input: sdcflows, LARMORX 1 thread reproduce, 14 default, 1 with fMRIPrep's libraries),
    `epi-mask` (the four BOLD references of measure_watershed_masks.py, sdcflows + both modes), `memory` (the four
    memory calls under /usr/bin/time -v: brain_masker 1,440 vs 455/431 MiB, IntensityClip 144 vs 85/84, RobustAverage
    149 vs 108/108, ReconstructFieldmap 1,440 vs 230/231). The check scripts need a short TMPDIR (sdcflows' app's
    multiprocessing manager: AF_UNIX paths at most 107 bytes): `_env` makes `/tmp/lxse-*`; the provenance drops it.
  - **The JSON** (`<scratch>/site/static/evidence/sdcflows-estimation.json`, exported from 682ca1f1, cache served;
    64 blobs, 4.5 MB (budget 40); the site 360 MB (500); the manifest gains 61 lines and loses none): **examples**
    `app-ds006736` (PEPOLAR: inputs the AP and PA volumes; outputs the field map, the EPI reference, the masker's
    probability, the mask over the input; panels sdcflows 1 thread, LARMORX 1/14 threads reproduce, 14 default,
    fMRIPrep's own files, LARMORX as fMRIPrep ran the stage, sdcflows' own 4-thread run: **lx-sdc bit-identical,
    lx14-lx1 bit-identical, fix-lx14 d13 (AFNI-99, ANTS-40, FSL-201 named: the default's field differs, up to 0.69
    Hz), lxf-fmriprep within-variability (N4 at 8 threads with glibc in fMRIPrep; the field and coefficients
    fMRIPrep's own), sdc4-sdc1 and fmriprep-sdc reference rows**), `app-ds005752` (a direct field map: the same
    rows; the default corrects ANTS-40, the field up to 31.1 Hz apart), `masker-ds002603` (probability, mask, masked
    magnitude: lx-sdc, lxf-fmriprep (fMRIPrep's libraries = fMRIPrep's own files) and lx14-lx1 bit-identical;
    fmriprep-sdc the libraries' step), `epi-mask-ds003592` (SCI-13/SCI-14 corrected by default: sdcflows' mask
    12,843 voxels, Dice 0.464 with fMRIPrep's mask, the corrected one Dice 0.904; reproduce = sdcflows). **Tables**
    `records` (6 records, 57 cases, all pass), `app-runs` (the 8 app runs, newest and shared 19/19 each, the
    benchmark's five times), `stage`, `epi-mask` (4 references), `corrections` (89 entries from the findings' new
    table, the Switch column from `larmorx._defects.defects()`), `step` (303 comparisons, 127 changed, reasons from
    sdcflows_scripts.REASONS/CHANGES), `memory`, `trial` (both trials: 180,542 + the AVX-512-off one) and `triage`
    (17). **speed** (the benchmark JSON: 4 calls at 1/14 threads, 5 app runs, the NaN random walker). **cases** 57
    (all bit-identical; the shared app runs as `shared/app-run-*`), implementation `port` validated. Credits attach
    passed (ds006736, ds005752, ds002603, ds003592: CC0-1.0). No machine path in the JSON.
  - **Findings** `docs/findings/sdcflows-estimation.md`: new section "Part B's register entries at a glance (relay
    29)" (89 rows: ID, original, class, defect, fMRIPrep's calls, LARMORX), generated by `relay29/findings_table.py`
    from `relay17/register_part_b.py`, **now in the repository** (from the scratch draft, lint-fixed and formatted,
    the same output; R12 regenerates the register text from it). README rows for both.
  - **The site clone** `<scratch>/site` (from site main 4d86f086 and evidence-blobs ae1a3b62, 1,343 blobs fetched;
    branch `fmap-b-pages`; the no-reply identity set): commit fb7d9c70 holds the JSON and the manifest (the page and
    the blobs branch: relay 30).
  - Gates at 682ca1f1 (clean; `<scratch>/r29/gates-python.sh`, boxed, log `oracles/logs/fmapb-r29-gates-python.log`):
    `gates-python.sh --no-build` with LARMORX_NC_BIN unset exit 0 (pytest 1,292 passed, 13 skipped; CI's environment
    901 passed, 13 skipped; ruff check and format), `tables --check` OK, `check_spdx.py` OK; no Rust change (no cargo
    gates). The site clone builds with 0 problems (`tools/build.py`, before the page). The worktree is clean. Handoff
    items: `<scratch>/handoff/relay29.md`.
- **Relay 29's plan for relay 30 = R11b, the page and the site's checks (FOR THE ORCHESTRATOR to agree; then 31 =
  R12 handoff):** in `<scratch>/site` (branch `fmap-b-pages`, commit fb7d9c70 on 4d86f086): (1) **before** the
  page: the before inventory of every page (`validation/investigations/cleanroom-bet/site_inventory.py count` on a
  build of 4d86f086 + its blobs: build a separate scratch checkout of 4d86f086, or `git stash` nothing: build from
  `git worktree add <scratch>/site-before 4d86f086` with the fetched blobs copied); (2) the page
  `content/tools/sdcflows-estimation.md` in part A's style (`content/tools/sdcflows-fieldmap.md`; `evidence: true`,
  `wide: true`; facts, what it is, `ev.agreement('sdcflows-estimation', 'port')`, the four examples, `ev.table` of
  records, app-runs, stage, epi-mask, corrections, step, memory, trial, triage, `ev.speed`, `ev.cases`), the tool's
  status `validated`; the navigation, status table, roadmap and summary are shared (`site.yml`, `data/status.yml`,
  `content/validation.md`, `static/evidence/summary.json` through `evidence summary`): write them in the handoff,
  and put only what the build needs in the branch (as part A did; check its merge a8e1b3a4); (3) `tools/build.py` 0
  problems, the number check, the viewer check and `drawn` (copy part A's `validation/investigations/
  sdcflows-fieldmap/site_checks.sh` to `validation/investigations/sdcflows-fieldmap-b/relay30/site_checks.sh` with
  this page's URL), the after inventory (after >= before on every page; every other page's HTML unchanged),
  screenshots 1440 and 390, light and dark (`site_checks.sh shots`); (4) `python tools/blobs.py commit` (the
  evidence-blobs branch: 1,343 + the page's, none replaced or removed) and the page's commit; (5) the gates once
  (`gates-python.sh --no-build` with LARMORX_NC_BIN unset, `tables --check`, `check_spdx.py`; no Rust change).
  Re-export only if a text or view must change (`<scratch>/r29/export.sh sdcflows-estimation --site <absolute site>
  --no-compute`, from a clean commit; never edit tracked files while a cache entry computes: its RUN_INFO is
  stamped at its end, `-dirty` then, and is redone, as relay 29 found).
- **Against relay 28's plan (FOR THE ORCHESTRATOR):** every planned piece is in the JSON except **a SyN viewer**
  (`syn_sdc_workflow` on ds000228-syn beside fMRIPrep's own field): not made. fMRIPrep's own SyN field comes from
  ANTs' registration at 4 threads with glibc, which no LARMORX run reproduces (LARMORX = ANTs at one thread with
  correctly rounded maths, byte for byte), so its comparison would need ANTs' own thread band for the call to carry a
  verdict. Recommendation: on the page, the SyN path through the records' cases (`syn-sdc`, `syn-sdc-workflow`:
  sdcflows' own graph 18/18, ANTs' one-thread warps byte for byte; `syn-preprocessing`, `syn-preprocessing-workflow`)
  and a text paragraph, the viewer with ANTs' thread band a follow-up for the final validation phase (next-steps
  through the handoff). The PEPOLAR example is ds006736 (relay 28: ds005040 or ds006736); brain_masker is ds002603's
  fMRIPrep call (as planned); the direct field map ds005752 (as planned).
- **Relay 30 = R11b, the page, the site's checks and the SyN viewer (done, 2026-11-10): larmorx commits f786069d,
  723df13c** (on 682ca1f1); **site `fmap-b-pages`: ec7f7b7f** (on fb7d9c70); **evidence-blobs (local, not pushed):
  d680cd2a, 1,424 blobs** (1,343 + 81: relay 29's 61 and relay 30's 20; none replaced or removed). Agent id: the
  orchestrator's to add (relay 30). Everything planned fitted in one relay (no split). Scratch helpers in
  `<scratch>/r30/` (`compute.sh` + `compute_entries.py` (the `syn` entry, boxed 64 GiB), `probe_syn.py`,
  `probe_band.py` (the fields' pairwise differences), `compare_html.py` (every built file before/after),
  `gates-python.sh`, `inventory/` (before/after counts of the 26 evidence pages), `shots/` (the screenshots),
  `union/` (git archive of main + the branch for the build's link check), `NOTES.before-relay30.md`).
  - **The SyN viewer (the orchestrator's 23:48 entry), made:** cache entry
    `oracles/figures-cache/sdcflows-estimation/syn-ds000228` from the clean commit f786069d (1,841 s; 2.4 GB; log
    `oracles/logs/fmapb-r30-syn-entry.log`): six runs side by side (LARMORX 14 threads reproduce, LARMORX 4 threads
    default, ANTs 2.6.5 at one thread with CORE-MATH preloaded, at one thread with glibc, twice at four threads with
    glibc (`never`: the original's own spread; antsRegistration above one thread is never cached anyway), every ANTs
    warp followed by sdcflows' steps (LARMORX's port, reproduce), then the steps on fMRIPrep's own warp (= its own
    files, field, corrected reference and coefficients, byte for byte) and the warps compared pairwise. Run module
    tasks `syn`, `syn-warps`; `LARMORX_PARITY_RUN_TIMEOUT=14400` (the one-thread runs took 1,802 and 1,808 s, past the
    default 1,800 s limit). **Results:** LARMORX = ANTs one thread correctly rounded: warp byte for byte (29,895,327
    values), field, reference, coefficients identical (bit-identical). ANTs' band (glibc): 4 vs 1 thread 1.70 Hz
    (warp 0.835 mm); **4 threads run to run 0.70 Hz (warp 0.659 mm): ANTs at four threads does NOT repeat itself on
    this call**; glibc vs correctly rounded at one thread 0.016 Hz (warp 7.4e-08 mm); the band's largest pair 1.82 Hz
    (one thread vs the second four-thread run). **fMRIPrep's own field: at most 1.80 Hz from LARMORX's (rms 0.136):
    within-variability** (inside the glibc band); against ANTs 2.6.5's first four-thread run 0.70 Hz (fMRIPrep's
    image: ANTs 2.6.2.dev1). LARMORX's default corrects **ITK-31** here (the first SyN stage has a fixed mask only):
    the field moves by up to 0.82 Hz (d13). The handoff item (with a proposed sentence for ants-syn.md, whose "(almost)
    independent of its thread count" does not hold on this call) is in `<scratch>/handoff/relay30.md`.
  - **Export** (from 723df13c, clean; `<scratch>/r29/export.sh sdcflows-estimation --site <abs site> --no-compute`):
    5 examples (app-ds006736, app-ds005752, **syn-ds000228** (8 panels, 7 comparisons, 20 blobs, 2.27 MB), masker,
    epi-mask), 9 tables, 57 cases, speed; 84 blobs, 6.83 MB (budget 40); the site 376.8 MB (500). The JSON diff
    against relay 29's: the SyN example added, the exporter hash, and the records table's six links fixed
    (`record:<name>`, which the site cannot resolve: the build reported them, → `repo:docs/validation/<name>.md`);
    blobs.sha256 +20 lines, none removed.
  - **The page** `content/tools/sdcflows-estimation.md` (part A's style; `evidence: true`, `wide: true`): facts
    (status validated, the six records, findings, benchmark, five API pages), what it is (the three estimators and
    what surrounds them, a code example checked against the API), the verdicts at a glance + the records table, the
    BIDS app (PEPOLAR ds006736, direct ds005752, the eight app runs, fMRIPrep's derivatives), **the fieldmap-less
    (SyN) estimation** (prose with the band's numbers + the example), the masks (brain_masker, epi_mask + its
    table), corrections (89 entries: 35 SDC, 23 PIPE, 27 SCI, 4 SKI; 42 rule A, 15 rule B; 61 with the switch; 19
    reach fMRIPrep's calls), the step, speed, memory, random testing (trial, triage), every case, known limits
    (3dQwarp's workflow, SDC-23). The shared files (site.yml's navigation, data/status.yml, roadmap, summary.json)
    are NOT in the branch (as part A): their exact text is in `<scratch>/handoff/relay30.md`.
  - **The site's checks:** `tools/build.py` 0 problems (without `--main-repo`, and with `--main-repo` the union of
    main and the branch: 185 links into the main repository checked; against the worktree alone, the five links of
    t2smap's page into main's files fail: the branch predates them). Every other page's built HTML byte-identical
    (84 files identical; sitemap.xml adds one URL; the page and its JSON added). `site_inventory.py compare` on the
    26 evidence pages: 0 losses (every count identical; the new page: 5 examples, 9+9+8+6+5 panels, 57 cases).
    Number check **527 of 527** agree; viewer check **76** values, none failing; `drawn`: all **5** viewers drawn
    (no default-size canvas). Screenshots 1440 and 390, light and dark, of the top and 18 components
    (`<scratch>/r30/shots/`, 76 photographs; looked at: the top, the agreement chart, the app runs' table, the PEPOLAR example at 390 dark, the SyN example at 1440 light and 390 dark, the speed chart; at 390 a tall element repeats its top below its end, the capture beyond the viewport as part A found, not the page). The checks ran again after the last two prose fixes (the app runs' datasets, the random walker's sentence): build 0 problems, HTML and inventory as above, numbers 527/527, viewers 76, drawn 5/5. The page's served build (`tools/build.py --serve`) must not run beside a plain build of the same clone (they write the same `_site`: 1,625 problems once, gone when run alone). `<scratch>/site-before` is a `git worktree` of the site clone at 4d86f086 (the before build); leave it or remove it with `git worktree remove` (a build checkout)). `python3 tools/blobs.py verify`: 1,424 blobs, 0 problems.
  - **Gates** at 723df13c (clean; `<scratch>/r30/gates-python.sh`, boxed, log
    `oracles/logs/fmapb-r30-gates-python.log`): `gates-python.sh --no-build` with LARMORX_NC_BIN unset exit 0
    (pytest 1,292 passed, 13 skipped; CI's environment 901 passed, 13 skipped; ruff check and format, 1,052 files),
    `tables --check` OK, `check_spdx.py` OK. No Rust change (no cargo gates). The worktree is clean.
  - Handoff items: `<scratch>/handoff/relay30.md` (the site's shared files' exact text: site.yml, data/status.yml
    implemented + planned, roadmap row and Completed line, summary.json at the merge; the SyN viewer's results and
    the ants-syn.md sentence; follow-ups).
- **Relay 30's plan for relay 31 = R12, the handoff and the part's final report (FOR THE ORCHESTRATOR to agree):** the
  brief's sub-task 5. (1) The register text regenerated from `validation/investigations/sdcflows-fieldmap-b/relay17/
  register_part_b.py` (relay 29's note: the scratch copy is stale); the `larmorx_core::defects` entries are in the
  branch already (relay 17). (2) `<scratch>/HANDOFF.toml` (`scripts/orchestration/apply_handoff.py --check`, with
  `--site <scratch>/site`, against a trial merge with main as it is then, in scratch worktrees only) and
  `<scratch>/HANDOFF.md`, gathering `<scratch>/handoff/relay17.md` .. `relay30.md` (each relay's items: the register
  and its legend, `upstream.tsv` (the Pins section's rows), `validation/tools.toml`'s entries (`tables`),
  `docs/validation/golden.md` (the part's golden cases), `docs/licensing.md`, next-steps (part B done, every follow-up
  named in the handoff files), README, the decision log's lines the orchestrator named, the site's shared files of
  relay30.md), with the expected conflicts (`validation/src/larmorx_validation/stages/catalogue.py` (d1491604),
  `crates/larmorx-transform/src/numpy_linalg.rs` dbdsqr_upper (t2smap's `_ncc` + part B's `clear_negative_zero`),
  shared registries). (3) The upstream drafts in `docs/upstream-reports/` ("draft, not yet filed"): PIPE-32, SCI-21,
  SCI-25, SCI-29, SDC-45, PIPE-36, PIPE-37. (4) The part's agents' ids in NOTES (the audit archives their
  transcripts). (5) The part's final report as the final message (the brief's list). No heavy run is needed; if
  the handoff's checks need a trial merge's build, boxed.
- **Relay 31 = R12a, the register text, HANDOFF.toml and HANDOFF.md (done, 2026-11-10): larmorx commits 2a87704a,
  c7808942, 47c0a5c7** (on 723df13c); **site `fmap-b-pages`: c1904f8e** (on ec7f7b7f); evidence-blobs unchanged
  (d680cd2a, 1,424). Agent id: the orchestrator's to add (relay 31). The orchestrator's split (00:51) taken: this relay
  = the register text + HANDOFF.toml/HANDOFF.md; **relay 32 = the upstream drafts + the final report** (plan below).
  Scratch helpers in `<scratch>/r31/` (`py/resolve.py` (the trial merge's conflicts), `check_merge.sh` (cargo check of
  the three workspaces), `py/make_handoff.py` + `handoff_docs.py` + `handoff_docs2.py` (the generator: every `old` and
  `after` checked against the trial merge and the site), `py/counts.py` (the register's counts after the handoff),
  `site_build_compare.sh`, `site-check/` (builds before and after), `shared-preview.diff`, `site-shared-preview.diff`,
  `gates-python.sh`, `findings_table.md`, `NOTES.before-relay31.md`).
  - **Branch fixes found while writing the handoff:** the register draft (`relay17/register_part_b.py`): PIPE-39's
    class `n` -> `b` (the register's class for nondeterminism; `register_counts.py` would count it under none) and
    SCI-20's test `max(abs(a)) · max(abs(1/a))` (its `|` split the table's columns); SCI-21's evidence gained relay
    24's measurement; the findings' table regenerated (two cells), the page re-exported (only those two cells: build 0
    problems, every other built file identical, the page's inventory the same). Licences: scikit-image's
    `filters/thresholding.py` is BSD-2-Clause with its own holders (`LICENSE.txt`): `otsu.rs` and
    `python/larmorx/skimage.py` now `Apache-2.0 AND BSD-2-Clause AND BSD-3-Clause` with the notice; scikit-image's
    copyright line as its LICENSE states it (`skimage/mod.rs`, PROVENANCE); `niworkflows/header.py` (transforms3d's
    `decompose44`) now `Apache-2.0 AND BSD-2-Clause` with its notice. `defects.rs`' `SDC_01 ="SDC-01"` spacing
    restored. `docs/findings/ants-syn.md`'s thread paragraph scoped (the orchestrator's 00:51 entry). The
    Validation page's summary table (`evidence/summary.py` PAGES) gains the page (port, validated, 57 cases).
  - **The trial merge** `<scratch>/r31/merge` (main d6d3e8e7 + the branch; 14 conflicting files, resolved: HANDOFF.md
    lists each; uncommitted) and `<scratch>/r31/site-merge` (the site clone at c1904f8e): `cargo check --workspace
    --all-targets --locked --offline` of root, crates-gpl, crates-nc pass (root lock +11 lines by `cargo metadata
    --offline`); `apply_handoff.py --check` then for real: **89 operations, 19 files**; `tables --site` (README row,
    the index's rows, the site's status row), `tables --check` OK; `register_counts.py` **541** ((a) 119, (b) 17,
    (c) 270, (d) 27, (e) 79, (f) 27; questionable 14; no decide, no yes; one rule 8); `evidence summary` adds the page;
    site build 0 problems (185 links into the trial merge); before/after: every page only gains (nav 7 fragments;
    home: the status row, the planned text without part B; roadmap: row 1 removed, renumbered, a completed row).
  - **Gates** at 47c0a5c7 (clean; `<scratch>/r31/gates-python.sh`, boxed; log `oracles/logs/fmapb-r31-gates-python.log`):
    `gates-python.sh --no-build` (LARMORX_NC_BIN unset) exit 0 (pytest 1,292 passed, 13 skipped; CI's environment
    901 passed, 13 skipped; ruff check and format, 1,052 files), `tables --check` OK, `check_spdx.py` OK; `cargo fmt
    --all --check` (root) OK and `cargo clippy -p larmorx-ndimage --all-targets --locked -D warnings` OK (the Rust
    changes are doc comments and one space in a macro: no cargo test re-run).
- **Relay 31's plan for relay 32 = R12b, the upstream drafts and the part's final report (FOR THE ORCHESTRATOR):**
  (1) the drafts in `docs/upstream-reports/` (the README's format and index there; each "draft, not yet filed"; no
  account name or machine path: `<workspace>`, `<scratch>`, `~`): **PIPE-32** (fMRIPrep: two participants sharing a
  `B0FieldIdentifier` stop a multi-participant run), **PIPE-36** (fMRIPrep: `extract_entities` parses absolute BOLD
  paths), **SDC-45** (sdcflows: `FieldmapFile` parses the absolute path; parse `relative_to_root(path)` as niworkflows'
  DerivativesDataSink does), **PIPE-37** (pybids: patterns matched against the absolute path; match below the root or
  anchor the derivatives patterns), **SCI-21** (SciPy: the 1-D running rank writes the input's type into an output of
  another type, past its end: memory safety first), **SCI-25** (SciPy: an iterated binary erosion into a float output
  erodes outside the mask), **SCI-29** (SciPy: `label` into a bool output writes the labels' bytes); each with the
  smallest reproducer (from the findings and the probes named in the register draft's evidence), SciPy 1.18.1's and
  1.15.3's behaviour where SciPy's, the expected result, and the larmorx entry; commit; (2) if main has moved, re-run
  `apply_handoff.py --check` against a fresh trial merge (`<scratch>/r31/py/make_handoff.py` regenerates
  HANDOFF.toml; the counts pass with `py/counts.py`), and put the branch's final head and commit count into
  HANDOFF.md's first section; (3) the part's final report as the final message (the brief's list: the port, each
  record, fMRIPrep's real calls, the step, defects by rule, golden, memory, the random-testing trial, speed, what
  was run, gates, commits, the site's checks, the handoff files, open questions, the agents' ids, the merge notes:
  numpy_linalg.rs dbdsqr_upper, catalogue.py, the golden NaN case on the other platforms, the site's shared files,
  the ndimage duplicate). No heavy run needed.
- ~~Next: relay 32 = R12b~~ done (below).
- **Relay 32 = R12b, the upstream drafts and the part's final report (done, 2026-11-10): larmorx commit
  `5385a9f9`** (on 47c0a5c7; **the branch's final head, 139 commits** from f5e89ff4); site and evidence-blobs
  unchanged (c1904f8e; d680cd2a, 1,424). Agent id a5c1cdfa1e9ad900e. Scratch helpers in `<scratch>/r32/`
  (`py/` = relay 31's generator with relay 32's texts and two resolver rules, `merge` and `site-merge` = the
  fresh trial merges, `pybids-master/` = a copy of pybids' master `src/` at 47eb9fb, `gates-python.sh`,
  `HANDOFF.relay31.{md,toml}`, `NOTES.before-relay32.md`, `counts.txt`, `site-build.log`).
  - **The seven drafts** (`docs/upstream-reports/`, index rows in its README): each project's issues searched
    first (GitHub, 2026-11-10). PIPE-32 (+ SDC-36) = a **comment** for the open sdcflows#455 (fMRIPrep#3354 the
    same failure); SDC-45, PIPE-36, SCI-21 (+ SCI-22, SCI-23), SCI-25 = new issues; PIPE-37 = **nothing to
    file** (pybids#1270, fixed on pybids' master by #1273, 2026-08-24, not released); SCI-29 = a **comment**
    for the open scipy#24502 (PR #24579 closed unmerged). Every reproducer in the repository
    (`validation/investigations/sdcflows-fieldmap-b/relay32/`, README row), run boxed in both environments
    and, for the four pybids paths, with pybids' master (`run_reproducers.py`, `outcomes-*.txt`).
  - **Found:** with pybids' fix, SDC-45 and PIPE-36 turn silent below a folder named as a datatype (`motion`:
    sdcflows' fieldmap-less estimator with an empty IntendedFor; fMRIPrep's get_sbrefs no reference); `lx.bids`'
    default index = pybids master's on the four folders (`pipe37_against_pybids_master.py`: ALL OK); PIPE-32's
    side effect: a participant's automatic name depends on the participants before it (sub-02 auto_00001 vs
    auto_00000 alone; it names the derivatives); SCI-26 fixed on SciPy's main by #25646 (milestone 2.0.0);
    SCI-21's 5-element case: 1 to 4 of 10 processes per type and release end abnormally (aborts after or
    before printing, numpy's "array is too big" in the next allocation), up to 7 of 10 with glibc's checking
    allocator. Findings: `sdcflows-estimation.md` "The upstream reports (relay 32)", `scipy-ndimage-filters.md`
    a paragraph. The register draft's sections carry each entry's upstream status (`UPSTREAM`; the rows and
    the findings' table unchanged: no re-export).
  - **The handoff regenerated** on a fresh trial merge (`<scratch>/r32/merge`: main d6d3e8e7 + 5385a9f9):
    **15 conflicts, not 14** (relay 31 copied branch files in, which hid `evidence/summary.py`'s PAGES
    conflict: both tuples, main's t2smap first; relay 31's trial summary had dropped t2smap's Validation row);
    `defects.rs`' hunk 1 no longer holds SDC_01 (rule updated). HANDOFF.toml: 89 operations, 19 files; only
    the intended texts changed (the D13 paragraph's upstream sentence, 11 register sections' evidence,
    next-steps: 32 relays, the upstream-reports bullet rewritten, a licence-header follow-up for `check_spdx.py`
    as the orchestrator asked at 01:26). `--check` then for real, `tables --site` + `--check` exit 0,
    `register_counts.py` 541 (unchanged), `evidence summary` 25 tools with t2smap kept, site build 0 problems
    (185 links), 56 identical / 31 changed / none removed against relay 31's before build. HANDOFF.md updated
    (final head and count, the trial merges, 15 conflicts, the agents' list from NOTES, the upstream reports'
    section; the reports' README is edited in the branch: not on CLAUDE.md's shared list).
  - **Gates** at 5385a9f9 (clean; boxed; log `oracles/logs/fmapb-r32-gates-python.log`): `gates-python.sh
    --no-build` (LARMORX_NC_BIN unset) exit 0 (pytest 1,292 passed, 13 skipped; CI's environment 901 passed,
    13 skipped; ruff check and format 1,063 files), `tables --check` OK, `check_spdx.py` OK. No Rust change.
- **The part is complete** (relay 32's final report is the part's final report; nothing runs; the four trial
  worktrees are left for the orchestrator's review, removable with `git worktree remove --force`).
- **R10 (records), notes from relay 23:** the three suites' records from a clean commit in newest with the step
  (`python -m larmorx_validation parity <suite> --tier standard --jobs N --out docs/validation`), boxed; the workflow
  suite's heavy cases run one at a time or two (`--jobs 2`, a 32 GiB box: the cases share it); the step's shared
  passes run only the step cases (seconds to minutes). Then tools.toml's `[[record]]` entries (handoff) and
  `tables`.
- **Relay 23 = R9c(c), the D12 parity suites: design notes from relay 21's reading (the next relays' time saved):**
  part A's `parity/sdcflows_fieldmap.py` runs its scripts in-process and reads `OK|FAIL name: detail` lines with
  `stepdigest.oracle` digests; **part B's scripts differ**: they print Tally summaries (`<name>: <N> cases, identical <K>`
  with `  (first: ...)` or a next line `   first difference: ...`; `check_scipy_inv.py`: `<family>: N cases, K identical,
  rcond a/b identical, ..., X refused, Y differ` and `singular ...: same`; `check_estimators.py`: `== <section>: N identical, M
  differ` with `DIFF <name>:` blocks), `INFO`/`EXPECTED` lines, and `ALL OK` or `FAIL <n>`/`DIFFERENCES: <n>`; they parse
  `sys.argv` (argparse: `--sections`, `--scratch`, `--runs`, `--datasets`, `--threads`), put stand-ins on `PATH`, set
  environment variables and logging levels, and `check_regrid.py` re-executes itself (`os.execv`, for
  `NPY_DISABLE_CPU_FEATURES`): **run each case as a subprocess** of the environment's interpreter (`python -P <script>
  <args>`, `PYTHONPATH`, `LARMORX_WORKSPACE`, `OPENBLAS_CORETYPE=Haswell`, `OPENBLAS_NUM_THREADS=1`, `LARMORX_NC_BIN` where
  needed, a scratch folder per case), one check per summary line (`identical == cases`; INFO lines as reported values;
  EXPECTED lines as expected), the exit status a check. Only four scripts import `stepdigest` (for `numpy_lapack` alone);
  no part B script calls `stepdigest.oracle`. **The originals' step**: proposed for the library scripts only
  (`check_ndimage`, `check_numpy`, `check_skimage`, `check_scipy_inv`, `check_regrid`, `check_epi_mask`,
  `check_match_histogram`: their compare helpers (`compare`, `_outcome`, `both`, `both_w`, `outcome`, the Tallies) are
  central, so `oracle()` on SciPy's/numpy's side there plus a per-name digest in `Tally.add`, printed as `STEP <name>
  <hex>` lines when an environment variable asks), the workflow scripts without one (their originals, sdcflows 2.17.1,
  niworkflows 1.15.4, nipype, are the same in both environments; the libraries' step under them is the library suites'):
  a decision for the orchestrator. Tiers: smoke = the seconds-scale scripts or sections (`check_qwarp_steps`,
  `check_nipype`, `check_numpy` without `--expf`, `check_ndimage --quick`, `check_scipy_inv --n 2000`,
  `check_match_histogram`, `check_merge --sections synth,errors,ras,gms`, `check_epi_mask --cases` small, `check_app
  --sections parser,toml,settings`), standard = minutes, long = the workflow runs (`check_pepolar` wf, `check_estimators`
  full, `check_app` run, `check_syn_*` workflow, `check_fmap_stage` run/fmriprep, `check_derivatives` preproc); every heavy
  case boxed as its README row says.

## Pins (tags under `<workspace>/tags/`, sdists from PyPI, SHA-256 checked against PyPI's JSON; fetched by relay 1)
| Tag dir | What | SHA-256 of the sdist |
|---|---|---|
| niworkflows-1.15.4 (+ -download) | newest niworkflows (PyPI 2026-10-26), the port's source | 58e514502c4ccacd25557bca305e427e1b137defbd0de3974e9407bd9be07dd3 |
| fmriprep-25.2.7 (+ -download) | newest fMRIPrep (2026-10-15; pins sdcflows ~=2.15.2, niworkflows <1.16) | 739fd88af47ff62c2677e789fd85764f44538b7210797ce52ebf95737c189c3e |
| sdcflows-2.15.2 (+ -download) | the 2.15 line's newest (2026-10-02, AFTER 2.17.1 of 2026-09-29): wrangler fixes 2.17.1 lacks | 7f7c6c4abd01dce37a34dff1217820e07b32df34630c5e3331e8da2d8b1775b3 |
| scikit-image-0.26.0 (+ -download) | newest scikit-image (both kept envs) | f5f970ab04efad85c24714321fcc91613fcb64ef2a892a13167df2f3e59199fa |
| scikit-image-0.25.2 (+ -download) | fMRIPrep 25.2.6's scikit-image | e5a37e6cd4d0c018a7a55b9d601357e3382826d3888c10d0213fc63bff977dde |
| nipype-1.12.0 (+ -download) | newest nipype = every environment's (the wrappers' command lines, `fname_presuffix`, `is_outlier`) | 784098a8a54839836fa0eba2fa1c2040a365a272d0b28692578b5b15b475aeeb |
| pybids-0.22.0 (+ -download) | newest pybids = both kept envs' (sources under `src/bids`) | 98e7957e47c08d3c131de6f6cef629d009abce888849102b33820ca6a9930152 |
| toml-0.10.2 (+ -download) | toml's `dumps` (sdcflows' settings file; relay 16, `toml_dumps`; `encoder.py` = both kept environments') | b3bda1d108d5dd99f4a20d24d9c348e91c4db7ab1b749200bded2f839ccbe68f |
Already there (part A and earlier): sdcflows-2.17.1, sdcflows-2.15.1, scikit-learn-1.9.1/1.6.1, scipy-1.18.1,
scipy-1.15.3, numpy-2.5.4/2.3.5, OpenBLAS-0.3.30/0.3.34, nitransforms-25.1.0.
Newest on PyPI checked 2026-11-08: sdcflows 2.17.1, niworkflows 1.15.4, fMRIPrep 25.2.7, nipype 1.12.0,
pybids 0.22.0, scikit-image 0.26.0, nilearn 0.14.1.
`upstream.tsv` rows through the handoff, format of rows 59-60:
`niworkflows-1.15.4  https://files.pythonhosted.org/packages/34/70/5d9c503727336498aab22c238f9a3d9af549ca039c82c1f3e6054d251faf/niworkflows-1.15.4.tar.gz  sha256:58e51450...7dd3  Apache-2.0  tags`;
fmriprep-25.2.7 (`.../81/92/8be5e572eb354b587c901a5f17ea6dc57360a29338cb9afc4475c08247ca/fmriprep-25.2.7.tar.gz`, Apache-2.0),
sdcflows-2.15.2 (`.../0d/14/1822d7a825db1afaa02ec27dc44181204b282a2b5330f9cdb9bb6ede85d0/sdcflows-2.15.2.tar.gz`, Apache-2.0),
scikit-image-0.26.0 (`.../a1/b4/2528bb43c67d48053a7a649a9666432dc307d66ba02e3a6d5c40f46655df/scikit_image-0.26.0.tar.gz`, BSD-3-Clause),
scikit-image-0.25.2 (`.../c7/a8/3c0f256012b93dd2cb6fda9245e9f4bff7dc0486880b248005f15ea2255e/scikit_image-0.25.2.tar.gz`, BSD-3-Clause),
nipype-1.12.0 (`.../e2/b0/430436448e7f30f0cb992f9b0a14c7e38ab16267f49494d4fb549c6a1601/nipype-1.12.0.tar.gz`, Apache-2.0),
pybids-0.22.0 (`.../ba/61/48e48fef36afbab3635bc4f7341641c4ed4f83bcdc3dfb484cc90a63b750/pybids-0.22.0.tar.gz`, MIT).
fMRIPrep 25.2.6's own sources (fmriprep, niworkflows 1.15.2, smriprep, sdcflows 2.15.1, scikit-image 0.25.2,
pybids 0.20.0, nipype's ants/fsl-epi/afni-preprocess interfaces, nipype's confounds.py) were copied out of the
image into `<scratch>/image-src/` (read-only reference; not pins).

### Environments (library versions on part B's paths)
| env | sdcflows | niworkflows | nipype | skimage | pybids | nibabel | SciPy | numpy | Python |
|---|---|---|---|---|---|---|---|---|---|
| newest `oracles/python-newest` | 2.17.1 | **1.15.3** | 1.12.0 | 0.26.0 | 0.22.0 | 5.4.2 | 1.18.1 | 2.5.4 | 3.12.16 |
| shared `.venv` | 2.17.1 | 1.15.3 | 1.12.0 | 0.26.0 | 0.22.0 | 5.4.2 | 1.15.3 | 2.3.5 | 3.12.16 |
| fMRIPrep 25.2.6 image | 2.15.1 | 1.15.2 | 1.12.0 | 0.25.2 | 0.20.0 | 5.3.2 | 1.15.2 | 2.2.6 | 3.12.11 |
Both kept envs have fmriprep 26.0.0.dev1+g21a490fb8 (the workspace clone), nilearn 0.14.1 (image 0.11.1),
templateflow 25.1.2. The newest env's niworkflows (1.15.3) is not the newest (1.15.4): see D8.
The fMRIPrep image has no `3dQwarp`, `3drefit`, `3dresample` (so fMRIPrep cannot run sdcflows' 3dQwarp
workflow at all); it has `mri_robust_template`, `antsRegistration`, `topup`, `3dvolreg`, `N4BiasFieldCorrection`,
`DenoiseImage`, `antsApplyTransforms`.

### The steps between versions on part B's paths (diffs read, `<scratch>/scripts/diff_skimage.sh`, `extract_pybids.sh`)
- **sdcflows 2.15.1 -> 2.17.1:** `workflows/fit/pepolar.py` (**large**: 2.15 averages each run with
  `RobustAverage` (`runwise_avg`, 3dvolreg), regrids the averages, pads an odd slice count (`PadSlices`),
  averages the merged set again with head motion (`setwise_avg`), runs topup with sdcflows' own
  `b02b0.cnf` (estmov=0, 7 levels; not FSL's file); 2.17.1 keeps at most 3 volumes per PE direction
  (`SelectPEVolumes`, new), regrids them, no padding, no averaging, picks FSL's `b02b0_4/_2/_1.cnf` by the
  grid's divisibility (`_select_topup_config`, new; `_quick` variants with sloppy), `topup_config` and
  `max_vols_per_pe` arguments, `fix_coeff`'s reference = the regridded first volume, outputs `xfms`,
  `out_warps`, `jacobians`, `topup_config`; debug: `ApplyCoeffsField` with `_topup_mats_to_world`'s
  matrices); `interfaces/epi.py` (`SelectPEVolumes` added); `interfaces/utils.py` (`UniformGrid`'s
  warning when it resamples a differently rotated grid; log only); `fieldmaps.py` (`FieldmapFile` accepts a
  broken symlink; `get_workflow` raises `FileNotFoundError` for MAPPED/PHASEDIFF sources that are not
  files); `viz/utils.py` (colorbar off: reports); new `data/flirtsch/*_quick.cnf`.
- **sdcflows 2.15.1 -> 2.15.2 (newer than 2.17.1!):** `utils/wrangler.py` only: `bids_filters` no longer
  applied to the anatomical reference and targets of fieldmap-less estimation (except `session`); a
  sessionless T1w fallback; `base_entities.get('acquisitions')` (a typo: the filter never applied) ->
  `'acquisition'`/`'ceagent'` with `listify`. **2.17.1 lacks these** (SDC-c below; D2).
- **niworkflows 1.15.2 -> 1.15.4:** `func/util.py` (`init_enhance_and_skullstrip_bold_wf`'s `init_aff`
  antsAI `Affine` -> `Rigid`: the BOLD reference, see D6) and `interfaces/confounds.py` only. Every
  interface part B ports is identical.
- **fMRIPrep 25.2.6 -> 25.2.7:** `interfaces/confounds.py` only. No step on the field-map paths.
- **scikit-image 0.25.2 -> 0.26.0:** no arithmetic change on part B's paths (`ball`, `threshold_otsu`,
  `random_walker` identical; docstrings, a `dilation` in the footprint decomposition path not reached).
- **pybids 0.20.0 -> 0.22.0:** `layout/index.py` (34 lines), `layout/models.py` (216), `layout/layout.py`;
  `config/bids.json` and `layout/utils.py` identical. To check case by case on the oracle datasets (the
  estimators and their ids are in each run's `fmriprep.log`).
- **SciPy 1.15 -> 1.18:** `ndimage` C sources for filters, morphology, measurements, labels identical;
  Python wrappers changed (new `vectorized_filter`, `axes`, contiguity) without changing these paths' values
  (measured: identical intermediates). **The sparse matrices changed** (measured, below: the random walker's
  Laplacian's column order within rows).

## Measurements made by relay 1 (scripts in `<scratch>/scripts/`)
1. **numpy's transcendental functions and the CPU** (`probe_numpy_exp.sh`): float32 `exp`/`log` give the same
   bits under AVX-512F and AVX2/FMA3 dispatch in numpy 2.2.6, 2.3.5 and 2.5.4 (numpy's own SIMD algorithm:
   38 % of 3 M float32 `exp` results differ from the correctly rounded value by 1-2 ulp, `log` 5 % by up to
   3 ulp: `probe_exp32_vs_cr.py`). float64 `exp`/`log` **differ** between AVX-512F and the AVX2 path
   (numpy's AVX-512F kernel vs libm; `NPY_DISABLE_CPU_FEATURES` changes them in 2.2.6/2.3.5; numpy 2.5.4
   ignored the variable: the dispatch stayed AVX-512F). fMRIPrep's oracle runs ran on the old laptop (AVX2:
   float64 exp = glibc's). Part B's random walker uses float32 `exp` (data float32): the port needs a
   translation of **numpy's float32 exp** (BSD-3; `numpy/_core/src/umath/loops_exponent_log.dispatch.c.src`),
   same bits on every platform with `f32::mul_add`.
2. **brain_masker, fMRIPrep's own inputs** (`probe_brain_masker.sh`, boxed 12 GiB): in fMRIPrep's image on the
   main machine (`OPENBLAS_CORETYPE=Haswell`) the mask, probseg and masked brain are **fMRIPrep's own files
   bit for bit** (ds002603 128^3, ds005454 112x112x70, ds005040's PEPOLAR reference); the shared env too
   (ds005040). The **newest env differs**: probseg in 90 % (ds002603), 85 % (ds005454), 74 % (ds005040) of
   voxels; the binary mask in 0, 0 and 1 voxel (ds005040). Times (one thread): 8.2 s / 3.1 s / 0.6 s newest,
   8.7 / 3.7 / 1.3 s image; peak RSS 1,438 / 710 / 217 MiB (image 1,646 / 794 / 256).
3. **Where the newest env's step comes from** (`probe_masker_steps.sh`, ds005040): every intermediate
   identical in both kept envs (percentile, grey closing, median filter, Otsu, binary morphology, labels,
   markers, `segtarget`, the edge weights, the Laplacian's `indptr`, the right-hand side, the diagonal)
   **except the Laplacian's `data` and `indices`**: SciPy 1.18's CSR (fancy row/column indexing or
   `setdiag`) orders the columns within a row differently from 1.15, so CG's matvec sums in another order
   and the probabilities differ. The port needs both orders (a SciPy 1.15 / 1.18 switch, as part A's
   LAPACK 3.12.0 / 3.12.1), the record shows the step per case.
4. **sdcflows' topup configurations and FSL** (`cmp_cnf.sh`): `data/flirtsch/b02b0_1.cnf`, `b02b0_2.cnf`,
   `b02b0_4.cnf` are **byte-identical to FSL's own files** (`$FSLDIR/etc/flirtsch/`, FSL Licence: comments
   included), redistributed in sdcflows' Apache sdist; `b02b0.cnf` (2.15's) and the `_quick` files are
   sdcflows' own. The main package must not ship FSL's files (D9).
5. **The workflows build** (`probe_qwarp_wf.sh`, newest env, no execution): `init_topup_wf` (20 nodes; debug
   21), `init_syn_sdc_wf` (25), `init_syn_preprocessing_wf` (28), `init_fmap_wf` phasediff (20) and mapped
   (16), and `init_3dQwarp_wf` (42) all build. `init_3dQwarp_wf` reads `inputnode.in_reference`, a field its
   `inputnode` does not have (fields `in_data`, `metadata`): its `fmap_ref` can never be produced (SDC-e).
Also read: nibabel 5.4.2's `affines.obliquity` returns **radians**; sdcflows' `OBLIQUE_THRESHOLD_DEG = 0.5`
is compared with it (so `Deoblique`/`Reoblique` act only beyond 28.6 degrees; SDC-f).

## Inventory (sdcflows 2.17.1 unless said; every option, message, failure case; P = fMRIPrep reaches it)
### sdcflows/utils/epimanip.py
- `get_trt(in_meta, in_file=None, *, use_estimate=False, fallback=None)` (P): `TotalReadoutTime` present ->
  returned as is (falsy -> `ValueError("'<value>'")`); `use_estimate` + `EstimatedTotalReadoutTime` (same
  check); else if the metadata has any of `EffectiveEchoSpacing`, `EstimatedEffectiveEchoSpacing`,
  `EchoSpacing`, `WaterFatShift` beyond `PhaseEncodingDirection` (`keys & {...} > {'PhaseEncodingDirection'}`,
  a proper-superset test: PhaseEncodingDirection AND one of the others, else straight to the fallback):
  `npe = nb.load(in_file).shape['ijk'.index(pe[0])]` (in_file None -> nibabel error);
  EES truthy -> `ees * (npe - 1)`; `use_estimate` + EstimatedEES -> same; `EchoSpacing` and
  `ParallelReductionFactorInPlane` -> `es * (npe // acc - 1)` (float acc: floor division in float);
  Philips `WaterFatShift` + `EPIFactor`: `wfs_hz = ImagingFrequency*3.39941 or MagneticFieldStrength*144.7383333
  or None`; `ees = wfs / (wfs_hz * (epifactor + 1))`, `* (npe - 1)`; then `fallback` if truthy (0.0 is not);
  else `ValueError('Unknown total-readout time specification')`. Doctests give the five cases (0.05251).
- `epi_mask(in_file, out_file=None)`: unused by any workflow (public): grey opening ball(3), closing ball(2),
  percentiles 30/90, `center_of_mass`, grey erosion ball(5), `watershed_ift`, binary dilation ball(2), uint8.
### sdcflows/interfaces/epi.py
- `GetReadoutTime` (P): in_file (optional), metadata (mandatory), use_estimate False, fallback (Float; `or
  None`: 0.0 -> None) -> readout_time (get_trt), pe_direction (metadata's; KeyError if absent), pe_dir_fsl
  (`i->x, j->y, k->z` by str.replace).
- `SelectPEVolumes` (2.17.1 only): in_data, pe_dirs_fsl, readout_times (zip strict: length mismatch ->
  ValueError), max_vols_per_pe 3: per file in order, skip when the direction's budget is full; squeeze;
  4D -> `four_to_three`; volumes `[:remaining]` written `fname_presuffix(in_file, suffix=f'_vol{i}', cwd)`
  (a 3D file too: `<name>_vol0`); counts `min(remaining, len(volumes))`.
- `SortPEBlips` (P): blips `+x`/`-x` (`len(pe)==1`), `sorted(zip(blips, readout_times, in_data))` (ties by
  readout time then path string), then sign back last and BIDS letters.
### sdcflows/interfaces/brainmask.py and utils/tools.py
- `BrainExtraction` (P) -> `brain_masker(in_file, fname_presuffix(in_file, '_mask', cwd), padding=5)`:
  float32 data padded 5 (np.pad constant 0); `data[data < percentile(data[data>0], 15)] = 0` (empty ->
  IndexError); `data[data>0] -= data[data>0].min()`; `datainv = -data; -= min; /= max` (constant -> NaN);
  `grey_closing(datainv, structure=ball(1))` (**a non-flat structure**: +1/-1 on the ball, 0 on the cube's
  corners: SDC-g); `median_filter(closed, footprint=ball(3))`; `threshold_otsu(denoised)`; `closedbin[closed <
  th] = 1`; `binary_opening(.., ball(3))`; `label` (6-connectivity), `sum(closedbin, label_im,
  range(n+1))`, keep `sizes == sizes.max()` (ties keep all; **no component -> the background label 0 kept:
  the whole image**, SDC-h); `binary_closing(.., ball(5))`; markers: 2 on the padded array's outer shell, 0
  inside, 0 where `binary_dilation(closedbin, ball(5))`, 1 where `binary_erosion(closedbin, ball(5))`;
  `closed[closed>0] -= min`; `segtarget = 2*closed/closed.max() - 1` (float32); `random_walker(segtarget,
  markers, spacing=zooms[:3], return_full_prob=True)` (beta 130, mode `cg_j`, tol 1e-3, prob_tol 1e-3), the
  padding cut; mask = `prob[0] >= 0.5` uint8 (header dtype uint8), probseg float32 named
  `re.sub(r'\.nii(\.gz)$', r'_probseg.nii\1', mask.replace('_mask.', '.'))`, brain = raw dataobj (scaled)
  zeroed where `prob[0] < 0.5`, saved with the input header (nibabel's scaled writer). Returns (brain,
  probseg, mask) strings.
- `BinaryDilation` (sdcflows; P: radius 5 in coeff2epi, 3 in SyN): `binary_dilation(dataobj > 0, ball(r))`,
  uint8 header, `<in>_dil`. `Union`: `(dataobj1 + dataobj2) > 0` (scaled sum; int overflow of uint8 + uint8
  stays in the scaled float64), uint8, `<in1>_union`.
- `reorient_pedir`, `ensure_positive_cosines`, `deoblique_and_zooms`, `resample_to_zooms`: part A.
### sdcflows/interfaces/utils.py
- `Flatten` (3dQwarp only): `_flatten(zip(in_data, in_meta), max_trs=50)`: 3D kept; 4D sliced `[..., :max_trs]`,
  `four_to_three`, written `<stem>_idx-{j:03}.nii.gz` (stem = name before `.nii`, absolute); outputs
  out_list/out_data/out_meta (`zip(*)`: an empty list -> ValueError).
- `UniformGrid` (P): reference index 0; ref passed through; each other file: shape and affine allclose ->
  the file itself when the affine is exactly equal, else the reference's affine written onto its data and
  header (`<name>_regrid{i:03d}`); else resampled with nitransforms `Affine(reference=ref).apply(nii-as-
  loaded-without-header)` (order 3, nitransforms' defaults) and written with the reference's affine and the
  input's header (on-disk dtype restored); 2.17.1 warns (LOGGER `nipype.interface`) when the rotation part
  differs ("UniformGrid resampled %d input image(s) onto the reference grid because they were not on a
  common sampling grid: %s. When feeding TOPUP, ... Verify that these inputs belong together."). ds001600's
  PEPOLAR (AP 80x80x60 3 mm, PA 64x64x44 3.75x3.75x4) reaches the resampling (same rotation: no warning).
- `ReorientImageAndMetadata` (P, LAS): target upper-cased and checked (`ValueError('Invalid orientation code
  ...')`) but `axcodes2ornt` gets the original string (lowercase -> nibabel error: SDC-i); identity ->
  passthrough; else `as_reoriented`, `reorient_pedir` per direction, `<name><TARGET>` (e.g. `..._mergedLAS`?
  `fname_presuffix(suffix=target)`).
- `ConvertWarp` -> `_qwarp2ants`: float32 `<f4`, intent vector, data squeezed `[..., newaxis, :]`,
  `<in>_warpfield`. `Deoblique` -> `_deoblique` (SDC-f): obliquity < 0.5 (radians) -> passthrough; else
  axcodes vs 'RAS' position by position, `diag(dirs*zooms)`, translation `-= (shape-1)*0.5` (voxels, not mm),
  q/sform code 1, `<in>_plumb`; the mask with the plumb file's affine. `Reoblique` (unused): EPI's affine and
  q/sform onto plumb, field and mask, `_oriented`. `DenoiseImage` = nipype's ANTs DenoiseImage +
  `CopyHeaderInterface` (copy_header True). `PadSlices` (2.15 path, P) -> `_pad_num_slices(in, ax=2)`: even ->
  passthrough False; else `np.pad(dataobj, (0,1))` on the axis (scaled data saved with the input header ->
  re-quantised), `img.header.set_data_shape` (mutates), `<in>_padded`, True. `PositiveDirectionCosines`
  (unused) -> `_ensure_positive_cosines`, `<in>_flipfree`, axcodes string.
### sdcflows/interfaces/reportlets.py and viz/utils.py
- `FieldmapReportlet` (P: `fmap_rpt`, 10-16 s per estimator): nilearn plotting, matplotlib, nireports'
  SVG composition (`plot_registration`, `coolwarm_transparent`). **Not numerics: belongs with larmorprepx's
  reports** (D7).
### sdcflows/workflows
- `fit/pepolar.py init_topup_wf(grid_reference=0, use_metadata_estimates=False,
  fallback_total_readout_time=None, omp_nthreads=1, sloppy=False, debug=False, name, topup_config=None,
  max_vols_per_pe=3)` (P in its 2.15 form; nodes and wiring in "The steps"); `_select_topup_config(in_file,
  sloppy)` (dims of the LAS image: all %4 -> 4, all %2 -> 2, else 1); `_topup_mats_to_world(in_mats,
  in_reference)` (nitransforms FSL loader with reference=moving=LAS image -> RAS matrices, `.tolist()`);
  outputnode `method = 'PEB/PEPOLAR (phase-encoding based / PE-POLARity)'`; `__desc__` with FSL's version.
  nipype's `TOPUP` wrapper: `--config --datain=<base>_encfile.txt --imain --out=<in>_base
  --iout=<in>_corrected --fout=<in>_field --jacout=jac --logout=<in>_topup.log --rbmout=xfm
  --dfout=warpfield`; encfile `np.savetxt(fmt="%d %d %d %.8f")` rows `[x==dir, y==dir, z==dir]*(+-1) + [ro]`
  (one readout time replicated; other length mismatch -> ValueError); outputs `<base>_fieldcoef`,
  `_movpar.txt`, `warpfield_NN`, `jac_NN`, `xfm_NN.mat` (NN from 01). `lx.fsl.topup` takes the same.
- `init_3dQwarp_wf(omp_nthreads=1, debug=False, name)`: never reached (sdcflows' `get_workflow` always uses
  topup for PEPOLAR; fMRIPrep's image lacks 3dQwarp). Python steps: `Flatten`, `_sorted_pe` (orthogonal PE ->
  `ValueError('Cannot handle orthogonal PE encodings.')`; args `-noYdis -noZdis` etc.), `ConvertWarp`,
  `CopyHeader`; programs waiting: AFNI `3dQwarp` (`QwarpPlusMinus` blur -1 -1, minpatch 9, nopadWARP, noweight,
  pblur 0.05 0.05, OMP <= 4), FreeSurfer `mri_robust_template` (`StructuralReference`, >1 file), and
  niworkflows' `init_enhance_and_skullstrip_bold_wf` (D6); ANTs (ported): `align_pes` (translation_rigid.json),
  `unwarp_reference` (Lanczos, float). **How the port reports it** (D10): the Python steps as functions; the
  workflow raises `NotImplementedError` naming the missing port before any work.
- `fit/syn.py init_syn_sdc_wf(*, use_metadata_estimates, fallback_total_readout_time, sloppy, debug, name,
  omp_nthreads, laplacian_weight=None)` (fMRIPrep with `--use-syn-sdc`; no oracle run): ANTs version check
  (<2.2.0 -> RuntimeError); GetReadoutTime; `_warp_dir` (nlevels 2: `[0.1,0.1,0.1]` with 1.0 on the fixed
  image's axis of the moving's PE axis by `aff2axcodes(.., ['RR','AA','SS'])`; warns "Running fieldmap-less
  registration on an oblique dataset" when `obliquity(fixed) > 0.05` (radians) through `Logger.warn`, an alias
  removed in recent Python versions: to verify on 3.13/3.14, SDC-j); BinaryDilation (3) of the anat mask,
  `amask2epi` ApplyTransforms MultiLabel identity onto the EPI mask; ImageMath `Laplacian 1.5 1` with
  copy_header (anat and clipped EPI) + `_norm_lap` (float64: minus median, positive part / p99.8, negative /
  -p0.2, clip [-1, 1], float32; empty sides -> IndexError, SDC-k); IntensityClip(35, 99.9) of the EPI;
  `Union` (EPI mask, anat mask in EPI); masks lists `['NULL', union]`, `[anat_mask, sd_prior]`; `_adjust_zooms`
  (`min(round(max(0.5*(anat+epi min zooms), 1.8), 2), 2.2)` x3) + `RegridToZooms` (computed but its output
  unconnected: dead node); `_mm2vox` (third transform parameter / PE resolution in the fixed image);
  `Registration(from_file=sd_syn[_sloppy].json)` with restrict_deformation, laplacian weights clipped to
  [0, 0.5] -> metric_weight, debug `--write-interval-volumes 2`; `DisplacementsField2Fieldmap` (part A);
  ApplyTransforms BSpline identity `-u float` onto the EPI; MultiLabel `-u uchar`; `BSplineApprox(recenter=
  False, extrapolate=not debug)` HF spacing, zooms_min 4 if sloppy (**no mask**: commented out); ApplyCoeffsField
  jacobian False (part A); method `'FLB ("fieldmap-less", SyN-based)'`.
- `init_syn_preprocessing_wf(*, atlas_threshold=3, debug, name, omp_nthreads, auto_bold_nss=False,
  t1w_inversion=False (unused), sd_prior=True)` (fMRIPrep: auto_bold_nss True): niworkflows'
  `init_epi_reference_wf` (ValidateImage, NonsteadyStatesDetector, RobustAverage (3dvolreg), IntensityClip,
  N4 (50x5, 1e-7, shrink 4), IntensityClip(2, 100), StructuralReference (one file: passthrough + FreeSurfer
  `lta_convert` writing an identity LTA, not consumed here; several: mri_robust_template), `_post_merge`);
  `Deoblique`; ApplyMask; IntensityClip(0, 99.8); DenoiseImage (copy_header); BrainExtraction;
  BinaryDilation x2; `Registration(affine.json)` epi->anat (masks `['NULL', dil]`); GenerateSamplingReference
  (nilearn `resample_img` nearest; a `RegridToZooms` 2 mm smooth in debug); ApplyTransforms anat2epi (inverse,
  Linear) and mask2epi (MultiLabel) + `_set_dtype` uint8; the prior: `fmap_atlas.nii.gz` (5.8 MB, D11) through
  `[epi2anat, std2anat, fmap_atlas_2_MNI152NLin2009cAsym_affine.mat]` inverted first, `Binarize(thresh_low=3)`;
  without the prior the anat mask. `match_histogram` (unused; skimage `match_histograms`).
- `fit/fieldmap.py init_fmap_wf` complete (P): part A did the phase-difference path; part B adds
  `init_magnitude_wf` (`IntraModalMerge(hmc=False, to_ras=False)` + `init_brainextraction_wf`) and the direct
  mode (`IntraModalMerge(zero_based_avg=False, hmc=False, to_ras=False)` of the field maps ->
  `CheckB0Units(units=metadata 'Units')` -> bs_filter); `bs_filter` gets the magnitude mask in both modes.
- `ancillary.py init_brainextraction_wf` (P x 9): `IntensityClip()` (35, 99.98) -> N4 (`-d 3 --convergence
  [ 50x50x50x50x50, 1e-07 ] --shrink-factor 4`, copy_header, bspline_dist unset) -> `IntensityClip(0.01,
  99.9)` -> BrainExtraction; outputs out_file (the clipped N4 image), out_brain, out_mask, out_probseg.
- `base.py init_fmap_preproc_wf(*, estimators, omp_nthreads, output_dir, subject, use_metadata_estimates,
  fallback_total_readout_time, sloppy, debug, name)` (P): readout check per EPI-like source (`RuntimeError(
  'Missing readout timing information for <path>.')`; fMRIPrep turns it into `sys.exit(EX_DATAERR)` with
  "Missing readout time information. See documentation for `--fallback-total-readout-time`."), per estimator
  its workflow, derivatives (`init_fmap_derivatives_wf`, write_coeff and write_mask True) and reports, outputs
  merged in estimator order (`no_flatten` for fmap and coeff).
- `outputs.py`: `init_fmap_derivatives_wf` (MergeSeries of the field(s); DerivativesDataSink: reference
  `desc-<epi|magnitude>` (`_getsourcetype`), `desc-preproc_fieldmap` with Units Hz, B0FieldIdentifier,
  RawSources, AnatomicalReference, AssociatedCoefficients, IntendedFor (`_selectintent`), `desc-brain_mask`,
  coefficients `desc-coeff[N]` (`_gendesc`); `fmapid` entity = the id without non-alphanumerics),
  `init_fmap_reports_wf` (FieldmapReportlet, `figures/` `desc-<type>_fieldmap.svg`). fMRIPrep sets
  `out_path_base = ''` on every `ds_` node.
- `fit/base.py init_sdcflows_wf()` (the BIDS app's): every subject's `find_estimators(fmapless=config)`, each
  estimator's workflow with sloppy/debug False, derivatives and reports. **ANAT estimators get no inputs**
  (SyN preprocessing is not wired): the app fails on them at run time (SDC-l).
- `apply/correction.py init_unwarp_wf(*, jacobian=True, use_metadata_estimates, fallback_total_readout_time,
  free_mem=None, omp_nthreads, debug, name)`: GetReadoutTime, ApplyCoeffsField (num_threads from free_mem:
  5 GB per thread rule), MergeSeries, RobustAverage(mc_method=None), init_brainextraction_wf. Not used by
  fMRIPrep (its own ResampleSeries).
- `apply/registration.py init_coeff2epi_wf(omp_nthreads, sloppy, debug, write_coeff=False, name)` (P x 7):
  BinaryDilation(5) of both masks; `Registration(fmap-any_registration[_testing].json)` fixed = fmap_ref,
  moving = target_ref, masks plural for ANTs >= 2.2.0; `write_coeff` warns (deprecated) and does nothing.
### sdcflows/fieldmaps.py, utils/bimap.py, utils/wrangler.py (P: the estimators of every run)
- `MetadataError`, `EstimatorType` (UNKNOWN, PEPOLAR, PHASEDIFF, MAPPED, ANAT), `MODALITIES`;
  `FieldmapFile(path, *, metadata={}, find_meta=True)`: path checks (not a file and not a symlink ->
  FileNotFoundError "File path <..> does not exist, or it is not a file"; not `.nii[.gz]` -> ValueError; suffix
  not in MODALITIES -> ValueError), pybids `parse_file_entities`, the sidecar beside it (`str.replace(ext,
  'json')`: no BIDS inheritance), `bids_root` from niworkflows' `relative_to_root`, T1w/T2w get
  TotalReadoutTime 0.0, required metadata per suffix (MetadataError messages), the readout warning (logger
  `sdcflows.fieldmaps` at ERROR level: printed only if the level allows).
- `FieldmapEstimation(sources, *, bids_id=None)`: the method inference (incompatible suffixes TypeError; the
  missing phase1/phase2 and magnitude(1/2) files added by name substitution; PEPOLAR needs > 1 EPI-like source
  and two PE directions; ANAT needs an EPI; messages), B0FieldIdentifier intersection (ValueError "No common"/
  "Multiple common"), the global registries `_estimators` (bidict, `auto_NNNNN` keys: `add` takes the LAST
  free index in range(len(used)+1), SDC-b) and `_intents`, `sanitized_id`, `paths()`, `get_workflow` (topup
  for every PEPOLAR; `wf_<sanitized_id>`), `get_identifier(filename, by)`, `clear_registry()`.
- `find_estimators(*, layout, subject, sessions=None, fmapless=True, force_fmapless=False, logger=None,
  bids_filters=None, anat_suffix='T1w')`: base entities (extension, part mag|None, scope raw), filters
  (session conflict ValueError), step 1 B0FieldIdentifier (bare and listed ids, regex `"<id>"`), step 2
  heuristics (fieldmap/phasediff/phase1 files; EPI pairs by acquisition/ceagent and directions grouped by
  IntendedFor; single-PE EPIs with IntendedFor + targets (sbref preferred)), fieldmap-less (anat file,
  `find_anatomical_estimators`: sbref before bold/dwi/asl, shortest echo first, IntendedFor of all echoes),
  debug logs. Needs a BIDS layout (D3).
### niworkflows 1.15.4 interfaces and workflows on part B's paths (P unless said)
- `images.IntraModalMerge` (in_files, in_mask, hmc True, zero_based_avg True, to_ras True,
  grand_mean_scaling False): reorient to RAS (`reorient`), squeeze, 5D -> RuntimeError, 4D split, concat,
  `<first>_merged`; 3D -> out_avg = merged; hmc + several files -> mcflirt (not on part B's paths: hmc=False);
  mean over axis 3 (float32), minus min if zero_based_avg; `<first>_avg` with the merged header (scaled write).
- `images.RobustAverage` (in_file, t_mask, mc_method AFNI|FSL|None, nonnegative True, num_threads, two_pass
  True): 3D -> passthrough; extensions cleared; squeeze; one volume -> `_squeezed`; t_mask length check and
  >= 1 volume (ValueErrors); `_sliced` = selected volumes clipped to [0 (or p0.2), p99.8], divided by the
  global-signal drift (mean over axes 0-2 / max), clipped again; 1 volume -> squeezed; 3dvolreg `-Fourier
  -twopass -zpad 4` (nipype Volreg: `_volreg`, `.aff12.1D`, `_md.1D`, `.1D`) or mcflirt sinc; clip; median
  over axis 3 -> `_average`; outputs out_hmc, out_hmc_volumes, out_drift.
- `nibabel.IntensityClip` -> `_advanced_clip(p_min=35, p_max=99.98, nonnegative=True, dtype='int16',
  invert=False)`: squeeze, 3D only (RuntimeError), `median_filter(footprint=ball(3))`, percentiles of the
  positive denoised values, clip the original, `-= min`, `/= max`, invert, `round(255*x).astype(dtype)` for
  uint8/int16 (NaN of a constant image -> undefined cast: PIPE-a), cal_max, `clipped.nii.gz` (fixed name).
- `nibabel.MergeSeries` (allow_4D True, affine_tolerance): squeeze, `allclose(atol=tol)` against the first
  (ValueError "Difference in affines greater than allowed tolerance"), 3D/4D, other ndim ValueError,
  `concat_images(check_affines=not tol)`, `<first>_merged`. `ApplyMask` (threshold 0.5; shape and affine
  checks with their messages; `dataobj * msk`, header kept, affine None -> the header's), `Binarize` (`>
  thresh_low`; `_masked` float, `_mask` uint8), `RegridToZooms` -> `utils.images.resample_by_spacing` (order 3,
  clip, smooth: gaussian_filter, `map_coordinates` mode constant prefilter), `ReorientImage` (target file or
  orientation; `reoriented.nii.gz`), `GenerateSamplingReference` -> `_gen_reference` (nilearn `resample_img`
  nearest; fov_mask; xform codes; descrip), `BinaryDilation`/`_dilate` (niworkflows' own, iterations).
- `header.CopyHeader` (`_fixhdr` in '.'), `header.ValidateImage` (the truth table, `_valid`, its HTML report;
  transforms3d for the report's angle), `fixes.FixHeaderApplyTransforms` and `FixHeaderRegistration`
  (`_copyxform` of the reference's q/sform and descrip onto the outputs), `utility.KeySelect`,
  `nitransforms.ConcatenateXFMs` (out_fmt itk: nitransforms' ITK text writer), `bold.NonsteadyStatesDetector`
  (nipype's `is_outlier` on the clipped global signal), `freesurfer.StructuralReference`.
- `func/util.init_skullstrip_bold_wf` (P x 7, after the field-map unwarping): BET `-f 0.20 -m`, 3dAutomask
  `-dilate 1 -apply_prefix`, fslmaths `-mul`, ApplyMask; its `mask_reportlet` is a report.
  `init_enhance_and_skullstrip_bold_wf` (fMRIPrep's coregistration reference; templates; D6).
### fMRIPrep 25.2.7 (= 25.2.6 on these paths) own field-map steps
- `workflows/base.py`: precomputed fieldmaps (`collect_fieldmaps` with the derivatives layout's `fmapid`),
  `map_fieldmap_estimation(layout, subject_id, session_id, bold_data, ignore_fieldmaps, use_syn, force_syn,
  filters)` (the `find_estimators` call, "Fieldmap-less (SyN) estimation was requested, but
  PhaseEncodingDirection information appears to be absent." (error if use_syn == 'error'), ANAT only with
  `--ignore fieldmaps`, the "Several fieldmaps <..> are 'IntendedFor' <..>, using <first>" warning, the 1-1
  map), `get_estimator(layout, fname)` (B0FieldSource, else IntendedFor via `get_identifier`), PEPOLAR
  "Sophisticated PEPOLAR schemes are unsupported." (NotImplementedError), the SyN preprocessing wiring, the
  jacobian rule (PEPOLAR only, `--force/--ignore fmap-jacobian`), the fmap buffers.
- `workflows/bold/fit.py`: `_select_ref` (first sbref, else boldref), `KeySelect`, `ReconstructFieldmap(
  inverse=[True])` -> `reconstruct_fieldmap` (direct B-spline evaluation when the transform chain is affine
  and the projected grid is aligned with the coefficients, after `ensure_positive_cosines`; else on the
  fieldmap reference (as_closest_canonical "hack"; "Reference passed is not aligned with spline grids"
  ValueError) then nitransforms resampling onto the target; intent 'estimate' 'fieldmap Hz'; `<coeff>rec`),
  `init_coeff2epi_wf`, `ConcatenateXFMs(out_fmt='itk')`, `DistortionParameters` (get_trt with fallback,
  KeyError/ValueError swallowed -> undefined outputs), `ResampleSeries(jacobian)` (`unwarp_boldref`: ported,
  `lx.transforms.resample_series`), `init_skullstrip_bold_wf`. `workflows/bold/apply.py`: `fmap_recon` and
  `distortion_params` per output space (`init_bold_volumetric_resample_wf`).

## The paths fMRIPrep runs and the oracle runs that reach them (`<scratch>/fmap_nodes_all.txt`, from `scripts/fmap_nodes.py`)
| path | runs (fMRIPrep 25.2.6, all on the old laptop) | calls |
|---|---|---|
| phase-difference estimator, magnitude prep (magmrg, clipper_pre, N4, clipper_post, masker) | ds001600 x3, ds002422 x2, ds002603, ds005454 | 7 each |
| PEPOLAR (2.15 wiring: runwise_avg, regrid, sort, concat, pad, setwise_avg, to_las, topup, fix_coeff, from_las x2, ref_average, brain extraction) | ds001600 (AP 80^2x60x5 3 mm + PA 64^2x44x5: regridded), ds005040 (Philips A/P 64^2x40x5) | 2 |
| SyN (`--use-syn-sdc`) | **none** | 0 |
| direct field map (MAPPED) | **none** (ds005752's is not linked) | 0 |
| fMRIPrep's BOLD field-map steps (boldref_fmap, fmap_recon x2, coregister, dilations, itk_mat2txt, distortion_params x4, unwarp_boldref, skullstrip_bold_wf) | ds002422 (3 BOLD), ds002603 (2), ds005040 (1, PEPOLAR), ds005454 (1) | ReconstructFieldmap 28, coregister 7, skullstrip 7 |
| estimator selection (`find_estimators` + `map_fieldmap_estimation`) | every run (ids and sources in each `fmriprep.log`; ds002603 shows the "Several fieldmaps" choice: its two EPI+BOLD PEPOLAR estimators dropped) | 15 |
Cached datasets that could reach the missing paths (`scripts/fmap_sources.py`, `bold_pe.py`): ds006736 sub-004
(AP/PA 3D EPIs 74x64x58, IntendedFor 6 files: a PEPOLAR estimator as it is), ds004958 sub-fsm64ps (se AP/PA
104^2x72x3 2 mm, gre phasediff; nothing linked), ds002603 (two single-PE EPIs with IntendedFor: the EPI+BOLD
path once the phasediff's IntendedFor is dropped), ds005752 (a direct Hz field map, not linked); SyN needs
PhaseEncodingDirection on the BOLD: ds000228 (EES), ds004884, ds005003 (no field maps), ds000256, ds004958,
ds002603, ds005752 (with `--ignore fieldmaps` or `--force syn-sdc`). ds000005, ds003592, ds000210 lack it.
A new run `ds000210-sub-02-noreconall` (multi-echo, BIDS view) started 2026-11-08 12:37 on the main machine
(the other part's, it seems).

## Defect candidates (SDC-12..30, PIPE-18..40; numpy NP-03.., SciPy SCI-06.., nibabel NIB-05.., scikit-image a new family (SKI-?) to be named by the orchestrator; decided by rules A-D in the relay that ports each)
- SDC-a `init_topup_wf` 2.15 -> 2.17.1: not a defect (a redesign); the record shows the step per case.
- SDC-b `bidict.add` takes the last free `auto_` index in `range(len(used)+1)`, not the first: deterministic
  names in derivatives; rule C (keep: names are in fMRIPrep's outputs).
- SDC-c wrangler in 2.17.1 lacks 2.15.2's fixes (filters applied to the fieldmap-less anatomical query;
  no sessionless fallback; `acquisitions` typo): rule A (upstream's own fix), `reproduce` = 2.17.1 (D2).
- SDC-d `get_trt`'s messages (`ValueError("'0'")` for a zero readout time) and `npe // acc` in float: C/cosmetic.
- SDC-e `init_3dQwarp_wf` reads `inputnode.in_reference`, which does not exist: its fmap_ref can never be
  computed; rule B (clear error) when ported with 3dQwarp; now NotImplementedError anyway.
- SDC-f `OBLIQUE_THRESHOLD_DEG = 0.5` compared with nibabel's obliquity in radians (28.6 degrees), so SyN's
  `Deoblique` is a no-op on real data; and `_deoblique` itself centres the plumb grid in voxels not mm and
  compares axcodes with 'RAS' by position (permuted axes flip): rule D (measure what SyN gives with the
  threshold in degrees and a correct plumb grid), then A or C.
- SDC-g `brain_masker`'s `grey_closing(structure=ball(1))`: a non-flat structure (+1 on the ball, 0 on the
  cube's corners) where a flat footprint was meant; in [0,1] data it equals the flat ball closing but loses
  float32 bits (x+1-1): rule D (measure), likely C.
- SDC-h `brain_masker` with no foreground component keeps label 0 (the background): the whole image is the
  rough mask; also the empty-percentile IndexError and the constant image's NaN: rule B (clear errors).
- SDC-i `ReorientImageAndMetadata` validates the upper-cased code but uses the original (lowercase crashes in
  nibabel): rule A (use the upper-cased code).
- SDC-j `_warp_dir` calls `Logger.warn` (deprecated alias; removed in a recent Python per its deprecation:
  verify on 3.13/3.14): a crash never reproduced; warn through `warning`.
- SDC-k `_norm_lap` with no positive or no negative value: IndexError: rule B.
- SDC-l sdcflows' BIDS app builds ANAT estimators without their inputs (fails at run time): rule B (a clear
  message before running) or A (wire the SyN preprocessing as fMRIPrep does): decide with the CLI relay.
- SDC-n `init_syn_sdc_wf`'s `zooms_epi` (`RegridToZooms`) output is never used: dead work; C (document; the
  port may skip computing it, results identical).
- PIPE-a niworkflows `_advanced_clip` on a constant image: 0/0 NaN cast to int16 (x86: -32768): the decided
  rule for undefined float-to-int conversions (clamp and warn) or B.
- PIPE-b niworkflows `RobustAverage`/`NonsteadyStatesDetector` with an all-zero series: 0/0 drift: B.
- PIPE-c fMRIPrep `DistortionParameters` swallows missing metadata: undefined outputs downstream: B?
- Library: SciPy 1.15 -> 1.18 sparse column order (a step, not a defect); numpy float64 exp's AVX-512F path
  differs from AVX2's (a numpy platform dependence: record in `docs/findings/numpy-blas.md`'s family).

## Design
- **Crates (Rust, the numeric work), all ports kept out of any later clean-room crate:**
  - `crates/larmorx-sdcflows` (extend): `masker` (brain_masker's arithmetic), `pepolar` (get_trt, select/sort
    PE volumes, the TOPUP encfile and config choice), `syn` (`_norm_lap`, `_warp_dir`, `_mm2vox`,
    `_adjust_zooms`), `fieldmap` (the magnitude merge's mean), reusing part A's `npy`, `blas` (Haswell sdot).
  - **new `crates/larmorx-ndimage`** (D4): SciPy ndimage beyond interpolation, ported exactly: binary
    erosion/dilation/opening/closing/propagation with structures (`generate_binary_structure`, iterations,
    border value, origin), grey erosion/dilation/opening/closing (flat footprint AND non-flat structure, double
    arithmetic as `ni_filters.c`), `median_filter`/`rank_filter` (footprint, modes), `label` (scipy's scan
    order and numbering), `sum_labels`/`center_of_mass`/`watershed_ift` (epi_mask); and scikit-image's `ball`,
    `threshold_otsu` (numpy histogram), `random_walker` (`cg_j`: the 6-neighbour Laplacian as SciPy 1.15 and
    1.18 store it, the Jacobi-preconditioned CG in float32 with OpenBLAS Haswell sdot, numpy's float32 exp,
    `_preprocess`'s propagation), `match_histograms`.
  - numpy's statistics as numpy computes them (percentile/quantile `linear`, median, mean/std pairwise
    float32 and axis reductions such as IntraModalMerge's axis-3 mean, `round`, `clip`, `histogram`), and
    numpy's float32 `exp` in `larmorx_core::math` (`expf_numpy`): **one shared home** to agree with the other
    part (D5): proposed `larmorx_core::numpy` (pure Rust, no allocation surprises).
- **Python** (plain functions; no nipype, no graph engine; in-memory images or paths, files named as the
  originals when `out_dir` is given; typed dataclass results; `n_threads` everywhere; `upstream_defects`):
  - `lx.sdcflows` (extend): `get_trt`, `get_readout_time`, `select_pe_volumes`, `sort_pe_blips`,
    `uniform_grid`, `reorient_image_and_metadata`, `flatten`, `pad_slices`, `deoblique`, `reoblique`,
    `positive_direction_cosines`, `qwarp2ants`, `brain_masker` (+ `brain_extraction`), `binary_dilation`,
    `union`, `epi_mask`, `norm_lap`, `match_histogram`; the workflows `brainextraction_workflow`,
    `magnitude_workflow`, `fmap_workflow` (both modes), `topup_workflow` (D1), `qwarp_workflow`
    (NotImplementedError), `syn_preprocessing_workflow`, `syn_sdc_workflow`, `unwarp_workflow`,
    `coeff2epi_workflow`, `fmap_preproc_workflow`; the estimators `FieldmapFile`, `FieldmapEstimation`,
    `EstimatorType`, `find_estimators`, `get_identifier`, `clear_registry` (or an explicit registry object:
    sdcflows' globals reproduced inside one), `sdcflows_workflow` (the BIDS app's).
  - **new `lx.niworkflows`** (D4): `robust_average`, `intra_modal_merge`, `intensity_clip`, `merge_series`,
    `apply_mask`, `binarize`, `binary_dilation`, `regrid_to_zooms` (`resample_by_spacing`), `reorient_image`,
    `copy_header`, `copy_xform`, `validate_image`, `generate_sampling_reference`, `nonsteady_states_detector`,
    `concatenate_xfms`, `key_select`, `epi_reference_workflow`, `skullstrip_bold_workflow`; and nipype's
    command lines as larmorx calls: `registration_settings(json)` -> `lx.ants.registration` stages exactly as
    nipype's `Registration(from_file=...)` builds the command (validated against every oracle `command.txt`),
    N4/ImageMath/DenoiseImage/ApplyTransforms with nipype's copy_header, Volreg/TOPUP/BET/3dAutomask/fslmaths
    argument lists, `fname_presuffix`/`split_filename` (move part A's private copies into one helper).
  - `lx.ndimage` (extend, SciPy names) and **new `lx.skimage`** (`ball`, `threshold_otsu`, `random_walker`,
    `match_histograms`).
  - fMRIPrep's own steps: `lx.transforms.reconstruct_fieldmap` and `distortion_parameters` (beside
    `resample_series`), and `larmorx.pipelines.larmorprepx.fieldmaps` (`map_fieldmap_estimation`,
    `get_estimator`, `collect_fieldmaps`, the BOLD fit's stage 3 as a function) (D4).
  - BIDS: **new `larmorx.bids`** (D3): a small index with pybids 0.22.0's entity patterns (`config/bids.json`,
    and nipreps' `fmapid` for derivatives) and metadata inheritance, the queries the wrangler and fMRIPrep
    use (get, get_metadata, get_B0FieldIdentifiers/sessions/acquisitions/ceagents/directions/subjects,
    `Query.REQUIRED/NONE`, regex search, get_file, relative paths), no database.
- **Threads:** `n_threads` on every call; deterministic: median/rank filters and morphology split voxels
  (outputs independent), the Laplacian matvec splits rows (each row a fixed-order sum), CG's dot products stay
  sequential (Haswell sdot's 32 lanes), `label` sequential (scipy's numbering), the PEPOLAR/SyN programs'
  threads through their ports; workflows pass `n_threads` down (nipype's omp_nthreads/num_threads mapped).
- **Memory:** sdcflows' brain_masker peaks at 1.4 GiB on 128^3 (several copies of the Laplacian: COO, CSR,
  sliced rows, sliced columns, B, M); larmorx holds one CSR of the unlabeled system and the CG vectors
  (target < 25 % of it); the rule's memory calls: brain_masker on ds002603, RobustAverage, IntensityClip, the
  topup workflow without topup, reconstruct_fieldmap.
- **Exactness:** every Python step bit-identical to its original with its libraries, in both kept envs where
  they differ (numpy 2.3.5/SciPy 1.15.3 and 2.5.4/1.18.1; switches as part A's LAPACK one), and to fMRIPrep's
  own node outputs (image libraries = the shared env's on the measured paths); programs through their ports
  (`lx.fsl.topup` replica for bit-identity, `lx.afni.volreg` replica, `lx.ants.*`, `lx.fsl.bet`/`maths`,
  `lx.afni.automask`); command lines checked string for string against the oracle `command.txt` files.
- **Command line:** sdcflows' BIDS app as a Python program (`larmorx sdcflows sdcflows <bids> <out>
  participant ...` through the `larmorx` console script; the standalone Rust binary says it needs the Python
  package) (D10); every option of `cli/parser.py`, `--dry-run`'s listing exact, exit codes.
- **Validation:** suites `sdcflows-fieldmap` (extended) or new `sdcflows-estimation` and `niworkflows-helpers`
  (D12): in-process originals (newest default, shared for the step) and fMRIPrep's image for its real calls
  (`OPENBLAS_CORETYPE=Haswell`), programs through the oracle cache (`use`); node-by-node replays of the oracle
  runs' working directories (inputs from `_report/report.rst`, outputs compared); golden cases (rule 10:
  seeded inputs, <= 32^3), memory calls, a difftest descriptor per family, a short trial; benchmark.

## Decisions for the orchestrator (each with a recommendation)
- **D1 PEPOLAR wiring.** sdcflows 2.17.1 (newest) and fMRIPrep 25.2.x's sdcflows 2.15 line differ completely
  on PEPOLAR (fMRIPrep's newest release 25.2.7 itself pins `sdcflows~=2.15.2`). Recommend: port both wirings
  (`topup_workflow(..., wiring="2.17" (default) | "2.15")`, the pieces exist in both), so fMRIPrep's own
  PEPOLAR runs are reproduced end to end and the final comparisons can use them.
- **D2 the wrangler's 2.15.2 fixes.** Recommend: correct by default (rule A: upstream's own fix, SDC-c),
  `reproduce` = 2.17.1's behaviour.
- **D3 BIDS layout.** Recommend `larmorx.bids`, an own index (pybids 0.22.0's entity config and inheritance,
  only the queries used), validated against pybids 0.22.0 and 0.20.0 on the oracle datasets, sdcflows'
  dsA/dsB/dsC skeletons and `test_wrangler.py`'s generated trees; not pybids as a dependency (SQLAlchemy,
  formulaic, pandas, bids-validator).
- **D4 homes.** Recommend the crates and families of "Design" (`larmorx-ndimage` with skimage's pieces;
  `lx.niworkflows`, `lx.skimage`; fMRIPrep's steps in `lx.transforms` and `larmorx.pipelines.larmorprepx`).
- **D5 numpy's statistics, shared with the other part** (t2smap needs numpy/SciPy numerics too). Recommend
  one module (`larmorx_core::numpy`, numpy's float32 exp in `larmorx_core::math`) and telling the other part,
  so the merge does not carry two percentiles.
- **D6 niworkflows' `init_enhance_and_skullstrip_bold_wf`** (fMRIPrep's coregistration reference, the target of
  its field-map registration; needs templateflow's boldref template and masks; niworkflows 1.15.4 changed its
  antsAI to Rigid). Recommend: with the templates part (or larmorprepx's BOLD reference); part B checks
  fMRIPrep's field-map steps with its outputs from the oracle runs. `init_skullstrip_bold_wf` in part B.
- **D7 reports and derivatives.** Recommend: reportlets (`FieldmapReportlet`, `SimpleShowMaskRPT`) with
  larmorprepx's reports; niworkflows' `DerivativesDataSink` ported in part B (the field-map derivatives are its
  first user; larmorprepx reuses it), so `fmap_preproc_workflow` and the BIDS app write sdcflows'/fMRIPrep's
  `fmap/` files; the app writes no figures until the reports exist, saying so.
- **D8 oracle runs for the paths no run reaches** (the orchestrator to make, boxed, recorded as the others):
  (a) SyN, fMRIPrep 25.2.6 `--use-syn-sdc`: ds000228 sub-pixar008, ds004884 sub-M2079, ds005003 sub-RS001
  (their panel views; no field maps acquired); (b) PEPOLAR: ds006736 sub-004 (3D AP/PA, linked as it is), a view
  of ds004958 sub-fsm64ps linking its se AP/PA pair (B0FieldIdentifier on the pair, B0FieldSource on the BOLD:
  derived sidecars, SHA-256 recorded), a view of ds002603 without the phasediff's IntendedFor (EPI+BOLD
  PEPOLAR); (c) direct: a view of ds005752 sub-ON01016 linking its Hz field map; (d) the newest sdcflows: the
  same estimators through sdcflows 2.17.1 itself in `oracles/python-newest` (its BIDS app with
  `--no-fmapless`, programs from the oracle builds: FSL 6.0.7.23 topup, ANTs 2.6.5, AFNI 26.3.05; and a small
  driver wiring SyN as fMRIPrep does), after upgrading that env's niworkflows to 1.15.4. Without (a) to (c)
  SyN and direct maps have synthetic and node-level evidence only.
- **D9 FSL's topup configurations.** `b02b0_1/2/4.cnf` are FSL's files (FSL Licence) inside sdcflows' Apache
  sdist. Recommend: the main package carries their parameter values as code (the numbers, without FSL's
  text) and passes them to `lx.fsl.topup` as options (or reads `$FSLDIR`/larmorx-nc's copy when present);
  sdcflows' own `b02b0.cnf` and `_quick` files may ship (Apache). A licensing call: the orchestrator or the user.
- **D10 the programs that wait and the command line.** Recommend: `qwarp_workflow` and StructuralReference with
  several files raise `NotImplementedError` naming 3dQwarp (AFNI 26.3.05, CC BY 4.0: a later replica) or
  FreeSurfer's `mri_robust_template`; the one-file StructuralReference's identity LTA (written by FreeSurfer's
  `lta_convert`, consumed by nothing on these paths) is not produced, documented; sdcflows' app as `larmorx
  sdcflows sdcflows ...` (Python only).
- **D11 the SyN prior** `fmap_atlas.nii.gz` (5.8 MB, Apache, sdcflows data): recommend fetching it on first use
  from sdcflows' release with a pinned SHA-256 (as the templates), not shipping image data in the wheel; the
  small JSON/.mat files ship.
- **D12 records.** Recommend two new records beside `sdcflows-fieldmap`: `sdcflows-estimation` (PEPOLAR, SyN,
  direct, masks, estimators, workflows, fMRIPrep's steps) and `niworkflows-helpers` (+ `ndimage-skimage` for
  the library ports, or inside them), each with the step newest/shared per case.

## Relay plan (one sub-task per relay; commit; update this file; stop)
- R2 numpy statistics + float32 exp (shared home, D5), SciPy ndimage morphology/median/label/measurements
  (`larmorx-ndimage`), bit-checked in both envs on synthetic and the oracle runs' masker/clipper inputs.
- R3 skimage `ball`, `threshold_otsu`, `random_walker` (both SciPy orders) -> `brain_masker`,
  `IntensityClip`, BinaryDilation/Union -> `brainextraction_workflow` (with `lx.ants` N4 + copy_header) ->
  `magnitude_workflow` (IntraModalMerge) -> `fmap_workflow` both modes; check: fMRIPrep's 7 estimators' nodes.
- R4 PEPOLAR: get_trt, GetReadoutTime, SelectPEVolumes, UniformGrid, SortPEBlips, MergeSeries, PadSlices,
  ReorientImageAndMetadata/ReorientImage, RobustAverage (lx.afni.volreg), TOPUP's call, `_select_topup_config`
  (D9), `_topup_mats_to_world`, `topup_workflow` both wirings (D1) + debug; check: ds001600, ds005040 node by
  node and end to end (2.15), synthetic for 2.17.1 (+ D8 runs when they exist).
- R5 nipype's command lines (Registration JSON, N4, ImageMath, DenoiseImage, ApplyTransforms, Volreg, BET,
  3dAutomask, fslmaths, TOPUP) as one module, checked against every oracle command.txt; FixHeader*/_copyxform,
  CopyHeader, ConcatenateXFMs, KeySelect; `init_skullstrip_bold_wf`; fMRIPrep's `reconstruct_fieldmap`,
  `DistortionParameters`, `coeff2epi_workflow`, the BOLD stage 3; check: 28 ReconstructFieldmap, 7
  coregisters, 7 skullstrips.
- R6 SyN: `syn_preprocessing_workflow` (ValidateImage, NonsteadyStatesDetector, epi reference, Deoblique,
  ApplyMask, DenoiseImage, GenerateSamplingReference (nilearn resample_img port), Binarize, the prior (D11)),
  `syn_sdc_workflow`, `_norm_lap` etc.; SDC-f measured; check: synthetic + D8(a) runs. (May split in two.)
- R7 BIDS index (D3) + estimators (FieldmapFile, FieldmapEstimation, registries, find_estimators,
  get_identifier; 2.15.2 fixes, D2) + fMRIPrep's `map_fieldmap_estimation`/`get_estimator`/`collect_fieldmaps`;
  `fmap_preproc_workflow`, DerivativesDataSink (D7); check: 15 oracle runs' estimators, dsA/dsB/dsC, the test
  skeletons; `qwarp_workflow`'s Python steps (Flatten, _sorted_pe, qwarp2ants) and its NotImplementedError;
  `unwarp_workflow`; `epi_mask`, `match_histogram`.
- R8 sdcflows' command line (`init_sdcflows_wf`, parser, config, `--dry-run`, exit codes; SDC-l).
- R9 the defects' switches and IDs, `.pyi`, API pages (option-mapping tables), PROVENANCE, SPDX, findings
  (`docs/findings/sdcflows-estimation.md`, `niworkflows-helpers.md`), parity suites (D12).
- R10 records from a clean commit, golden, memory, difftest descriptors + trial (boxed), benchmark, gates.
- R11 the site (`fmap-b-pages`); R12 the handoff and the final report. Relays may split; say so here.
- (relay 3) R3 split: relay 3 = the library ports and masks (done); relay 4 = R3b (IntraModalMerge, the
  brain-extraction, magnitude and field-map workflows, the speed profile: done); R4 onwards follow, one relay later
  (relay 5 = R4 PEPOLAR).
- Handoff items from relay 4: the register and `larmorx_core::defects`: PIPE-18 (niworkflows, rule A), PIPE-19
  (niworkflows, rule B); `validation/tools.toml` entries `niworkflows-helpers` (lx.niworkflows: intensity_clip,
  intra_modal_merge) and the workflows under `sdcflows-estimation` (R9/R10); `docs/validation/golden.md`: the two new
  cases; upstream.tsv: nothing new (niworkflows 1.15.4, nipype 1.12.0 already listed by relay 1); the findings'
  "PRELUDE's output name" fix touches part A's `phdiff_workflow` (its record `sdcflows-fieldmap` does not cover
  `.nii` phases: no record change expected; R10 re-makes the records anyway).
- Handoff items so far (relay 3): NOTICE: scikit-image 0.26.0 (BSD-3-Clause; `crates/larmorx-ndimage/src/skimage`)
  and OpenBLAS's `ddot` beside its `sdot` (`larmorx_core::numpy::dot`); `validation/tools.toml` entries for
  `skimage` (lx.skimage), `sdcflows-masks`/`niworkflows-helpers` at R9/R10; the register: SKI-01 (+ the
  candidates); the numpy 2.2 sums finding for next-steps / the t2smap part; `docs/validation/golden.md`: the three
  new cases.

## Helper scripts of relay 1 (in `<scratch>/scripts/`; commit the ones behind findings with R2)
`versions.py`, `image_versions.sh` (library versions per env), `extract_image_src.sh`, `extract_skimage.sh`,
`extract_pybids.sh` (sources out of the image; pybids diff), `diff_skimage.sh`, `fetch_tags.sh`,
`fetch_tags2.sh` (the pins), `find_estimators.py`, `list_nodes.py` (earlier attempt), `fmap_nodes.py` (every
field-map node's interface, duration and inputs from report.rst; output `<scratch>/fmap_nodes_all.txt`),
`fmap_cmdlines.py` (nipype's command lines), `fmap_sources.py`, `bold_pe.py` (the cached datasets' field maps
and BOLD metadata), `probe_numpy_exp.{py,sh}`, `probe_exp32_vs_cr.py` (numpy exp/log dispatch),
`probe_brain_masker.{py,sh}` (brain_masker vs fMRIPrep's files, time, memory), `probe_masker_steps.{py,sh}`
(the SciPy 1.18 Laplacian step), `cmp_cnf.sh` (FSL's configs), `probe_qwarp_wf.{py,sh}` (the workflows build).
Sandbox notes (part A's still hold): write scripts with the Write tool and run `bash <script>`; no `cd`
outside the worktree, no `git -C`, no variables in complex constructs; `python -I` for scripts reading
untrusted files, `python -P` with PYTHONPATH.

## FROM THE ORCHESTRATOR (2026-11-08 13:54, after relay 1)
- Relay 1's agent id: a4cf64edc320dd324 (the stopped first attempt: aaea7f61392fc0d27, a few minutes of reading). Its
  relay plan R2-R12 is agreed.
- **D1:** both PEPOLAR wirings, 2.17.1 by default and 2.15 for fMRIPrep's own runs (the end-to-end comparisons).
- **D2:** 2.15.2's wrangler fixes by default (rule A, upstream's own fix), `reproduce` 2.17.1's behaviour.
- **D3:** `larmorx.bids`, an own index validated against pybids 0.22.0 and 0.20.0 as you propose; no pybids dependency.
- **D4:** as you propose, with SciPy's ndimage functions in the existing `lx.ndimage` family (its Rust in the crate you
  name, `larmorx-ndimage`, or beside `larmorx-interp`'s map_coordinates: your call, said in the API page);
  `lx.niworkflows`, `lx.skimage`; fMRIPrep's steps in `lx.transforms` and larmorprepx. scikit-image's family ID: SKI-.
- **D5:** one home: `larmorx_core::numpy`. The t2smap part's relay 2 wrote numpy's reductions and percentiles there
  (bit-identical to numpy 2.3.5 and 2.5.4); the orchestrator brings those two commits into main now as a small verified
  change (gates, smoke, golden, CI green: the user's rule), then tells you; relay 2 extends that module (float32 `exp`,
  anything missing) and never writes a second percentile.
- **D6, D7, D10, D11, D12:** as you propose.
- **D8:** the orchestrator makes the runs, boxed, one at a time after the two t2smap runs (ds000210, ds000258): (a) SyN on
  ds000228, ds004884, ds005003; (b) PEPOLAR on ds006736 and the views of ds004958 (its se AP/PA pair linked; derived
  sidecars with SHA-256, as ds004884's lesion mask) and ds002603 (the phasediff's IntendedFor removed); (c) direct on a
  view of ds005752; (d) sdcflows 2.17.1's own runs in a **separate** environment (`oracles/python-sdcflows-2.17`: the
  newest environment stays as the records were made with it). Relays that need them (R4, R6) use what exists by then.
- **D9 (FSL's `b02b0_1/2/4.cnf` in sdcflows): asked of the user.** Not needed before R4.
- **Do not start relay 2 before the orchestrator's message** that main holds the shared numpy module.
- (orchestrator, 14:16) A finding of the t2smap part's relay 3 for this part's relay 2: `numpy_linalg`'s `svd3` in the
  LAPACK 3.12.0 path turns a -0 singular value into +0, which LAPACK 3.12.0 does not (only an exactly singular 3 x 3
  matrix reaches it): fix it with a case, rule-8 exactness, both environments. The t2smap branch also extends
  `numpy_linalg` (`lstsq`, a long-vector `dnrm2`, `dbdsqr` with right-hand sides): keep to your own functions there.

## FROM THE ORCHESTRATOR (2026-11-08 14:51, relay 2 may start)
- Main f5e89ff4 holds the shared numpy module (`larmorx_core::numpy`: reductions, `mean`/`var`/`std`, `nansum`/`nanstd`,
  weighted `average`, `percentile(method="higher")`, `nanpercentile`, SciPy's `scoreatpercentile`, numpy's float64
  `exp`/`log` (`numpy::umath`); `math::logf_numpy`): verified, published, CI green on all 13 jobs. Extend it; never a second copy.
- Relay 2 starts in a fresh harness worktree from that main: write its path at the top of this file.

## FROM THE ORCHESTRATOR (2026-11-08 16:00, after relay 2)
- Relay 2's agent id: a33e6cc1a7edea6ca; its worktree (agent-a33e6cc1a7edea6ca, the harness's) is this part's from now on:
  later relays work there. Its plan for relay 3 is agreed.
- **Library defect IDs between the two parts running:** this part SCI-06 to SCI-12, NP-05 to NP-08, NIB-05 to NIB-08, and
  the new scikit-image family SKI-01 onwards; the t2smap part NP-03 to NP-04 and SCI-13 onwards.
- t2smap's `nanpercentile` with q = -0.0 (relay 2's finding): fix it here, in main's `larmorx_core::numpy`, with a case
  (the module's home is main; the t2smap branch takes your version at its merge).
- Golden: the four `py.ndimage.*` cases recorded with the part's golden step (R10), as planned.

## FROM THE ORCHESTRATOR (2026-11-08 16:57, after relay 3)
- Relay 3's agent id: ac1175bc60550e332. Its split (relay 4 = R3b: the three workflows and IntraModalMerge, and the
  one-thread profile of `brain_masker`: 16.2 s against sdcflows' 8.2 s; the port must not be slower than the original at
  one thread) is agreed. R4 (PEPOLAR) becomes relay 5; D9 is still with the user.
- `random_walker`'s `mode='bf'` and `copy=False`: ported only if a path sdcflows or fMRIPrep reaches needs them; otherwise a
  clear NotImplementedError, documented on the API page (as now).
- (orchestrator, 18:01) D8 runs: the three SyN runs are DONE (exit 0): `oracles/fmriprep-25.2.6/ds000228-sub-pixar008-
  noreconall-syn`, `ds004884-sub-M2079-noreconall-syn`, `ds005003-sub-RS001-noreconall-syn`. The PEPOLAR and direct runs run
  now, one after the other (views by `validation/investigations/fmap-oracle-runs/derived_views.py`, main ce57fb5a):
  `ds006736-sub-004-noreconall`, `ds004958-sub-fsm64ps-noreconall-pepolar`, `ds002603-sub-01-noreconall-epi`,
  `ds005752-sub-ON01016-noreconall-direct`; progress in `oracles/logs/fmap-queue.log`.

## FROM THE ORCHESTRATOR (2026-11-08 18:12, after relay 4)
- Relay 4's agent id: a1fd06ee6416ffe30. Its plan for relay 5 (R4, PEPOLAR) is agreed.
- **D9, for now (the user has not answered; the orchestrator's recommendation, the conservative choice):** the main
  package ships no FSL file and no FSL configuration's values: `b02b0_1/2/4.cnf` are read from `$FSLDIR/etc/flirtsch/` or
  from larmorx-nc's FSL files when installed (add them there, under FSL's Licence, as BET's standard files: through the
  handoff), else a clear error naming both; sdcflows' own Apache `b02b0.cnf` (fMRIPrep's call) and `_quick` ship. Keep the
  lookup in one function, so a later decision by the user changes one place.
- Follow-up noted: `bspline_approx`'s one-thread time on small fields (684 CG iterations on the golden-size field):
  measure against sdcflows on the same field; part A's, so a follow-up unless it is slower than sdcflows.

## FROM THE ORCHESTRATOR (2026-11-08 19:22, after relay 5)
- Relay 5's agent id: a58deb7daf1547dd5 (written in the agents' line above). Its plan for relay 6 (R5) and the renumbering
  (R6 -> relay 7, ..., R12 -> relay 13) are agreed; its handoff items (SDC-12, FSL's three `.cnf` into larmorx-nc, NOTICE,
  golden.md, tools.toml) are the orchestrator's at the merge.
- D8 runs: `ds006736-sub-004-noreconall` DONE (18:46, exit 0); `ds004958-sub-fsm64ps-noreconall-pepolar` running, then
  `ds002603-sub-01-noreconall-epi`, `ds005752-sub-ON01016-noreconall-direct`. A run counts as finished only when
  `oracles/logs/fmap-queue.log` has its `<run> exit 0` line; use only those.
- D9 is still with the user: keep the conservative choice and its one function.

## FROM THE ORCHESTRATOR (2026-11-08 21:12, the D8 runs (b) and (c) done)
- All four PEPOLAR and direct runs finished with exit 0 (`oracles/logs/fmap-queue.log`, FMAP-QUEUE-END):
  `ds006736-sub-004-noreconall` (18:46), `ds004958-sub-fsm64ps-noreconall-pepolar` (19:40), `ds002603-sub-01-noreconall-epi`
  (20:32, two estimators), `ds005752-sub-ON01016-noreconall-direct` (21:11); each built its field-map estimator(s) under
  `fmap_preproc_wf`. D8 (d) (sdcflows 2.17.1's own runs, a separate environment) is the orchestrator's next.
- The stage catalogue (`validation/src/larmorx_validation/stages/catalogue.py`): the orchestrator adds a roadmap entry in
  main for the SyN runs' unclassified nodes (`ref_anat`, `lap_anat`, `lap_epi`, `n4_avgs`, `amask2epi`, `anat2epi`,
  `mask2epi`, `prior2epi`, `zooms_bmask`, `zooms_field`, `epi_merge`), so the smoke tier passes on main; part B turns it into
  stages when it ports SyN (the merge keeps both).
- (orchestrator, 21:21) **D8 (d) running:** sdcflows 2.17.1's own BIDS app (`--no-fmapless`, every estimator it finds) in
  the separate environment `oracles/python-sdcflows-2.17` (python-newest's packages with niworkflows 1.15.4; main 38383dfb:
  `validation/investigations/fmap-oracle-runs/make_python_sdcflows.sh`, `run_sdcflows_oracle.sh`), ANTs 2.6.5, AFNI
  26.3.05, FSL 6.0.7.23, 4 threads, boxed: runs `oracles/sdcflows-2.17.1/{ds005040-sub-002, ds006736-sub-004,
  ds004958-sub-fsm64ps-pepolar, ds002603-sub-01, ds001600-sub-1, ds005454-sub-16, ds002422-sub-02,
  ds005752-sub-ON01016-direct}` one after the other (`oracles/logs/sdcflows-queue.log`: `<run> exit 0`, then
  SDCFLOWS-QUEUE-END). Use them for R8 (sdcflows' command line, now relay 9) and the end-to-end checks; SyN has no app run
  (it needs fMRIPrep's anatomical outputs): its workflows in-process, as planned.
- (orchestrator, 21:47) **D8 (d) DONE:** all eight sdcflows 2.17.1 runs exit 0, no crash file, every estimator's outputs
  (`oracles/sdcflows-2.17.1/<run>/out`, 55 field-map files from 19 estimators; `oracles/logs/sdcflows-queue.log`). Every D8
  run is documented in main's `docs/validation/oracle-runs.md` (d34848c6). D8 is complete.

## FROM THE ORCHESTRATOR (2026-11-08 22:08, after relay 6)
- Relay 6's agent id: ac5079c785f056c44 (in the agents' line). The split (relay 7 = R5b, fMRIPrep's own field-map steps;
  then R6 SyN onwards one relay later) and relay 7's plan are agreed. Handoff items as listed: the orchestrator's at the merge.
- **The ANTs seed** (fMRIPrep's `ANTS_RANDOM_SEED`): a good finding; the `ants_random_seed` argument as you made it, and the
  same for every later ANTs call that reads it (SyN, templates).
- **The D8 fMRIPrep runs and OpenBLAS's kernels:** the orchestrator is measuring the cause now (the image's own topup on
  ds004958-pepolar's node inputs on this machine, with and without `OPENBLAS_CORETYPE=Haswell`). If the kernels are the cause,
  the orchestrator re-makes today's nine fMRIPrep runs with Haswell kernels (as the old laptop's runs had: they are the
  reference) and archives the present ones as evidence; it tells you here. Until then: exactness claims against fMRIPrep's
  own files only from the earlier runs (made on the old laptop); today's runs as information, as you did.

## FROM THE ORCHESTRATOR (2026-11-08 22:42, after relay 7)
- Relay 7's agent id: a2d2e605fadc15ca8 (in the agents' line). Its split (relay 8 = R5c, the BOLD fit's stage 3 as one
  function; R6 SyN onwards one relay later again: R6 -> relay 9, ..., R12 -> relay 15) and relay 8's plan are agreed.
- PIPE-20 (a missing readout time silently applies no field map): decide it by rules A-D in relay 8, as planned; the
  orchestrator only needs the decision and its reason in the notes.
- The seven field-map runs are being re-made with Haswell's kernels (`oracles/logs/fmap-queue-haswell.log`; ds006736 first,
  then ds004958-pepolar, ds002603-epi, ds005752-direct, then the three SyN runs). Re-run the checks relay 7 listed on each
  as it gets its `exit 0` line; a run without that line is not there yet.

## FROM THE ORCHESTRATOR (2026-11-08 23:57, after relay 8)
- Relay 8's agent id: a15a4cf4128e2669f (in the agents' line). Its split of R6 (relay 9 = R6a: the re-checks, then
  `syn_preprocessing_workflow` with `antsApplyTransforms` runnable and nilearn's `resample_img`; relay 10 = R6b:
  `syn_sdc_workflow`; R7 -> relay 11, ..., R12 -> relay 16) and relay 9's plan are agreed. PIPE-20's decision (rule B at
  the pipeline level, `reproduce` fMRIPrep's silent skip with a warning; the interface keeps rule C) is agreed.
- The Haswell re-runs: ds006736 and ds004958-pepolar done (`oracles/logs/fmap-queue-haswell.log`); ds002603-epi,
  ds005752-direct, then the three SyN runs follow (about 50 minutes each). Re-check each as it gets its `exit 0` line; the
  SyN runs are relay 9's and 10's real data.
- Main has moved: the t2smap replica is merged (712daf52; its `numpy_linalg.rs` has the `_ncc` `dbdsqr_upper` the merge
  note names). Do not merge main into this branch now; the orchestrator does it at the part's merge (or a relay of its
  choosing before the records relay).

## FROM THE ORCHESTRATOR (2026-11-09 01:40, after relay 9)
- Relay 9's agent id: ae78b46850d0dea1a (in the agents' line). Its plan for relay 10 (R6b: the SyN re-checks as the runs
  get their `exit 0`, then `syn_sdc_workflow` with SDC-13 measured, end to end on the three SyN runs) is agreed;
  R7 -> relay 11, ..., R12 -> relay 16 as before.
- The Haswell queue: ds006736, ds004958-pepolar, ds002603-epi, ds005752-direct all `exit 0` (re-checked by relays 8 and
  9); ds000228-syn started 01:17, then ds004884-syn and ds005003-syn (about 50 minutes each; the queue's final line is
  `FMAP-QUEUE-HASWELL-END`). Re-check each as it gets its `exit 0` line; one that has none yet is not there.
- nilearn's affine inverse (`scipy.linalg.inv` against numpy's): a replica is bit-exact first, so port SciPy 1.18.1's
  4 x 4 `inv` (and 1.15.3's, for the shared environment's step) in this part, in a relay before the records relay
  (R10), not as a follow-up. If it turns out larger than a relay's piece, measure its size, write it here and the
  orchestrator decides.
- The crates-gpl and crates-nc gates: part B changes Apache crates both workspaces build by path; the records relay runs
  `scripts/orchestration/gates-rust.sh <checkout> all` (and refreshes their `Cargo.lock` if a dependency changed).
- `test_mri_replica.py::test_the_replica_itself` with `LARMORX_NC_BIN` set and no nc build: a follow-up for next-steps
  (put its exact text in the handoff); not this part's file.

## FROM THE ORCHESTRATOR (2026-11-09 03:35, after relay 10)
- Relay 10's agent id: a5033e389fa9c9e19 (in the agents' line). Its numbering is agreed: relay 11 = R7a, relay 12 = R7b,
  relay 13 = R8, relay 14 = R9, relay 15 = R9b (SciPy's 4 x 4 `inv` for both kept SciPy versions, with niworkflows'
  `RegridToZooms` and its `gaussian_filter`), relay 16 = R10 (records), relay 17 = R11 (site), relay 18 = R12 (handoff).
  SDC-13 rule C after measuring, SDC-14 rule B, SDC-15 rule C (until R9b computes `zooms_epi`), SDC-16 rule A: agreed.
- The Haswell queue: ds000228-syn (02:04) and ds004884-syn (03:18) `exit 0`; ds005003-syn is the last, running from
  03:18 (about 50 minutes); the queue then writes `FMAP-QUEUE-HASWELL-END`. Relay 11 starts with its re-check
  (`<scratch>/r10/recheck_syn.sh`). The multi-echo re-runs (ds000210, ds000258) follow in the orchestrator's queue and
  are not this part's.
- The oracle cache's `NSLOTS` key (a one-thread ANTs run not shared between a direct call and nipype's stand-in): a
  follow-up for next-steps in the handoff, as written; do not change the cache's key in this part.

## FROM THE ORCHESTRATOR (2026-11-09 04:45, after relay 11)
- Relay 11's agent id: a1a6fcf4ee03fcfb5 (in the agents' line; the orchestrator stopped it after its report, with its
  monitor loop, its work committed and the worktree clean). Its numbering is agreed: relay 12 = R7b, relay 13 = R7c,
  relay 14 = R8, relay 15 = R9, relay 16 = R9b, relay 17 = R10 (records), relay 18 = R11 (site), relay 19 = R12 (handoff).
  SDC-17 rule A (2.15.2's search by default), SDC-18 rule C, SDC-19 never reproduced (nondeterminism, D13), SDC-20 rule
  A, SDC-21 rule A, PIPE-25 rule A: agreed.
- All seven Haswell field-map runs are done and re-checked (RUN_INFO in `docs/validation/oracle-runs.md` on main,
  49752493). The orchestrator is re-making ds000210 (running since 04:27) and then ds000258 with Haswell's kernels: until
  `ME-QUEUE-HASWELL-END` is in `oracles/logs/me-queue-haswell.log`, skip those two runs in any check (as relay 11 did).

## FROM THE ORCHESTRATOR (2026-11-09 05:16, after relay 12)
- Relay 12's agent id: ab43113819ad79ad9 (in the agents' line). Its plan for relay 13 (R7c: fMRIPrep's own
  `init_fmap_preproc_wf` call, qwarp's Python steps, `unwarp_workflow`, `epi_mask`, `match_histogram`) is agreed. Relay
  13 decides the split itself: if it does not fit, it ends at a clean point and names relay 14 = R7d in NOTES, and every
  later relay moves one number on (R8 = 15, ..., R12 = 20); the orchestrator agrees that numbering now.
- PIPE-26 never reproduced (nondeterminism, D13), PIPE-27 rule A, PIPE-28 rule B, PIPE-29 clamp with a warning (D13's
  rule), PIPE-30 rule A, SDC-22 rule B: agreed. The data sinks' LAPACK 3.12.0 for fMRIPrep's image files (3.12.1 by
  default): write it into the part's findings with the ds006736 case (the records relay's text), as relay 12 did.
- ds000210 re-made with Haswell's kernels (exit 0, 05:06); ds000258 is running (`ME-QUEUE-HASWELL-END` when done).

## FROM THE ORCHESTRATOR (2026-11-09 05:55, after relay 13)
- Relay 13's agent id: a9b207f19eb5645ce (in the agents' line). The split is agreed: relay 14 = R7d, relay 15 = R8,
  relay 16 = R9, relay 17 = R9b, relay 18 = R10 (records), relay 19 = R11 (site), relay 20 = R12 (handoff).
  PIPE-31 rule B, SDC-23 rule B, SDC-24 rule A: agreed.
- **First in relay 14, before R7d's ports: close relay 13's open question; it must not wait for the records relay.**
  (a) Run the stage end to end against fMRIPrep's own `out/` and work folders the way fMRIPrep ran it: sdcflows
  2.15.1's PEPOLAR wiring (fMRIPrep 25.2.6's), not 2.17's, so ds001600's PEPOLAR estimator is compared like for like.
  (b) Find the cause of ds001600's three phase-difference `desc-preproc` field maps differing from fMRIPrep's although
  their coefficients are identical. Separate the causes (porting guide): the field's own computation, the data sink's
  header step (LAPACK 3.12.0), the magnitude/N4 input if the field depends on it, fMRIPrep's library versions. A larmorx
  mistake is fixed with a permanent check; an explained difference is written into the part's findings with its
  numbers. No unexplained difference goes to the records.
- All nine fMRIPrep runs of 2026-11-08 are now made with Haswell's kernels (`ME-QUEUE-HASWELL-END`, 05:41; RUN_INFO in
  `docs/validation/oracle-runs.md` on main): ds000210 and ds000258 are no longer to be skipped.

## FROM THE ORCHESTRATOR (2026-11-09 06:35, after relay 14)
- Relay 14's agent id: a7ededd34215e0b82 (in the agents' line). The split is agreed: relay 15 = R7e (`epi_mask` with
  SciPy's `watershed_ift` and `center_of_mass` in Rust), relay 16 = R8, relay 17 = R9, relay 18 = R9b, relay 19 = R10
  (records), relay 20 = R11 (site), relay 21 = R12 (handoff). SDC-25 rule C, SDC-26 rule B, SKI-02 rule A, SDC-27 (D13's
  clamp with a warning): agreed. Relay 13's open question is closed as relay 14 explained it (FSL's version stamp,
  N4's 8 threads, numpy 2.3's sum order): those numbers go into the part's findings and the records as written.
- Relay 15 changes Rust: its gates are the root workspace's (`scripts/orchestration/gates-rust.sh <checkout> root`), and
  a `cargo check` of crates-gpl and crates-nc if the Apache crates' public signatures it uses change.

## FROM THE ORCHESTRATOR (2026-11-09 07:27, after relay 15)
- Relay 15's agent id: a1daa4575f3643c99. Relay 16 = R8 (sdcflows' command line), then 17 = R9 (defects), 18 = R9b,
  19 = R10 (records), 20 = R11 (site), 21 = R12 (handoff), as agreed.
- **Defect-ID ranges, widened:** the t2smap replica is merged and used no SciPy ID (main's register holds SCI-01 to
  SCI-05), so this part's SciPy range is now **SCI-06 to SCI-20**: the defects relay replaces SCI-W1..W4 with free
  numbers in it. **SDC-12 to SDC-45** (SDC-28 is used) and PIPE-18 to PIPE-40 as before; NP-05..08, NIB-05..08, SKI-01
  onwards unchanged. A relay that needs more asks the orchestrator first.
- **The corrected watershed (SCI-W1..W4) by rule A needs the measurement behind it before the defects relay registers
  it:** the default changes `epi_mask` a lot (ds000228 from the whole image to 47,081 voxels, ds003592 from 12,843 to
  51,162). sdcflows' own workflows do not call `epi_mask` (only its tests; the SyN workflow's `epi_mask` is
  `BrainExtraction`'s), so the pipeline's results do not move; the library default does. In the defects relay: for
  each of the four oracle BOLD references, compare both masks with an independent brain mask on the same grid
  (fMRIPrep's own BOLD brain mask of that run, Dice and the voxels outside the head); write the numbers in the findings.
  If the corrected mask is not better on that evidence, the default follows SciPy (rule C) and the correction stays a
  switch. The independent Python implementation's 600/600 shows the labels follow SciPy's documented algorithm; that
  is the reason, the masks are the measured effect.
- The 64-squared references' slower first erosion (nibabel's Fortran order on the slow path): a follow-up for
  next-steps in the handoff, unless the records or site relay finds it cheap to close.

## FROM THE ORCHESTRATOR (2026-11-09 08:50, after relay 16)
- Relay 16's agent id: a19e3c20cbb8f7312. SDC-29 rule B, SDC-30..32 rule A, SDC-33 rule B, SDC-34 never reproduced
  (nondeterminism), SDC-35 and SDC-36 rule A: agreed. Relay 16's candidates (fMRIPrep's `map_fieldmap_estimation` with
  SDC-36's defect, pybids' database written into the dataset, the import without `HOME`, the reports' figure names):
  relay 17 decides them by rules A-D.
- **One agent at a time again** (the user, 2026-11-09 08:00): nothing runs beside this part; the relays go on as
  planned (relay 17 = R9, 18 = R9b, 19 = R10 records, 20 = R11 site, 21 = R12 handoff).
- **D9 is decided (the user, 2026-11-09; main's decision log, d6d3e8e7): a replica and a clean-room original of FSL's
  `topup` settings files.** Relay 17 is **the spec writer for this** (porting guide §1: behaviour and values only, no
  code) and the replica's author:
  (a) FSL 6.0.7.23's six `topup` settings files (`b02b0.cnf`, `b02b0_1.cnf`, `b02b0_2.cnf`, `b02b0_4.cnf`,
  `b02b0_7T.cnf`, `test_acqp.cnf`; FSL's install `~/fsl-6.0.7.23/etc/flirtsch/` (written `$FSLDIR/etc/flirtsch/` in committed files), checked against FSL's conda package
  that ships them, as the README beside does for the standard images) byte for byte into
  `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/etc/flirtsch/`, with their MANIFEST rows, the README's table and
  `notices.rs`' check updated (that README and MANIFEST are larmorx-nc's own files, not shared documents);
  (b) each file's settings into `specs/topup.md` beside sdcflows' `b02b0.cnf` table (§2.5): every option and value per
  level, the comments' meaning in the spec's words (not FSL's text), and the rule sdcflows uses to choose among
  `_1`/`_2`/`_4`; (c) the main package's lookup (`topup_config_file`) keeps the interim order (`$FSLDIR`, then
  larmorx-nc's copy, then the clear error) and says in its message and API page that the main package's own settings
  come with `t2smap`'s clean-room part; (d) the handoff's register/NOTICE lines for the new files. The clean-room
  original itself is not this part's (an agent that never opened FSL's files writes it).

## FROM THE ORCHESTRATOR (2026-11-09 09:05, after relay 17)
- Relay 17's agent id: ae878eb5ad8d31a90. Its numbering is agreed: relay 18 = R9b (SciPy's 4 x 4 `inv` for both kept
  SciPy versions, niworkflows' `RegridToZooms` with `gaussian_filter`, `zooms_epi` computed), relay 19 = R9c (the D12
  parity suites; the audit of the part's `.pyi`, API pages, PROVENANCE and SPDX), relay 20 = R10 (records), relay 21 =
  R11 (site), relay 22 = R12 (handoff).
- **The replica's `--config` looking in larmorx-nc's copy after FSL's own places: confirmed.** It is the 2026-11-04
  decision's pattern (FSL's data shipped in larmorx-nc and read when FSL is absent; the copy is FSL's own file, so the
  result is FSL's). FSL's own order stays first (as named, `.cnf`, `$FSLDIR/etc/flirtsch/`), FSL's messages stay exact
  for a name found nowhere, and the API page and `specs/topup.md`'s lookup paragraph say where the copy comes in.
- The watershed's rule A stands on relay 17's numbers (better on 3 of 4 references, the same on 1, worse on none):
  those numbers go into the findings and the record as written.
- **PIPE-32** (fMRIPrep: two participants sharing a field-map identifier stop a multi-participant run): the handoff
  relay drafts an upstream report in `docs/upstream-reports/` ("draft, not yet filed"; the user files it).


## FROM THE ORCHESTRATOR (2026-11-09 10:24, after relay 18)
- Relay 18's agent id: a7ba42bb360567da2. SCI-18 rule A: agreed. **PIPE-35 rule D: agreed** (measured with the
  anatomical brain extraction's port, niworkflows' sigma in both modes until then): a register row as a rule-D entry
  with its measurement named, and a next-steps follow-up through the handoff; a `larmorx_core::defects` entry only when
  a correction (and so a switch) exists.
- **One relay added (no `NotImplementedError` left on paths a valid input reaches; "Fixes ... are not postponed"):**
  relay 19 = **R9b2**, then relay 20 = R9c (the D12 parity suites and the part's `.pyi`/API/PROVENANCE/SPDX audit, as
  planned), 21 = R10 (records), 22 = R11 (site), 23 = R12 (handoff). R9b2:
  (a) SciPy 1.18.1's `inv` complete: its symmetric path (the structure detection's symmetric branches: Cholesky
  `potrf`/`pocon`/`potri`, Bunch-Kaufman `sytrf`/`sycon`/`sytri`, as SciPy chooses them) and the condition estimates
  (`gecon`, `trcon`, and the symmetric ones) behind its `LinAlgWarning`, every message and threshold, with the wheel's
  OpenBLAS on the Haswell kernels; a symmetric affine (axes swapped, zero origin) is a valid header nilearn's
  `resample_img` inverts. Relay 18's two candidates (the general path's infinity norm of `A`; the diagonal path printing
  the condition number as "rcond") decided by rules A-D. `check_scipy_inv.py` extended with seeded symmetric
  (definite and indefinite), triangular and near-singular families per condition band, in both environments.
  (b) SciPy's Gaussian derivatives (`order > 0`, per axis, for `gaussian_filter1d` and `gaussian_filter`) and complex
  input and weights in `correlate1d` and the Gaussian filters, as SciPy defines them; `check_regrid.py --sections
  filters` extended.
  (c) OpenBLAS 0.3.28 and SciPy 1.18.1's 0.3.31.dev pinned under `<workspace>/tags/` (sources, SHA-256 checked against
  their release or the scipy-openblas wheel's record) if (a) reaches kernels whose 0.3.30 equivalence is not shown
  bit for bit; otherwise the reason in the findings.
  (d) Unit tests, stubs, API pages, findings, PROVENANCE; a golden case only for a distinct new code path (extend the
  two registered, unrecorded cases where it fits); the gates as relay 18 ran them.
  If (a) alone fills the relay, stop after it with NOTES saying so: (b) and (c) move to relay 20, and the numbering
  shifts by one.

## FROM THE ORCHESTRATOR (2026-11-09 11:02, after relay 19)
- Relay 19's agent id: a902bb371637afc9b. The split is agreed: relay 20 = R9b3 ((b): SciPy's Gaussian derivatives and
  complex input and weights), 21 = R9c (the D12 parity suites and the part's `.pyi`/API/PROVENANCE/SPDX audit), 22 = R10
  (records), 23 = R11 (site), 24 = R12 (handoff). (c) closed without pins on relay 19's evidence: agreed.
- SCI-19 and SCI-20 kept (rule C, a message only; no switch, no `larmorx_core::defects` entry): agreed.
- **SciPy's ID range widened: SCI-06..30** (main's register ends at SCI-05; no other part runs: one agent at a time).

## FROM THE ORCHESTRATOR (2026-11-09 11:34, after relay 20)
- Relay 20's agent id: aa4f59554aad847a1. Numbering unchanged: relay 21 = R9c (with relay 20's addition: the other
  `lx.ndimage` filters' argument handling as SciPy's), 22 = R10 (records), 23 = R11 (site), 24 = R12 (handoff).
- **OpenBLAS's threaded `dgemv` (above 460,800 elements the last bits follow the machine's thread count): larmorx
  follows one thread, agreed** (CLAUDE.md rule 5, results independent of the thread count; D13: what depends on the
  machine is never reproduced; as `ddot`). The findings paragraph stands; no register entry.
- **`np.longdouble` sigma (relay 21, small): no `NotImplementedError`.** numpy's long double is a different type per
  platform (80-bit on x86-64 Linux and macOS Intel, float64 on Windows and Apple silicon, 128-bit on Linux aarch64), so
  SciPy's kernel for such a sigma differs between platforms; larmorx gives one result everywhere: the kernel from the
  sigma as float64, documented on the API page, with a warning naming the platform's wider type when the sigma is
  wider than float64. The parity check on this machine measures the difference from SciPy's 80-bit kernel (counts and
  the largest difference, in the findings, as an explained difference); exactness for x86-64's 80-bit arithmetic only if
  it is cheap.

## FROM THE ORCHESTRATOR (2026-11-09 12:13, after relay 21)
- Relay 21's agent id: af0974b3dbdc43289. **The split is agreed:** relay 22 = R9c(b) (binary morphology, `label`,
  `sum_labels`, `center_of_mass`, `watershed_ift`: argument handling as SciPy 1.18.1's; then the part's audit of
  `.pyi`, API rows, SPDX and PROVENANCE), 23 = R9c(c) (the D12 parity suites), 24 = R10 (records), 25 = R11 (site),
  26 = R12 (handoff).
- SCI-21 (rule A; `reproduce` = SciPy's bytes, never past the output's end: D13), SCI-22 and SCI-23 (rule A), SCI-24
  (1.15.3's, fixed in 1.18.1, followed): agreed. The long-double sigma as done: agreed.
- **The parity suites' design (relay 21's notes): agreed as recommended.** Each case a subprocess of the environment's
  interpreter, one check per summary line, the exit status a check; the originals' step (`stepdigest.oracle`, `STEP`
  lines) for the library scripts only (`check_ndimage`, `check_numpy`, `check_skimage`, `check_scipy_inv`,
  `check_regrid`, `check_epi_mask`, `check_match_histogram`); the workflow scripts without one (sdcflows 2.17.1,
  niworkflows 1.15.4 and nipype are the same in both environments, and the libraries under them have their own step);
  the tiers as proposed, every heavy case boxed.

## FROM THE ORCHESTRATOR (2026-11-09 13:03, after relay 22)
- Relay 22's agent id: a694a281b3b4a0f88. Numbering unchanged: relay 23 = R9c(c) (the D12 parity suites, the design
  agreed at 12:13), 24 = R10 (records), 25 = R11 (site), 26 = R12 (handoff).
- SCI-25 to SCI-30 (rule A, SCI-28 rule B; each with its `reproduce`) and the `label` numbering fix: agreed.
- **SciPy's ID range widened: SCI-06..40.**
- **`_core.ndimage_sum_labels`: kept** (no deletion; the Rust API `larmorx_ndimage::sum_labels_bincount` stays the
  crate's own): its stub's docstring says the Python wrapper follows SciPy's `_stats` with numpy's `bincount` and the
  binding exposes the Rust kernel; one unit test keeps it equal to that path on a few seeded cases, so it cannot rot.
- **Upstream report drafts (relay 26 = R12, in `docs/upstream-reports/`, "draft, not yet filed"; the user files them):**
  PIPE-32 (fMRIPrep) as asked, and SciPy's SCI-21 (the 1-D rank filter writing past the output's end: memory safety
  first), SCI-25 and SCI-29; each with the smallest reproducer, SciPy 1.18.1 and 1.15.3's behaviour, and the
  expected result.

## FROM THE ORCHESTRATOR (2026-11-09 14:01, after relay 23)
- Relay 23's agent id: a8db1ba28a0b88624. **The split is agreed:** relay 24 = R9c(d) (the step's remaining changes as
  relay 23's plan says), 25 = R10 (records), 26 = R11 (site), 27 = R12 (handoff).
- The check-script fix in `check_app.py` (the settings section's search; SDC-30/31 now 4/4): agreed, not a larmorx mistake.
- **A suite's case count must not vary between runs** (relay 23's note: check_ndimage's SCI-21 count can differ by one
  when SciPy's process crashes): relay 24 makes it deterministic. Each SciPy call that writes past the output's end
  (SCI-21) runs in its own child process, and its crash, hang (time limit) or result is one recorded outcome; the
  comparison there is `reproduce`'s defined bytes against SciPy's when SciPy returns, and "SciPy crashed" counted as
  such otherwise, so the counts are the same on every run. These cases are `case_never` for the oracle-output cache
  (memory never written, CLAUDE.md's cache rule).
- **The full tier's 9 cases:** the records relay (25) runs them once, from its clean commit, as the record's source
  (boxed; the long ones in the background while it does the rest of the records), not before.

## FROM THE ORCHESTRATOR (2026-11-09 14:26, after relay 24)
- Relay 24's agent id: a3f4ba1c1e9c59f40. The four reasons, the digest fixes and SCI-21's retried calls (each call one
  case, never cached, the INFO line for a call no process answered): agreed. Numbering unchanged: relay 25 = R10
  (records), 26 = R11 (site), 27 = R12 (handoff).
- **Relay 25's resources: one agent runs at a time** (the user, 2026-11-09), so this relay may use up to 28 threads
  (cargo `-j 28`, parity `--jobs` to match where the cases allow) and memory boxes up to about 90 GiB together
  (each heavy case in its own box, as its README row says). R10's whole list is in relays 21's to 24's plans (the
  three suites' records with the step, the full tier's 9 cases once from the clean commit, the app's whole runs and
  more D8(d) datasets, the memory calls, the 37 golden cases with `record_golden.py`, the difftest descriptors and one
  short boxed trial, the smoke tier, the three workspaces' gates); split at a clean point if it does not fit (records
  first, then golden and the trial).

## FROM THE ORCHESTRATOR (2026-11-09 17:38, after relay 25)
- **Numbering agreed:** 26 = R10b (relay 25's plan (a) to (e)), 27 = R11 site, 28 = R12 handoff; relay 25's split
  (26 = (a) + (b), 27 = (c) + (d) + (e), then 28 site, 29 handoff) agreed too if it does not fit: say so in NOTES.
- **SDC-45 rule A agreed** (a dataset below any folder is a sensible input; the stop is wrong): the BIDS part by default,
  `reproduce` = sdcflows' absolute-path parse with its error. Every other absolute-path parse found (fMRIPrep's
  `extract_entities`, `lx.bids`' index) decided by the rules A-D, each a register entry of its own family if it is a
  defect (PIPE-36 on for fMRIPrep's). **ID range widened: SDC-12..55.**
- **d1491604 (main's stages-catalogue roadmap entry for the SyN runs) into the branch:** `git cherry-pick -x d1491604`
  (one file, `validation/src/larmorx_validation/stages/catalogue.py`; no other main commit). Its message's plan, part B
  turns the roadmap entry into stages: do it if part B's ports cover those eleven nodes (the catalogue's other ported
  entries the model), else keep the entry and say so in NOTES. Then the smoke tier must pass whole, and
  `record_golden.py` runs normally (no `--skip-parity`) from a clean commit, boxed, in the worktree. I resolve the
  catalogue at the merge (MERGE NOTE).
- The records' `/tmp/lxpb_*` folder names in five reported lines: left as they are (no account name; compare_records
  strips them); a follow-up for next-steps in the handoff.
- Resources as relay 25: up to 28 threads (cargo -j 28, parity --jobs/--parity-workers to match), boxes ~90 GiB in total,
  each heavy case in its own box. Smart testing, not extensive (the user).

## FROM THE ORCHESTRATOR (2026-11-09 19:00, after relay 26)
- **Numbering agreed:** 27 = R10c (random testing, relay 26's plan (c)), 28 = (d) benchmark + (e) gates (or both with 27
  if they fit: say so in NOTES, and the rest moves up), 29 = R11 site, 30 = R12 handoff.
- **SDC-45, PIPE-37 (rule A) and PIPE-36 (found, not ported; rule A when larmorprepx's BOLD steps are ported) agreed.**
  The upstream drafts of R12 (in `docs/upstream-reports/`, "draft, not yet filed"; the user files them): PIPE-32, SCI-21,
  SCI-25, SCI-29, and now SDC-45 (sdcflows), PIPE-36 (fMRIPrep), PIPE-37 (pybids). Free IDs from here: PIPE-38..40,
  SCI-31..40, SKI-05 on, SDC-46..55.
- **The two "unexplained" stage nodes of the scratch check on ds000228-syn** (within threshold, the reference re-runs
  skipped): left for the final comparisons with fMRIPrep (the user: port every tool first, compare at the end); a
  follow-up line for next-steps in the handoff, no more work on it in this part.
- `test_the_replica_itself`'s failure with `LARMORX_NC_BIN` set: the known follow-up (relay 9's); gates with it unset.
- Resources as before: up to 28 threads, boxes ~90 GiB in total, each case under the trial's memory cap, memory findings
  re-checked one at a time, boxed. Smart testing, not extensive (the user).

## FROM THE ORCHESTRATOR (2026-11-09 21:25, after relay 27)
- **Numbering agreed:** 28 = (d) benchmark + (e) golden for the new code paths + the parity smoke tier (gates again only if
  28 changes code), 29 = R11 site, 30 = R12 handoff.
- **PIPE range widened: PIPE-41..50** (18..40 used up). Free IDs: PIPE-41..50, SCI-33..40, SKI-05 on, SDC-47..55.
- SCI-31, SCI-32 (out-of-bounds: never reproduced, D13), SDC-46, PIPE-38 (C), PIPE-39 (nondeterministic: not
  reproduced), PIPE-40 agreed.
- **The random walker's slow non-converging iterations (about 9x SciPy's per iteration):** in relay 28's benchmark,
  profile it once on the trial's case (where the time goes). If the cause is contained and the bits stay the same
  (the permanent cases and `reproduce` against skimage on the trial's brain_masker cases), fix it; else a follow-up for
  next-steps in the handoff with the profile's numbers. No more than that in this part.
- Resources as before (up to 28 threads, boxes ~90 GiB in total). Smart testing, not extensive (the user).

## FROM THE ORCHESTRATOR (2026-11-09 22:57, after relay 28)
- **Numbering agreed:** 29 = R11 the site (relay 28's plan), 30 = R12 the handoff; if the site does not fit: 29 = R11a
  (evidence module + exports, committed in the larmorx branch), 30 = R11b (the pages + the site's checks), 31 = R12.
- The random walker's NaN stop (2b07a2e7) and the benchmark agreed; IntensityClip's and RobustAverage's one-thread speed
  a follow-up line for next-steps in the handoff (no more work on it in this part).
- The site: the clone from the site's `main` and `evidence-blobs` as they are now (site main 4d86f086, evidence-blobs
  ae1a3b62, 1,343 blobs: check before you start; added blobs only, none replaced or removed), branch `fmap-b-pages`,
  the no-reply identity first. The site's evidence only ever improves (the user): every other page's HTML unchanged,
  the before/after inventory per page (after >= before), screenshots at 1440 and 390 in light and dark, `tools/build.py`
  0 problems, every view drawn. Real images only inside each step's mask, dilated (no faces).
- Resources as before. Smart testing, not extensive (the user).

## FROM THE ORCHESTRATOR (2026-11-09 23:48, after relay 29)
- **Numbering agreed:** 30 = R11b (the page and the site's checks, relay 29's plan) WITH the SyN entry below; 31 = R12
  the handoff. If it does not fit: 30 = the SyN entry (cache entry, export) + the page, 31 = the site's checks, the
  inventory, the screenshots and the blobs, 32 = R12. Say so in NOTES.
- **The SyN viewer: made in this part, not deferred** (the site shows every path of the part; SyN is one of the three
  estimators). Bounded, smart: `syn_sdc_workflow` on ds000228-syn: larmorx's field beside the ANTs oracle (GCC 14.3
  build) at one thread with the CORE-MATH preload (byte for byte, relay 10), ANTs' own thread band for this call (the
  oracle with glibc's maths as fMRIPrep's image runs it, at 1 and 4 threads, and 4 threads once more if ANTs repeats
  itself at a fixed thread count is not already known: at most four ANTs runs, boxed, through `oracle_cache.run` with
  `case_never` where a run measures the original's own spread), and fMRIPrep's own 4-thread field placed against that
  band with its verdict from `data/verdicts.yml` and the reason. One cache entry from a clean commit (never edit
  tracked files while it computes). If ANTs at 4 threads proves not to repeat itself, say so: the band then holds
  those runs' spread.
- PEPOLAR on ds006736 agreed. The register draft in the repository (`relay17/register_part_b.py`) is the one relay 31
  (or 32) regenerates the register text from; the stale scratch copy left as it is.
- Resources as before (up to 28 threads, boxes ~90 GiB in total). Smart testing, not extensive (the user).

## FROM THE ORCHESTRATOR (2026-11-10 00:51, after relay 30)
- **Numbering agreed:** 31 = R12, the handoff and the part's final report (relay 30's plan). If it does not fit: 31 =
  the register text + HANDOFF.toml/HANDOFF.md (every `<scratch>/handoff/relay17..30` item), 32 = the upstream drafts
  + the final report. Say so in NOTES.
- **`docs/findings/ants-syn.md`:** edit it in the branch (a findings file, not a shared document): keep the earlier
  measurement, scoped to its own cases (2, 4 and 8 threads: 0 to 3.6e-14 mm), and add relay 30's on sdcflows'
  fieldmap-less call (relay 30's proposed sentence: four threads up to 0.835 mm from one thread's, 0.659 mm run to run;
  the field 1.70 and 0.70 Hz; the Mattes histograms' sums per work unit; larmorx the one-thread result at any thread
  count), so the "In short" no longer claims thread independence in general. Name the change in the final report.
- The upstream drafts (`docs/upstream-reports/`, "draft, not yet filed"; the user files them): PIPE-32, SCI-21,
  SCI-25, SCI-29, SDC-45 (sdcflows), PIPE-36 (fMRIPrep), PIPE-37 (pybids). No account name or machine path in any
  of them (placeholders `<workspace>`, `<scratch>`, `~`).
- The final report: what was built, covered and not, the numbers, the findings, the gates, the commits, the agents'
  ids (relays 1 to 31, from NOTES' agents line), the merge notes (numpy_linalg.rs dbdsqr_upper; catalogue.py; the
  golden NaN case on the other platforms; the site's shared files), as earlier parts' reports.

## FROM THE ORCHESTRATOR (2026-11-10 01:26, after relay 31)
- **Numbering agreed:** 32 = R12b, the seven upstream drafts and the part's final report (relay 31's plan (1) to (3)).
- **The agents' ids:** NOTES' agents line (line 25) now holds every relay's id from 1 to 31 (27 = aff910a486c582678,
  28 = aa113697e2f6b45b7, 30 = a0e0bb7a68ebd3989, 31 = a56b27a2845fe52d5 were missing: added). Bring HANDOFF.md's
  agents' list and the final report's in line with it; relay 32's own id is the orchestrator's to add at the merge.
- The ndimage duplicate (SciPy's Gaussian filter, correlate1d, binary morphology and label in larmorx-interp, t2smap's,
  and larmorx-ndimage, part B's): a follow-up in next-steps' part B block (already there); not done in this part.
- `check_spdx.py` passing compound licence headers without checking their terms: a follow-up line for next-steps in the
  handoff (relay 31 found two files it would not have caught).
