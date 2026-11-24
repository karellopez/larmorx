# fs.volume's and fs.bspline's clean rooms against FreeSurfer 8.2.0, as a black box (parts 1 to 10, §54 to §57, fs.bspline)

larmorx's clean rooms of FreeSurfer's volume functions (`crates/larmorx-fs-core/src/volume/`, written
from [`specs/freesurfer-volume.md`](../../../specs/freesurfer-volume.md) §1 to §53 and §54 to §58 alone) and of its
B-splines (`crates/larmorx-fs-core/src/bspline/`, from
[`specs/freesurfer-bspline.md`](../../../specs/freesurfer-bspline.md) alone) compared
with the prebuilt probe of the open folder [`../freesurfer-volume-probe/`](../freesurfer-volume-probe/README.md)
(FreeSurfer 8.2.0's library as the official build compiles it, with the fs.maths probe's guard
allocator). The probe is only run, never built.

| Path | What it is |
|---|---|
| `candidate/` | `fs-volume-candidate batch <file>`: the probe's commands and print forms over the clean room (its own Cargo workspace, a validation tool outside larmorx's build); each line on a process state of its own, `LARMORX_UPSTREAM_DEFECTS` the mode, the defect notices of a line on its standard error |
| `hand/` | hand cases that settled a reading the groups do not show, run on the probe and the candidate alike (its README) |
| `speed/` | the timing lines (`mk.py` writes them, `run.sh <probe> <candidate>` times them in a memory box and checks that the result lines agree) |
| `../../../crates/larmorx-fs-core/tests/volume_cmd/` | the commands themselves (the probe's grammar and print forms; the volume, geometry, matrix, colour-table and transform encodings are fs.mri's, `tests/mri_cmd/args.rs`, included by path), shared with the crate's golden cases (`tests/golden_volume.rs`); the candidate includes the folder by path |

## Running it

```sh
(cd candidate && cargo build --release)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 16 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe \
  --candidate <target>/release/fs-volume-candidate --scratch <scratch>/volume --mode reproduce   # then fix
```

The golden cases: `cargo test -p larmorx-fs-core --test golden_volume` (record with
`FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`); under the AArch64 NaN emulation, the test binary
(`target/<profile>/deps/golden_volume-*`) with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
NAN_TRAP_DIR=<scratch>/nt`, the library built from `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
as that folder's README says. The defects' own tests: `cargo test -p larmorx-fs-core --test defects_volume`.

## Results (2026-11-24, relay 21, seed 7, every group of the volume spec's §69.2 and the B-spline spec's §9; the probe's reference hashes are the specs', the probe of commit `fa3837bc`)

`compare.py` per group: calls, identical, FreeSurfer did not finish, memory, defect corrected,
unexplained (log `<workspace>/oracles/logs/fs-cleanroom-2-relay21-chain-a.log`).

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
| vlinear | 360 | 346 / 12 / 1 / 1 / **0** | 217 / 12 / 4 / 127 / **0** |
| vresamp | 272 | 253 / 17 / 1 / 1 / **0** | 158 / 17 / 1 / 96 / **0** |
| vrules7 | 12 | 12 / 0 / 0 / 0 / **0** | 10 / 0 / 0 / 2 / **0** |
| vbspline | 410 | 377 / 24 / 9 / 0 / **0** | 318 / 24 / 10 / 58 / **0** |
| vbsrules | 71 | 68 / 0 / 3 / 0 / **0** | 64 / 0 / 3 / 4 / **0** |
| vbsvol | 70 | 66 / 4 / 0 / 0 / **0** | 29 / 4 / 1 / 36 / **0** |
| vstat7 | 481 | 474 / 0 / 7 / 0 / **0** | 432 / 0 / 7 / 42 / **0** |
| vrules8 | 32 | 29 / 0 / 3 / 0 / **0** | 27 / 0 / 3 / 2 / **0** |
| vimage | 290 | 274 / 15 / 1 / 0 / **0** | 133 / 15 / 1 / 141 / **0** |
| vnbhd | 441 | 428 / 10 / 3 / 0 / **0** | 379 / 10 / 5 / 47 / **0** |
| vltapv | 135 | 134 / 1 / 0 / 0 / **0** | 52 / 1 / 0 / 82 / **0** |
| vrules9 | 34 | 30 / 1 / 3 / 0 / **0** | 18 / 1 / 4 / 11 / **0** |
| vrules10 | 65 | 65 / 0 / 0 / 0 / **0** | 27 / 0 / 0 / 38 / **0** |
| vrules11 | 28 | 28 / 0 / 0 / 0 / **0** | 28 / 0 / 0 / 0 / **0** |
| vpvlab | 90 | 83 / 1 / 3 / 3 / **0** | 83 / 1 / 3 / 3 / **0** |
| vfr8 | 490 | 482 / 7 / 1 / 0 / **0** | 395 / 7 / 10 / 78 / **0** |
| vmri2 | 390 | 384 / 3 / 3 / 0 / **0** | 321 / 3 / 4 / 62 / **0** |
| vrules12 | 112 | 105 / 6 / 1 / 0 / **0** | 80 / 6 / 2 / 24 / **0** |
| vrules13 | 13 | 6 / 7 / 0 / 0 / **0** | 6 / 7 / 0 / 0 / **0** |
| vlta54 | 225 | 217 / 1 / 7 / 0 / **0** | 121 / 1 / 8 / 95 / **0** |
| vrules14 | 109 | 96 / 3 / 10 / 0 / **0** | 51 / 3 / 10 / 45 / **0** |
| vaseg | 219 | 219 / 0 / 0 / 0 / **0** | 216 / 0 / 0 / 3 / **0** |
| vsegs | 270 | 270 / 0 / 0 / 0 / **0** | 243 / 0 / 0 / 27 / **0** |
| vcrops | 95 | 95 / 0 / 0 / 0 / **0** | 77 / 0 / 0 / 18 / **0** |
| vrules15 | 70 | 58 / 9 / 1 / 2 / **0** | 37 / 9 / 2 / 22 / **0** |
| vrules16 | 68 | 50 / 5 / 0 / 13 / **0** | 29 / 5 / 0 / 34 / **0** |
| vv2v | 415 | 407 / 8 / 0 / 0 / **0** | 357 / 8 / 1 / 49 / **0** |
| vregion | 160 | 160 / 0 / 0 / 0 / **0** | 108 / 0 / 0 / 52 / **0** |
| vrules17 | 45 | 44 / 1 / 0 / 0 / **0** | 31 / 1 / 0 / 13 / **0** |
| vrules18 | 29 | 29 / 0 / 0 / 0 / **0** | 10 / 0 / 0 / 19 / **0** |
| vcov | 410 | 405 / 5 / 0 / 0 / **0** | 330 / 5 / 3 / 72 / **0** |
| vvoxel | 420 | 396 / 7 / 17 / 0 / **0** | 311 / 7 / 19 / 83 / **0** |
| vrules19 | 49 | 46 / 1 / 2 / 0 / **0** | 40 / 1 / 2 / 6 / **0** |
| vtal | 240 | 240 / 0 / 0 / 0 / **0** | 190 / 0 / 1 / 49 / **0** |
| vdil | 270 | 270 / 0 / 0 / 0 / **0** | 266 / 0 / 0 / 4 / **0** |
| vrules20 | 111 | 108 / 3 / 0 / 0 / **0** | 76 / 3 / 1 / 31 / **0** |
| vrules21 | 71 | 65 / 6 / 0 / 0 / **0** | 56 / 6 / 0 / 9 / **0** |
| vconf | 71 | 71 / 0 / 0 / 0 / **0** | 46 / 0 / 0 / 25 / **0** |
| vset | 500 | 494 / 3 / 3 / 0 / **0** | 452 / 3 / 5 / 40 / **0** |
| vmorph | 340 | 340 / 0 / 0 / 0 / **0** | 326 / 0 / 0 / 14 / **0** |
| vmask | 110 | 108 / 2 / 0 / 0 / **0** | 95 / 2 / 0 / 13 / **0** |
| vrules22 | 134 | 121 / 11 / 1 / 1 / **0** | 97 / 11 / 3 / 23 / **0** |
| vhist | 605 | 582 / 23 / 0 / 0 / **0** | 397 / 23 / 4 / 181 / **0** |
| vrules23 | 130 | 117 / 13 / 0 / 0 / **0** | 82 / 13 / 1 / 34 / **0** |
| all | 20,600 | 19,363 / 979 / 230 / 28 / **0** | 14,767 / 979 / 621 / 4,233 / **0** |

Relay 21 (66 groups, 20,600 calls): `reproduce` 19,363 identical, 979 where FreeSurfer did not finish,
230 memory lines, 28 lines §69.3 allows (the 27 before and `vrules22`'s destination too small, now named
by the loop's `both_lines`), **none unexplained**; `fix` 14,767 identical (979 not finished, 621 memory,
4,233 corrected by a noted defect), none unexplained. Relay 21 followed the spec writer's answers to relay
20's items 119 to 123 (no code change: the empty box, the conform hash's start state and §50's readings
were the candidate's already; `vmorph`, `vmask`, `vrules21`, `vrules22` and `vrules23`'s first 34 calls
identical with `reproduce`), then part 10c (§52, §53: `vhist` and `vrules23`, identical with `reproduce` in
every line but where FreeSurfer crashed or hung: FS-498's NaN bins, FS-499's `free()` abort and a given
result too small, FS-501's ints and infinity, FS-502's limit -2 after the alarm, a region missing). Of the
earlier groups only `vres`, `vfr8`, `vlta54` and `vrules14` moved, between "identical", "memory" and "did
not finish" (the probe's own runs). `models.py region` and `cov` with the candidate: none different. fs.mri
(16 groups, 9,511 calls): none unexplained in either mode.

Relay 20 (64 groups, 19,865 calls): `reproduce` 18,663 identical, 947 where FreeSurfer did not finish,
227 memory lines, the same 27 lines §69.3 allows, and **one unexplained**: `vrules22`'s
`MRIdilateThreshLabel` copying an 8-voxel source into a 4-voxel destination, where `MRIcopy` ends the
program in both modes (fs.mri §13's "the destination is too small") and FreeSurfer writes past the
destination the same way in each of the probe's four runs, so the loop cannot see it as a memory line
(a question for the spec writer); `fix` 14,287 identical (947 not finished, 613 memory, 4,018
corrected by a noted defect), none unexplained. Relay 20 followed the spec writer's answers to relay
18's items 107 to 111 (`MRIdilate`'s neighbours clamped to the volume, its search frames outermost;
`vtal`, `vdil`, `vrules20` and the new fixed group `vrules21` identical with `reproduce` but its six
FS-252 crash lines) and found, with hand cases (`hand/`), that both dilations' box is not grown when
the search finds nothing; then part 10b (§49 to §51: `vconf`, `vset`, `vmorph`, `vmask`, `vrules22`).
Of the earlier groups only `vfr8` and `vrules14` moved, between "identical", "memory" and "did not
finish" (the rebuilt probe's runs classify its memory lines anew). `models.py region` and `cov` with
the candidate: none different. fs.mri (16 groups, 9,511 calls): none unexplained in either mode.

Relay 18's totals: `reproduce` 17,467 of 18,639 identical (924 where FreeSurfer did not finish, 9 of them
FS-314's aborts that the candidate gives too, which §69.3 checks; 221 memory lines; 27 lines where the
candidate noted a defect whose value in FreeSurfer is memory, or whose end the loop allows in both
modes, as §69.3 says: FS-300 in `vsample` and `vresamp`, FS-325 in `vlinear`, FS-469 in 3 `vpvlab`
lines, a border voxel's own label outside the label table, whose mean FreeSurfer reads beside the
table, FS-509 in `vrules15`, a tissue type of 100 counted past FreeSurfer's array, and FS-517 in
`vrules15`'s crop around a voxel whose last column is exactly `int`'s maximum and 13 `vrules16`
lines), none unexplained; `fix` 13,216 identical (924 not finished, 605 memory, 3,894 corrected by a
noted defect), none unexplained. Relay 18 followed the spec writer's answers to relay 17's items (§41:
`MRIvol2VolTLKernel`'s work volume made before `M`, `MRIvol2VolDelta`'s and `TransformRegDat2LTA`'s
orders, `MRImaskAndUpsample`'s quotient over the source's extent in single precision; §43's room after
the box; §56: `MRIvol2VolTkRegVSM`'s `Tmov`, its inverse, then `Ttarg`; `vrules20`'s fixed cases), then
part 10a (§46 to §48: `vtal`, `vdil`, `vrules20`, identical in every line with `reproduce` but where
FreeSurfer crashed: FS-487's points without a matrix and FS-488's missing target). Of the earlier
groups, a few lines moved between "did not finish", "memory" and "identical" (`vextract`, `vres`,
`vderiv`, `vpvlab`, `vlta54`, `vrules14`, `vcov`): the rebuilt probe's four runs classify its memory
lines anew (the reference hashes are unchanged); none of them is unexplained. The open probe's
`models.py region --candidate` (185 `rgbb` lines, `vrules20`'s included) and `models.py cov
--candidate` (111 `cov` lines) agree with the candidate in both modes, `fix` against FS-269's and
FS-483's decided values. fs.mri (16 groups, 9,511 calls) was run again: none unexplained in either
mode. fs.bspline's own findings: `docs/findings/fs-bspline-clean-room.md`.
`nan-order/mk.py` holds the black-box check of §35's label sums' NaN order (its docstring says how to
run it and what it showed).

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
  `regrid`, with the diagnostic bits' prints of `rsmat` and `rsfill`; part 7a's 11 commands, the
  group `info`; fs.bspline's 10, the group `bspline`, its transform arrays on clean geometries too;
  the cubic B-spline paths, the group `bsvol`; part 7b's 16 commands, the groups `image` and `nbhd`;
  §54's changes of type, the groups `ltapv` and `ltatypes`; part 8's commands, the groups `fr8` and
  `vox8`; §54's other operations, the group `lta54`; §55 to §57's 18 commands, the group `segs57`;
  part 9's 16 commands with §55's upsampling and §56's shift-map resampling, the group `part9`;
  part 10a's 9 commands, with `NO_AVERAGE305` and the diagnostic bits, the group `part10`;
  part 10b's 34 commands (conforming's matrix and template: `MRIconform`'s 256^3 result is beyond
  the golden size, `vconf` holds it), with `FS_MRIMASK_ALLOW_DIFF_GEOM` and a diagnostic bit, the
  group `part10b`; part 10c's 9 commands (the histograms, with given histograms, regions outside,
  NaN and infinite floats, the diagnostic and heartbeat bits of `MRIhistoSegment`), the group
  `part10c`), both modes, 70 SHA-256 over 35 groups. Identical in debug and release builds and
  under the AArch64 NaN emulation of both (every test of the crate: 228 in each of the four
  configurations, relay 21).
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
  and FS-301's corrections avoid those walks); part 7a's FS-329 (a 64-bit volume's statistics),
  FS-330 (an RGB volume's printed statistics), FS-462 (a sampled extent of 0), FS-463 (a string of
  one character) and FS-464 (a 2 x 2 matrix); fs.bspline's FS-962 (an axis of one voxel doubled),
  FS-963 (halved) and FS-964 (degrees 10 and -1 on one voxel, 10 on 3 x 3 x 3: its two ends); part
  7b's FS-465 (an image too small for `MRItoImage`), FS-466 (an RGB image of two frames), FS-467
  (negative labels in the smoothing) and FS-468 (T1 values outside the byte range), §54's FS-505 and
  FS-506 (a `vltapv` line), `MRIcomputeLabelNbhd` with values and no means; part 8's FS-469 to FS-481
  (FS-470's and FS-471's errors, FS-471's limits -1 to -3 off the border, FS-479's error, fewer powers
  than frames); §54's FS-504 (no arrays, arrays without transforms, a 3 x 4 matrix printed, the
  diagnostic print of an empty array), FS-507 (REGISTER_DAT to RAS-to-RAS of two transforms), FS-508
  (a registration that cannot be inverted changed to FSL, and its place: the first read after the
  later transforms' conversions), the unsupported targets' messages, the diagnostic print and a NaN
  element printed; FS-266 (fewer powers than frames); §55 to §57's FS-267 (a tissue table of no
  places, also after an earlier vote in the same process: the candidate's own command `a2vtwice`),
  FS-509 (a label past the table, an empty place, a type of 100), FS-510, FS-512, FS-513, FS-514,
  FS-515, FS-516 and FS-517 on `vrules15`'s lines; FS-268 (an `SRA` crop, and `RAS` where nothing is
  noted), FS-515 under `reproduce` (`RRS`, the later axis's write kept), FS-517 at `int`'s maximum and
  FS-511 (tensor and RGB volumes) on `vrules16`'s lines; part 9's FS-269 (a box, a pad of 1, the even
  and odd box, the mask-and-upsample cut), FS-482, FS-483, FS-484 and FS-323 in §44 on `vrules17` to
  `vrules19`'s lines, FS-270 on a `vv2v` line, and FS-485 (not in the probe: the default's full window
  checked against an independent computation of §44's text on an exact `float` volume); part 10a's
  FS-487 (an array of no transform through `MvoxelFromTalairach`, and points through a 3 x 3 transform
  matrix), FS-488 (`MRIfromTalairachEx` without a target), FS-489 (the segmentation as its own mask,
  with and without 0 in the list), FS-490 (a histogram of no bins) and FS-12 in `HISTOseg` (a NaN width,
  NaN values) on `vrules20`'s lines, and `TransformWithMatrix`'s NaN operands against §46's text; part
  10b's FS-491 to FS-497 on `vrules22`'s lines; part 10c's FS-498 (a constant volume of 300 in one bin, a
  first window of one value), FS-499 (a low limit of -1, a given result too small), FS-500, FS-501 (ints,
  a later window's infinity) and FS-502 (a gray-matter limit of -2) on `vrules23`'s lines and FS-965 (a
  range of 100,000,001 bins, not probed) on a line written for it; FS-503 has no notice (one
  histogram's sums at any thread count); also FS-12, FS-47, ITK-112, FS-52, FS-53 and FS-58.
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
  bytes); every line identical (relay 11's run of the whole kit). fs.bspline and the cubic B-spline
  paths (relay 12, `mk.py bspline`, byte volumes): the coefficients of a 128^3 volume 1.2x (against
  the probe at one OpenMP thread), of 64^3 with 20,000 samples 1.4x, `MRIlinearTransformBSpline` 1.3x,
  halving a 64^3 volume 1.0x, doubling a 32^3 one 1.1x, the linear transform and `MRIresampleFill`
  with the cubic B-spline 1.2x each; every line identical. The numbers in the findings. Part
  5's functions (label tests and walks over a segmentation) were not timed: §24 names no speed
  check. Part 7b (relay 13, `mk.py part7b`; §36 names none): the parcellation's smoothing of 64^3
  shorts 6.2x, the mask 1.45x, the clipping of bright white matter 1.17x, a 256 x 256 slice as an
  image 1.27x, the label mean and spread 0.92x, window counts 0.80x, the label borders 0.61x; every
  line identical. Parts 9 and 10a were not timed: §45 and §47 name no speed check.
