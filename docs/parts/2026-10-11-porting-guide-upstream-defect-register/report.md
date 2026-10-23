# Final report: Porting guide + upstream defect register

Agent `a6cbcf2eeebad0df0`, 2026-10-11. The agent's report to the orchestrator, verbatim.

I've written both documents, linked them everywhere you asked and committed. Gates pass (ruff check, ruff format --check, check_spdx). `pytest tests/golden` gave 83 passed. Nothing is pushed.

**Before you merge:** the rule 8 text, the D13 line and the "up to two agents" line in CLAUDE.md, and the decision-log D13 and "Oracle datasets" entries, exist only as uncommitted edits in the main checkout. My branch starts from main at 4556425 and does not have them. My CLAUDE.md edits keep clear of those lines (I touched rule 1, rule 7, the findings bullet and the repo map). So committing your edits first and then merging should be clean. Both new docs refer to those decision-log headings by name.

## `docs/porting-guide.md` (method only, so clean-room implementers can read it)
It opens with an 11-step checklist, then one section per step:
1. The tracks, by the licence of each file; what the spec writer, the clean-room implementer and the replica author may each read.
2. The source is the specification: the N4 case, how to tell a compiler artefact from what the source says (C/C++ type rules, a second build, `objdump`, fused multiply-adds, undefined behaviour), record the oracle's exact build, compare against a second build such as fMRIPrep's image or ANTsPy.
3. Measure the original's own variability first: threads (ITK's variables, `NSLOTS`, work units), repeated runs, seeds, small input changes (mcflirt's band), neighbouring versions.
4. Oracle data that cover every code path. This is your added point: ds000005 left 3dTshift and distortion correction untested; ds001600, ds005454 and ds005752 fill that gap.
5. Read the parameters the pipeline really passes (`shrink_factor` dropped, `NSLOTS`, `ANTS_RANDOM_SEED=50495`, rewritten headers, `OMP_NUM_THREADS=1`), confirmed on a `work/` directory.
6. Take differences apart one cause at a time, using the stage comparison's chain (threads, the CORE-MATH preload, the build, larmorx), plus BLAS kernels.
7. Validation records lead with the unmodified original; statuses follow PLAN §4 and rule 8.
8. D11 and D13: a class-by-class table and how parity uses the compatibility switch. The switch's shape is only a proposal.
9. Determinism and platforms, including turning racing scatters into gathers, golden tests and the CI budget.
10. Working safely: commit early, at most two agents, wait loops, never delete, which larmorx the shared venv imports, the crates-gpl lockfile, deterministic test data, SPDX headers and notices.
11. The per-tool contract as a 12-item table, with the models you named.

## `docs/findings/upstream-defects.md` (the register)
- **Layout:** a banner and class legend, a 120-row summary, the D13 gaps, then the entries grouped as ANTs/ITK; fMRIPrep and its libraries, SciPy, numpy, nibabel, glibc, ANTsPy; and two restricted sections, AFNI 3dTshift and FSL mcflirt.
- **Restricted entries** are marked †. Clean-room implementers must not read those two sections. The summary and the gap lists describe them by observable behaviour only.
- **Defects by class** (entries with two classes count under the first):

| Class | Entries |
|---|---|
| (a) crash, undefined behaviour, out-of-bounds read | 13 |
| (b) race or nondeterminism | 6 |
| (c) deterministic bug that changes results | 54, of which 15 are questionable |
| (d) imprecision | 16 |
| (e) command-line or parsing quirk | 25 |
| (f) documentation or message only | 6 |

- **Gap column:** 47 must switch, 25 need a decision first, 1 is a rule-8 item (N4), 1 is still unexplained (ANTS-46, the early-stopping registration, left for A4).

## D13 gaps for the next task (larmorx reproduces these by default today)
**ANTs and ITK, main package:**
- ANTS-20 `GetLargestComponent` makes an empty result a full image (`larmorx_ants::image_math::components::largest_component`); sMRIPrep calls it 4 times.
- ANTS-38 N4's histogram range drops a first-voxel minimum (`larmorx_image::n4::sharpen`).
- ANTS-43 DenoiseImage's Rician correction: NaN becomes a bias of 0 (`larmorx_image::adaptive_denoising`).
- ANTS-12 `TruncateImageIntensity` range (`image_math::intensity::truncate_image_intensity`).
- ANTS-19, class (a): `FillHoles` with `holeparam ≤ 1` reads outside the image (`image_math::morphology::fill_holes`).
- ANTS-11 `Neg`; ANTS-08, 09, 10 (`/`, `total`/`mean`, `vtotal`) in `image_math::arithmetic`; ANTS-13 `Normalize` (`intensity::normalize`).
- ANTS-14 `G` gives NaN for large variances (`larmorx_image::discrete_gaussian`).
- ANTS-25 Otsu with a mask (`larmorx_ants::threshold_image`, line 106); ANTS-39 N4's FFT size (`n4.rs:157`).
- ITK-01 floor of tiny negatives (`larmorx_image::grid`, in the 4D resample paths); ITK-02 extra dimensions lose their direction (`larmorx_ants::image::from_itk`); ITK-03 float labels stop at 2^24 (`float_labels`).

**ANTs command lines, main package:**
- ANTS-04 ImageMath exits 0 after a failure (`cli::image_math`).
- ANTS-27 MultiplyImages multiplies by 0 when a file is missing (`cli::multiply_images`).
- ANTS-35, 36 N4 options without brackets; unreadable mask; `-r` without a mask (`cli::n4_bias_field_correction`).
- ANTS-16 `Laplacian` flag and ANTS-15 sigma count (`cli::image_math::gaussian`, `cli::smooth_image`).
- ANTS-03 short file names (`image.rs:384,398`); ANTS-05 operand parsing (`cli::cstd::from_string_f32`); ANTS-24 ThresholdImage mode names.
- ANTS-30 ResampleImage's seventh argument (needs a design); ANTS-31 ResampleImageBySpacing's arguments; ANTS-33 antsApplyTransforms `-u` and ignored interpolator parameters.

**fMRIPrep's libraries and SciPy:**
- PIPE-03 `ResampleSeries`' Jacobian along the wrong target axis (`larmorx_transform::resample_series`). It affects PEPOLAR field maps in T1w and standard spaces.
- PIPE-04 nitransforms reads `.h5` composites in name order (`python/larmorx/transforms/chain.py:348`); latent.
- SCI-03 `reflect`/`grid-mirror` splines do not interpolate (`larmorx_interp::ndimage::splines`).

**Head-motion correction (`larmorx-mri`; the spec already describes each one):**
- FSL-01 last volume not initialised from the previous one (`hmc::estimate::chains`).
- FSL-14 one NaN voxel makes the matrix NaN.
- FSL-03 in-plane mode pads twice (`estimate.rs:336`).
- FSL-08 `-stats` end slices and names; FSL-09 output types; FSL-10 units forced to mm and s (`image.rs:315`).
- FSL-05, 06, 07, 13 command-line quirks (`cli::hmc`).

**3dTshift, both implementations:**
- AFNI-01 negative `scl_slope`; AFNI-05 `-no_detrend` voxel pairs; AFNI-03 header slice code 2; AFNI-08 the all-zero slice-time list.
- AFNI-02 (float32 scaled twice) and AFNI-07 (metre units): the replica needs a new default. The original already behaves correctly but needs a spec addition before it can get the warning and the switch.

## Decisions the user needs to make, listed in the register
1. Undefined float-to-integer conversions (ANTS-06, SCI-02). larmorx's command lines reproduce x86-64 here, decided on 2026-10-09, which conflicts with D13.
2. The 12 questionable behaviours of D13's scope, including N4's non-positive log values and no displacement beyond a field's edge. (Of the 15 questionable entries, ANTSPY-01 needs no larmorx change, PIPE-08 is under item 6, and ANTS-46 is open.)
3. Whether replicas share the corrected default. `implementation="auto"` runs the replica when installed, so I assumed they must.
4. mcflirt's inflated weight count (FSL-02): fixing it changes every run, so measure it against mcflirt's band first.
5. Refusing instead of offering a switch, for cases where the original's output is garbage or a failure (8 entries).
6. Pipeline choices for larmorprepx: the N4 shrink factor (PIPE-01) and the motion-parameter centre (PIPE-08).
7. The design of ResampleImage's corrected command line (ANTS-30).

I also listed the work already decided under rule 8, not D13: N4's GCC 11 artefact and `--float 1`.

## Other changes
- While writing the register I checked three things and recorded them in their findings files:
  - nitransforms' `.h5` group order: read in its source; not tried with a real 11-transform file.
  - larmorx's `.h5` writer gives the same bytes on every write: checked by writing one transform twice.
  - ImageMath `Normalize`'s 1e9 starting minimum: read in the ANTs source.
- `docs/validation/ants-apply-transforms.md` still says larmorx uses the libm crate, which is out of date. I left it alone, since records are not edited by hand.

## Commits (worktree branch `worktree-agent-a6cbcf2eeebad0df0`, author karellopez, no AI attribution)
- `65538e1` porting guide
- `42c3da7` defect register (with the porting guide's agents-line update)
- `6865b26` links from CLAUDE.md, overview.md, findings/README.md, architecture.md
- `2d840aa` three findings additions

Files are in `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a6cbcf2eeebad0df0/`:
- `docs/porting-guide.md`
- `docs/findings/upstream-defects.md`
