# fs.volume's clean room against FreeSurfer 8.2.0, as a black box (parts 1 to 6c)

larmorx's clean room of FreeSurfer's volume functions (`crates/larmorx-fs-core/src/volume/`, written
from [`specs/freesurfer-volume.md`](../../../specs/freesurfer-volume.md) §1 to §26 alone) compared
with the prebuilt probe of the open folder [`../freesurfer-volume-probe/`](../freesurfer-volume-probe/README.md)
(FreeSurfer 8.2.0's library as the official build compiles it, with the fs.maths probe's guard
allocator). The probe is only run, never built.

| Path | What it is |
|---|---|
| `candidate/` | `fs-volume-candidate batch <file>`: the probe's commands and print forms over the clean room (its own Cargo workspace, a validation tool outside larmorx's build); each line on a process state of its own, `LARMORX_UPSTREAM_DEFECTS` the mode, the defect notices of a line on its standard error |
| `speed/` | the timing lines (`mk.py` writes them, `run.sh <probe> <candidate>` times them in a memory box and checks that the result lines agree) |
| `../../../crates/larmorx-fs-core/tests/volume_cmd/` | the commands themselves (the probe's grammar and print forms; the volume, geometry, matrix, colour-table and transform encodings are fs.mri's, `tests/mri_cmd/args.rs`, included by path), shared with the crate's golden cases (`tests/golden_volume.rs`); the candidate includes the folder by path |

## Running it

```sh
(cd candidate && cargo build --release)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 12 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe \
  --candidate <target>/release/fs-volume-candidate --scratch <scratch>/volume --mode reproduce   # then fix
```

The golden cases: `cargo test -p larmorx-fs-core --test golden_volume` (record with
`FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`); under the AArch64 NaN emulation, the test binary
(`target/<profile>/deps/golden_volume-*`) with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
NAN_TRAP_DIR=<scratch>/nt`, the library built from `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
as that folder's README says. The defects' own tests: `cargo test -p larmorx-fs-core --test defects_volume`.

## Results (2026-11-18, relay 11, seed 7, every group of §69.2; the probe's reference hashes are the spec's, the probe of commit `20578b55`)

`compare.py` per group: calls, identical, FreeSurfer did not finish, memory, defect corrected,
unexplained.

| Group | Calls | reproduce | fix |
|---|---|---|---|
| vgblock | 560 | 560 / 0 / 0 / 0 / **0** | 495 / 0 / 0 / 65 / **0** |
| vgcopy | 1,080 | 1,080 / 0 / 0 / 0 / **0** | 1,080 / 0 / 0 / 0 / **0** |
| vmake | 1,050 | 890 / 135 / 25 / 0 / **0** | 497 / 135 / 110 / 308 / **0** |
| vrange | 800 | 685 / 115 / 0 / 0 / **0** | 496 / 115 / 3 / 186 / **0** |
| vscale | 1,100 | 918 / 178 / 4 / 0 / **0** | 506 / 178 / 114 / 302 / **0** |
| vtwo | 200 | 164 / 32 / 4 / 0 / **0** | 98 / 32 / 6 / 64 / **0** |
| vextract | 600 | 501 / 43 / 56 / 0 / **0** | 144 / 43 / 131 / 282 / **0** |
| vcoords | 1,350 | 1,350 / 0 / 0 / 0 / **0** | 1,178 / 0 / 0 / 172 / **0** |
| vstats | 750 | 711 / 39 / 0 / 0 / **0** | 475 / 39 / 12 / 224 / **0** |
| vheader | 606 | 606 / 0 / 0 / 0 / **0** | 498 / 0 / 0 / 108 / **0** |
| vrules | 84 | 73 / 11 / 0 / 0 / **0** | 34 / 11 / 0 / 39 / **0** |
| vsample | 540 | 514 / 19 / 0 / 7 / **0** | 444 / 19 / 0 / 77 / **0** |
| vmove | 300 | 235 / 65 / 0 / 0 / **0** | 122 / 65 / 3 / 110 / **0** |
| vskull | 160 | 160 / 0 / 0 / 0 / **0** | 80 / 0 / 0 / 80 / **0** |
| vltaread | 309 | 299 / 10 / 0 / 0 / **0** | 212 / 10 / 0 / 87 / **0** |
| vrules4 | 94 | 94 / 0 / 0 / 0 / **0** | 42 / 0 / 0 / 52 / **0** |
| vcma | 377 | 354 / 13 / 10 / 0 / **0** | 296 / 13 / 10 / 58 / **0** |
| vrules5 | 46 | 27 / 7 / 12 / 0 / **0** | 25 / 7 / 12 / 2 / **0** |
| vres | 830 | 716 / 77 / 37 / 0 / **0** | 498 / 77 / 98 / 157 / **0** |
| vrules6 | 16 | 15 / 1 / 0 / 0 / **0** | 13 / 1 / 0 / 2 / **0** |
| vframes | 404 | 401 / 3 / 0 / 0 / **0** | 371 / 3 / 0 / 30 / **0** |
| vderiv | 382 | 358 / 22 / 2 / 0 / **0** | 342 / 22 / 4 / 14 / **0** |
| vlinear | 360 | 346 / 12 / 1 / 0 / **1** | 217 / 12 / 4 / 127 / **0** |
| vresamp | 272 | 253 / 17 / 1 / 0 / **1** | 158 / 17 / 1 / 96 / **0** |
| all | 12,270 | 11,310 / 799 / 152 / 7 / **2** | 8,321 / 799 / 508 / 2,642 / **0** |

Totals: `reproduce` 11,310 of 12,270 identical (799 where FreeSurfer did not finish, 9 of them
FS-314's aborts that the candidate gives too, which §69.3 checks; 152 memory lines; 7 `vsample`
lines where the candidate noted FS-300, which §69.3 allows in both modes) and 2 lines where
FreeSurfer uses a value it never wrote that is the same in the probe's four runs by accident: the
`vlinear` line of a sinc transform into an integer destination (FS-325: FreeSurfer's -1, the spec's
0) and the `vresamp` line of a cubic resampling (FS-300's window points: one voxel 0x7f against
0x8d), both noted by the candidate and reported to the spec writer (§69.3 allows FS-300 only in
`vsample`); `fix` 8,321 identical (799 not finished, 508 memory, 2,642 corrected by a noted defect),
none unexplained. Relay 11 followed the spec's answers to relay 10's items (`MRIchangeType`'s low
end up to all 1000 bins; `MRIfillFG`'s box clamped to the extent on every axis, a seed past the depth
writing into a later frame: `vrules6`) and parts 6b (§27 to §29: `vframes`, `vderiv`, `vlinear`) and
6c (§31: `vresamp`); `MRIindexNotInVolume` now rounds the double (findings), with which every earlier
line is as before.

- **Where FreeSurfer did not finish** (crashes: header copies from a volume without its cached
  matrix, FS-252; volumes without voxels walked directly, FS-58; smaller second volumes and frames
  past the volume's, FS-290; rows of a negative length, FS-294; destinations of another type
  FreeSurfer frees, FS-293; the principal components of covariances with infinite or NaN entries,
  FS-47 and ITK-112; rotations about an axis without an inverse, FS-304; names of 500 bytes, FS-306;
  `.lta` files of one line, FS-309; `.xfm` files whose `src` or `dst` word is last, FS-310; the
  `.xfm` files FreeSurfer never finishes, FS-307; the statistics cache's aborts, FS-314): the
  candidate finishes, with the spec's refusal, result or end of the program.
- **FS-315's lists of 0 columns**: most such lines of `vcma` and `vrules5` are memory lines
  (FreeSurfer reads past the lists); the others read the same in all four probe runs (`nswaps 0`).
  The candidate's `reproduce` reads the numbers past a list's end as NaN, equal to no label (§23),
  which gives those lines; the default ends the program (code -4) before any voxel is written.
- **`ReadCachedBrainVolumeStats` without a cache** calls the caller's `ComputeBrainVolumeStats`
  (not in the spec: it reads a subject's surfaces). The candidate's stand-in prints what the
  probe's run shows there without a subject's files and without `FREESURFER_HOME` (two lines,
  status 255): it checks only that the reader announces and calls it at that point.
- Behaviours measured beyond the spec's text: the findings, `docs/findings/fs-volume-clean-room.md`.

## Golden cases, defect tests, speed

- `cargo test -p larmorx-fs-core --test golden_volume`: every command of the probe's grammar on
  seeded arguments (volumes of at most 3 x 3 x 3 voxels and 2 frames, every type, header-only ones,
  special values; the head's box on volumes of up to 24 voxels a side; the transform reader on
  in-memory files, `ltamem` and `ltamems`, a named volume that exists included, verbose
  diagnostics' INFO lines; the CMA labels and segmentation functions on painted segmentations, the
  statistics cache's text and reading in memory, `bvtext` and `bvparse`; part 6a's 11 commands,
  the group `res`; part 6b's 11 commands and 6c's 2, the groups `frames`, `deriv`, `linear` and
  `regrid`, with the diagnostic bits' prints of `rsmat` and `rsfill`), both modes, 40 SHA-256 over
  20 groups (0.05 s in a debug build). Identical in debug and release builds and under the AArch64
  NaN emulation of both.
- `cargo test -p larmorx-fs-core --test defects_volume`: each defect of the spec's §69.4 (FS-265,
  FS-285 to FS-318) and FS-332, FS-258 noted in both modes on a line of the comparison's own batches
  (the shortest), FS-292, FS-296, FS-297, FS-302, FS-305, FS-311 to FS-315 and FS-258 on lines
  written for them, the two modes' results different where the correction changes them (the ends
  of the program that §22's table gives in both modes differ only by FS-01's NUL); FS-316 (a seed
  row past the height), FS-317 (a factor of 0 under `reproduce`; the conversion's default in
  `src/volume/resample.rs`'s unit test, as the default would ask for a volume 2^31 - 1 voxels wide)
  and FS-318 on lines written for them; FS-320, FS-321, FS-322, FS-325, FS-327 and FS-328 on lines
  of the batches; FS-323 (an index 600 voxels away) and FS-324 (scales of +infinity and 1e308, the
  messages checked), FS-326 (280 and 320 votes in one voxel), FS-327 (a vote onto a template of
  unknown orientation) and FS-461 (a two-frame byte volume) written for them; FS-319 is not reachable: `MRIupsampleNConserve` passes
  fractions of 0, with which §25's histogram range is never empty (findings); FS-960 under
  `reproduce` only (by default no function of parts 1 to 5 walks a tensor volume directly: FS-291's
  and FS-301's corrections avoid those walks); also FS-12, FS-47, ITK-112, FS-52, FS-53 and FS-58.
- Speed (part 4; the spec's plan names none): one batch line each through the probe and the
  candidate, end to end, best of 3: 20,000 points on a 32^3 float volume, trilinear 1.3x faster than
  FreeSurfer, cubic 1.1x, nearest 1.5x, windowed sinc 0.72x (the main package's `sin` and `cos`
  ports); the head's box of a 64^3 volume 1.4x. Part 6a (§26 names none either; `speed/`, 64^3
  volumes): the flood fill 2.3x (all voxels) and 1.6x (a threshold), `MRIdownsampleN` by 2 1.2x,
  `MRIdownsample2` 0.87x, `MRIupsampleN` of 32^3 by 2 1.4x, `MRIchangeType` with its histogram
  (48^3) 1.2x, `MRIlimits` 1.1x. Parts 6b and 6c (relay 11; §30 and §32 name none): 20,000 points of
  a 32^3 float volume, `MRIsampleSeqVolume` 1.2x, splatting 1.4x, the directional derivative 1.05x;
  the linear transform of a 32^3 volume by a rotation 1.03x (trilinear, floats) and 1.14x (nearest,
  bytes); `MRIresampleFill` onto a 40^3 template 0.98x (trilinear, floats) and 1.09x (nearest,
  bytes); every line identical (relay 11's run of the whole kit). The numbers in the findings. Part
  5's functions (label tests and walks over a segmentation) were not timed: §24 names no speed
  check.
