# fs.volume's clean room against FreeSurfer 8.2.0, as a black box (parts 1 to 3)

larmorx's clean room of FreeSurfer's volume functions (`crates/larmorx-fs-core/src/volume/`, written
from [`specs/freesurfer-volume.md`](../../../specs/freesurfer-volume.md) §1 to §17 alone) compared
with the prebuilt probe of the open folder [`../freesurfer-volume-probe/`](../freesurfer-volume-probe/README.md)
(FreeSurfer 8.2.0's library as the official build compiles it, with the fs.maths probe's guard
allocator). The probe is only run, never built.

| Path | What it is |
|---|---|
| `candidate/` | `fs-volume-candidate batch <file>`: the probe's commands and print forms over the clean room (its own Cargo workspace, a validation tool outside larmorx's build); each line on a process state of its own, `LARMORX_UPSTREAM_DEFECTS` the mode, the defect notices of a line on its standard error |
| `../../../crates/larmorx-fs-core/tests/volume_cmd/` | the commands themselves (the probe's grammar and print forms; the volume, geometry, matrix, colour-table and transform encodings are fs.mri's, `tests/mri_cmd/args.rs`, included by path), shared with the crate's golden cases (`tests/golden_volume.rs`); the candidate includes the folder by path |

## Running it

```sh
(cd candidate && cargo build --release)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe \
  --candidate <target>/release/fs-volume-candidate --scratch <scratch>/volume --mode reproduce   # then fix
```

The golden cases: `cargo test -p larmorx-fs-core --test golden_volume` (record with
`FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`); under the AArch64 NaN emulation, the test binary
(`target/<profile>/deps/golden_volume-*`) with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
NAN_TRAP_DIR=<scratch>/nt`, the library built from `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
as that folder's README says. The defects' own tests: `cargo test -p larmorx-fs-core --test defects_volume`.

## Results (2026-11-17, seed 7, every group of §69.2; the probe's reference hashes are the spec's, cold and warm)

`compare.py` per group: calls, identical, FreeSurfer did not finish, memory, defect corrected,
unexplained.

| Group | Calls | reproduce | fix |
|---|---|---|---|
| vgblock | 560 | 560 / 0 / 0 / 0 / **0** | 495 / 0 / 0 / 65 / **0** |
| vgcopy | 1,080 | 1,080 / 0 / 0 / 0 / **0** | 1,080 / 0 / 0 / 0 / **0** |
| vmake | 1,050 | 890 / 135 / 25 / 0 / **0** | 497 / 135 / 110 / 308 / **0** |
| vrange | 800 | 685 / 115 / 0 / 0 / **0** | 496 / 115 / 3 / 186 / **0** |
| vscale | 1,100 | 911 / 178 / 4 / 0 / **7** | 506 / 178 / 114 / 302 / **0** |
| vtwo | 200 | 162 / 32 / 4 / 0 / **2** | 98 / 32 / 6 / 64 / **0** |
| vextract | 600 | 501 / 43 / 56 / 0 / **0** | 144 / 43 / 131 / 282 / **0** |
| vcoords | 1,350 | 1,350 / 0 / 0 / 0 / **0** | 1,178 / 0 / 0 / 172 / **0** |
| vstats | 750 | 711 / 39 / 0 / 0 / **0** | 475 / 39 / 12 / 224 / **0** |
| vheader | 606 | 606 / 0 / 0 / 0 / **0** | 498 / 0 / 0 / 108 / **0** |
| all | 8,096 | 7,456 / 542 / 89 / 0 / **9** | 5,467 / 542 / 376 / 1,711 / **0** |

Totals: `reproduce` 7,456 of 8,096 identical (542 where FreeSurfer did not finish, 89 memory lines),
9 unexplained; `fix` 5,467 identical (542 not finished, 376 memory, 1,711 corrected by a noted
defect), none unexplained.

- **The 9 unexplained lines** (`vscale` 7, `vtwo` 2) are FS-291's `reproduce` of `MRIinvertContrast`
  and `MRIscaleAndMultiply` on volumes that are not of bytes: the spec says FreeSurfer takes the
  first `width` bytes of each row as byte voxels, the clean room does exactly that, and FreeSurfer
  writes only the destination's first row (with the first row's values when the source is a tensor
  volume, with the last row's when it is an int or short volume). The spec writer is asked for the
  rule (the findings). `fix` refuses these calls (FS-291, noted).
- **Where FreeSurfer did not finish** (crashes: header copies from a volume without its cached
  matrix, FS-252; volumes without voxels walked directly, FS-58; smaller second volumes, FS-290;
  rows of a negative length, FS-294; destinations of another type FreeSurfer frees, FS-293; and the
  principal components of covariances with infinite or NaN entries, FS-47 and ITK-112): the
  candidate finishes, with the spec's refusal or result.
- Behaviours measured beyond the spec's text (`docs/findings/fs-volume-clean-room.md`): the voxel
  reads and writes of the types without a value need no voxels, `MRIclear` clears a bitmap volume,
  `MRIlinearScale`'s other voxels keep their own value, `MRIadd` of sources without direct access
  writes nothing, the frame copies' messages end with their own new line, `MRIclone` into itself
  loses its colour table (FS-332, new).

## Golden cases, defect tests, speed

- `cargo test -p larmorx-fs-core --test golden_volume`: every command of the probe's grammar on
  seeded arguments (volumes of at most 3 x 3 x 3 voxels and 2 frames, every type, header-only ones,
  special values), both modes, 20 SHA-256 over 10 groups (0.02 s in a debug build). Identical in
  debug and release builds and under the AArch64 NaN emulation of both.
- `cargo test -p larmorx-fs-core --test defects_volume`: each defect of the spec's §69.4 (FS-285 to
  FS-297) and FS-332 noted in both modes on a line of the comparison's own batches (the shortest),
  FS-292, FS-296 and FS-297 on lines written for them, the two modes' results different where the
  correction changes them; also FS-12, FS-47, ITK-112, FS-52, FS-53 and FS-58.
- Speed: the spec's plan names no speed measure for these parts (voxel loops of the volume's size;
  each probe call is dominated by its own process); not measured.
