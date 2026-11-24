# fs-volume: FreeSurfer 8.2.0's volume functions against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running
`fsvolume` is a black-box run of FreeSurfer's code).

- `fsvolume.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-volume/fsvolume` (`build.sh
  <workspace>`): FreeSurfer's `fs.volume` functions linked with the unit oracle's libraries as the
  official programs are. One line per case: every case runs in a child process whose standard output
  and error go to a scratch file beside the cases; `<result> | <text>` with the text C-escaped, or
  `EXIT <status> | <text>`, `CRASH <signal> | <text>`. Floats and doubles are their bits in hex,
  byte strings (files) hex.
- The port's side: `crates-freesurfer/larmorx-freesurfer-volume/examples/fs_volume_probe.rs` (`cases
  <seed> <n>` writes the cases, `run <cases> fix|reproduce` answers them; the generator and the
  answers in `tests/fs_volume/cases.rs`, shared with the hash test `tests/fs_volume_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through FreeSurfer by the oracle-output
  cache at one thread, the port in both modes, the comparison (`reproduce` identical but where the
  port notes FS-53, FS-58, FS-288, FS-290 or FS-294 (memory FreeSurfer does not own, crashes), counted
  and not compared; `fix` different only with a notice), and the hash the test checks. Run it inside
  `scripts/orchestration/boxed.sh`.
- `bright_closed.py <worktree> <open probe> <replica example> <scratch>` (track E's relay 22,
  2026-11-24): §59's FS-1233 text checked against the replica's default. The open probe's `fbnwm`
  lines are converted to this probe's case format and the result compared with `models.py bright`'s
  model: 92 lines with FS-1233, 92 as the model. Run it inside `scripts/orchestration/boxed.sh`.

Commands (part 1, `transform.cpp`'s geometry functions): `vgwrite`, `vgread` (twice in the cycle),
`vgprint0`, `getvg`, `copyvg`, `vgi2r`, `vgr2i`, `tkrv2r`, `vgeq0`. Part 2 (3.1-35's voxel arithmetic):
`clone`, `clonesp`, `clear`, `match`, `cpframe`, `cpframes`, `linscale`, `smul`, `smulf`, `thresh`,
`uthresh`, `invc`, `binz`, `abs`, `pos`, `neg`, `mul2`, `sub2`, `add2`, `div2`, `sam`, `vrange`,
`vrangef`, `vrangereg`, `lrange`, `extract`, `cras`. A volume of the cases (`get_vol`): its extent,
type, frames, whether it has voxels, a fill seed (float voxels from a table of special values, other
types' bytes from a formula), its geometry (but its extent), header fields, and its cached matrices
kept, recomputed or (only for `cras`: `MRIcopyHeader` crashes on a source without them) dropped.

Measured 2026-11-13 (part 1): seed 7, 2,000 cases: 1,980 identical with `reproduce`, 20 not
comparable (FS-288); `fix` different only with notices (103 lines: FS-285, FS-286, FS-287). Seed 1,
400 cases (`check.py --seed 1 --n 400`, boxed 8 GiB): 393 identical, 7 not comparable, hash
`2487de11dfff688092644b55b65d2770b54c4d8581e7878b93f9c9be6edf7429`, passing with AArch64's NaN
emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with
`NAN_TRAP_DIR`) in debug and release.

Measured 2026-11-13 (parts 1 and 2, 37 commands): seed 7, 37,000 cases (`check.py --seed 7 --n 37000`
inside `boxed.sh 10`, about 10 minutes): 34,624 identical with `reproduce`, 2,376 not comparable (456 of
them identical all the same), 0 different; `fix` different on 5,580 lines, each with a notice. Seed 1,
740 cases: 686 identical, 54 not comparable, hash
`3a5943ae54ab373ec10757ccc3aa7b5561ec09592777384c022b863a7f010f5d`, passing with AArch64's NaN emulated in
debug and release. The official build's operand orders behind the NaN payloads, its crashes and
`readVolGeom`'s frame: `docs/findings/freesurfer-volume.md`.

Measured 2026-11-13 (part 3, 3.1-35's coordinates and geometry and `MRIaverage`: `avg`, `dsign`, `sras`,
`spt`, `ostr`, `vtal`, `com`, `pca`, `cmph`, `dump`; 46 then 47 commands): seed 7, 46,000 cases (46
commands, boxed 10 GiB, log `oracles/logs/fs-found-c-r12-volume-probe-46000.log`): 43,544 identical with
`reproduce`, 2,456 not comparable (479 identical all the same), 0 different; `fix` different only with
notices. FreeSurfer's eigen-systems that never return (`pca` on infinite voxels: the child's alarm,
`CRASH 14`) are counted as not comparable (FS-47, FS-48, ITK-112, ITK-115, ITK-116). Seed 7, 4,700 cases
(47 commands): 4,456 identical, 244 not comparable, 0 different. Seed 1, 940 cases: 886 identical, 54 not
comparable, hash `2b37ea96a0d3665073c7c7b2fc0563aa984f9826fc4d736a8f2c8a702114b30b`, passing with AArch64's
NaN emulated in debug and release.

Measured 2026-11-13 (part 4: 3.1-36's samplers `stri`, `styp`, `sfrm`, `sfty`, `scub`, `scubf`, `ssinc`,
`ssincf`, `coef`, `hsinc`; the moves `trans`, `rot`, `rotx`, `roty`, `rotz`; the head box `skull`; reading
transforms `tftype`, `ltaread`; 65 commands; `check.py --only <commands>` keeps some commands while
porting, without the official hash): seed 7, 10,000 sampler cases: 9,444 identical with `reproduce`, 556
not comparable, 0 different; 6,000 move and head-box cases: 5,110 identical, 890 not comparable, 0
different (641 of the 1,000 `skull` cases find a box); 2,000 transform-reading cases: 1,839 identical,
161 not comparable, 0 different; `fix` different only with notices. Seed 1, 1,300 cases (all 65
commands, log `oracles/logs/fs-found-c-r13-seed1.log`): 1,224 identical, 76 not comparable, hash
`2499daba233eec8c8278d5d5c3b9be2d32569776f9f5d0d4a7eed9571bf94791`. Not comparable now also: FS-300 (a
cubic window never read), FS-302 to FS-310 (crashes, hangs, memory never set).

Measured 2026-11-13 (relay 15: `ltaread` sets `USE_AVERAGE305` per case, unset, `true` or empty, in
the official child's environment and in the port's `LtaIo::env`): seed 1, 1,300 cases (log
`oracles/logs/fs-found-c-r15-avg.log`): 1,198 identical, 102 not comparable, hash
`571a038fdaebd6929c884ca4065eebd07e8143122f07e57fcfc9c6b71175f1c1`. Seed 7, 923 `ltaread` cases: 867
identical, 55 not comparable, 1 different (an `errno` the port kept after `sscanf` met the end of a line:
fixed in the next commit, `scanf_word_errno.c`). After it (log `oracles/logs/fs-found-c-r15-fixcheck.log`):
seed 7, 3,076 `ltaread` cases: 2,908 identical, 168 not comparable, 0 different; seed 13, 6,152 `ltaread`
and `tftype` cases: 5,691 identical, 461 not comparable, 0 different; seed 1 unchanged (hash `571a038f...`).
`scanf_word_errno.c` (build and run with `gcc -O0 scanf_word_errno.c && ./a.out`) measures what glibc's
`sscanf` leaves in `errno` when a conversion meets the end of the string: 0, or, after an earlier item that
ended at the end, the value `errno` had when that item met it (before its own `ERANGE`).

Measured 2026-11-13 (part 5, relay 15: the CMA labels and segmentation functions `cmalab`, `cmavol`,
`lrswap`, `fixrib`, `lrseg`, `ohe`, `bvc`, `bvr`; 73 commands; logs `oracles/logs/fs-found-c-r15-cma3.log`
and `-cma4.log`): seed 7, 8,000 cases of the new commands: 7,427 identical with `reproduce`, 573 not
comparable (FS-314: FreeSurfer aborts, `CRASH 6`; FS-315: lists read past their numbers; FS-53, FS-58),
0 different; seed 13, 16,000: 14,785 identical, 1,215 not comparable, 0 different; all 73 commands at
seed 7, 7,300 cases: 6,841 identical, 459 not comparable, 0 different; `fix` different only with
notices. Seed 1, 1,460 cases: 1,365 identical, 95 not comparable, hash
`dfb6450d2777761356d5587e0e2c2b8bd737ed1d1e1b82352d58a8e59b6ac530`.

Measured 2026-11-13 (part 6, relay 16: 3.1-36's resolution, planes, fills, resampling by whole factors and
changes of type `setres`, `vfill`, `fillfg`, `explane`, `fillplane`, `ups`, `dns2`, `dnsn`, `mft`, `lim`,
`chtype`; 84 commands; logs `oracles/logs/fs-found-c-r16-resize3.log`, `-seed7.log`, `-seed1.log`): seed 13,
11,000 cases of the new commands: 9,164 identical with `reproduce`, 1,835 not comparable (FS-53, FS-58,
FS-290 of the voxel access), 1 different (`MRIsetResolution`'s `thick` of +0 and -0: `macros.h`'s `MAX` is
`a >= b ? a : b`; fixed); `fix` different only with notices. After the fix, all 84 commands at seed 7, 8,400
cases: 7,746 identical, 654 not comparable, 0 different. Seed 1, 1,680 cases: 1,546 identical, 134 not
comparable, hash `919eb356853935742497415be1a6e96cc024dc5e7efdff0dd7499caf1638bf6e`, passing with AArch64's
NaN emulated in debug and release (`oracles/logs/fs-found-c-r16-trap-volume.log`). `dnsn` draws no NaN
factor: with `reproduce` FreeSurfer's extent is then -2147483648 (FS-317), whose allocation error fs.mri's
`Mri::new` does not model yet (`docs/findings/freesurfer-volume.md` §9, for track A).

Measured 2026-11-13 (part 7, relay 17: 3.1-36's frame-range samplers, trilinear kernel and splatting
`splat`, `sseq`, `sxyz`, `tkern`; derivatives, gradients and neighbour counts `deriv`, `grad`, `nbrs`;
linear transforms `lxf`, `rsmat`; 93 commands; logs `oracles/logs/fs-found-c-r17-*.log`): seed 7, 8,000
cases of `splat sseq sxyz tkern`: 7,399 identical with `reproduce`, 601 not comparable (FS-290), 0
different (after one fix: the nearest frame-range sampler keeps a signalling NaN voxel as the official
build does); seed 7, 9,000 cases of `deriv grad nbrs`: 7,422 identical, 1,578 not comparable (FS-53,
FS-58, FS-290, FS-322 to FS-324), 0 different; seed 7, 5,000 cases of `lxf rsmat`: 4,923 identical, 77
not comparable, 0 different; `fix` different only with notices. Seed 1, 1,860 cases: 1,713 identical, 147
not comparable, hash `718b2164c1606fdac499763c22925f59159265321405289580253ec012eefa7c`, passing with
AArch64's NaN emulated in debug and release (`oracles/logs/fs-found-c-r17-trap-volume.log`). The port's
debug and release builds agree on 42,460 cases (seeds 1, 7 and 13).

Measured 2026-11-13 (part 8, relay 18: 3.1-37's resampling onto a template `rsfill` (`MRIresampleFill`,
`MRIresample`; types nearest, trilinear, sinc, cubic, weighted, vote and an unknown one; one case in twelve
an 8 x 8 x 8 source voting into one voxel) and 3.1-38's `repval` (`MRIreplaceValues`,
`MRIreplaceValuesUchar`); 95 commands; logs `oracles/logs/fs-found-c-r18-*.log`): seed 7, 400 cases: 392
identical with `reproduce`, 8 not comparable, 0 different; seed 13, 4,000 cases (the generator's later form:
most templates on the source's grid): 3,888 identical, 112 not comparable (FS-53, FS-58, FS-290, FS-300,
FS-327), 0 different; `fix` different only with notices. Seed 1, 1,900 cases: 1,745 identical, 155 not
comparable, hash `0b5f671441495eed2d7a7aa8001aa5317d3c1cf986fe06d3c1bf0a911498246b`, passing with
AArch64's NaN emulated in debug and release (`oracles/logs/fs-found-c-r18-trap-volume.log`). The port's
debug and release builds agree on the 4,000 cases of seed 13 in both modes.

Measured 2026-11-13 (part 9, relay 19: 3.1-37's statistics, orientation strings and voxel-to-RAS matrices
`stats`, `type2s`, `cropbb`, `crass`, `chkostr`, `o2dc`, `s44`, `vx2rx`, `v2v`, `setv2r`; 3.1-35's 2-D images
`toimgv`, `toimg`, `img2mri` (images of every format, given images of the right and other sizes); 3.1-37's
neighbourhoods and labels `smpar`, `erasebp`, `nbhd`, `nearnz`, `lmeans`, `mkpos`, `mlbv`, `lnbhd`, `linbhd`,
`lbbox`, `fillbox`, `mskz`, `clipwm` (volumes painted with small labels); 121 commands; logs
`oracles/logs/fs-found-c-r19-*.log`): seed 13, piece a 4,000 cases: 3,992 identical with `reproduce`, 8 not
comparable; images 3,000 cases: 2,944 identical, 56 not comparable; piece b 13,000 cases: 11,761 identical,
1,239 not comparable (FS-53, FS-58, FS-163, FS-290, FS-467), 0 different; `fix`
different only with notices. Seed 1, 2,420 cases: 2,254 identical, 166 not comparable, hash
`3834ef65feb37ff13903d5bf1458c793ceae37307d8779a87e70c4b755ef40a7`, passing with AArch64's NaN emulated in
debug and release (`oracles/logs/fs-found-c-r19-trap-volume.log`). `check.py`'s `SKIP` adds FS-163 (MRIcopy past a
smaller destination), FS-330, FS-463, FS-465 and FS-467.

Measured 2026-11-14 (part 10, relay 20: the partial-volume count `pvlab`; 3.1-37 piece c `randn`, `const`, `nseq`,
`nfr`, `sort`, `revsl`, `cdtype`, `nonmax`, `xrpad`, `ordidx`, `vperm`, `rms`, `vfrac`, `cdt`, `diffv`, `tiv`; piece
d's first part `fpow`, `mbin`, `reshape`, `mmax`, `mconst`, `fbin`, `vote`, `msq`, `crop`, `revs`, `cutend`,
`segids`, `hcos`; 151 commands; `fill_paint` fills whole frames and paints voxels of any frame; logs
`oracles/logs/fs-found-c-r20-*.log`): `pvlab` seed 7, 2,000 cases: 1,540 identical with `reproduce`, 460 not
comparable, 0 different; piece c seed 7, 16,000 cases: 14,704 identical, 1,296 not comparable, 0 different; seed 13
(with `pvlab`), 34,000 cases: 30,950 identical, 3,050 not comparable, 0 different; piece d seed 7, 13,000 cases:
11,988 identical, 1,012 not comparable, 0 different; seed 13, 26,000 cases: 23,856 identical, 2,143 not
comparable, 1 different (`revs`: `MRIsetVoxelToRasXform`'s centre adds column 4 last in the official build; fixed,
then `revs`, `setv2r`, `vx2rx` at seed 13, 12,000 cases: 11,700 identical, 300 not comparable, 0 different); `fix`
different only with notices (GLIBC-07 for `powf` and `cos`). Seed 1, 3,020 cases: 2,813 identical, 207 not comparable, hash
`0b75c43a5ca8662cac67b4bce54b64c9d718499cda9360cecd3086fb190d45d3`, passing with AArch64's NaN emulated in debug
(36,191 flips) and release (30,739) (`oracles/logs/fs-found-c-r20-trap-volume.log`); the port's debug and release
builds agree on 47,000 cases in both modes. `check.py`'s `SKIP` adds FS-469, FS-470, FS-471, FS-473, FS-475 and
FS-479. `diffv` seeds the generator before `CTABalloc` (its random colours otherwise print `<program> supposed to be
reproducible but seed not set`).

Measured 2026-11-14 (part 11, relay 21: 3.1-37 piece d's rest and 3.1-38's first files: `vv`, `vvsm`, `vvk`, `vvd`,
`maskup`, `cov`, `mpca`, `pcast`, `fdr`, `rgbb`, `vxn`; 162 commands; logs `oracles/logs/fs-found-c-r21-*.log`): the
resampling commands at seeds 7 and 13, 2,000 cases each: 1,976 and 1,957 identical with `reproduce`, 0 different;
the PCA and FDR commands seed 7, 1,000 cases: 934 identical, 0 different; seed 13, 3,000 cases: 2,829 identical, 0
different (after FS-484); the regions and voxel measures with `maskup`, seeds 7 and 13, 3,000 cases each: 2,277 and
2,262 identical, 0 different (after `FZERO`'s 1e-5 threshold in `region.cpp` and `mrivoxel.cpp`, which a complex
volume's 5.4e-7 voxel showed); `fix` different only with notices. Seed 1, 3,240 cases: 2,992 identical, 248 not
comparable, hash `f38160d248ccc2da57ad5e1109d8b8e978231de8a9466a4c1693df2b08ee243c`, passing with AArch64's NaN
emulated in debug and release (`oracles/logs/fs-found-c-r21-trap-volume.log`); the port's debug and release builds
agree on 11,000 cases of the new commands in both modes. `check.py`'s `SKIP` adds FS-482, FS-484, FS-485.
`pcast` calls `fflush(NULL)` after `WritePCAStats` (the file it leaves open reaches the disk at the program's end).

Measured 2026-11-14 (part 12, relay 22: 3.1-38's `talairachex.cpp`: `talcras`, `talmat`, `talpt`, `talvol`; 166
commands; logs in `<scratch>/C/r22/`): seed 7, 2,000 cases: 1,941 identical with `reproduce`, 59 not comparable
(FS-487, FS-488, FS-53, FS-325; 35 of them identical), 0 different; seed 13, 4,000 cases: 3,859 identical, 141 not
comparable (84 identical), 0 different; `fix` different only with notices. The port's debug and release builds agree
on seed 13's 4,000 cases in both modes. Seed 1, 3,320 cases: 3,083 identical, 237 not comparable, hash
`c3cfc45ecb3faa4cac3084f1f8b08334eab226e31b8e8627585fdd19774571c9`, passing with AArch64's NaN emulated in debug and
release (`oracles/logs/fs-found-c-r22-trap-volume2.log`). `check.py`'s `SKIP` adds FS-487, FS-488. `talvol` left
out `SAMPLE_CUBIC_BSPLINE` until relay 39 (fs.bspline). Relay 22 also measured `fdr` with `log10` now
correctly rounded by default (seeds 7 and 13, 987 cases each: 958 and 947 identical with `reproduce`, 0 different;
`fix` different only with notices, GLIBC-07 among them).

Measured 2026-11-14 (part 13, relay 22, for fs.filter: `dilate`, `dilu`, `msegs`, `sstat`, `hseg`; 171 commands): seeds 7
and 13, 2,000 cases each: 1,878 and 1,882 identical with `reproduce`, the rest not comparable (FS-53, FS-58, FS-290,
FS-490), 0 different; `fix` different only with notices (FS-489 among them). The port's debug and release builds
agree on seed 13's 4,000 cases in both modes. Seed 1, 3,420 cases: 3,145 identical, 275 not comparable, hash
`d67e661fe83e2a5e38001491e6b0b8afd724b5a995ba9a2675034e540593182b`, passing with AArch64's NaN emulated in debug and
release (`oracles/logs/fs-found-c-r22-trap-volume3.log`) and on Windows x64 (track A's
`validation/investigations/windows-cross-check/`, the test cross-built and run on the Windows helper). `check.py`'s
`SKIP` adds FS-490.

Measured 2026-11-14 (part 14, relay 23: 3.1-38's `mri_conform.cpp` and the rest of `mriset.cpp`; 211 commands; logs in
`<scratch>/C/r23/`): `conform` (`MRIconform`'s 256^3 result hashed with FNV-1a 64 on both sides): seed 7, 200 cases,
199 identical with `reproduce`, 1 not comparable, 0 different; seed 13, 400 cases, 392 identical, 8 not comparable, 0
different. The 39 mriset commands (`union isect mand mxor compl edges erodenn erode erode2d erodeseg dilseg dtl dtli
dlu dlab openn mopen mclose dil6 rlist rvonly mdiff mmask tmask mnot slv b2c nmask von vinl linv cent pthr cnbhd grow
onfg offbg cplab cplv`): seed 7, 3,900 cases, 3,659 identical, 241 not comparable, 0 different; seed 13, 7,800 cases,
7,318 identical, 482 not comparable, 0 different (after FS-497: a complex volume dilated until nothing changes hangs
FreeSurfer, `CRASH 14`, and hung the port until it was detected); `fix` different only with notices. Seed 1, 4,220
cases: 3,877 identical, 343 not comparable, 4 `MRIconform` resamplings hashed as `HEAVY` (identical; `check.py`'s
`heavy`: too slow for the recorded test's debug build), hash
`551daf3a76aa59b45c0fcbccc1ac8c8ba764416f7367e11dd4f0beae5bb74483`. `check.py`'s `SKIP` adds FS-491, FS-493, FS-496,
FS-497. The port's `run_line` now runs each case on a thread with a 32 MiB stack (the debug build's frame of `case`
passed a test thread's 2 MiB).

Measured 2026-11-14 (part 15: 3.1-38's `mrihisto.cpp`, relay 24, 220 commands; logs `oracles/logs/fs-found-c-r24-*`):
`hreg` (`MRIhistogramRegion`), `hvox` (`MRIhistogramVoxel`, with and without a threshold volume), `hthr`
(`MRIhistogramRegionWithThreshold`), `hist` (`MRIhistogram`), `geq` (`MRIgetEqualizeHisto`), `heq` (`MRIhistoEqualize`),
`hlr` (`MRIhistogramLabelRegion`), `hlab` (`MRIhistogramLabel`), `hsegm` (`MRIhistoSegment`, labels of 128, with and
without `DIAG_SHOW`). Volumes are filled by `fill_pat` (two populations around two levels with a spread, from a seed:
bimodal histograms, so `MRIhistoSegment` finds peaks and valleys: about 40 % of its comparable cases label voxels
white and non-white); given histograms (`get_histo`: an allocation, the bins in use, old contents) are printed by
`put_histo` (the bins in use up to the allocation). The generator keeps to bytes, ints, longs, floats, shorts and
unsigned shorts and values up to 4096.5 (with NaN and infinities): an RGB volume's values span about 2^30 and a
histogram of one bin per intensity then takes 7 GB on both sides. Seed 7, 1,800 cases: 1,489 identical with
`reproduce`, 311 not comparable, 0 different; seed 13, 1,800 cases: 1,513 identical, 287 not comparable, 0 different;
`fix` different only with notices; debug and release identical on seed 13's 1,800 cases in both modes. Not comparable:
FS-498 (NaN bins: `CRASH 11`), FS-499 (`free(): corrupted unsorted chunks`, `CRASH 6`), FS-501, FS-502 (`CRASH 14`) and
the shared FS-53, FS-58, FS-289, FS-290. Seed 1, 4,400 cases: 4,076 identical, 324 not comparable, 3 `MRIconform`
resamplings hashed as `HEAVY`, hash `57e8624ef07b6982203b2398d1e26004c15d18d0836be0363340247b7e170574`. `check.py`'s
`SKIP` adds FS-498, FS-499, FS-501, FS-502.

Measured 2026-11-14 (part 16: relay 25, 246 commands; logs `oracles/logs/fs-found-c-r25-*`): transform.cpp's LTA
operations `ltafill` (`LTAfillInverse`), `ltainv` (`LTAinvert`: new, given and in place), `ltachg` (`LTAchangeType`:
every pair of types, `FSLOUTPUTTYPE` unset or one of four, `Gdiag_no`), `ltacat` (`LTAconcat`, 0 to 3 arrays, chained,
inverted and mismatched, with and without `Reduce`), `ltared` (`LTAreduce`), `ltasrc` (`LTAmriIsSource` /
`LTAmriIsTarget`), `ltaprt` (`LTAprint`); arrays from `get_lta_full` (every transform's centre, spread, type, label,
matrix, geometries and optionally its inverse) along a pool of four scanner-like geometries per case, printed whole by
`put_lta_full`. Seeds 7 and 13, 8,400 cases each: 8,191 and 8,198 identical with `reproduce`, the rest not comparable
(FS-504, FS-508), 0 different. resample.cpp's `a2v` (`MRIaseg2vol`, with colour tables from `get_ctab`: 50 or 1,001
places, empty places, tissue types -2 to 100, tissue-type tables of 4, 101 or none; `setRandomSeed(1)` before
`CTABalloc`), `a2vmu` (`MRIaseg2volMU`, `USF` -1, 1, 2), `spmw` (`MRIapplySpmWarp`), `icode` (`interpolation_code`,
`float2int_code`): seeds 7 and 13, 4,800 cases each, 4,700 and 4,719 identical, 0 different. mri2.cpp's `vvtk`
(`MRIvol2VolTkRegVSM`, `vvsm`'s tokens), `sbound`, `sborder`, `idxno`, `ssrob`, `sfavg`, `zcat`, `aedit`, `shdim`: seed
7, 2,700 cases, 2,501 identical; seed 13, 5,400, 4,930 identical; 0 different. `fento`, `lacj` (given output), `lacjn`
(new output with `CTABalloc`'s 7,032 colours), `cfov`, `ccrs`, `crras` (`MRIcropAroundRAS`; `cras` is relay 19's):
seed 7, 2,400 cases, 2,332 identical; seed 13, 3,600, 3,455 identical; 0 different. `fix` different only with notices
everywhere. Seed 1, 4,920 cases: 4,552 identical, 368 not comparable, 22 hashed as `HEAVY` (`check.py`'s `heavy`: the
`MRIconform` resamplings and `lacjn`), hash `1acd334c6204657f39e004854539ea20a8405b3bc252092b59abcbc37450cc2e`.
`check.py`'s `SKIP` adds FS-504, FS-508, FS-509, FS-511, FS-515, FS-517.
Measured 2026-11-15 (part 17: relay 29, 257 commands; logs `oracles/logs/fs-found-c-r29-*`): mri2.cpp's `vvfill`
(`MRIvol2VolFill`: one-transform LTAs from or to the source, two transforms, neither end, voxel-to-voxel, RAS and
tkregister types, `FSLOUTPUTTYPE`, upsampling factors -1 to 3, `DoConserve` 0 to 2, `Gdiag_no` with `Gstdout` set as the
programs' `DiagInit` sets it, masks, given outputs), `fbnwm` (`MRIfindBrightNonWM`), `qecc` (`QuadEulerCharChange`, with
masks, centres outside and the debugging voxel `Gx Gy Gz`), `vtero` (`MRIvolTopoErodeOne`, keep masks), `vtdil`
(`MRIvolTopoDilateOne`), `fscm` (`FixSubCortMassHA::FixSCM`), `binmri` (`BinarizeMRI`: every field, a second call with
the frames the first left, `qualifies` at one voxel), `demb`, `demt`, `demm`, `demc` (`DEMorphBinVol`'s `morph` of a
binary volume, by thresholds and by a match list, `check`, `dump` and `copyFrame`). The volumes take a new token group,
`fill_box` (a box set to one value in every frame), between `fill_pat` and `fill_paint`. Seed 7, 11,000 cases: 10,110
identical with `reproduce`, 890 not comparable (the shared FS-53, FS-58, FS-290: frames and voxels past the volumes), 0
different; seed 13, 11,000 cases: 10,111 identical, 889 not comparable, 0 different; `fix` different only with notices
(FS-802, FS-803, FS-805 to FS-808 among them); the port's debug and release builds agree on seed 13's 5,500 cases in
both modes. Seed 1, 5,140 cases: 4,765 identical, 375 not comparable, 26 hashed as `HEAVY`, hash
`d18ca1b6afbd2c9bedc741eaeda8e14266b41035990f1df7c0de987e3eb4c402`, passing with AArch64's NaN emulated in debug and
release (`oracles/logs/fs-found-c-r29-trap-volume.log`) and on Windows x64 (the cross-check tool). `check.py`'s `SKIP`
is unchanged.

Measured 2026-11-16 (part 18: relay 34, 275 commands; logs `oracles/logs/fs-found-c-r34-*`): transform.cpp's writers
`ltawr` (`LTAwriteEx`: arrays of 0 to 2 transforms of every type between the pool's geometries, names ending `.lta`,
`.LTA`, `.xfm`, `.XFM`, `.dat`, `.reg`, `.fslmat`, `.mat`, none, a `.` in a folder, folders that do not exist; `USER`,
`LOGNAME`, both unset or `USER` empty, `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` always set; `FSLOUTPUTTYPE`;
`Gdiag_no`; the return, `errno` and the file's bytes), `ltadvg` (`LTAdumpVolGeom`); the moves `ltaxf`
(`LTAtransformInterp`: volumes of up to 4 x 4 x 4 voxels and 2 frames, given destinations, 0 to 3 transforms of every
type, interpolation 0 to 3, `USE_AVERAGE305` unset, `true` or empty, `DIAG_VERBOSE`, `DIAG_HEARTBEAT`), `ltapt`
(`LTAtransformAtPoint`, given and new matrices), `ltacor` (`LTAvoxelTransformToCoronalRasTransform`), `fixmni`
(`FixMNITal`), `ltamod` (`LTAmodifySrcDstGeom`, `vg_isEqual_Threshold`), `devxfm` (`DevolveXFMWithSubjectsDir` in a
subjects folder the case builds beside the cases: `orig.mgz` (gzipped MGH), `orig.mgh`, a slice folder, a file that is
not gzipped; `talairach.xfm` or `reg.lta` texts, good and damaged; `SUBJECTS_DIR`, a given folder, none; a given `XFM`;
what a case that ended the child left is removed first); `angrm`, `affp2m`, `affm2p`, `ltarot`, `rmsmj` (rotations,
affine parameters, Jenkinson's RMS); voxlist.cpp's `vlcre`, `vlsort`, `vlstat`, `vlcopy`, `vladd`. `fsvolume` is linked
with `--wrap=atan2`: `affm2p` and `ltarot` take CORE-MATH's correctly rounded `atan2` (`oracles/core-math-preload`,
loaded at start), the project's decision for `reproduce` (`docs/findings/freesurfer-libm.md` §2); with glibc's, 2 of
1,000 `affm2p` cases (seed 13) differed in `p[4]`'s last bit. `ltawr` orients standard error first (`fwide`): glibc's
`perror` on a write-only standard error not yet oriented leaves `EINVAL` (track A's finding). Seed 7, 18,000 cases of
the new commands: 17,193 identical with `reproduce`, 807 not comparable (177 identical all the same), 0 different;
seed 13, 18,000 cases: 17,183 identical, 817 not comparable (208 identical), 0 different; `fix` different only with
notices (FS-888, FS-891, FS-506, FS-507, FS-12, FS-52, FS-164, GLIBC-07, FS-01); the port's debug and release builds
agree on seed 13's 9,000 cases in both modes. Seed 1, 5,500 cases: 5,115 identical, 385 not comparable, 23 hashed as
`HEAVY`, hash `e5554290c86893a1d7372b8cc0bce17444979537407874d10d3631134e5a7a8d`, passing with AArch64's NaN emulated in
debug and release (`oracles/logs/fs-found-c-r34-trap-volume.log`) and on Windows x64 (the cross-check tool).
`check.py`'s `SKIP` adds FS-886, FS-887, FS-889, FS-890 and fs.io's FS-167 (an MGH header cut short: memory contents).

Measured 2026-11-16 (part 19: relay 35, 279 commands; logs `oracles/logs/fs-found-c-r35-*`): transform.cpp's
`ltaixf` (`LTAinverseTransformInterp`: as `ltaxf`, with inverse transforms given or left as `LTAalloc`'s), `ltav2r`
(`LTAvoxelToRasXform`: 0 to 3 transforms of every type, with and without source and destination volumes), `ltar2v`
(`LTArasToVoxelXform` with `LTAsetVolGeom`: a destination volume or none), `ltacmp` (`LTAcompose`: a left matrix, a right
one, both or none; into a new array, another array of 0 to 2 transforms, or the source itself). `fsvolume.cpp` declares
`LTAinverseTransformInterp` itself (transform.h does not). Seed 7, 4,000 cases of the new commands: 3,739 identical with
`reproduce`, 261 not comparable (FS-504; FS-957: all 111 such `ltar2v` cases checked are `CRASH 11` in the original; the
shared voxel defects), 0 different; seed 13, 4,000 cases: 3,708 identical, 292 not comparable, 0 different; `fix`
different only with notices (FS-954, FS-956, FS-957 and the shared ones); the port's debug and release builds agree on
seed 13's 2,000 cases in both modes. Seed 1, 5,580 cases: 5,185 identical, 395 not comparable, 26 hashed as `HEAVY`,
hash `34c600cf7461870de2c46970677d0bdeff03f1932250cae9569ec1338a3d8a53`, passing with AArch64's NaN emulated in debug
(72,725 flips) and release (62,790) (`oracles/logs/fs-found-c-r35-trap-volume.log`) and on Windows x64 (the cross-check
tool). `check.py`'s `SKIP` adds FS-957. The binary before this part: `<scratch>/C/fsvolume.before-r35`.

Measured 2026-11-17 (relay 37: tensor volumes, FS-960; no new command, `fsvolume` unchanged): the generator draws tensor
volumes (type 6) among its rare types and as second volumes, and as the destinations of `invc`, `sam` and the moves; the
equalisations `geq` and `heq` take short volumes where they drew int or long ones (an int volume whose voxels keep the
fill formula's bytes spans about 2^31 intensities: a histogram of 14 GB on both sides; the first seed-7 run of every
command was killed by its 24 GiB box in the port's `run` there, the two cases found by running the cases in 6 GiB boxes
1,000 at a time: FreeSurfer's child 12 GB at its alarm, the port 14 GB in 204 s and more than 20 GiB). `check.py --dump
<file>` writes every case's lines. Seed 7, 27,900 cases (every command, logs `oracles/logs/fs-found-c-r37-all7c.log`):
25,912 identical with `reproduce`, 1,988 not comparable, 0 different (one `ltapt` line differed first: `MatrixAdd`'s NaN
order, `docs/findings/freesurfer-maths.md`, fixed); seed 13, 27,900: 25,867 identical, 2,033 not comparable, 0 different;
seeds 7 and 13, the 8,000 cases each of `invc`, `sam`, `trans`, `rot`, `rotx`, `roty`, `rotz`, `fpow`: 6,683 and 6,697
identical, 0 different. The lines where the port notes FS-960 and compares: seed 7 168 of those commands and 33 of every
command, seed 13 166 and 34, all identical with `reproduce`; with the rows stepped (the port before this part) 18, 9, 15
and 7 of them were. `fix` differs only with notices. Seed 1, 5,580 cases: 5,190 identical, 390 not comparable, 26 hashed as
`HEAVY`, hash `04bbf7500f1221f31acac11580a834c5fa87936f592c4670a29f9562400c69f2`.

Measured 2026-11-18 (relay 38: no new command, `fsvolume` unchanged; logs `oracles/logs/fs-found-c-r38-*`). `check.py
--cases <file>` runs a file of hand-written cases (`hand_cases.py`: `lta`, `dns`, `bvr`; no official hash) and
`hand_cases.py compare <dump>` compares a `--dump` with `CRASH 6` (the official child's abort) and `EXIT 134` (the
port's) as one. `lta_hand_cases.txt` (FS-307's bounds, track E's open-probe group `vrules4`): 18 `.xfm` texts whose
comment loop meets the end of the file, 15 identical with `reproduce` (`could not read row 1 ... (<the raw line read
last>)`: a `#` or blank line, white space, a NUL byte, the end of a long line), 3 FS-307 hangs (`CRASH 14`, the alarm:
a `%` line read last). `dns_hand_cases.txt` (FS-317, FS-961): 16 factor triples (0, -0, NaNs, -1, -3, -0.5, plus and
minus infinity, 1e-40, 1e-30) on two seeded `dnsn` cases without and two with a destination: 64 of 64 identical with
`reproduce` (the allocation's errors, an infinite factor's volume 0 voxels wide, the destination's mismatch); `fix`
refuses each, noted. `bvr_hand_cases.txt` (FS-314): 13 cache lines, 11 aborts (`std::bad_alloc` up to 15-byte words,
`std::length_error` from 16, an earlier long word included): 13 of 13 identical with `reproduce`. The generator draws the
downsampling factors -1, 0, -0, NaN, infinity, -3 and 1e-30 (1 in 30 factors), which shifts every later case. Seed 7,
27,900 cases (every command): 25,912 identical with `reproduce`, 1,988 not comparable (478 identical all the same), 0
different (a first run had one `ssrob` line different: `MRIsegStatsRobust`'s min and max keep a signalling NaN,
`docs/findings/freesurfer-volume.md`, fixed); seed 13, 7,000 cases of `ssrob`, `sstat`, `sfavg`, `dnsn`, `ltaread`,
`lrseg`, `bvr`: 5,777 identical, 1,223 not comparable, 0 different, and its 1,000 `bvr` lines (186 FS-314 aborts) 1,000
of 1,000 identical by `hand_cases.py compare`; `fix` differs only with notices. Seed 1, 5,580 cases: 5,145 identical, 435
not comparable, 24 hashed as `HEAVY`, hash `ae9084ceb1f8c1428a5499df49ca822bd1682966625bf5204174d5f816ec5d43`.

Measured 2026-11-18 (relay 39, part 20: fs.bspline, `utils/mriBSpline.cpp`; 286 commands; logs
`oracles/logs/fs-found-c-r39-*`). New commands: `bsp` (`MRItoBSpline` of a volume of the case, with no B-spline, an
allocated one of the case's extent, one without coefficients or one whose coefficients are a volume of the case; a
degree from 2 to 9, now and then 0, 1, 10 or -1; then `MRIfreeBSpline` once or twice), `bsps` (`MRIsampleBSpline` at
points inside, at the half-voxel edges, outside and special, frames in and out of range), `bspseq`
(`MRIsampleSeqBSpline`, frame ranges past the volume's now and then), `bsplx` (`MRIlinearTransformBSpline`,
`MRIapplyRASlinearTransformBSpline` on the pool's geometries), `bsplta` (`LTAtransformBSpline`, `USE_AVERAGE305`),
`bspdn`, `bspup` (`MRIdownsample2BSpline`, `MRIupsample2BSpline`, with and without a destination). The sources have
one axis of up to 90 voxels now and then (past the causal start's horizons, 21 to 73 samples) and axes of one voxel
often; a quarter of the paints are NaNs of four payloads (a signalling one among them) and infinities. A degree
written over a computed B-spline (`bsps`, `bspseq`, `bsplx`) reaches FS-964. `lxf`, `rsfill`, `vv`, `vvsm`, `vvtk`,
`talvol`, `ltaxf` and `ltaixf` now draw `SAMPLE_CUBIC_BSPLINE` (5): the port's error there is gone (fs.bspline). Seed 7,
700 cases of the new commands: 644 identical with `reproduce`, 56 not comparable, 0 different; seed 13, 1,400: 1,278
identical, 122 not comparable, 0 different; seed 7, 1,500 cases of the new commands and the eight callers (105 of the
callers' cases through `MRItoBSpline`): 1,400 identical, 100 not comparable, 0 different; `fix` different only with
notices. `check.py`'s `SKIP` adds FS-964 and FS-965 (relay 39 also met FS-965: an `hlab` case of the new seed-1 stream
made the port's `fix` run grow past its 24 GiB box, `docs/findings/freesurfer-volume.md` §31). Seed 1, 5,720 cases:
5,284 identical, 436 not comparable, 24 hashed as `HEAVY`, hash
`a2c71444db657bf3d3d59048d6910a4301c20fe27581b8867b44d3bd2f989409`. Seed 7, 28,600 cases (every command, the final code):
26,580 identical, 2,020 not comparable (524 identical all the same), 0 different. The speed and memory against
FreeSurfer's own code:
`validation/probes/fs-bspline-bench/`.

Measured 2026-11-22 (relay 43: the label sums' operand order and FS-471's negative limits; two commands
`lmeansp` and `lnbhdp`, `lmeans` and `lnbhd` with the source painted, used only by hand cases: the seeded
cases are unchanged). `nan_hand_cases.txt` (`hand_cases.py nan`): the five label means and
`MRIcomputeLabelNbhd` of 2 x 2 x 1 float volumes holding two to four NaN payloads, infinities of both signs and
a signalling NaN: 72 of 72 identical with `reproduce`, `fix` the same (FreeSurfer keeps the last NaN value met;
relays 19 and 20 kept the first). `pv471_hand_cases.txt` (`hand_cases.py pv471`, track E's open group
`vrules13` in this probe's form): 8 of 15 identical (limits -1 to -3 with every counted voxel off the border,
and a volume without the label), 7 FreeSurfer's crashes (`SKIP`, FS-471). Both files' official hashes are
`tests/fs_volume_official.rs`'s `hand_cases_give_the_official_outputs`. With the probe rebuilt (`fsvolume` from
relay 43; the earlier binary in `<workspace>/archive/2026-11-22-fsvolume-closed-probe-before-r43/`): seed 1,
5,720 cases: hash `a2c71444...` unchanged (436 `SKIP`, 24 `HEAVY`); seed 7, the 609 `lmeans`, `lnbhd` and `pvlab`
cases of 58,000: 540 identical, 69 not comparable, 0 different (logs `oracles/logs/fs-found-c-r43-*`).

Measured 2026-11-23 (relay 45 of track C: track E's R16.5, FS-268 in `MRIcropAroundRAS`, FS-515's
`reproduce`, FS-269 decided in `REGIONgetBoundingBox(M)`, FS-270 in `MRIaseg2volMU`; `check.py`'s `SKIP`
gains FS-270): `--only crras,ccrs,rgbb,maskup,a2vmu`, seeds 7 and 13 (500 cases each of 28,600; log
`oracles/logs/fs-found-c-r45-volume-crops.log`): `reproduce` 0 different (457 and 470 identical, 43
and 30 not comparable); `fix` different only with notices (FS-268 15 and 9 times, FS-269 105 and 112).
Seed 1's 5,720 cases: hash `a2c71444...` unchanged (436 `SKIP`, 24 `HEAVY`). FS-515's `reproduce` lines
stay `SKIP`: this probe's `fsvolume` children see other stack contents than 0 in the unset field of view
(1693078050 among them, and allocation failures).
