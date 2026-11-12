# FreeSurfer 8.2.0: the programs recon-all and fMRIPrep run

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; see
[README.md](README.md)).

Relay 1 of FreeSurfer's preparation, 2026-11-12. Everything below is read from the source; each
claim gives the file and line it comes from. The recordings (`<workspace>/oracles/freesurfer-8.2.0/runs/`)
confirm or correct it: at the end of relay 1, the first ones (ds000005 sub-01, v8 steps 1-8 and
classic steps 1-12) matched §3's order and arguments exactly, N4's call inside
`mri_nu_correct.mni` included, and corrected one reading (`rca-talairach`'s thread count, §4).
Relay 2 (2026-11-12) checked ds002422 sub-02: v8 recorded steps 1-48 (§3's stages 1-55, through
`mris_autodet_gwstats rh`) and classic steps 1-14 (stages 1-27, through `mri_ca_normalize`) match
§3's order, arguments and timed/bare split, the bare stages included as the files they leave
between two recorded steps (v8: `aseg.auto_noCCseg.mgz` and `synthseg.tiv.dat` before
`rca-talairach`; `mca-dura.mgz`, `vsinus.mgz` and `brainmask.mgz` between `mri_normalize` and
`mri_em_register`; `entowm.mgz` and `entowm.stats` before `seg2cc`; `aseg.presurf.mgz` before the
second `mri_normalize`; `brain.finalsurfs.manedit.mgz` before `AntsDenoiseImageFs`). The
per-hemisphere stages run as §3 says: Tessellate's three programs for `lh`, then for `rh`; each
later stage `lh` then `rh`. Nested programs (inside `rca-talairach`, `fs-synthmorph-reg`,
`seg2cc`, `defect2seg`, `mri_nu_correct.mni`) have no step of their own. The next relays check the
rest as the runs finish.

Sources read:

- `<workspace>/tags/freesurfer-8.2.0/scripts/recon-all` (10,128 lines; GitHub's `v8.2.0` archive),
  `distribution/etc/recon-config.yaml`, `distribution/etc/global-expert-options.v8.txt`, and the
  scripts recon-all calls (named in §4);
- fMRIPrep 25.2.6 as its image installs it, with sMRIPrep 0.19.2, niworkflows 1.15.2 and
  nipype 1.12.0 (read from `oracles/images/fmriprep-25.2.6.sif`, whose own FreeSurfer is 7.3.2); the
  analysis `docs/analysis/02-anatomical-smriprep-niworkflows.md`.

Notation: `<FS>` is `$FREESURFER_HOME`, `<subj>` the subject, `<sd>` its directory, `N` the thread
count `-openmp N` sets (the reference runs use `-openmp 4 -itkthreads 4`). Paths are relative to
the directory the step runs in (`mri/`, `scripts/` or `surf/`, as recon-all `cd`s).

## 1. How recon-all 8.2.0 decides what runs

**The stream.** Line 17 sets `FS_V8_XOPTS=1` unless the environment says otherwise; `-v8` / `-no-v8`
on the command line override it (lines 422-435). With v8 on, the first line of
`etc/global-expert-options.v8.txt` is prepended to the arguments (lines 438-446):
`-fix-ento-wm -transfer-base-bfs -fix-vsinus -fix-mca-dura -fix-ga -fix-acj -synthstrip -synthseg
-synthmorph`, and its other lines become expert options of six steps (`fsr-getxopts` matches the
first word of a line exactly, `scripts/fsr-getxopts` lines 21-29):

| Key in the v8 file | Options | Applied in 8.2.0? |
|---|---|---|
| `WhitePreAparc` | `--rip-bg-no-annot --rip-bg --rip-bg-lof --restore-255 --outvol mrisps.wpa.mgz` | yes (recon-all line 3946) |
| `PlaceWhiteSurf` | `--restore-255 --outvol mrisps.white.mgz --rip-bg-lof` | yes (line 4459) |
| `T1PialSurf` | `--restore-255 --outvol mrisps.pial.mgz` | **no**: recon-all looks up `PlaceT1PialSurf` (line 4521); `--restore-255` reaches the pial step anyway through `-fix-ento-wm` (line 4514), `mrisps.pial.mgz` is never written |
| `mri_ca_label` | `-vent-fix -1 7000 1 -insert-wm-bet-putctx 1` | **no**: v8 never runs `mri_ca_label` (`-synthseg` turns it off, line 8354), and `-no-v8` reads no v8 file |
| `mri_edit_wm_with_aseg` | `-fix-scm-ha 1` | yes, on the main call (line 3355) |
| `mris_fix_topology` | `-threads 1` | yes (a repeat: the command already has `-threads 1`, line 3732) |

**The defaults of `recon-config.yaml`** (parsed by `scripts/rca-config`, Python's argparse; a flag
with `action: disable` is `store_false` with the YAML value as default, lines 83-99): `DoAntsN4`
**true** (only `-no-ants-n4` turns it off), `DoAntsDenoising` **true**, `DoRemesh` **true**,
`DefectsCortex` true, `GCA = RB_all_2020-01-02.gca`, `GCASkull = RB_all_withskull_2020_01_02.gca`;
everything else false. So **in both streams** the intensity correction is ITK's N4
(`AntsN4BiasFieldCorrectionFs`), not MNI's N3; the white-matter segmentation runs on an
`AntsDenoiseImageFs` copy of `brain.mgz`; and the fixed surface is remeshed (`mris_remesh`).
`nu_correct` (MNI's N3) runs only with `-no-ants-n4` (recon-all line 779, `mri_nu_correct.mni`
lines 118-176).

**`check_params`** (lines 8223-8806) then settles the flags: `-synthseg` (or `-samseg`) turns off
`mri_ca_label` and turns on SynthMorph (8353-8357); SynthMorph turns off the Talairach step
(`talairach_avi`), its check (`talairach_afd`) and `mri_ca_register`, but **keeps `mri_em_register`**
("keep em_reg", 8359-8365), so `mri_ca_normalize` runs in both streams; `-synthstrip` replaces
`mri_watershed` (2253, 2289). `-all` sets every stage of §3 except `-qcache`, `-label-v1`, `-lgi`,
`-label-exvivo-ec`, the T2/FLAIR stages, `-subfields` and the cleaning flags (7198-7249).

**Threads.** `-openmp N` sets `OMP_NUM_THREADS` and `FS_OMP_NUM_THREADS` (8068-8076); `-itkthreads N`
sets `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (8078-8082); without them both are 1 (229-237).
`-parallel` would also set 4 threads and run the hemispheres side by side (8057-8063); the reference
runs do not use it. Programs that take a thread argument get `$OMP_NUM_THREADS` (marked `N` below);
`mris_fix_topology` is always given `-threads 1` (the comment at 3728-3731: it "became
nondeterministic with multiple threads").

**Seeds.** `NoRandomness` is 1 and `RngSeed` 1234 by default (160-169): `-seed 1234` goes to
`mri_normalize`, `mris_smooth`, `mris_sphere`, `mris_fix_topology`, `mris_curvature`,
`mris_ca_label`, and `--seed 1234` to `mri_segstats`.

**Recorded steps and bare commands.** recon-all runs most programs as `$fs_time <program> ...`
(set at 536-539 when `fs_time` works): these are the recorded steps. Some it runs bare (`$cmd`),
which a recording sees only as a change between steps: marked **bare** in §3. Some scripts call
`fs_time` themselves, so their inner programs are recorded as steps nested in the script's step:
`seg2cc`, `rca-surfreg`, `fs-synthmorph-reg`, `mri_mcadura_seg` (marked **nested**).

## 2. Before the first step (both streams)

Run once per call, outside any stage: `rca-config` and `rca-config2csh` (Python, the YAML), many
`fsr-getxopts` (tcsh), `fs-check-version` with `fs_temp_file`, `fs-check-os` (8564-8584), `which bc`
(682), then every program of the version list with `-all-info`, its output into the log (693-776:
64 programs, among them `mri_aparc2aseg`, `mris_make_surfaces` and others the stages no longer run),
`mri_motion_correct.fsl -version` (777), and `nu_correct -version` only without N4 (779). They write
only to `scripts/recon-all.log`; recon-all also copies itself to `scripts/recon-all.local-copy`
(891), and the YAML to `scripts/recon-config.yaml` (580).

## 3. The stages of `recon-all -all`, in the order they run

`v8` / `cl` (classic, `-no-v8`): the stage runs in that stream. Per-hemisphere stages run `lh`
first, then `rh`, each stage for both before the next stage (`rh` uses label value 127 where `lh`
uses 255). Lines are recon-all's.

### autorecon1

| # | Stage (log label) | Program and arguments | v8 | cl | How | Line |
|---|---|---|---|---|---|---|
| 1 | input | `mri_convert <T1w> <sd>/mri/orig/001.mgz`; then `mri_info --nrows/--ncols/--nslices` (bare) | x | x | timed | 1242 |
| 2 | MotionCor | `mri_info --nframes 001.mgz` (bare); one input: `cp 001.mgz rawavg.mgz` (bare); several: `mri_robust_template --mov ... --average 1 --template rawavg.mgz --satit --inittp 1 --fixtp --noit --iscale --iscaleout ... --subsample 200 --lta ...` (timed) | x | x | bare / timed | 1424-1500 |
| 3 | MotionCor | `mri_convert rawavg.mgz orig.mgz --conform` | x | x | timed | 1534 |
| 4 | MotionCor | `mri_add_xform_to_header -c transforms/talairach.xfm orig.mgz orig.mgz` (the transform does not exist yet) | x | x | timed | 1557 |
| 5 | MotionCor | `mri_info rawavg.mgz`, `mri_info orig.mgz` (FOV check, error above 256 mm) | x | x | bare | 1504-1591 |
| 6 | | `lta_convert --inlta identity.nofile --src rawavg.mgz --trg orig.mgz --outlta mri/rawavg2orig.lta --subject <subj>` | x | x | timed (`fs_time`) | 1600 |
| 7 | | `mri_synthstrip --threads N -i orig.mgz -o synthstrip.mgz` | x | | timed | 1616 |
| 8 | | `mri_synthseg --i orig.mgz --o synthseg.rca.mgz --threads N --vol stats/synthseg.vol.csv --keepgeom --addctab --cpu`; then `ln -sf synthseg.rca.mgz aseg.auto_noCCseg.mgz` (bare) | x | | timed | 1635, 1645 |
| 9 | | `csvprint --csv stats/synthseg.vol.csv --f "total intracranial" > stats/synthseg.tiv.dat` | x | | bare | 1670 |
| 10 | Talairach | `mri_nu_correct.mni --no-rescale --i orig.mgz --o orig_nu.mgz --ants-n4 --n 1 --proto-iters 1000 --distance 50` (N4 ignores the last three) | | x | timed | 1781 |
| 11 | Talairach | `talairach_avi --i orig_nu.mgz --xfm transforms/talairach.auto.xfm` | | x | timed | 1811 |
| 12 | Talairach | `cp transforms/talairach.auto.xfm transforms/talairach.xfm` | | x | bare | 1845 |
| 13 | | `lta_convert --src orig.mgz --trg <FS>/average/mni305.cor.mgz --inxfm transforms/talairach.xfm --outlta transforms/talairach.xfm.lta --subject fsaverage --ltavox2vox` | | x | bare | 1865 |
| 14 | Talairach Failure Detection | `talairach_afd -T 0.005 -xfm transforms/talairach.xfm`; `awk -f <FS>/bin/extract_talairach_avi_QA.awk transforms/talairach_avi.log` (bare); on failure, stages 10-14 again with `--atlas 3T18yoSchwartzReactN32_as_orig`, then with MINC's `talairach` | | x | timed | 1901, 1916 |
| 15 | | `rca-talairach --s <subj> --threads N` (affine to `mni305.cor.stripped.mgz`, writes `talairach.xfm` and `.xfm.lta`; §4) | x | | timed | 2024 |
| 16 | | `fs-synthmorph-reg --s <subj> --threads N --i <sd>/mri/orig.mgz --test` (nonlinear to MNI152 2009c, into `transforms/synthmorph.1.0mm.1.0mm/`; §4) | x | | timed | 2034 |
| 17 | Nu Intensity Correction | `mri_nu_correct.mni --i orig.mgz --o nu.mgz --uchar transforms/talairach.xfm --n 2 --ants-n4` (§4) | x | x | timed | 2054-2090 |
| 18 | Nu Intensity Correction | `mri_add_xform_to_header -c <sd>/mri/transforms/talairach.xfm nu.mgz nu.mgz` | x | x | timed | 2093 |
| 19 | Intensity Normalization | `mri_normalize -g 1 -seed 1234 -mprage nu.mgz T1.mgz` | x | x | timed | 2193 |
| 20 | MCADura Segmentation | `mri_mcadura_seg --i <sd>/mri/nu.mgz --o <sd>/mri/mca-dura.mgz --threads N --synthmorphdir <sd>/mri/transforms/synthmorph.1.0mm.1.0mm` (§4) | x | | bare | 2220 |
| 21 | VSinus Segmentation | `mri_vsinus_seg --s <subj> --rca-synthseg --threads N --synthmorphdir ...` (§4) | x | | bare | 2241 |
| 22 | | `mri_mask <sd>/mri/T1.mgz <sd>/mri/synthstrip.mgz <sd>/mri/brainmask.mgz` | x | | bare | 2259 |
| 23 | Skull Stripping | `mri_em_register -skull nu.mgz <FS>/average/RB_all_withskull_2020_01_02.gca transforms/talairach_with_skull.lta` | | x | timed | 2391-2399 |
| 24 | Skull Stripping | `mri_watershed -T1 -brain_atlas <FS>/average/RB_all_withskull_2020_01_02.gca transforms/talairach_with_skull.lta T1.mgz brainmask.auto.mgz` | | x | timed | 2554 |
| 25 | Skull Stripping | `cp brainmask.auto.mgz brainmask.mgz` | | x | bare | 2635 |

### autorecon2

| # | Stage | Program and arguments | v8 | cl | How | Line |
|---|---|---|---|---|---|---|
| 26 | EM Registration | `mri_em_register -uns 3 -mask brainmask.mgz nu.mgz <FS>/average/RB_all_2020-01-02.gca transforms/talairach.lta` | x | x | timed | 2676 |
| 27 | CA Normalize | `mri_ca_normalize -c ctrl_pts.mgz -mask brainmask.mgz nu.mgz <GCA> transforms/talairach.lta norm.mgz` | x | x | timed | 2718-2742 |
| 28 | CA Reg | `mri_ca_register -nobigventricles -T transforms/talairach.lta -align-after -mask brainmask.mgz norm.mgz <GCA> transforms/talairach.m3z` | | x | timed | 2763-2794 |
| 29 | EntoWM Segmentation | `mri_entowm_seg --s <subj> --conform --threads N` (§4); then `grep` of `stats/entowm.stats`: a hemisphere's volumes under 200 / 50 turn off `-fix-ga` | x | | bare | 2867, 2888-2897 |
| 30 | SubCort Seg | `mri_ca_label -relabel_unlikely 9 .3 -prior 0.5 -align norm.mgz transforms/talairach.m3z <GCA> aseg.auto_noCCseg.mgz` | | x | timed | 3004-3027 |
| 31 | CC Seg | `seg2cc --s <subj>` → `mri_cc -aseg aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta transforms/cc_up.lta <subj>` (in v8 the input is the SynthSeg link) | x | x | timed + nested | 3074 |
| 32 | Merge ASeg | `cp aseg.auto.mgz aseg.presurf.mgz` | x | x | bare | 3105 |
| 33 | Intensity Normalization2 | `mri_normalize -seed 1234 -mprage -aseg aseg.presurf.mgz -mask brainmask.mgz norm.mgz brain.mgz` | x | x | timed | 3131-3145 |
| 34 | Mask BFS | `mri_mask -T 5 brain.mgz brainmask.mgz brain.finalsurfs.mgz` | x | x | timed | 3166 |
| 35 | Mask BFS | `mri_mask -oval 1 -invert brain.finalsurfs.mgz <sd>/mri/mca-dura.mgz brain.finalsurfs.mgz`; the same with `vsinus.mgz` | x | | timed | 3174, 3180 |
| 36 | Mask BFS | `mri_edit_wm_with_aseg -sa-fix-ento-wm entowm.mgz 2 255 255 brain.finalsurfs.mgz brain.finalsurfs.mgz`; `mri_edit_wm_with_aseg -sa-fix-acj aseg.presurf.mgz 255 255 brain.finalsurfs.mgz brain.finalsurfs.mgz` | x | | timed | 3228, 3236 |
| 37 | Mask BFS | `cp brain.finalsurfs.mgz brain.finalsurfs.manedit.mgz` | x | x | bare | 3243 |
| 38 | WM Segmentation | `AntsDenoiseImageFs -i brain.mgz -o antsdn.brain.mgz` | x | x | timed | 3312 |
| 39 | WM Segmentation | `mri_segment -wsizemm 13 -mprage antsdn.brain.mgz wm.seg.mgz` | x | x | timed | 3322-3328 |
| 40 | WM Segmentation | `mri_edit_wm_with_aseg -keep-in [v8: -fix-ento-wm entowm.mgz 3 255 255 -fix-acj aseg.presurf.mgz 255 255 -fill-seg-wm -fix-scm-ha 1] wm.seg.mgz brain.mgz aseg.presurf.mgz wm.asegedit.mgz` | x | x | timed | 3356-3362 |
| 41 | WM Segmentation | `mri_pretess wm.asegedit.mgz wm norm.mgz wm.mgz` | x | x | timed | 3380-3382 |
| 42 | WM Segmentation | `mri_edit_wm_with_aseg -sa-fix-ento-wm entowm.mgz 3 255 255 wm.mgz wm.mgz`; `mri_edit_wm_with_aseg -sa-fix-acj aseg.presurf.mgz 255 255 wm.mgz wm.mgz` | x | | timed | 3388, 3395 |
| 43 | Fill | `mri_fill -a ../scripts/ponscc.cut.log -xform transforms/talairach.lta -segmentation aseg.presurf.mgz -ctab <FS>/SubCorticalMassLUT.txt wm.mgz filled.mgz`; then `cp filled.mgz filled.auto.mgz` (bare) | x | x | timed | 3443-3470 |
| 44 | Tessellate lh/rh | `mri_pretess ../mri/filled.mgz 255 ../mri/norm.mgz ../mri/filled-pretess255.mgz` | x | x | timed | 3570 |
| 45 | Tessellate | `mri_tessellate ../mri/filled-pretess255.mgz 255 ../surf/lh.orig.nofix`; `rm -f` of the pretess volume (bare) | x | x | timed | 3580 |
| 46 | Tessellate | `mris_extract_main_component ../surf/lh.orig.nofix ../surf/lh.orig.nofix` | x | x | timed | 3590 |
| 47 | Smooth1 | `mris_smooth -nw -seed 1234 ../surf/lh.orig.nofix ../surf/lh.smoothwm.nofix` | x | x | timed | 3624-3627 |
| 48 | Inflation1 | `mris_inflate -no-save-sulc ../surf/lh.smoothwm.nofix ../surf/lh.inflated.nofix` | x | x | timed | 3658 |
| 49 | QSphere | `mris_sphere -q -p 6 -a 128 -seed 1234 ../surf/lh.inflated.nofix ../surf/lh.qsphere.nofix` | x | x | timed | 3690 |
| 50 | Fix Topology | `mris_fix_topology -threads 1 -mgz -sphere qsphere.nofix -inflated inflated.nofix -orig orig.nofix -out orig.premesh -ga -seed 1234 [v8: -threads 1] <subj> lh` | x | x | timed | 3732-3737 |
| 51 | Fix Topology | `mris_euler_number ../surf/lh.orig.premesh` (bare, into a log); a defect index other than 0 sends that hemisphere to `mris_topo_fixer -orig_name orig.nofix -out_name orig.premesh -mgz -warnings -seed 1234 <subj> lh` | x | x | bare (fixer timed) | 3755, 3809 |
| 52 | Fix Topology | `defect2seg --s <subj> --cortex` (§4) | x | x | timed | 3783 |
| 53 | Fix Topology | `mris_remesh --remesh --iters 3 --input <sd>/surf/lh.orig.premesh --output <sd>/surf/lh.orig` | x | x | timed | 3837 |
| 54 | Fix Topology | `mris_remove_intersection ../surf/lh.orig ../surf/lh.orig`; `rm -f ../surf/lh.inflated` (bare) | x | x | timed | 3848 |
| 55 | AutoDetGWStats | `mris_autodet_gwstats --o ../surf/autodet.gw.stats.lh.dat --i brain.finalsurfs.mgz --wm wm.mgz --surf ../surf/lh.orig.premesh` | x | x | timed | 3877 |
| 56 | WhitePreAparc | `mris_place_surface --adgws-in ../surf/autodet.gw.stats.lh.dat --wm wm.mgz --threads N --invol brain.finalsurfs.mgz --lh --i ../surf/lh.orig --o ../surf/lh.white.preaparc --white --seg aseg.presurf.mgz [v8: --restore-255] --nsmooth 5 [v8: the WhitePreAparc options above]` | x | x | timed | 3937-3947 |
| 57 | CortexLabel | `label-cortex --s <subj> --lh [v8: --fix-ga]` (§4) | x | x | timed | 3972 |
| 58 | CortexLabel+HipAmyg | `mri_label2label --label-cortex ../surf/lh.white.preaparc aseg.presurf.mgz 1 ../label/lh.cortex+hipamyg.label` | x | x | timed | 3988 |
| 59 | Smooth2 | `mris_smooth -n 3 -nw -seed 1234 ../surf/lh.white.preaparc ../surf/lh.smoothwm` | x | x | timed | 4020-4023 |
| 60 | Inflation2 | `mris_inflate ../surf/lh.smoothwm ../surf/lh.inflated` (writes `lh.sulc`) | x | x | timed | 4055 |
| 61 | Curv .H and .K | `mris_curvature -w -seed 1234 lh.white.preaparc`; `rm`/`ln -s lh.white.preaparc.{H,K} lh.white.{H,K}` (bare); `mris_curvature -seed 1234 -thresh .999 -n -a 5 -w -distances 10 10 lh.inflated` | x | x | timed | 4088, 4128-4131 |

### autorecon3

| # | Stage | Program and arguments | v8 | cl | How | Line |
|---|---|---|---|---|---|---|
| 62 | Sphere | `mris_sphere -threads N -seed 1234 ../surf/lh.inflated ../surf/lh.sphere` | x | x | timed | 4184-4188 |
| 63 | Surf Reg | `rca-surfreg --s <subj> --threads N --tif-path <FS>/average --tif-name folding.atlas.acfb40.noaparc.i12.2016-08-02.tif` → per hemisphere `mris_register -curv -threads N <sd>/surf/lh.sphere <FS>/average/lh.folding.atlas.acfb40.noaparc.i12.2016-08-02.tif <sd>/surf/lh.sphere.reg` (nested) and `ln -sf lh.sphere.reg lh.fsaverage.sphere.reg` (bare) | x | x | timed + nested | 4213 |
| 64 | Jacobian white | `mris_jacobian ../surf/lh.white.preaparc ../surf/lh.sphere.reg ../surf/lh.jacobian_white` | x | x | timed | 4242 |
| 65 | AvgCurv | `mrisp_paint -a 5 <FS>/average/lh.folding.atlas.acfb40.noaparc.i12.2016-08-02.tif#6 ../surf/lh.sphere.reg ../surf/lh.avg_curv` | x | x | timed | 4315 |
| 66 | Cortical Parc | `mris_ca_label -l ../label/lh.cortex.label -aseg ../mri/aseg.presurf.mgz -seed 1234 <subj> lh ../surf/lh.sphere.reg <FS>/average/lh.DKaparc.atlas.acfb40.noaparc.i12.2016-08-02.gcs ../label/lh.aparc.annot` | x | x | timed | 4350-4359 |
| 67 | WhiteSurfs | `mris_place_surface --adgws-in ../surf/autodet.gw.stats.lh.dat --seg aseg.presurf.mgz --threads N --wm wm.mgz --invol brain.finalsurfs.mgz --lh --i ../surf/lh.white.preaparc --o ../surf/lh.white --white --nsmooth 0 --rip-label ../label/lh.cortex.label --rip-bg --rip-surf ../surf/lh.white.preaparc --aparc ../label/lh.aparc.annot [v8: --restore-255 and the PlaceWhiteSurf options]` | x | x | timed | 4435-4460 |
| 68 | T1PialSurf | `mris_place_surface --adgws-in ... --seg aseg.presurf.mgz --threads N --wm wm.mgz --invol brain.finalsurfs.mgz --lh --i ../surf/lh.white --o ../surf/lh.pial.T1 --pial --nsmooth 0 --rip-label ../label/lh.cortex+hipamyg.label --pin-medial-wall ../label/lh.cortex.label --aparc ../label/lh.aparc.annot --repulse-surf ../surf/lh.white --white-surf ../surf/lh.white [v8: --restore-255]`; `ln -sf lh.pial.T1 lh.pial` (bare) | x | x | timed | 4509-4534 |
| 69 | white/pial curv, area | `mris_place_surface --curv-map ../surf/lh.white 2 10 ../surf/lh.curv`, `--area-map ../surf/lh.white ../surf/lh.area`, `--curv-map ../surf/lh.pial 2 10 ../surf/lh.curv.pial`, `--area-map ../surf/lh.pial ../surf/lh.area.pial` | x | x | timed | 4760-4762 |
| 70 | thickness | `mris_place_surface --thickness ../surf/lh.white ../surf/lh.pial 20 5 ../surf/lh.thickness` | x | x | timed | 4785 |
| 71 | area and vertex vol | `vertexvol --s <subj> --lh --th3` (§4) | x | x | timed | 4807 |
| 72 | Curvature Stats | `mris_curvature_stats -m --writeCurvatureFiles -G -o ../stats/lh.curv.stats -F smoothwm <subj> lh curv sulc` | x | x | timed | 4834 |
| 73 | Cortical ribbon mask | `mris_volmask --aseg_name aseg.presurf --label_left_white 2 --label_left_ribbon 3 --label_right_white 41 --label_right_ribbon 42 --save_ribbon --parallel <subj>` | x | x | timed | 4854-4864 |
| 74 | Cortical Parc 2, 3 | `mris_ca_label` as 66 with `lh.CDaparc.atlas.acfb40.noaparc.i12.2016-08-02.gcs` → `lh.aparc.a2009s.annot`, and `lh.DKTaparc.atlas.acfb40.noaparc.i12.2016-08-02.gcs` → `lh.aparc.DKTatlas.annot` | x | x | timed | 4888, 4932 |
| 75 | WM/GM Contrast | `pctsurfcon --s <subj> --lh-only` (§4) | x | x | timed | 4975 |
| 76 | Relabel Hypointensities | `mri_relabel_hypointensities aseg.presurf.mgz ../surf aseg.presurf.hypos.mgz` | x | x | timed | 5007-5013 |
| 77 | APas-to-ASeg | `mri_surf2volseg --o aseg.mgz --i aseg.presurf.hypos.mgz --fix-presurf-with-ribbon <sd>/mri/ribbon.mgz --threads N --lh-cortex-mask <sd>/label/lh.cortex.label --lh-white <sd>/surf/lh.white --lh-pial <sd>/surf/lh.pial` and the same for `rh` | x | x | timed | 5030-5039 |
| 78 | | `mri_brainvol_stats --subject <subj>` | x | x | timed | 5059 |
| 79 | AParc-to-ASeg aparc, a2009s, DKTatlas | `mri_surf2volseg --o aparc+aseg.mgz --label-cortex --i aseg.mgz --threads N --lh-annot <sd>/label/lh.aparc.annot 1000 --lh-cortex-mask ... --lh-white ... --lh-pial ... --rh-annot ... 2000 ...`; a2009s with 11100 / 12100, DKTatlas with 1000 / 2000 | x | x | timed | 5082-5092 |
| 80 | WMParc | `mri_surf2volseg --o wmparc.mgz --label-wm --i aparc+aseg.mgz --threads N --lh-annot ... 3000 ... --rh-annot ... 4000 ...`; `mri_segstats --seed 1234 --seg mri/wmparc.mgz --sum stats/wmparc.stats --pv mri/norm.mgz --excludeid 0 --brainmask mri/brainmask.mgz --in mri/norm.mgz --in-intensity-name norm --in-intensity-units MR --subject <subj> --surf-wm-vol --ctab <FS>/WMParcStatsLUT.txt --etiv [v8: --stiv stats/synthseg.tiv.dat]` | x | x | timed | 5121, 5148-5161 |
| 81 | Parcellation Stats (1, 2, 3) | `mris_anatomical_stats -no-th3 -mgz -cortex ../label/lh.cortex.label -f ../stats/lh.aparc.stats -b -a ../label/lh.aparc.annot -c ../label/aparc.annot.ctab <subj> lh white`, the same on `pial` into `lh.aparc.pial.stats`, then a2009s and DKTatlas on `white` | x | x | timed | 5190-5271 |
| 82 | ASeg Stats | `mri_segstats --seed 1234 --seg mri/aseg.mgz --sum stats/aseg.stats --pv mri/norm.mgz --empty --brainmask mri/brainmask.mgz --brain-vol-from-seg --excludeid 0 --excl-ctxgmwm --supratent --subcortgray --in mri/norm.mgz --in-intensity-name norm --in-intensity-units MR --etiv [v8: --stiv stats/synthseg.tiv.dat] --surf-wm-vol --surf-ctx-vol --totalgray --euler --ctab <FS>/ASegStatsLUT.txt --subject <subj>` | x | x | timed | 5301-5339 |
| 83 | BA_exvivo Labels | `ln -s <FS>/subjects/fsaverage` into `$SUBJECTS_DIR` if missing (bare); per hemisphere: `mri_label2label --srcsubject fsaverage --srclabel $SUBJECTS_DIR/fsaverage/label/lh.<L>.label --trgsubject <subj> --trglabel ./lh.<L>.label --hemi lh --regmethod surface` for 14 `*_exvivo` and 8 `*.mpm.vpnl` labels, `mris_label2annot --s <subj> --ctab <FS>/average/colortable_vpnl.txt --hemi lh --a mpm.vpnl --maxstatwinner --noverbose --l ...`, the 14 `.thresh` labels, `mris_label2annot` for `BA_exvivo` and `BA_exvivo.thresh`, `mris_anatomical_stats -no-th3 -mgz -f ../stats/lh.BA_exvivo.stats -b -a ./lh.BA_exvivo.annot -c ./BA_exvivo.ctab <subj> lh white` and the `.thresh` one | x | x | timed | 5395-5595 |

Recon-all's `-all` therefore ends after 83 rows; `scripts/recon-all.done` is written (5945-5959).

## 4. The scripts recon-all calls, and what they run

Each is a program to port in its own right (the plan's rule: "Scripts recon-all calls ... are
programs too"). Lines are each script's.

- **`mri_nu_correct.mni`** (tcsh, 546 lines; both streams; stages 10, 17). With `--ants-n4` (the
  default): `AntsN4BiasFieldCorrectionFs -i orig.mgz -o <tmp>/nu0.mgz --dtype uchar` (no mask, no
  threads argument; 125-130); then, unless `--no-rescale` (stage 17 rescales, stage 10 does not):
  `mri_binarize --i nu0.mgz --min -1 --o ones.mgz`, `mri_segstats --id 1 --seg ones.mgz --i <in>
  --sum ... --avgwf input.mean.dat` and the same on the output, `bc` for the ratio, `mris_calc -o
  nu0.mgz nu0.mgz mul <ratio>` (184-208); `mri_convert nu0.mgz <out> --like <in>` (220); with
  `--uchar`: `mri_make_uchar <out> transforms/talairach.xfm <out>` (226-229). Without N4 it would run
  `mri_convert` to MINC, `nu_correct -clobber ... [-iterations 1000 -distance 50]` `--n` times
  (138-176), all bare. Environment: `FS_ANTS_N4_REPLACE_ZEROS` (48-51).
- **`talairach_avi`** (tcsh, 253 lines; classic, stage 11): `mri_convert <in> talsrcimg.img`
  (ANALYZE), `mpr2mni305 talsrcimg` with `REFDIR=<FS>/average` and target `711-2C_as_mni_average_305`
  (84-89), `avi2talxfm <in> <FS>/average/mni305.cor.mgz talsrcimg_to_711-2C_as_mni_average_305_t4_vox2vox.txt
  <xfm>` (97), `lta_convert --src <in> --trg mni305.cor.mgz --inxfm <xfm> --outlta <xfm>.lta --subject
  fsaverage` (121), all bare, logging to `transforms/talairach_avi.log`. `mpr2mni305` (csh) runs
  `analyzeto4dfp talsrcimg -O0 -y`, `gauss_4dfp talsrcimg 1.1`, `imgreg_4dfp` six times against
  `<FS>/average/711-2C_as_mni_average_305` (modes 4355, 4355, 1283, 3335, 2311, 2311; the first pass
  unmasked, the others with `711-2B_as_mni_average_305_mask`; 161-183) and `compute_vox2vox`
  (189); `t4img_4dfp` and `ifh2hdr` only with `debug` (198-202), `zero_lt_4dfp` in an `if (0)`
  block (142-153). These are Washington University's 4dfp tools, C and Fortran
  (`talairach_avi/CMakeLists.txt`). `avi2talxfm` (csh, 71 lines) runs `fs_temp_dir`, `mri_info
  --vox2ras` twice and `mri_matrix_multiply -fsl` (47-56).
- **`rca-talairach`** (tcsh, 299 lines; v8, stage 15): input forced to `mri/synthstrip.mgz` (258);
  `fs-synthmorph-reg --i synthstrip.mgz --t $FREESURFER/average/mni305.cor.stripped.mgz --affine-only
  --o mri/transforms/synthmorph.mni305 --threads $OMP_NUM_THREADS` (87; `--threads` resets
  `OMP_NUM_THREADS`, 165-170, so recon-all's count reaches it: the ds000005 v8 recording shows
  `--threads 4` and `mri_synthmorph ... -j 4`); then (bare) `lta_convert --ltavox2vox --inlta
  aff.lta --outlta transforms/talairach.xfm.lta` and `lta_convert --inlta talairach.xfm.lta --outmni
  transforms/talairach.xfm` (103-106).
- **`fs-synthmorph-reg`** (tcsh, 1001 lines; v8, stages 15, 16, and inside 20, 21). Defaults:
  SynthMorph (not ANTs), crop the input (`mri_mask -bb 3`) and the target, MNI target
  `<FS>/average/mni_icbm152_nlin_asym_09c/reg-targets/mni152.1.0mm.nii.gz` and its cropped versions,
  compute the inverse (21-45). `--t` (stage 15) sets a target and turns the MNI target and both crops
  off (`--t`: MNITarget 0, then 815-819). Programs: `mri_mask -bb 3` (bare), `lta_convert identity`
  (bare), `mri_synthmorph -m affine -t aff.lta <in> <target> -j <threads>` (timed, 315),
  `mri_concatenate_lta -invert1 -invertout ...`, `mri_concatenate_lta -invert2 ...`, `lta_convert
  --invert` (timed), `lta_convert --outmni reg.invol_to_targ.xfm` (bare), stage 16 only:
  `mri_synthmorph -m deform -t deform.mgz -i aff.lta <in> <target> -j <threads> -o synthmorph.out.mgz`
  (`-o` from `--test`; 406), `mri_warp_convert --inras deform.mgz --insrcgeom <in-crop> --outfswarp
  warp.to.mni152.1.0mm.1.0mm.nii.gz --vg-thresh 1e-4 --lta1-inv ... --lta2 ...` (423),
  **`mri_ca_register -invert-and-save <warp> <warp.inv>`** (437), and for `--test`: `mri_vol2vol
  --regheader`, `mri_convert -rt nearest <in> -at <warp> test.nii.gz` (timed), `mri_diff --po`
  (bare). It sets `OMP_NUM_THREADS` and `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` to its `--threads`
  (113-114). Python: `mri_synthmorph` (TensorFlow, with `voxelmorph` and `neurite`).
- **`mri_mcadura_seg`** (tcsh, 507 lines; v8, stage 20): reuses recon-all's SynthMorph directory
  (no new registration when `--synthmorphdir` is given); `mri_vol2vol --reg <mni linear reg> --mov
  <FS>/average/mca-dura.prior.warp.mni152.1.0mm.{lh,rh}.nii.gz --o prior` (128), `mri_mask -T .001
  -crop-to-fov-mm <fov> ...` (139), `mri_vol2vol --regheader` (149), for the right hemisphere
  `mri_convert --left-right-reverse-pix` (162), **`mri_sclimbic_seg --model
  $FREESURFER_HOME_FSPYTHON/models/mca-dura.both-lh.nstd21.fhs.h5 --ctab ... --keep_ac --percentile
  99.9 --vmp --output-base mcadura --conform --logfile mri_mcadura_seg.log --no-cite-sclimbic
  --threads N --fov 72 --i <crop> --o <seg>`** (173), `mri_convert --left-right-swap-label-table ...
  --left-right-reverse-pix` (188), `mri_vol2vol --regheader --interp nearest` (200), `mri_concat
  <lh> <rh> --sum --ctab ... --o mca-dura.mgz` (214), `mri_binarize --match 3 42` and `mri_segstats
  --accumulate` for its stats (224-233).
- **`mri_vsinus_seg`** (tcsh, 673 lines; v8, stage 21): `mri_vol2vol --mov
  <FS>/average/vsinus.no-sp.prior.mni152.1.0mm.mgz --lta ...` (133), `mri_mask -T .001 -crop 0`
  (142), `mri_binarize --crop-around-ras ...` (151), **`mri_sclimbic_seg --model
  .../models/vsinus.no-sp.m.all.nstd10-070.h5 ...`** (162), `mri_binarize --match 3 42 --inv` on the
  SynthSeg cortex (`--rca-synthseg`, 177), `mri_vol2vol --regheader --interp nearest` (187),
  `mri_mask` (202), `mri_segstats --subject <subj> --etiv` (214). Recon-all builds its command with
  the `$xopts` left from the previous step (recon-all 2241, before 2243 reads its own): harmless
  without expert options.
- **`mri_entowm_seg`** (tcsh, 180 lines; v8, stage 29, and inside `label-cortex --fix-ga`): one
  **`mri_sclimbic_seg --no-cite-sclimbic --model $FREESURFER/models/entowm.fsm31.t1.nstd00-30.nstd21-108.h5
  --keep_ac --ctab .../entowm.ctab --percentile 99.9 --vmp --output-base ...`** (67), with `--s`: the
  input `nu.mgz`, outputs `mri/entowm.mgz` and `stats/entowm.stats`.
- **`seg2cc`** (tcsh, 362 lines; both, stage 31): `mri_info --conformed-to-min` (bare); conformed
  input: `mri_cc -aseg aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta transforms/cc_up.lta <subj>`
  (`fs_time`, 96-102); otherwise `mri_convert --conform_min`, `mri_label2vol`, `mri_cc -norm`,
  `mri_label2vol`, `mri_binarize`, `mergeseg` (113-158). The recorded heads are conformed at 1 mm, so
  the first branch.
- **`defect2seg`** (tcsh, 387 lines; both, stage 52), bare: per hemisphere `mri_label2label
  --label-cortex <surf> <aseg> 0 <label>`, `mri_label2vol --defects <surf> <defects> <template>
  <offset> 0 <out> <label>`, `mris_defects_pointset -s <surf> -d <defects> -o <pointset>` (96-132).
- **`label-cortex`** (tcsh, 515 lines; both, stage 57), bare: `mri_label2label --label-cortex
  ../surf/lh.white.preaparc aseg.presurf.mgz 0 ../label/lh.cortex.label` (106); with v8's `--fix-ga`
  the label is first made in a temporary folder and the gyrus ambiens is added: `mri_binarize --match
  3201|4201 --i entowm.mgz`, `mri_vol2surf --regheader <subj> --hemi lh --surf white.preaparc
  --projdist-max -1 0 .1`, `mris_convert --to-scanner -n` (normals), `mri_binarize --frame 0 ...` and
  `--frame 2 ...`, `fscalc ... and ...`, `mri_cor2label --i ... --surf-path ... --id 1 --thresh 0.5`,
  `mri_mergelabels` (135-221).
- **`rca-surfreg`** (tcsh, 385 lines; both, stage 63): §3 row 63; `josareg` only with `-josareg`.
- **`vertexvol`** (tcsh, 228 lines; both, stage 71), bare: `mris_calc -o lh.area.mid lh.area add
  lh.area.pial`, `mris_calc ... div 2`, and with `--th3`: `mris_convert --volume <subj> lh
  <sd>/surf/lh.volume` (54-83).
- **`pctsurfcon`** (tcsh, 394 lines; both, stage 75), bare: `mri_vol2surf --mov <sd>/mri/rawavg.mgz
  --hemi lh --noreshape --interp trilinear --projdist -1 --o <tmp>/lh.wm.mgh --regheader <subj>
  --cortex` (white matter 1 mm inside the white surface) and the same without `--projdist` (on the
  surface itself) for the gray matter (25-42, 101-123); `mri_concat <wm> <gm> --paired-diff-norm --mul
  100 --o <sd>/surf/lh.w-g.pct.mgh`, `mri_segstats --in ... --annot <subj> lh aparc --sum
  <sd>/stats/lh.w-g.pct.stats --snr` (128-139).
- **Helpers with no image output**: `rca-config`, `rca-config2csh`, `csvprint` (Python);
  `fsr-getxopts`, `fsr-checkxopts`, `fs-check-version`, `fs-check-os`, `UpdateNeeded`, `getfullpath`,
  `fname2stem`, `fname2ext` (tcsh/csh); `fs_temp_file` (C++); `bc`, `awk`, `cp`, `ln`, `rm` (system).
  LARMORX's `recon-all` (wave 3.6) reproduces what they decide, not the programs themselves.

The Python networks and what they load (the weights are data files: sub-task 4):

| Program | Framework | Model loaded by recon-all's call |
|---|---|---|
| `mri_synthstrip` | PyTorch, `surfa` | `<FS>/models/synthstrip.1.pt` (no `--no-csf`; `mri_synthstrip` 255-266) |
| `mri_synthseg` | TensorFlow/Keras, `surfa`, SciPy, nibabel | `synthseg_2.0.h5` (no `--robust`, `--parc`, `--qc`, `--v1`; `mri_synthseg` 109-126) |
| `mri_synthmorph` | TensorFlow, `voxelmorph`, `neurite`, `surfa` | its affine and deformable models (`mri_synthmorph/synthmorph.{rigid.1,affine.2,deform.3}.h5`; which file each mode loads: sub-task 2) |
| `mri_sclimbic_seg` | TensorFlow/Keras, `surfa`, SciPy | `mca-dura.both-lh.nstd21.fhs.h5`, `vsinus.no-sp.m.all.nstd10-070.h5`, `entowm.fsm31.t1.nstd00-30.nstd21-108.h5` (`mri_sclimbic_seg/`) |

## 5. fMRIPrep 25.2.6 and sMRIPrep 0.19.2

What the image's packages run (file:line in the installed package; copies read from the image).

**recon-all, in pieces** (sMRIPrep `workflows/surfaces.py`, `interfaces/freesurfer.py` 123-210,
which builds the step flags from nipype's lists and adds `-no<step>` for steps whose outputs exist):

| Node | Call | Line |
|---|---|---|
| `autorecon1` | `recon-all -autorecon1 -noskullstrip -noT2pial -noFLAIRpial [-cw256 if the FOV exceeds 256 mm] [-hires and expert option mris_inflate -n 50 if the T1w voxel is below 0.95 mm] [-T2 <t2w> or -FLAIR <flair>] -i <T1w reference> -openmp N`; never resumed | 216-228; `_check_cw256` 1727; niworkflows `detect_inputs` |
| `skull_strip_extern` | no FreeSurfer program: niworkflows `inject_skullstripped` writes `brainmask.auto.mgz` and `brainmask.mgz` from fMRIPrep's own brain mask (nilearn resampling) | 230 |
| `gcareg` | `recon-all -gcareg -openmp N` | 500-506 |
| `autorecon2_vol` | `recon-all -autorecon2-volonly -openmp N` | 508-514 |
| `autorecon_surfs` | `recon-all -autorecon-hemi lh|rh -noparcstats -noparcstats2 -noparcstats3 -nohyporelabel -nobalabels -openmp N` | 516-534 |
| `cortribbon` | `recon-all -cortribbon -parallel [-T2pial|-FLAIRpial]` | 541-546 |
| `parcstats` | `recon-all -autorecon-hemi lh|rh -nohyporelabel -openmp N` | 550-558 |
| `autorecon3` | `recon-all -autorecon3 -openmp N` | 562-568 |

Every one of these flags exists in 8.2.0's recon-all (checked case by case). With FreeSurfer 8.2.0
these calls would run the **v8 stream** (no `-no-v8` is passed): `-synthstrip` then meets
`-noskullstrip` and the injected mask (an open question, §7). fMRIPrep 25.x itself ships
FreeSurfer 7.3.2.

**Outside recon-all:**

| Program | Call (nipype interface → command) | Where | When |
|---|---|---|---|
| `mri_robust_template` | `StructuralReference(auto_detect_sensitivity, initial_timepoint=1, intensity_scaling, subsample_threshold=200, fixed_timepoint, no_iteration, transform_outputs)` → `mri_robust_template --satit --mov ... --inittp 1 --iscale --subsample 200 --fixtp --noit --lta ... --template ...` (without `--fixtp --noit` for an unbiased template) | sMRIPrep `workflows/anatomical.py` 1511-1522; niworkflows `interfaces/freesurfer.py` 48-102 | only with several T1w (or T2w) images; one image is copied (`echo Only one time point!`) |
| `lta_convert` | `--inlta identity.nofile --src <img> --trg <conformed> --outlta` | anatomical.py 1493; niworkflows freesurfer.py 93 | always (per input image) |
| `mris_expand` | `MakeMidthickness(thickness=True, distance=0.5)` → `mris_expand -thickness <sd>/surf/lh.white 0.5 lh.midthickness` (or `cp` of an existing `graymid`), `OMP_NUM_THREADS` = floor(1.5 × threads) | surfaces.py 236-241; sMRIPrep `interfaces/freesurfer.py` 302-350 | with FreeSurfer |
| `mri_robust_register` | `RobustRegister(auto_sens, est_int_scale)` → `mri_robust_register --satit --iscale --mov <sd>/mri/T1.mgz --dst <T1w ref> --lta ...` (then the filename lines of the LTA truncated) | surfaces.py 320-329 | with FreeSurfer, unless the transform is precomputed |
| `bbregister` | nipype's `BBRegister` (`dof` is `--%d`, so `--6`): sMRIPrep T2w: `bbregister --s <subj> --mov <t2w> --t2 --init-coreg --6 --reg ... --lta ... --gm-proj-abs 2 --wm-proj-abs 1`; fMRIPrep BOLD: `bbregister --s <subj> --mov <boldref> --t2 --6 --reg ... --lta ... --initcost bbregister.initcost --init-reg <mri_coreg's LTA>` (or `--init-header`); `--bold2anat-dof` 9 or 12 gives `--9`/`--12` | anatomical.py 1145-1153; fMRIPrep `workflows/bold/registration.py` 343-354 | with FreeSurfer (T2w: only with a T2w and FreeSurfer) |
| `mri_coreg` | `MRICoreg(dof=6, sep=[4], ftol=0.0001, linmintol=0.01)` → `mri_coreg --s <subj> --sd ... --mov <boldref> --lta ... --dof 6 --sep 4 --ftol 1.000000e-04 --linmintol 1.000000e-02` (nipype's `%e`), `--ref-mask` changed when a T2w is used (`reference_mask = False`; how nipype writes that: from a run) | registration.py 333-341; nipype `registration.py` | with FreeSurfer, BOLD to T1w |
| `mri_vol2vol` | `ApplyVolTransform(interp='nearest')` → `mri_vol2vol --mov <aseg|aparc> --targ <T1w> --lta ... --interp nearest --o seg.nii.gz` | surfaces.py 1309 | with FreeSurfer |
| `mris_convert` | `MRIsConvert(out_datatype='gii', to_scanner=...)` → `mris_convert --to-scanner <surf> <out>.gii` (spheres without `--to-scanner`); `MRIsConvertData` → `mris_convert -c <sd>/surf/lh.thickness <sd>/surf/lh.white <out>.gii` (thickness, sulc, curv) | surfaces.py 906, 1001 | with FreeSurfer |
| `mri_vol2surf` | `SampleToSurface(interp_method='trilinear', out_type='gii', override_reg_subj, sampling_method='average', sampling_range=(0, 1, 0.2), sampling_units='frac')` → `mri_vol2surf --mov <bold_t1w> --reg <lta> --hemi lh --interp trilinear --projfrac-avg 0 1 0.2 --trgsubject <fsnative|fsaverageN> --srcsubject <subj> --o <out>.gii` | fMRIPrep `workflows/bold/resampling.py` 175-184 | surface outputs (`fsnative`, `fsaverage*`: the fsaverage step is inside `mri_vol2surf`, no `mri_surf2surf`) |
| `mri_convert` | the licence check: `mri_convert <sentinel.nii.gz> <tmp>/out.mgz` | niworkflows `utils/misc.py` 359-378 | at start |

`bbregister` (tcsh, `scripts/bbregister`) runs, for these calls: `mri_convert` (the template),
`mri_coreg --s <subj> --mov ... --regdat ... --dof ... --sep 4 --ftol .0001 --linmintol ...`
(`--init-coreg`, 260), `tkregister2_cmdl` (277-292, header and conversions), `lta_convert --outreg`
(306), `mri_segreg` twice (the brute-force first pass 332, the fine pass 373), `mri_surf2surf --reg
... --reg-diff ... --sval-xyz white --rms ...` per hemisphere (its check of how far the surfaces
moved, 421-430), `tkregister2_cmdl --ltaout` (452). `mri_fwhm`, `fslregister`, `spmregister`,
`mri_robust_register` and `mni152reg` only for other options.

Not FreeSurfer, though in the same nodes: `Binarize` is niworkflows' pure-Python one; fsaverage's
resampling and the CIFTI steps use Workbench (`wb_command`), the surfaces track.

## 6. The programs, counted

`programs.toml` lists every program above once (kind, source, streams, the steps and options, how it
is run, what it calls): 110 entries, 102 of them on a default path (`recon-all -all` in either
stream, or fMRIPrep's calls) and 8 only on other paths (`mri_robust_template` with several inputs,
`mris_topo_fixer`, `mri_seg_diff`, `mri_compute_seg_overlap`, `mergeseg`, `fsr-checkxopts`,
`nu_correct`, MINC's `talairach`). The 102: 66 compiled C++ programs of FreeSurfer (with
`AntsN4BiasFieldCorrectionFs` and `AntsDenoiseImageFs`, its ITK wrappers, and the helpers
`fs_temp_file`, `fs_temp_dir`), 4 compiled 4dfp tools of Washington University (C, two with
Fortran), 4 Python network programs, 24 tcsh/csh scripts (recon-all and its helpers among them),
3 Python helpers and one awk script. By stream: v8 84 programs, classic 80, fMRIPrep's calls 14;
16 run only in v8, 13 only in classic. The list grows by what the recordings show.

**Named by the plan or the version list, but not run by 8.2.0's `recon-all -all`:**
`mri_segment_hypointensities` (no call at all), `mris_make_surfaces` (replaced by
`mris_place_surface`), `mri_aparc2aseg` (replaced by `mri_surf2volseg`), `mri_cc` directly (only
through `seg2cc`), `mris_thickness` (replaced by `mris_place_surface --thickness`), `mri_remove_neck`
and the second `mri_em_register` (`-rmneck`, `-skull-lta`, off in `-all`), `mri_gcut`
(`-gcut`), `mri_robust_template` (only with several inputs), `mris_topo_fixer` (only when
`mris_fix_topology` leaves a defect), `nu_correct` (only `-no-ants-n4`), MINC's `talairach` (only
when the Talairach check fails twice), `mri_seg_diff` (only with edits), `mris_reposition_surface`
(only with `repos.*.json`), `mri_stopmask` (`-stopmask-scm`), `mri_synthsr`, `samseg`, `topofit`,
`conf2hires`.

## 7. What this changes in the plan, and open questions

For the handoff (`docs/freesurfer-plan.md`, its waves; the final relay writes the exact text):

1. **The intensity correction is N4 by default in 8.2.0** (`DoAntsN4: True`), in both streams:
   `AntsN4BiasFieldCorrectionFs` (FreeSurfer's wrapper of ITK's N4, `--dtype uchar`), not MNI's N3;
   `nu_correct` only with `-no-ants-n4`. The plan's "runs MNI's N3 ... `--ants-n4` off by default" is
   to be corrected; wave 3.2's `mri_nu_correct.mni` builds on the ITK N4 port larmorx already has.
2. **Two more programs in both streams**: `AntsDenoiseImageFs` (ITK's non-local-means denoising
   before `mri_segment`) and `mris_remesh` (after the topology fix).
3. **The v8 stream keeps `mri_em_register` and `mri_ca_normalize`** (`norm.mgz` comes from them), and
   runs `mri_ca_register -invert-and-save` inside `fs-synthmorph-reg`: wave 3.3's "classic stream's
   `mri_em_register`, `mri_ca_normalize`" is shared, not classic-only.
4. **A fourth network program**: `mri_sclimbic_seg` with three models (MCA/dura, venous sinuses,
   entorhinal white matter), run by the v8 stream's `mri_mcadura_seg`, `mri_vsinus_seg`,
   `mri_entowm_seg` (and `label-cortex --fix-ga`); and `rca-talairach` (SynthMorph's affine to
   `mni305.cor.stripped.mgz`) is how v8 makes `talairach.xfm`.
5. **`mri_cc` runs through `seg2cc`**; `mri_segment_hypointensities` is not run in 8.2.0 (remove it
   from wave 3.3); the v8 stream adds `mri_edit_wm_with_aseg`'s `-sa-fix-ento-wm`, `-sa-fix-acj`,
   `-fix-ento-wm`, `-fix-acj`, `-fill-seg-wm`, `-fix-scm-ha` and `mri_mask -invert`.
6. **fMRIPrep's `mri_surf2surf` comes through `bbregister`** (its RMS check), not from fMRIPrep's own
   workflows; fsaverage sampling is `mri_vol2surf --trgsubject`. `mri_segreg` and `tkregister2_cmdl`
   join wave 3.5 with `bbregister`.
7. **The default GCA** is `RB_all_2020-01-02.gca` (and `RB_all_withskull_2020_01_02.gca`);
   `RB_all_2019_10_25.talxfm.mni305.gca` is read only when `mri_em_register` is turned off. The v8
   stream also reads `mni305.cor.stripped.mgz`, the MNI152 2009c registration targets, two MCA/dura
   priors and a venous-sinus prior (for the data-file list).

Script behaviours worth a finding when the recordings confirm them (rules A-D apply when ported):
the v8 `T1PialSurf` key that never matches, so `mrisps.pial.mgz` is never written (§1); the v8
`mri_ca_label` options that no stream applies (§1); `mri_vsinus_seg` receiving the previous step's
expert options (§4).

Open questions (also in the relay notes):

- **sMRIPrep on 8.2.0**: its recon-all calls would run the v8 stream. Should LARMORX's pipeline
  (larmorprepx) call recon-all's v8 stream, or `-no-v8` as fMRIPrep's 7.3.2 behaves? And with v8,
  what `-noskullstrip` with `-synthstrip` and the injected `brainmask.mgz` do in 8.2.0 (the v8
  `mri_mask T1 synthstrip brainmask` step is under `DoSkullStrip`, so `-noskullstrip` skips it and
  the injected mask stays; to be confirmed by a run).
- Which stream is ported first is the user's choice (decision log); the inventory covers both.
