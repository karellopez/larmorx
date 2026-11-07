# PRELUDE's clean-room original: findings

The clean-room implementer's findings while writing `larmorx fsl prelude` (crate `larmorx-fsl`,
`prelude` and `cli::prelude`, Apache-2.0) from `specs/prelude.md`, from black-box runs of FSL
6.0.7.23's `prelude` (through the oracle cache), of the replica (`larmorx-nc fsl prelude`, as a
program) and of the spec writer's probe and model (run, not read). No FSL source and no replica
code were read. The record: [fsl-prelude](../validation/fsl-prelude.md); the tools behind each
finding: [`validation/investigations/fsl-prelude-original/`](../../validation/investigations/fsl-prelude-original/README.md)
and [`fsl-pair-header/`](../../validation/investigations/fsl-pair-header/README.md).

## 1. A pair's header file holds NIfTI's four extension bytes (every FSL program)

FSL writes a NIfTI pair's header file (`.hdr`, `.hdr.gz`, `.hdr.zst`, `.hdr.bz2`; NIfTI-1 and
NIfTI-2) as the header block followed by NIfTI's four extension bytes: 352 bytes for NIfTI-1, 544
for NIfTI-2, the bytes `00 00 00 00`; with the input's extensions kept
(`FSL_LOAD_NIFTI_EXTENSIONS=1`), `01 00 00 00` and every extension in full. A pair's `vox_offset`
is 0, so FSL-56 (a single file's data offset counting the first extension's size once per
extension) does not reach pairs. Observed with fslmaths, fslroi, bet2, fast, flirt, mcflirt,
prelude, topup and applytopup, every pair type (`fsl-pair-header/probe.py`, 106 runs).

The clean-room writers wrote the header block alone (larmorx's NIfTI writer follows nibabel,
which writes the four bytes into a pair's header file only when extensions follow), except
prelude's first version and FLIRT's uncompressed `.hdr`, which patched it afterwards; and a zstd
or bzip2 pair lost the kept extensions. The parity suites never saw it: they compare the header
fields nibabel reads (the 348- or 540-byte block). Corrected in the two shared writers
(`larmorx_fsl::bet::image::write`, `larmorx_mri::hmc::image::write_like_files`): 105 of 106 runs'
header files identical to FSL's, the other FSL-56's single file (corrected by default, identical
with `reproduce`). The pair cases of the suites `fsl-bet`, `fsl-maths`, `fsl-topup` and
`fsl-flirt` (22 cases) give the results of main's records unchanged.

For a spec writer: `specs/prelude.md` §3.3 and `specs/bet.md` §3.3 could state the four bytes.

## 2. mcflirt writes ANALYZE's `extents` and `regular` (16384, `r`)

FSL's mcflirt, like every FSL program the probe ran, writes `extents` = 16384 and `regular` = `r`
into every NIfTI-1 header, whatever the input holds; the clean-room head-motion correction
(`hmc::image`) kept the input's (0 and 0 in a nibabel image). Corrected with finding 1 (single
files and pairs). The suite `mri-hmc` compares the header's grid, types, units and forms, not
these two fields, so its record does not move; the golden cases of `lx.mri.hmc` and `larmorx fsl
mcflirt` change their header checksums (NIfTI-1 outputs), their data not.

## 3. The time step as FSL's image layer reads it (random testing)

The original's first random-testing trial (seed `prelude-trial`, the replica trial's) differed
from FSL on series whose time step (`pixdim[4]`) was 0 or negative: FSL reads an image's time step
as its magnitude, and as 1 for an image with a time axis (`dim[0]` 4 or more, one volume or
several) whose time step is 0; every output keeps the time step of the image its header comes from
(the unwrapped, raw phase and labels the phase's, the mask the magnitude's or the `-m` image's). A
3D image's 0 stays 0. A complex image (`-c`) keeps its own: its magnitude, a 0 kept
(`fsl-pair-header/time_step.py`; observed: a 4D phase of 0, 2 and -2 gives 1, 2 and 2, a 3D phase
of -2 gives 2, a complex series of 0 and -2 gives 0 and 2). `specs/flirt.md` §4.1 states the
same reading for FLIRT. The original kept the stored value; corrected in `cli::prelude` (the
images' reading). For a spec writer: §3.1 and §3.2 of `specs/prelude.md` could state it.

**The same in the other clean-room originals (settled in relay 3, 2026-11-07).** The rule
belongs to FSL's image layer: `time_step.py`, extended to every FSL program with a clean-room
original that writes images (66 runs), found every original but FLIRT's (which reads it itself)
writing the stored time step (47 runs differed: fslmaths, fslroi, bet2, bet `-F`, fast,
applyxfm4D, mcflirt, topup's `--iout` and `--fout`). The shared readers apply it now
(`larmorx_mri::hmc::image::time_step_as_read` in `read_series_files` and `read_stored_files`,
and `bet::image`'s ANALYZE and lean readers), prelude's own reading removed: 63 of the 66 runs
identical to FSL's, the other 3 FSL-262's corrected default (identical with `reproduce`).
`pixdim[5..7]` are kept as stored, as FSL keeps them. The records re-made with it: see their
findings files.

## 4. Messages and notices

- No `Loading volumes` before a complex image: §4.11 lists it without saying that `-c` prints
  nothing before its image (§3.1); FSL, observed through the replica, prints nothing. For a spec
  writer: clarify §4.11.
- After FSL-263's refusal the replica prints its refusal and a "corrected" notice; the original
  the refusal only. Both name FSL-263, both are larmorx's own lines (left out of the comparisons
  with FSL); the suite compares the defects named.
- GLIBC-07 is named in both modes where the correctly rounded arctangent and glibc's `atan2f`
  differ on a voxel of a complex image (§6.4), whether or not an output changes (the voxel may lie
  outside the mask). The replica suite's D13 check, which the original's suite runs as a black box,
  expects a named defect only where an output changes; for GLIBC-07 alone that check is reported,
  not gated (the replica never names GLIBC-07).

## 5. The 2D labels after a slice without regions

§9.3 calls §4.6's numbering after a slice without regions unreachable (the mask is one connected
component); with FSL-269's correction (NaN phases removed from the mask after its component) a
slice can be emptied. The original follows §4.6's rule as written (the numbers continue from the
previous slice's largest). For a spec writer: confirm or decide.

## 6. Built for speed

What FSL spends its time on is the merging (`O(L^2)` per volume for `L` regions). The original
keeps the constraints in slots reused by the merges, an indexed 4-ary heap keyed by (C descending,
the pair ascending) updated in place, each group's list of constraint slots (scanned for the
constraint between two groups) and a weighted union-find of the turns, so a choice costs a heap
operation and a merge the neighbours of the group that joins; the merge order and each merge's
arithmetic are FSL's (§4.8, §10). The linear parts run on the call's threads, with the same bits at
any thread count. The numbers: [benchmarks](../benchmarks/fsl-prelude.md).

## 7. Memory on large maps (relay 3, 2026-11-07)

On the benchmark's 128³ map (66,020 regions, 295,535 constraints) the original's peak was 78.2 MiB
at one thread against FSL's 64 MiB, above the memory rule's 78.4 for the standalone program with
little margin. Where it went: two full copies of the phase (one kept for the other rule's result,
FSL-261 and FSL-269's notices), three masks (the source's, the phase grid's copy, `--savemask`'s
copy), the unwrapped phase allocated before the merging, and the constraints' map alive while the
merge built its slots, heap and lists. Now (`a85f764`): the phase is copied only where the other
rule's result can be needed (a NaN phase, `--removeramps`), the source's mask is used as it is on
the phase's grid and returned for `--savemask`, the unwrapped phase is allocated after the merging
(the slices mode writes each slice's into it in place), and the merge makes its slots, frees the
map, then builds its lists at their exact sizes. The same bits everywhere (the large maps' outputs
and fMRIPrep's calls compared). Peaks, standalone program, one thread: 128³ **59.1 MiB** (was
78.2), with `-s` 43.0 (61.8), with `--removeramps` 67.1 (77.7; the phase kept for FSL-261's
notice), with `-s --removeramps` and every output 50.9 (62.7); 256 × 256 × 160 210 MiB (272);
at 12 threads 75 MiB on 128³ (the threads' own buffers). The memory test gained the 128³ map's two
calls (`test_prelude_original_large_map_needs_no_more_memory_than_fsl`, through Python, against
FSL's peaks in the oracle cache): 69.8 and 54.4 MiB against FSL's 67.3 and 56.8 (allowances 88.0
and 76.5).
