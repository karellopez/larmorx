# FreeSurfer 8.2.0's volume structure: the clean room's black-box findings

What black-box runs of the prebuilt probe `fs_mri_probe` (FreeSurfer 8.2.0's library, the official
compiler and flags) showed while larmorx's clean room of `specs/freesurfer-mri.md` (`fs.mri`, crate
`larmorx-fs-core`, module `mri`) was written. Black-box facts only: no FreeSurfer source was read.
The comparison loop and the candidate: `validation/investigations/freesurfer-cleanroom-mri/`
(README: every group's result).

## The comparison (spec §23, seed 7, 9,379 calls)

`compare.py compare` on the reference machine (x86-64, glibc 2.35, Linux 6.8), 2026-11-13, again after
the spec's amendments (`023bbf8a`, with the fixed batches `matrices-shear` and `tkreg-given`):

| Mode | Identical | FreeSurfer did not finish | Memory lines | Defect lines allowed | Unexplained |
|---|---:|---:|---:|---:|---:|
| `reproduce` | 9,026 | 191 | 135 | 27 (FS-264's refusals in the size batches) | 0 |
| `fix` | 6,656 | 191 | 202 | 2,330 (noted corrections) | 0 |

Per group (reproduce, identical of all): alloc 1,014 / 1,014; matrices 943 / 999; voxels 902 / 1,000;
headers 589 / 590; ctabs 252 / 252; geometry 594 / 620; tkreg 572 / 595; registrations 1,074 / 1,080;
coords 300 / 300; printing 600 / 600; names 218 / 218; copies 568 / 600; shapes 100 / 100; bbox
483 / 500; lta 817 / 911. Every other line is one where FreeSurfer crashed (the candidate finished), a
memory line, or one of FS-264's refusals.

## Behaviour measured beyond the spec's text

Measured on the spec of `278e0be0`; the spec writer checked each on the probe and amended the spec to
them (`1f748c3e`, `023bbf8a`, with the fixed batches `matrices-shear` and `tkreg-given`).

1. **`RASFromSurfaceRAS_` given a matrix larger than 4 x 4** (the spec's §3 and §9 say its top-left
   4 x 4): FreeSurfer computes the tkregister matrix's inverse *into* the given matrix as if it were
   4 x 4 (its 16 numbers laid out across the larger matrix's rows) and then the product
   `scanner vox2ras x that matrix` into it again. A 4 x 5 matrix comes back filled with numbers that
   are not the tkregister-to-scanner matrix (`given 4 5 ...`); a 5 x 5 or 5 x 4 matrix gives nothing,
   after `MatrixMultiply: m1 cols 4 does not match m2 rows 5`. 17 lines of the batch `tkreg-sizes`
   (4 x 5, 5 x 4, 5 x 5). Registered as **FS-331** (rule A): by default the result goes into the
   top-left 4 x 4 (the rest kept), `reproduce` gives FreeSurfer's (identical to the probe on all 17).
   The inverse into the larger matrix counts as failed when an element (r, c), r, c ≤ 4, read through
   that matrix's own rows and columns, is not finite (the elements the inverse's 16 numbers do not
   reach keep the given values), and the program then ends in the product (`m2 is null`): the
   spec's amended text (`1f748c3e`); the clean room reads the inverse's result so whenever the output
   is larger than the input (`specs/freesurfer-maths.md` §8.8's check, FS-28's layout): 10 lines of
   the fixed batch `tkreg-given` differed before, none after (the maths `minverse` group unchanged).
2. **`surfaceRASFromRAS_`** is `K x inv(T)` (the tkregister vox2ras times the inverse of the
   scanner vox2ras), not the inverse of `RASFromSurfaceRAS_`: inverting the product differs in the
   last bit of some elements and in the sign of zeros (121 lines of the batch `tkreg` before the
   change, none after).
3. **`MRIcopy` from float to unsigned char** rounds first (`nint`) and clips at 255 after: a float
   beyond the int range rounds to -2147483648 and keeps its low byte, 0 (the spec's §13 says "first
   clipped at 255"; with that order 4.04e25 gives 255, the probe gives 0). 2 lines of `copies`.
4. **`MRIfixTkReg` with a registration of four columns and another row count** (FS-165): FreeSurfer
   crashes on the product that fails (`inv(K mov) x (R P)`), after its mismatch messages; when the
   moving geometry's tkregister matrix has no inverse it ends first with `MatrixMultiply: m1 is null`
   (status 252, 3 lines of `registrations-sizes`). larmorx's FS-165 refusal is at that place.
5. **`MRItkreg2FSL`'s chain** multiplies into its running result and keeps the product's value: a
   step that does not fit leaves nothing (a 4 x 3 registration: `MatrixMultiply: (4 x 4) * (4 x 3) !=
   (4 x 4)`, then `m1 is null`, status 252; 5 lines of `registrations-sizes`). `MRItkReg2Native` and
   `MRItkRegMtx` keep their running result on a step that does not fit (as the spec says).
6. **The shape messages of the tkregister matrices**: each tkregister vox2ras prints §4's message once
   per extent that is not positive; the geometry-valued conversions (`VGtkreg2RAS`, `VGras2tkreg`,
   a geometry's `get_RAS2TkregRAS` and `get_TkregRAS2RAS`) twice (their volume header, then the
   tkregister matrix): 7 messages for one non-positive extent in the `tkreg` command, 6 in `ras2s`.
7. **The shear matrix** of §3 is the 4 x 4 identity with the shears, multiplied on the right of the
   scanner matrix (`M x Sh`), and the `DIAG_INFO` print shows it 4 x 4 (the spec amended to this;
   the fixed batch `matrices-shear`, 69 lines, identical in both modes).
8. **FS-252** (§6, amended): a header copy from a source without its cached voxel-to-RAS matrix
   (FreeSurfer crashes; its own volumes always have one, so the probe cannot show it) copies none in
   both modes and notes FS-252 (the clean room's own test).

## Golden cases, defects, speed

- Golden cases (`crates/larmorx-fs-core/tests/golden_mri.rs`): every probe command on seeded
  arguments, 13 groups x 2 modes, 26 SHA-256s, 0.03 s; identical in debug and release builds and
  under the AArch64 NaN emulation of both (about 1,000 NaN results flipped in each run).
- Defect tests (`tests/defects_mri.rs`, 18 tests): FS-52 to FS-60, FS-161 to FS-166, FS-195 to
  FS-198, FS-252, FS-264 and FS-331 (its non-finite check too) in both modes, the cases the probe
  cannot show included (FS-53's writes and reads, FS-58, FS-165, FS-197, FS-198, FS-252, FS-264).
- Speed (`speed.py`, black box, one thread, indicative, two runs): the bounding box of a 160³ volume
  (float, int) 0.2-0.8 ms a call against 2.3-4.4 ms in the probe (5 to 18 times faster: the
  first frame scanned row by row, each row's first and last voxel above the threshold);
  `MRIfixTkReg` 0.04-0.05 ms against 0.11-0.14 ms (2 to 3.7 times).
