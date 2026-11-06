# 3dvolreg: the replica against AFNI, case by case

The helpers of the 3dvolreg replica part's first relay (2026-11-06), which showed the replica
(`crates/larmorx-afni-replica`, `larmorx afni 3dvolreg` with `LARMORX_IMPLEMENTATION=replica`)
bit-identical with AFNI 26.3.05's `3dvolreg` (`oracles/afni-26.3.05`, built by
`scripts/build_afni_oracle.sh --add ... 3dvolreg`) on 59 cases (`reproduce`): every interpolation method, the
final method, the clipping, the padding, an internal and an external base, the weights
(`-weight`, `-wtinp`), the edging, the iteration controls, `-twopass` with its options, the
outputs (`-dfile`, `-1Dfile`, `-1Dmatrix_save`, `-maxdisp1D`, `-rotcom`, `-prefix NULL`), the
messages with `-verbose` and `-verbose -verbose`, int16, scaled, byte, oblique and sagittal
inputs, the option errors; and fMRIPrep's call (`-Fourier -twopass -1Dfile -1Dmatrix_save
-prefix -zpad 4 -maxdisp1D`, niworkflows' `RobustAverage`) on inputs re-made from three
oracle runs (ds000005 64 × 64 × 34, ds003592 echo 1 64 × 64 × 43, ds005454 160 × 160 × 96: the
subsampled coarse search), 20 volumes each, with `reproduce` (ds000005 by default too). The
parity suite and the
validation record (sub-task 2) supersede them; they stay as the record of how the first
comparisons were made.

| Script | What it does |
|---|---|
| `make_series.py <out.nii.gz>` | a synthetic moving series (40 × 44 × 24, 6 volumes, float32) |
| `variants.py <folder>` | variants of `<folder>/s1.nii.gz`: LPI int16, int16 with a slope, oblique with odd sizes, sagittal, a 3D base, a weight dataset, bytes |
| `sliced.py <gen_avg node> <BIDS root> <out.nii.gz>` | re-makes `RobustAverage`'s `_sliced.nii.gz` (the volumes of `t_mask`, clipped and divided by the drift as niworkflows does) from an fMRIPrep oracle run's `gen_avg` node |
| `compare.py <input> <fix\|reproduce> <options...>` | runs AFNI and the replica on the input in two folders (`<input's folder>/cmp/afni`, `cmp/lx`; every NIfTI file of the input's folder linked), with a clean environment, and compares exit statuses, messages (upstream-defect notices left out), text outputs byte for byte and NIfTI data bit for bit |
| `batch.py <cases file>` | `compare.py` over a list of cases, one line each (`OK` or `BAD` with the differences) |
| `cases1.txt`, `cases2.txt` | the cases run (paths relative to the folder holding the inputs) |
| `variants2.py <folder> <AFNI's 3dvolreg>` | relay 2's inputs: slice timing (`-tshift`, a negative slope), `-gridparent` grids (27, 24 and 22 slices, a moved origin, wrong sizes and spacing), a shifted base (`-twodup`), one-slice, four-slice and constant series, and `-rotparent` datasets made by AFNI's own 3dvolreg (its `VOLREG_*` attributes) |
| `cases3.txt` | relay 2's cases: `-tshift`, `-savedisp`/`-SAVEDISP`, `-rotparent`/`-gridparent`, `-twodup`, the scan's message order, AFNI-89 and AFNI-93's volumes |

The workspace (`LARMORX_WORKSPACE`, default the repository's parent), AFNI's program
(`AFNI_VOLREG`) and larmorx's (`LARMORX_BIN`, default `target/release/larmorx`) can be set in the
environment. In `fix` mode two cases differ from AFNI by design: a byte input (AFNI-87) and
`-nocoarserot` (AFNI-86).

The routines themselves were compared one at a time with AFNI's library by
`validation/probes/afni-volreg-trace` (the blur on every path, the rotations of every method,
with padding and flips).

## Relay 2 (2026-11-06)

`cases3.txt` (49 cases): 45 bit-identical with AFNI 26.3.05 (files, NIfTI headers, messages,
exit statuses); the other 4 by design: the replica's error where AFNI crashes on a constant
base (AFNI-89, both modes), AFNI-93's empty coarse region corrected by default, and two that
were fixed and then identical (the obliquity warning once per run, the base's name in the
mismatch messages). The cases `compare.py` cannot express (standard input for `-@`, an option
given last, a 300-byte `-weight` name) were run side by side by hand: identical, but for the
long name, where AFNI aborts (buffer overflow) and the replica's `reproduce` refuses it with an
explanation (AFNI-90). Four cases (fMRIPrep's call, `-tshift` with `-savedisp`, `-rotparent`
with `-gridparent`, int16) are bit-identical with AFNI 25.2.09 too (`AFNI_VOLREG` set to its
3dvolreg). `compare.py` now also compares the NIfTI headers' geometry, time and scaling fields.

The blur's mirrors (rule D): `cargo run --release -p larmorx-afni-replica --example
volreg_mirrors -- <series.nii.gz> [heptic]` registers a series with AFNI's mirrors and with
whole-point mirrors on every axis and prints the largest differences
(`docs/findings/afni-volreg.md` has the numbers).
