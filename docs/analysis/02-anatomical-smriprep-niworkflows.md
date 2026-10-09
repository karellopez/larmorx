<!-- Generated 2026-10-08 by a read-only analysis agent against shallow clones in the parent directory.
     fmriprep @ 21a490fb (26.0.0.dev). Line numbers refer to those clones and will drift. -->

# sMRIPrep / niworkflows anatomical map for larmorx (read-only analysis)

Paths are relative to the workspace root (the directory that contains `larmorx/` and the upstream clones). Abbreviations: SMR = `smriprep/src/smriprep`, NWF = `niworkflows/niworkflows`, NR = `nireports/nireports`.

## 0. Key findings

- **Main entry point:** `init_anat_fit_wf` (SMR/workflows/anatomical.py:468) is what fMRIPrep calls (fmriprep/workflows/base.py:174,364). `init_anat_preproc_wf` (:98) is a compatibility wrapper that adds outputs in standard spaces and CIFTI.
- **Every stage can be skipped** if the derivative already exists (`precomputed`, from `collect_anat_derivatives`, SMR/utils/bids.py:122).
- **Hard non-portable dependencies:**
  - FreeSurfer: `recon-all` is a tcsh script (`#!/bin/tcsh -f`) and needs a licence key.
  - FSL FAST: non-commercial licence.
  - MSM with the HOCR optimiser: research/non-commercial only (MSM_HOCR/Licenses/*). The ELC part needs the author's consent to redistribute, and FastPD is covered by pending patents. A Rust port of that code would inherit those terms, so it needs a clean-room design or a different optimiser.
  - Connectome Workbench is GPLv2. Porting its code makes the result GPLv2; reimplementing from the documentation avoids that.
  - ANTs is Apache-2.0 and AFNI is mostly public domain, so both are safe to port.
- **Windows blocker in sMRIPrep itself:** SMR/cli/run.py:373 calls `set_start_method('forkserver')`, which does not exist on Windows. fMRIPrep does the same (fmriprep/config.py:104).
- **Latent bugs that matter for exact reproduction:**
  - NWF/func/util.py:487-493: `shrink_factor=2` is passed to `pe.Node(...)`, not to N4. nipype's Node silently drops unknown kwargs (nipype/pipeline/engine/nodes.py:105,181), so BOLD-reference N4 actually runs with ANTs' default shrink factor.
  - Only `t1w-mni_registration_{precise,testing}_000.json` set `write_composite_transform: true`. If attempt 000 fails and 001/002 run, `composite_transform` is Undefined, but sMRIPrep reads it (fit/registration.py:241-244), so a fallback success would break downstream.
  - SMR/workflows/anatomical.py:1446: the boilerplate f-string uses `{image}`, which renders the nipype `image` module instead of the image type.
  - SMR/cli/run.py:140: the default `'first'` for `--subject-anatomical-reference` is not one of the allowed choices (`first-lex` / `unbiased` / `sessionwise`).

## 1. sMRIPrep anatomical workflow, in execution order

### Stage 1 – conformation and averaging
`init_anat_template_wf` (anatomical.py:1388); called for T1w at :787 and for T2w at :1137.

1. `TemplateDimensions` (:1459; NWF/interfaces/images.py:402) – pure Python.
   - Reorients every input to RAS.
   - Target voxel size = smallest per axis; target shape = largest per axis.
   - Iteratively drops images that would need more than `max_scale` (default 3×) upsampling.
   - Writes an HTML conformation report.
2. ANTs `DenoiseImage -n Rician` (:1460-1464), one call per input, `num_threads=omp`. This runs **even when there is only one image**.
3. `Conform` (:1465; images.py:496) – pure Python (nibabel + `nilearn.resample_img`).
   - RAS reorientation; resamples only if voxel size differs by more than 0.05 mm or the shape differs.
   - Changes zooms in the affine if they differ by more than 0.001 mm.
   - Writes a voxel-to-voxel `.mat`.
4. One image: the output is the conformed image plus the bundled `itkIdentityTransform.txt` (:1481-1489). No N4 at this stage.
5. More than one image:
   - `lta_convert --inlta identity.nofile` per image (:1491).
   - `N4BiasFieldCorrection -d 3` with ANTs defaults, `n_procs=1` (:1502-1507).
   - `mri_robust_template` through `StructuralReference` (:1509-1521; NWF/interfaces/freesurfer.py:48) with:
     - `--satit` (auto sensitivity), `--inittp 1`, `--iscale` (7 DOF: rigid plus intensity), `--subsample 200`
     - `--fixtp` and `--noit` unless longitudinal
     - LTA outputs, threads = min(n, omp)
   - nipype `image.Reorient` (pure Python), because `mri_robust_template` may write LIA.
   - `ConcatenateXFMs(inverse=True)` (nitransforms).
6. `ValidateImage` (anatomical.py:777) – pure Python header and qform/sform sanitising.

### Stage 2 – N4 and brain extraction
Code: anatomical.py:828-937; the brain-extraction workflow is `init_brain_extraction_wf` (NWF/anat/ants.py:42).

**Mode selection** (`--skull-strip-mode`, default `auto`, cli/run.py:225-229):
- `_is_skull_stripped` (:1638) sums |edge-voxel values| and calls the image skull-stripped if the sum is < 10.
- Image is pre-stripped → `init_n4_only_wf` (ants.py:826): mask = `Binarize(thresh_low=2)`, then N4 and Atropos refinement.
- Precomputed mask → `ApplyMask`, then n4_only.

**Default template:** `OASIS30ANTs` (cli/run.py:213-218), at resolution 1 with fallback (`get_template_specs`, NWF/utils/misc.py:41).

Brain-extraction steps, in order:

| # | Node (ants.py line) | Tool | Parameters |
|---|---|---|---|
| a | `truncate_images` :238 | ImageMath `TruncateImageIntensity` | `0.01 0.999 256` |
| b | `inu_n4` :243 | N4BiasFieldCorrection | `-d 3 -s 4 -b [200] -c [50x50x50x50,1e-7]`, no bias field saved |
| c | `res_tmpl` / `res_target` :258-262 | `RegridToZooms` (pure Python) | 4 mm, smoothed |
| d | `init_aff` :271-292 | antsAI | `-m Mattes[32,Regular,0.25] -t Affine[0.1] -s [15,0.1] -g [40,0x40x40] -p 0 -c [10,1e-6,10]`; fixed mask = template `desc-BrainCerebellumExtraction_mask` (:218) |
| e | `lap_tmpl` / `lap_target` :377-385 | ImageMath `Laplacian` | `1.5 1` (σ = 1.5, normalised) |
| f | `norm` :295-310 | antsRegistration from `antsBrainExtraction_precise.json` (`_testing.json` with `--sloppy`) | two channels (T1w + Laplacian); initialised from antsAI; `--float`; fixed mask = BrainCerebellumExtraction (`fixed_image_masks` for ANTs ≥ 2.2) |
| g | `map_brainmask` :312 | antsApplyTransforms, Gaussian interpolation | template `label-brain_probseg` (or `desc-brain_mask`) mapped to subject via reverse transforms |
| h | `thr_brainmask` :319 | ThresholdImage | 0.5–1 → 1/0 |
| i | `map_wmmask` :351-375 | antsApplyTransforms, Gaussian | `label-WM_probseg` (+ `label-BS_probseg`, combined by pure-Python `_imsum`) |

Atropos refinement (`init_atropos_wf`, ants.py:466). Model for T1w: K = 3, CSF = 1, GM = 2, WM = 3.

1. ImageMath `MD 2` → `GetLargestComponent` (:573-580).
2. **Atropos** (:583-599): `-d 3 -i KMeans[3] -k Gaussian -m [0.1,1x1x1] -c [3,0.0]`, posteriors saved, `--use-random-seed 1` unless `--skull-strip-fixed-seed`. Input is the N4 image from step b.
3. Pad 10 → split labels (pure Python) → `GetLargestComponent` on WM and GM → GM `FillHoles 2` × multiply → CSF `ME 10` → `addtozero` → relabel by multiplying → GM+WM (:602-656).
4. "Superstep 7": `addtozero` → `ME 2` → `GetLargestComponent` → `MD 4` → `FillHoles 2` → `addtozero` with the padded prior mask → `MD 5` → `ME 5` → depad −10 (:660-689).
5. `_conform_mask` and `CopyXForm` (pure Python).
6. Final N4 (:703-726): input = the **original, non-truncated** image; `-s 4 -c [50x50x50x50x50,1e-7] -b [200] -r` (rescale intensities); bias field saved.
   - Weight image when the template has a WM prior: `_improd` = 0.5·(norm(WM posterior × template WM prior, masked) + mask).
   - The WM posterior is chosen by best `FuzzyOverlap` with the prior (:793-820).
7. `ApplyMask`.
8. Outputs: `t1w_preproc` (final N4), mask, `ants_seg` (dseg), tpms.

These parameters match the reference `ANTs/Scripts/antsBrainExtraction.sh` (lines 78-94, 349-359), except that the nipype port uses 5 levels in the final N4 and `-b [200]` instead of `[1x1x1,3]`.

### Stage 3 – tissue segmentation (FSL FAST, not Atropos)
anatomical.py:940-987.

- `FAST(segments=True, no_bias=True, probability_maps=True, bias_iters=0)`, giving `fast -I 0 -N -g -p -S 1`, run on the *refined* brain (`refined_buffer.t1w_brain`).
  - smriprep/interfaces/fsl.py:37 patches the trait so `-I 0` is allowed.
- Atropos output is used only for the brain mask and as an input to `RefineBrainMask`.
- `apply_lut` (SMR/utils/misc.py:26) maps FAST labels to BIDS: LUT (0,3,1,2) gives 1 = GM, 2 = WM, 3 = CSF.
- `_probseg_fast2bids` (anatomical.py:1633) reorders probability maps from CSF/GM/WM to GM, WM, CSF.

### Stage 4 – spatial normalisation
`init_register_template_wf` (fit/registration.py:40). It has an iterable over templates (:159).

- `TemplateFlowSelect(resolution=1+sloppy)` (:171; SMR/interfaces/templateflow.py:61,213-218) fetches T1w, brain mask and T2w.
- `_set_reference` (:269): when the moving image is T2w but the template has no T2w, it falls back to the T1w template with histogram matching off.
- `ImageMath TruncateImageIntensity 0.01 0.999 256` on the moving image (:187-190). The moving image is `t1w_preproc`, **not** brain-extracted.
- `SpatialNormalization(float=True, flavor='precise'|'testing')` (:192-200; NWF/interfaces/norm.py:125):
  - Settings files are globbed as `t1w-mni_registration_{flavor}_*.json` and tried in sorted order 000 → 001 → 002 until one succeeds (norm.py:158-238).
  - If no initial transform is given, `antsAffineInitializer` runs inside the interface (nipype defaults: search 15°, arc fraction 0.1, principal axes off) (:175-193).
  - `explicit_masking=True` by default: the moving image is masked by the refined mask, the fixed image is masked by template `desc-brain_mask`, and no masks are passed to ANTs.
  - A lesion mask (`--roi`) becomes a cost-function mask via `create_cfm` (:276-345).
  - Template resolution: precise = 1, testing/fast = 2 (:404).
- Outputs: `composite_transform` (`.h5`) and the inverse transform.

**Registration JSON files** (NWF/data). Abbreviations: iters = iterations per level, shrink = shrink factors, σ = smoothing sigmas in voxels unless "mm", HM = histogram matching, wins = winsorize quantiles, conv = convergence threshold/window. All files collapse output transforms.

| File | Stages, metric (bins/radius, sampling) | iters | shrink | σ | conv | HM / wins / composite / interp |
|---|---|---|---|---|---|---|
| **t1w-mni_registration_precise_000** (default) | Rigid[0.05] Mattes56 Reg 0.25 · Affine[0.08] Mattes56 Reg 0.25 · SyN[0.1,3,0] CC4 | [100,100] · [100,100] · [100,70,50,20] | [2,1] · [2,1] · [8,4,2,1] | [2,1] · [1,0] · [3,2,1,0] | 1e-6; windows 20/20/10 | HM on · 0.005–0.995 · composite yes · Lanczos |
| precise_001 | Rigid[0.05] · Affine[0.1] Mattes56 Reg 0.3 · SyN[0.2,3,0] Mattes56 0.5 + CC4 0.5 | [100,100]·[100,100]·[100,30,20] | [2,1]·[2,1]·[4,2,1] | [2,1]·[2,1]·[1,0.5,0] | 1e-8, 1e-8, −0.01 | HM [F,F,T] · composite **no** |
| precise_002 | as 001; Affine[0.05]; SyN[0.01,4,0] Mattes32 + CC4; rigid/affine 32 bins | same | same | same | same | HM [F,F,T] · composite **no** |
| testing_000 (sloppy) | Rigid[1.0] · Affine[1.0], Mattes56 Random 0.2/0.1 | [20]·[15] | [2]·[1] | [4]·[2] vox | 1e-7/1e-8 | HM [F,T] · composite yes |
| testing_001 / 002 | Rigid[0.5]/Affine[0.1]; 002: Rigid[0.1]/Affine[0.1] (002 samples 0.5/0.2) | [20]·[15] | [2]·[1] | [4]·[2] (σ units declared as mm) | | composite **no** |
| t1w-mni_registration_fast_000 (sMRIPrep does not use it) | Rigid[0.01] Mattes32 Random 0.15 · Affine[0.08] · SyN Mattes56 | [1000]·[500,250,100]·[50,20] | [4]·[4,2,1]·[2,1] | [4]·[4,2,0]·[1,0] | 1e-6 | |
| antsBrainExtraction_precise | Rigid[0.1] MI32 Reg 0.25 · Affine[0.1] MI32 · SyN[0.1,3,0] CC4 ×2 channels (weight 0.5 each) | [1000,500,250,100]×2 · [50,10,0] | [8,4,2,1]×2 · [4,2,1] | [4,2,1,0]×2 · [2,1,0] | 1e-8, 1e-8, 1e-9; windows 10/10/15 | HM on · 0.025–0.975 · Lanczos |
| antsBrainExtraction_testing | same, iters [100,100,50,10]×2 · [5,0] | | SyN [2,1] | SyN [1,0] | | |
| antsBrainExtractionNoLaplacian_{precise,testing} | single-channel CC; testing iters [100,500,250,100] · [5,10,0] | | | | | |
| boldref-mni_registration_{precise,testing}_00{0,1,2} / epi_atlasbased_brainmask / petref-mni_…precise_000 | BOLD/PET side (fMRIPrep). `epi_atlasbased_brainmask`: Affine[0.1] Mattes64 Random 0.2, [200], shrink [2], σ 2 mm, conv 1e-9 | | | | | |

### Stage 5 – FreeSurfer surface reconstruction
`init_surface_recon_wf` (SMR/workflows/surfaces.py:67). Runs only with FreeSurfer enabled (`--fs-no-reconall` disables it). First, `fs_isRunning` (SMR/utils/misc.py:50) deletes `scripts/IsRunning*` if `recon-all.log` is more than 24 h old, otherwise raises.

1. `FSDetectInputs` (:216; NWF/interfaces/freesurfer.py:189,426) – pure Python.
   - `-hires` if max voxel size < 0.95 mm, plus expert option `mris_inflate -n 50`.
   - T2w (preferred) or FLAIR is used only if voxel size < 1.2 mm.
2. `_check_cw256` (:1728) adds `-cw256` if the FOV is > 256 mm. Default flags are `-noskullstrip -noT2pial -noFLAIRpial` (:219).
3. `recon-all -autorecon1 [-T2 f|-FLAIR f] [-hires] -openmp N` (:221-228). The `-i` input is the Stage 1 T1w reference. This node is never resumed (`_can_resume=False`, `_always_run`).
4. `FSInjectBrainExtracted` (:230; NWF freesurfer.py:404-423) – pure Python, no binary.
   - Writes `brainmask.auto.mgz` = T1.mgz × nearest-neighbour resample of (T1w brain > 0).
   - **Hard-links** it to `brainmask.mgz`.
5. `init_autorecon_resume_wf` (:430). Its custom `ReconAll` (SMR/interfaces/freesurfer.py:131) compares output/input timestamps and adds `-no<step>` flags; it returns `echo recon-all: nothing to do` when nothing is left. Sub-steps:
   - `gcareg` (:503); FreeSurfer 7.3 removed it from volonly.
   - `-autorecon2-volonly` (:511)
   - `-autorecon-hemi {lh,rh} -noparcstats -noparcstats2 -noparcstats3 -nohyporelabel -nobalabels`, one per hemisphere in parallel (:519-537)
   - `-cortribbon [-T2pial|-FLAIRpial] -parallel`, n_procs = 2 (:544-549). The T2/FLAIR pial refinement happens here.
   - `-autorecon-hemi {lh,rh} -nohyporelabel` (parcstats) (:553-561)
   - `-autorecon3` (:565-571)
6. Midthickness: `mris_expand -thickness ?h.white 0.5 ?h.midthickness` (:236-241). OMP threads = 1.5 × requested (SMR/interfaces/freesurfer.py:346-348). If a graymid/midthickness already exists, the command becomes `cp` (NWF freesurfer.py:143). Written back with `DataSink`.
7. `mri_robust_register --satit --iscale`: T1.mgz (fsnative) → T1w reference, giving `fsnative2t1w_xfm` (:323-332). Skipped if precomputed.
8. `--fs-no-resume`: `ValidateSubjectDir` only (:288-309).

**Programs run inside recon-all** (from `freesurfer/scripts/recon-all`). These are what a Rust replacement would have to reproduce. Runtimes are typical FreeSurfer 7 single-thread estimates.

| recon-all step | Binaries / scripts | Approx. time |
|---|---|---|
| motioncor | mri_convert, mri_robust_template (if >1 input), mri_add_xform_to_header | 1–2 min |
| talairach | talairach_avi (AVI 4dfp tools), lta_convert, talairach_afd | 1–3 min |
| nuintensitycor | mri_nu_correct.mni (tcsh + MNI N3 Perl tools; FS ≥ 7.2 may use AntsN4BiasFieldCorrectionFs) | 2–4 min |
| normalization | mri_normalize | 2 min |
| skullstrip | skipped (`-noskullstrip`) | – |
| gcareg / canorm / careg / calabel | mri_em_register, mri_ca_normalize, **mri_ca_register** (dominant), mri_ca_label (GCA atlas) | 10–15 / 2 / 45–90 / 15–30 min |
| normalization2, maskbfs, segmentation, fill | mri_normalize -aseg, mri_mask, mri_edit_wm_with_aseg, mri_segment, mri_pretess, mri_fill | ~10 min |
| tessellate, smooth1, inflate1, qsphere | mri_tessellate, mris_extract_main_component, mris_smooth, mris_inflate, mris_sphere | ~5 min/hemi |
| fix | **mris_fix_topology**, mris_euler_number, mris_remove_intersection, defect2seg | 10–60 min/hemi |
| autodet-gwstats, white, cortex label, smooth2, inflate2, curvHK | mris_autodet_gwstats, **mris_place_surface**, label-cortex, mris_curvature | 15–30 min/hemi |
| sphere, surfreg | **mris_sphere**, **mris_register** (rca-surfreg; folding atlas .tif) | 20–40 + 20–40 min/hemi |
| jacobian, avgcurv, cortparc (1/2/3) | mris_jacobian, mrisp_paint, mris_ca_label (.gcs atlases) | ~5 min/hemi |
| pial | mris_place_surface (T2pial: mris_place_surface + mri_concatenate_lta) | 15–30 min/hemi |
| cortribbon | mris_volmask | 10–20 min |
| parcstats, pctsurfcon | mris_anatomical_stats, pctsurfcon (tcsh) | ~5 min |
| autorecon3 volumetric | mri_relabel_hypointensities, mri_surf2volseg, mri_segstats, mri_brainvol_stats, mri_label2label (BA_exvivo) | 20–40 min |

Total: roughly 5–8 h on one core, roughly 2.5–4 h with `-openmp` and hemispheres in parallel. `-hires` raises this substantially.

### Stage 6 – mask refinement (`RefineBrainMask`)
`init_refinement_wf` (surfaces.py:338).

- `init_segs_to_native_wf` (:1255): `ConcatenateXFMs(out_fmt='fs')`, then `mri_vol2vol --nearest` brings aseg into the T1w grid.
- `RefineBrainMask` (NWF freesurfer.py:299; algorithm `grow_mask` :482 and `refine_aseg` :449) – pure numpy/scipy/skimage:
  1. Binarise aseg; binary closing with ball(4); fill holes.
  2. Shell = dilate(ball(4)) − mask.
  3. For each shell voxel: add it if ANTs labelled it GM (2). Otherwise take a 14³ window over aseg cortex (3/42), compute GM mean and SD, and add the voxel if |z| < 2.
  4. Binary opening with ball(4).
- `fsl.ApplyMask` (`fslmaths -mas`, anatomical.py:1113) applies the refined mask, giving `refined_buffer` (the brain used by FAST and as the normalisation moving mask).
- **Dependency ordering:** FAST (Stage 3) and the Stage 4 moving mask consume the refined mask, so when FreeSurfer is on, segmentation and normalisation wait for the full recon-all.

### Stage 7 – T2w (FreeSurfer only)
anatomical.py:1135-1189.

1. Stage 1 template workflow on the T2w images.
2. `bbregister --t2 --init-coreg --6 --lta --gm-proj-abs 2 --wm-proj-abs 1` to fsnative.
3. Concatenate with fsnative → T1w.
4. antsApplyTransforms `LanczosWindowedSinc --float`.
5. Output `desc-preproc_T2w`.

### Stage 8 – GIFTI surfaces, morphometrics, ribbon
- Surfaces: `init_gifti_surfaces_wf` (surfaces.py:846).
  - `_get_surfaces` globs `surf/[lr]h.*` (graymid is accepted as midthickness).
  - `mris_convert --to-scanner` (spheres use `to_scanner=False`) (:908).
  - `NormalizeSurf` (SMR/interfaces/surf.py:186; pure nibabel + nitransforms): applies the fsnative→T1w LTA inverse to coordinates, strips VolGeom metadata, fixes `GeometricType` "Sphere" → "Spherical", sets midthickness metadata.
  - Surfaces handled: white, pial, midthickness, sphere, sphere.reg.
- Morphometrics: `init_gifti_morphometrics_wf` (:943) uses `mris_convert -c ?h.{thickness,sulc,curv} ?h.white` (`MRIsConvertData`, SMR/interfaces/freesurfer.py:271).
- Ribbon (8a): `init_anat_ribbon_wf` (:1336).
  - `wb_command -create-signed-distance-volume` for white and pial on the T1w grid (`-fill-value 0 -exact-limit 5 -approx-limit 20 -approx-neighborhood 2 -winding EVEN_ODD`, SMR/interfaces/workbench.py:54-89).
  - `MakeRibbon` (pure): (white distance > 0) & (pial distance < 0), OR-ed across hemispheres.

### Stages 9–11 – fsLR registration, MSMSulc, cortex mask
- **Stage 9 (fsaverage-based):** `wb_command -surface-sphere-project-unproject ?h.sphere.reg fsaverage.{L,R}.sphere.164k_fs_{L,R} fs_{L,R}-to-fs_LR_fsaverage.{L,R}_LR.spherical_std.164k_fs_{L,R}` (surfaces.py:713-741), giving `space-fsLR desc-reg sphere`.
- **Stage 10 (MSMSulc, on by default, `--no-msm` disables)** (:754-843):
  1. `wb -surface-affine-regression` (native sphere → fsLR-reg sphere).
  2. `-surface-apply-affine`.
  3. `-surface-modify-sphere 100`.
  4. Invert sulc with `MetricMath` (pure; SMR/interfaces/gifti.py:54).
  5. `msm --conf=MSMSulcStrain{Final|Sloppy}conf --inmesh --refmesh=fsaverage.{L,R}_LR.spherical_std.164k_fs_LR --indata --refdata={L,R}.refsulc.164k_fs_LR --out={lh.,rh.} --verbose`.
  - Final config: `--simval=3,2,2,2 --sigma_in/ref=0 --lambda=0,10,7.5,7.5 --it=50,10,15,15 --opt=AFFINE,DISCRETE×3 --CPgrid=6,2,3,4 --SGgrid/--datagrid=6,4,5,6 --regoption=3 (strain) --regexp=2 --dopt=HOCR --VN --rescaleL --triclique --k_exponent=2 --bulkmod=1.6 --shearmod=0.4`.
  - Sloppy config: 2 levels (AFFINE, DISCRETE), `--it=20,5`.
- **Stage 11 (cortex mask)** (:1161):
  1. abs(thickness), then bin (> 0) – pure.
  2. `wb -metric-fill-holes`.
  3. `wb -metric-remove-islands`, both on midthickness.

### After the fit (`anat_preproc_wf` and fMRIPrep)
- Standard-space volumes (outputs.py:928-1060):
  - `GenerateSamplingReference` (pure).
  - antsApplyTransforms: T1w Lanczos `--float`; mask and dseg `MultiLabel`; tpms `Gaussian`.
- Surface derivatives (surfaces.py:602): inflated GIFTI, curv, aseg and aparc+aseg in T1w space (`mri_vol2vol` nearest).
- CIFTI (`--cifti-output 91k|170k`):
  - `init_hcp_morphometrics_wf` (:1034): pure invert/abs, then `wb -metric-dilate 10 -nearest` on curv and thickness.
  - `init_resample_surfaces_wf` (:1401): `wb -surface-resample BARYCENTRIC` onto `tpl-fsLR den-32k|59k sphere`.
  - `init_morph_grayords_wf` (:1535): `wb -metric-resample ADAP_BARY_AREA -area-surfs (native midthickness → fsLR midthickness) -current-roi cortex` → `wb -metric-mask {L,R}.atlasroi` → `GenerateDScalar` (pure nibabel CIFTI-2; SMR/interfaces/cifti.py:53).

### Derivative reuse, longitudinal and session options
- **Reuse:** `io_spec.json` queries (SMR/data/io_spec.json):
  - baseline: preproc, mask, dseg, tpms, t2w_preproc
  - transforms: from-T1w/to-T1w, mode-image `.h5`/`.txt`, per standard space plus fsnative
  - surfaces: needs exactly 2 hemispheres (white, pial, midthickness, sphere, sphere_reg, thickness, sulc, sphere_reg_fsLR, sphere_reg_msm, cortex mask)
  - masks: ribbon
  - Later `--derivatives` datasets override earlier ones (bids.py:122-154). Each stage tests for its keys (anatomical.py:631-635, 992-1001, 1087, 1206, 1282, 1310, 1332, 1360).
- **Longitudinal:** `--subject-anatomical-reference unbiased` (or the deprecated `--longitudinal`) turns off `fixed_timepoint` and `no_iteration` in `mri_robust_template` (anatomical.py:1515-1516), so the template is unbiased and iterated.
- **Sessionwise:** one workflow per session; the FreeSurfer subject becomes `sub-X_ses-Y` (base.py:178-190, 492-506). Long session lists are hashed into the name (`stringify_sessions`, SMR/utils/misc.py:85).
- FreeSurfer's own longitudinal stream (`-base`/`-long`) is not used. `--fs-no-resume` allows importing an external base/FastSurfer directory.

## 2. Consolidated binary table

Weight: L < 5 min, M 5–30 min, H > 30 min, VH = hours. Difficulty = Rust re-implementation effort.

| Binary / subcommand | Toolkit | file:line | Purpose | Key parameters | Weight | Difficulty / notes |
|---|---|---|---|---|---|---|
| DenoiseImage | ANTs | SMR/workflows/anatomical.py:1460 | Rician non-local-means denoising before conform | `-n Rician`, defaults (patch 1, search 2, shrink 1) | M | Med (NLM with Rician bias term) |
| N4BiasFieldCorrection (pre-template) | ANTs | anatomical.py:1502 | INU before averaging | `-d 3`, ANTs defaults | L | Med (B-spline field fit; ITK reference is Apache-2.0) |
| mri_robust_template | FS | anatomical.py:1509; NWF/interfaces/freesurfer.py:48 | Robust rigid + intensity averaging | `--satit --inittp 1 --iscale --subsample 200 [--fixtp --noit]`, LTA outputs | M | High (Tukey M-estimator, pyramid, median template) |
| lta_convert (identity) | FS | anatomical.py:1491; NWF freesurfer.py:91 | Identity LTA | `--inlta identity.nofile` | L | Low (write LTA text) |
| ImageMath TruncateImageIntensity | ANTs | NWF/anat/ants.py:238; SMR fit/registration.py:187 | Intensity clipping | `0.01 0.999 256` | L | Low |
| N4 (brain extraction pass 1) | ANTs | ants.py:243 | INU | `-s 4 -b [200] -c [50x50x50x50,1e-7]` | L | Med |
| antsAI | ANTs | ants.py:271 | Affine initialisation at 4 mm | `Mattes[32,Regular,0.25]`, `Affine[0.1]`, `-s [15,0.1]`, `-g [40,0x40x40]`, `-c [10,1e-6,10]`, fixed mask | L | Med (rotation grid search + short optimisation) |
| ImageMath Laplacian | ANTs | ants.py:378 | Laplacian feature channel | `1.5 1` | L | Low |
| antsRegistration (brain extraction) | ANTs | ants.py:300 | Template ↔ subject SyN | antsBrainExtraction_*.json (MI32, CC4×2 channels; shrink 8-4-2-1; σ 4-2-1-0 vox; Lanczos; HM; winsorize 0.025/0.975) | H | **Very high** (multi-metric multi-resolution SyN; exact parity unrealistic, aim for equivalence) |
| antsApplyTransforms (mask/WM priors) | ANTs | ants.py:312, 353 | Warp template priors | Gaussian interpolation, reverse transforms + invert flags | L | Med (transform stack; affine .mat + warp field) |
| ThresholdImage | ANTs | ants.py:319 | Binarise | 0.5–1 | L | Low |
| ImageMath MD / ME / GetLargestComponent / FillHoles / PadImage / addtozero; MultiplyImages | ANTs | ants.py:573-689 | Mask morphology | MD 2/4/5, ME 10/2/5, FillHoles 2, Pad ±10 | L | Low |
| Atropos | ANTs | ants.py:583 | K-means + EM + MRF segmentation | `KMeans[3] Gaussian -m [0.1,1x1x1] -c [3,0]`, random seed | M | Med-high (ICM/MRF; seeding) |
| N4 final / n4_only | ANTs | ants.py:703, 926 | INU with WM weight | `-s 4 -b [200] -c [50×5,1e-7] -r`, weight image | L | Med |
| fast | FSL | anatomical.py:948; SMR/interfaces/fsl.py:37 | GM/WM/CSF dseg + probseg | `-I 0 -N -g -p -S 1` (n = 3, T1) | M | High (HMRF-EM + PVE; FSL licence forbids porting source) |
| antsAffineInitializer | ANTs | NWF/interfaces/norm.py:177 | Initialisation for MNI registration | nipype defaults: 15 / 0.1 / 0 / 10 | L | Med |
| antsRegistration (MNI) | ANTs | SMR fit/registration.py:192; norm.py:196-238 | T1w → each template | t1w-mni_registration_{precise,testing}_00x.json, explicit masks, `--float` | H (per template) | **Very high** |
| antsApplyTransforms (outputs / reports) | ANTs | SMR/workflows/outputs.py:139-156, 966-981 | Resample into template | Lanczos / MultiLabel / Gaussian; `.h5` composite | L–M | Med (needs ITK HDF5 composite reader: libhdf5 dependency or a pure-Rust HDF5 subset) |
| antsApplyTransforms (T2w) | ANTs | anatomical.py:1156 | T2w → T1w | Lanczos, `--float` | L | Med |
| recon-all autorecon1 | FS | SMR/workflows/surfaces.py:221 | orig, talairach, nu, T1 | `-noskullstrip -noT2pial -noFLAIRpial [-cw256] [-hires] [-T2/-FLAIR] -openmp` | M | Very high |
| recon-all -gcareg, -autorecon2-volonly | FS | surfaces.py:503, 511 | GCA registration and labelling | – | VH (1–2 h) | Very high (needs FreeSurfer GCA atlas data) |
| recon-all -autorecon-hemi ×2 | FS | surfaces.py:519 | Surfaces through sphere.reg, pial, parcellations | `-no{parcstats,parcstats2,parcstats3,hyporelabel,balabels}` | VH (2–4 h/hemi) | Very high (topology fix, surface placement, spherical registration) |
| recon-all -cortribbon | FS | surfaces.py:544 | Ribbon, T2/FLAIR pial | `-parallel [-T2pial]` | M | High |
| recon-all -autorecon-hemi (parcstats) ×2, -autorecon3 | FS | surfaces.py:553, 565 | Stats, aparc+aseg, wmparc | `-nohyporelabel` | M | High |
| mris_expand | FS | surfaces.py:236; SMR/interfaces/freesurfer.py:310 | Midthickness | `-thickness white 0.5` | M | Med (normal displacement with intersection avoidance) |
| mri_robust_register | FS | surfaces.py:324 | fsnative → T1w LTA | `--satit --iscale` | L | Med; T1.mgz is a conformed resample of the same input, so header-derived affine + verification is Low |
| mri_vol2vol (ApplyVolTransform) | FS | surfaces.py:1309 | aseg/aparc → T1w | `--nearest`, LTA | L | Low (nitransforms) |
| fslmaths -mas | FSL | anatomical.py:1113 | Apply refined mask | – | L | Low |
| bbregister | FS | anatomical.py:1144 | T2w → fsnative | `--t2 --init-coreg --6 --gm-proj-abs 2 --wm-proj-abs 1` | M | High (needs surfaces) |
| mris_convert --to-scanner | FS | surfaces.py:908 | Surface → GIFTI | `to_scanner` True for anatomy, False for spheres | L | Low (nibabel + c_ras) |
| mris_convert -c | FS | surfaces.py:1001; SMR freesurfer.py:271 | curv/thickness/sulc → GIFTI | – | L | Low (nibabel) |
| wb -create-signed-distance-volume | WB | surfaces.py:1371; SMR/interfaces/workbench.py:166 | Ribbon | exact 5 mm, approx 20 mm, neighbourhood 2, EVEN_ODD | L | Med |
| wb -surface-sphere-project-unproject | WB | surfaces.py:728; SMR workbench.py:560 | fsaverage → fsLR registration | bundled fs_L/fs_R spheres | L | Med (spherical barycentric lookup) |
| wb -surface-affine-regression / -surface-apply-affine / -surface-modify-sphere | WB | surfaces.py:775, 786, 793 | MSM preprocessing | radius 100 | L | Low |
| msm | MSM (HOCR) | surfaces.py:817; SMR/interfaces/msm.py:170 | MSMSulc | config above | H (est. 20–60 min/hemi) | **Very high** plus licence block (HOCR/ELC/FastPD non-commercial) |
| wb -surface-resample BARYCENTRIC | WB | surfaces.py:1488; SMR workbench.py:688 | Surfaces → fsLR 32k/59k | – | L | Med |
| wb -metric-dilate | WB | surfaces.py:1119; NWF/interfaces/workbench.py:161 | Dilate curv/thickness | 10 mm, `-nearest` | L | Med (geodesic) |
| wb -metric-fill-holes / -metric-remove-islands | WB | surfaces.py:1226-1227; NWF workbench.py:736, 803 | Cortex ROI | – | L | Low-med (mesh connected components) |
| wb -metric-resample ADAP_BARY_AREA | WB | surfaces.py:1690; NWF workbench.py:279 | Morphometrics → fsLR | `-area-surfs`, `-current-roi` | L | Med-high (adaptive area-weighted barycentric) |
| wb -metric-mask | WB | surfaces.py:1695; NWF workbench.py:669 | Medial-wall mask | atlasroi | L | Low |
| mri_convert (licence check) | FS | NWF/utils/misc.py:358-378; SMR/cli/run.py:388 | Validate licence | sentinel.nii.gz | L | n/a |
| pandoc (optional) | – | SMR/cli/run.py:733-765 | CITATION.html/.tex | `--citeproc` / `--natbib` | L | Optional |
| svgo, cwebp (optional) | Node / libwebp | NWF/viz/utils.py:51-123; NR/reportlets/utils.py:100-150 | Compress reportlets | `-p 3`; webp q80 | L | Low (drop, or use Pillow WebP) |

## 3. niworkflows inventory

### A. Wraps external binaries
| Class / function (file:line) | Binary | Base class / what it adds |
|---|---|---|
| FixHeaderApplyTransforms (interfaces/fixes.py:56) | antsApplyTransforms | `ants.ApplyTransforms`; copies reference xform to output (`_copyxform`) |
| FixHeaderRegistration (fixes.py:93) | antsRegistration | `ants.Registration`; adds `restrict_deformation`; header copy on warped outputs |
| FixN4BiasFieldCorrection (fixes.py:136) | N4BiasFieldCorrection | Shifts negative intensities (writes `_scaled` copy), reports `negative_values` |
| SpatialNormalization (norm.py:125) | antsAffineInitializer + antsRegistration | `BaseInterface`; retry over JSON files, masking logic, `create_cfm` |
| StructuralReference (freesurfer.py:48) | mri_robust_template; `echo`; lta_convert | `fs.RobustTemplate`; pass-through for a single volume |
| MakeMidthickness (freesurfer.py:106) | mris_expand or `cp` | `fs.MRIsExpand` |
| PatchedConcatenateLTA / PatchedLTAConvert / PatchedRobustRegister (freesurfer.py:248 / 264 / 281) | mri_concatenate_lta / lta_convert / mri_robust_register | `TruncateLTA` mixin (:215): rewrites filename lines ≥ 255 characters |
| PatchedBBRegisterRPT / PatchedMRICoregRPT (freesurfer.py:273 / 277) | bbregister / mri_coreg, + mri_vol2vol for the report | Reportlet mixins |
| `mri_info()` (freesurfer.py:563) | `mri_info` via `Popen(shell=True)` | Unquoted path; uses deprecated `np.fromstring` |
| PoissonRecon (surf.py:450) | PoissonRecon | `CommandLine` |
| MetricDilate :138, MetricResample :255, VolumeToSurfaceMapping :526 (workbench.py) | wb_command | `WBCommand` + `OpenMPCommandMixin` (sets OMP_NUM_THREADS) |
| MetricMask :654, MetricFillHoles :720, MetricRemoveIslands :787 (workbench.py) | wb_command | `WBCommand` |
| IntraModalMerge (images.py:112) | optional mcflirt (:165) | – |
| RobustAverage (images.py:240) | AFNI 3dvolreg (`-Fourier -twopass -zpad 4`, default `mc_method='AFNI'`) or FSL mcflirt (:320-341) | – |
| MultiApplyTransforms (itk.py:100) | antsApplyTransforms via ThreadPool (:139-187) | – |
| `check_valid_fs_license` (utils/misc.py:358) | mri_convert | – |
| `svg_compress` (viz/utils.py:51) | svgo, cwebp (`shell=True`) | Optional |

**Reportlets** (`interfaces/reportlets/*`; nireports has duplicates at NR/interfaces/reporting/*):

| Reportlet | Wraps |
|---|---|
| BETRPT masks.py:52 | bet |
| BrainExtractionRPT masks.py:92 | antsBrainExtraction.sh |
| SpatialNormalizationRPT registration.py:66 | ANTs |
| ANTSRegistrationRPT :100 | antsRegistration |
| ANTSApplyTransformsRPT :128 | antsApplyTransforms |
| ApplyTOPUPRPT :152 | applytopup |
| FUGUERPT :181 | fugue |
| FLIRTRPT :208 | flirt |
| ApplyXFMRPT :229 | flirt -applyxfm |
| BBRegisterRPT :259 | bbregister + mri_vol2vol in `_post_run_hook` |
| MRICoregRPT :299 | mri_coreg + mri_vol2vol |
| FASTRPT segmentation.py:43 | fast |
| ReconAllRPT segmentation.py:88 | recon-all |
| MELODICRPT segmentation.py:124 | melodic |
| ICA_AROMARPT (nireports only) NR/interfaces/reporting/segmentation.py:202 | ICA_AROMA |

### B. Pure Python (already portable)
- **bids.py:** BIDSInfo :108, BIDSDataGrabber :234, PrepareDerivative :325, SaveDerivative :742, **DerivativesDataSink** :817, ReadSidecarJSON :1249, **BIDSFreeSurferDir** :1362 (copies `$FREESURFER_HOME/subjects/fsaverage*` with `shutil.copytree`), BIDSURI :1497.
- **header.py:** CopyXForm :54, CopyHeader :125, ValidateImage :155, MatchHeader :328, SanitizeImage :375.
- **images.py:** RegridToZooms :74, TemplateDimensions :402, Conform :496, SignalExtraction :685.
- **nibabel.py:** ApplyMask, Binarize, BinaryDilation, SplitSeries, MergeSeries, MergeROIs, RegridToZooms, DemeanImage, FilledImageLike, GenerateSamplingReference :400, IntensityClip, MapLabels, ReorientImage.
- **nitransforms.py:** ConvertAffine :60, ConcatenateXFMs :132.
- **cifti.py:** GenerateCifti :100, CiftiNameSource :136 (uses TemplateFlow).
- **confounds.py:** all classes (FSLMotionParams, FSLRMSDeviation, FramewiseDisplacement, NormalizeMotionParams, ExpandModel, SpikeRegressors).
- **bold.py:** NonsteadyStatesDetector :67.
- **freesurfer.py:** FSInjectBrainExtracted :157, FSDetectInputs :189, RefineBrainMask :299, MedialNaNs :340.
- **surf.py:** everything except PoissonRecon (NormalizeSurf, Path2BIDS, GiftiNameSource, GiftiSetAnatomicalStructure, CSV↔Gifti, SurfacesToPointCloud, PLYtoGifti, UnzipJoinedSurfaces, CreateSurfaceROI).
- **morphology.py, probmaps.py, space.py, utility.py** (KeySelect etc.), **plotting.py**, **nilearn.py** (MaskEPI, Merge, ComputeEPIMask).
- **patches.py:** RobustA/TCompCor (nipype algorithms), FreeSurferSource (file globbing).
- **itk.py:** MCFLIRT2ITK.
- **engine/workflows.py** (LiterateWorkflow), **engine/splicer.py**, **engine/plugin.py** (ProcessPoolExecutor, `mp_context` from plugin_args :461).

### C. niworkflows/anat/*
- `ants.py`: covered in Stage 2.
- `skullstrip.py:33 afni_wf`: FixN4 (`rescale`, bias saved) → `3dSkullStrip` → `3dcalc 'a*step(b)'` → Binarize; optional `3dUnifize` before and `3dUnifize -GM` after. Not used by sMRIPrep.
- `coregistration.py:32 init_bbreg_wf`: `mri_coreg` (`dof`, `--sep 4 --ftol 1e-4 --linmintol 0.01`) and/or `bbregister --t2`; `compare_xforms` (:297, pure) picks the fallback.
- `freesurfer.py:43 init_gifti_surface_wf` (legacy): FreeSurferSource, mri_robust_register, mris_expand, mris_convert, NormalizeSurf. Reads `SUBJECTS_DIR` at import time (:40).

### D. niworkflows/func/util.py
- **`init_bold_reference_wf` (:51):**
  - ValidateImage.
  - NonsteadyStatesDetector (pure).
  - RobustAverage → **3dvolreg** (or mcflirt).
  - `init_enhance_and_skullstrip_bold_wf`.
  - Optional SimpleShowMaskRPT; SBRefs merged with MergeSeries.
- **`init_bold_premask_wf` (:282):**
  - antsAI: Rigid[0.1], Mattes[32,Regular,0.2], search [20,0.12], grid [40,0x40x40]; fixed = `MNI152NLin2009cAsym res-2 desc-fMRIPrep boldref` with `res-2 desc-brain_mask`.
  - antsRegistration with `epi_atlasbased_brainmask.json`.
  - antsApplyTransforms (Linear) of `res-1 label-brain_probseg`.
  - CopyHeader.
- **`init_enhance_and_skullstrip_bold_wf` (:395):**
  - N4 `-b [200] -r`, n_procs = 1, weight = premask (its `shrink_factor=2` at :489 is ignored, see section 0).
  - `bet -f 0.2 -m`.
  - BinaryDilation r = 6 (pure) → ApplyMask.
  - `3dUnifize -T2 -clfrac 0.2 -rbt 18.3 65.0 90.0`.
  - `3dAutomask -dilate 1`.
  - `fslmaths -mul` (intersection of masks) → ApplyMask.
- **`init_skullstrip_bold_wf` (:562):** bet 0.2 + 3dAutomask dilate 1 + fslmaths mul + SimpleShowMaskRPT.

## 4. How reports are generated
- **Reportlet SVGs** are made with matplotlib, nilearn `plot_anat`/contours, seaborn and svgutils (NWF/viz/utils.py: `plot_registration` :316, `plot_segs` :223, `compose_view` :402, `cuts_from_bbox` :158). NWF uses svgutils from PyPI; NR vendors it (MIT licence, needs lxml).
- **Binaries in reports:**
  - Only the optional `svgo`/`cwebp` compression, used only if both are on PATH.
  - The BBRegister/MRICoreg reportlets call `mri_vol2vol`.
  - The legacy ReconAllRPT needs recon-all. sMRIPrep instead uses `FSSurfaceReport` (SMR/interfaces/reports.py:167), which reads `brain.mgz`/`ribbon.mgz` with nibabel – no binary.
- **sMRIPrep reportlets:** ROIsPlot (dseg/mask, outputs.py:104), SimpleBeforeAfterRPT (normalisation), conform HTML (TemplateDimensions), FSSurfaceReport, SubjectSummary/AboutSummary (HTML strings).
  - `SubjectSummary` builds an `fs.ReconAll` command line (no execution) just to detect a pre-existing directory (reports.py:110-119).
- **HTML assembly:** `nireports.assembler.tools.generate_reports` (SMR/cli/run.py:471, 650): Jinja2 + pybids layout + YAML spec (`SMR/data/reports-spec.yml`).
  - At *view* time the template loads jQuery and Bootstrap 5.2.3 from CDNs (NR/assembler/data/report.tpl:8-10), so offline viewing degrades.
  - LaTeX is used for matplotlib text only if `latex` is on PATH, and only in DWI plots (NR/reportlets/utils.py:548-550).
- **Portability concerns:**
  - `shell=True` subprocess calls (fine if absent).
  - Hard-link copies (`copyfile(..., use_hardlink=True)` at NR/assembler/reportlet.py:245, 284 and NWF/reports/core.py:207) fall back to copying in nipype.
  - `html_anchor` is a `Path` interpolated into `data="./{name}"` (reportlet.py:240, 266-270), so Windows produces backslash URLs. Browsers usually tolerate this; `.as_posix()` would be the fix.

## 5. Data files and templates

**TemplateFlow queries:**
- **OASIS30ANTs** (brain extraction default), resolution 1: T1w, `label-brain_probseg` (or `desc-brain_mask`), `desc-BrainCerebellumExtraction_mask`, `label-WM_probseg`, `label-BS_probseg` (NWF/anat/ants.py:201-220, 351, 360).
- **Normalisation targets** (`--output-spaces`; sMRIPrep has no default space, fMRIPrep defaults to `MNI152NLin2009cAsym:res-native`, fmriprep/cli/parser.py:875-877): T1w (`desc=None`), `desc-brain_mask` / `label-brain_mask`, T2w (SMR/interfaces/templateflow.py:213-218; NWF norm.py:415-431). Template metadata (`get_metadata`) is used for the boilerplate (fit/registration.py:134).
- **fsLR:** `den-32k|59k sphere.surf.gii` (surfaces.py:1495, 1660); `desc-nomedialwall dparc` (SMR/interfaces/cifti.py:126; NWF/interfaces/cifti.py:214; NWF freesurfer.py:548); surfaces for plots (NWF/viz/plots.py:1020).
- **MNI152NLin6Asym:** `atlas-HCP dseg res-02|06` (NWF cifti.py:226) for BOLD CIFTI.
- **MNI152NLin2009cAsym:** res-2 `desc-fMRIPrep boldref`, res-2 brain mask, res-1 brain probseg (func/util.py:328-365).
- **fsaverage\*:** comes from `$FREESURFER_HOME/subjects`, not TemplateFlow (NWF bids.py:1362-1450).
- **MNIInfant / cohort templates:** handled generically through `:cohort-`; no infant-specific code in sMRIPrep.

**TemplateFlow client** (python-client/templateflow):
- Cache: `TEMPLATEFLOW_HOME` or `platformdirs.user_cache_dir('templateflow')` (conf/env.py:54-55), so Windows gets `%LOCALAPPDATA%`.
- Skeleton: a bundled zip of zero-byte placeholders (conf/_s3.py:38-110). Files are fetched on demand over HTTPS from `templateflow.s3.amazonaws.com` with requests (client.py:376-410).
- DataLad is optional (`TEMPLATEFLOW_USE_DATALAD`); git-annex symlinks are a Windows problem.
- No file locking around downloads, so parallel workers can race. Pre-fetching is recommended.

**Bundled data:**
- NWF/data: 20 registration JSONs (above), `itkIdentityTransform.txt`, `nipreps.json` (pybids entity config), `sentinel.nii.gz` (licence check), `reports/default.yml` and `report.tpl`.
- SMR/data:
  - `msm/MSMSulcStrain{Final,Sloppy}conf`
  - `atlases/`:
    - `{L,R}.atlasroi.{32k,59k}_fs_LR.shape.gii`
    - `{L,R}.refsulc.164k_fs_LR.shape.gii` (GIFTI provenance: HCP "Q1-Q6_Related449" group-average sulc, `wb_command -cifti-average`)
    - `fs_{L,R}/fsaverage.{L,R}.sphere.164k_fs_{L,R}.surf.gii` and `fs_{L,R}-to-fs_LR_fsaverage.*_LR.spherical_std.164k_fs_{L,R}.surf.gii`, plus `fsaverage.{L,R}_LR.spherical_std.164k_fs_LR.surf.gii` (Caret 5.65, 2013; HCPpipelines standard_mesh_atlases)
    - No licence or README is bundled; HCPpipelines licence and HCP data-use terms should be checked.
  - `io_spec.json`, `reports-spec.yml`, `boilerplate.bib`, `itkIdentityTransform.txt`.

**Licences visible in the clones:**

| Component | Licence |
|---|---|
| smriprep, niworkflows, nireports, nipype, templateflow client | Apache-2.0 |
| ANTs | Apache-2.0 (ANTs/COPYING.txt) |
| AFNI | Public domain with third-party exceptions (afni/LICENSE.txt) |
| FreeSurfer | FreeSurfer Software License v1.0 (freesurfer/LICENSE.txt): allows derivative works with attribution and "modified" marking; a runtime licence key is still required |
| Workbench | GPLv2 (workbench/LICENSE) |
| MSM_HOCR | Non-commercial research only; ELC redistribution needs consent; FastPD patents pending (MSM_HOCR/Licenses/*) |
| svgutils (vendored) | MIT |

Template licences are not in these repos; check each `template_description.json` in TemplateFlow. Commonly: OASIS30ANTs CC-BY-4.0, MNI152NLin2009cAsym MNI permissive licence, fsaverage under the FreeSurfer licence, fsLR HCP-derived. FSL was not cloned; its licence is non-commercial.

## 6. POSIX-specific and portability hazards

| file:line | Issue |
|---|---|
| SMR/cli/run.py:373 (also fmriprep/config.py:104) | `set_start_method('forkserver')` raises on Windows; needs `spawn` |
| freesurfer/scripts/recon-all, mri_nu_correct.mni, pctsurfcon, rca-* | tcsh scripts using `ln`/`mv`/`cp`/`awk`; FreeSurfer has no Windows build |
| NWF/interfaces/freesurfer.py:143 | `cp src dst` as the interface command line; fails in cmd.exe |
| NWF freesurfer.py:83; SMR/interfaces/freesurfer.py:207 | `echo ...` used as a no-op command (works in cmd.exe but relies on the shell) |
| NWF freesurfer.py:564-569 | `Popen(f'mri_info --{arg} {fname}', shell=True)`; unquoted paths |
| NWF/viz/utils.py:62-104; NR/reportlets/utils.py:108-150 | `shell=True` pipelines into svgo/cwebp |
| NWF freesurfer.py:421; NWF/utils/misc.py:291; NWF/reports/core.py:207; NR/assembler/reportlet.py:245, 284 | Hard links (`use_hardlink=True`); fine on NTFS, nipype falls back to copy |
| SMR/utils/misc.py:50-82 | FreeSurfer `IsRunning*` lock files, removed when older than 24 h |
| NWF/anat/freesurfer.py:40 | `SUBJECTS_DIR` read at import time. Elsewhere the subjects directory is passed explicitly; nipype FS interfaces export `SUBJECTS_DIR` per call. SMR/workflows/base.py:165-176 builds `<out>/sourcedata/freesurfer`-style directories through BIDSFreeSurferDir and copies fsaverage from `FREESURFER_HOME` |
| SMR/cli/run.py:384-393 | `FS_LICENSE` environment variable and licence check through `mri_convert` |
| NWF/interfaces/workbench.py:34-41; SMR freesurfer.py:346-348 | Thread control through the `OMP_NUM_THREADS` environment variable (portable, but Rust code should take explicit thread counts) |
| NWF freesurfer.py:215-245 | LTA filename lines ≥ 255 characters crash FreeSurfer, so they are truncated. Deep nipype work directories also run into the Windows 260-character MAX_PATH; enable long paths or use short work dirs |
| NR/assembler/reportlet.py:266-270 | Path with backslashes interpolated into HTML |
| python-client/templateflow/client.py:376-410 | Network access on first use and no download locking |
| SMR/cli/run.py:733-765 | Optional `pandoc` |
| (none found) | No `os.symlink`, `fcntl`, `signal`, `/tmp` literals (other than doctest examples) or `pwd`/`getuid` in sMRIPrep or niworkflows runtime code |
