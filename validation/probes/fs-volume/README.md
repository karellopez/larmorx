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
agree on 11,000 cases of the new commands in both modes. `check.py`'s `SKIP` adds FS-482, FS-484, FS-485, FS-486.
`pcast` calls `fflush(NULL)` after `WritePCAStats` (the file it leaves open reaches the disk at the program's end).

Measured 2026-11-14 (part 12, relay 22: 3.1-38's `talairachex.cpp`: `talcras`, `talmat`, `talpt`, `talvol`; 166
commands; logs in `<scratch>/C/r22/`): seed 7, 2,000 cases: 1,941 identical with `reproduce`, 59 not comparable
(FS-487, FS-488, FS-53, FS-325; 35 of them identical), 0 different; seed 13, 4,000 cases: 3,859 identical, 141 not
comparable (84 identical), 0 different; `fix` different only with notices. The port's debug and release builds agree
on seed 13's 4,000 cases in both modes. Seed 1, 3,320 cases: 3,083 identical, 237 not comparable, hash
`c3cfc45ecb3faa4cac3084f1f8b08334eab226e31b8e8627585fdd19774571c9`, passing with AArch64's NaN emulated in debug and
release (`oracles/logs/fs-found-c-r22-trap-volume2.log`). `check.py`'s `SKIP` adds FS-487, FS-488. `talvol` leaves
out `SAMPLE_CUBIC_BSPLINE` (fs.bspline is not ported yet). Relay 22 also measured `fdr` with `log10` now
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
