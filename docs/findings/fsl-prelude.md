# Findings: FSL PRELUDE (FSL 6.0.7.23's `prelude`)

**Restricted: written from FSL's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/prelude.md`.

What the replica (`crates-nc/larmorx-nc-fugue`, with newimage's complex reader added to
`crates-nc/larmorx-nc-core`, 2026-11-06, from fsl-fugue 2603.0's `prelude.cc` and `unwarpfns.cc`,
fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-newnifti 5.0.0 and fsl-utils 2412.6) found in
`prelude`: how it computes its result, where FSL's code does something undefined or
questionable, and what larmorx does. Each entry says how it is known: *read* (the source),
*validated* (the parity suite `fsl-prelude-replica`, every case against FSL's program, bit for
bit), *probed* (`validation/investigations/fsl-prelude/`).

## How prelude works, for the replica

- **The command line** (*read*, *validated*). fsl-utils' option parser, in the order of the usage
  text: `-c,--complex`, `-a,--abs`, `-p,--phase`, `-o,--out,-u,--unwrap` (compulsory),
  `-n,--numphasesplit` (8), `--labelslices`, `-s,--slices`, `-f,--force3D`, `-t,--thresh`,
  `-m,--mask`, `--start`, `--end`, `--savemask`, `-r,--rawphase`, `-l,--labels`, `--removeramps`,
  `-v,--verbose`, `-h,--help`. A long option takes its value after `=` only (`--start 1` is
  "Missing non-optional argument"); `--complex=` (empty) is a missing argument. An option error
  prints the usage then the parser's message (exit 1); `-h` prints the usage (exit 1); no `-o`
  prints the compulsory-options text and the usage (exit 1); `-c` with `-p` or `-a`, `-p`
  without `-a` (or the reverse) and `-n` below 2 ("Always set --numphasesplit greater than 1.\nNOT
  <n>") print the usage and their message (exit 1). The title is "prelude (Version 2.0 in c#
  minor)".
- **The inputs** (*read*, *validated*). `-c` reads the complex image with newimage's
  `read_complexvolume` (NewNifti's raw buffer, no scaling) and makes the phase with `atan2f(imag,
  real)` and the magnitude with `sqrt` of the float sum of squares; `-p` and `-a` read two float
  series (with their scaling). A missing file, or no input at all, is newimage's exception #22,
  which nothing catches (`terminate`, SIGABRT: status -6 from a shell's view; the replica aborts
  the same way after the same message). `--start` and `--end` outside the series (or negative)
  are newimage's exception #61 ("Invalid t index in [] operator"), also uncaught.
- **The phase range** (*read*, *validated*). Before any output, for every volume from `--start`
  to `--end`: when its maximum minus its minimum (in float) is above 3 pi, prelude prints "ERROR:
  input phase image exceeds allowable phase range.\nAllowable range is 6.283 radians.  Image
  range is: <range> radians.\nAborting." (the range with the stream's 6 significant digits) on
  standard error and exits 255; when the maximum is above 1.001 pi or the minimum below -1.001
  pi, every value of the volume is rewrapped, `theta - round(theta / 2pi) * 2pi` in float with
  float constants ("Rewrapping phase range to [-pi,pi]" with `-v`). The raw phase written by
  `-r` is the whole series as prelude holds it after this (and after the ramps' removal and
  restoration of each processed volume).
- **2D or 3D** (*read*, *validated*). The labels are 2D (slice by slice, numbered on through the
  slices) when the first processed volume is larger than 64 voxels along x or y, or with
  `--labelslices`; `-s` labels and unwraps slice by slice; `-f` turns both off. Without `-s`
  the unwrapping is 3D even with 2D labels.
- **The mask** (*read*, *validated*). With `-m` the mask image's volume `min(n, last)` serves
  volume `n`, thresholded at 0.5 unless `-t` is given; otherwise the magnitude's volume `n`,
  thresholded at `-t` or, by default, at `0.1 * (p98 - p2) + p2` of all its values (the 2nd and
  98th percentiles of newimage's `percentile`, over libstdc++'s `std::sort` of the values). The
  mask is the values in `[threshold, maximum]`, then its largest 6-connected component (with
  `-s`: each slice's own maximum and largest 4-connected component); the first of equally large
  components wins.
- **The ramps** (*read*, *validated*). `--removeramps` estimates one slope per axis, the mean of
  the wrapped forward differences over the voxel pairs inside the mask, removes it inside the
  mask (`wrap(ph - ramp . (r - centre))`, the centre `(max + min) / 2` of each axis, in double
  then float), unwraps, and adds it back to the unwrapped and to the raw phase inside the mask.
  With `-v`: "Removing linear ramps", "Estimated linear ramps", "Removed linear ramps", "Ramp
  values: " and the three values in fixed notation with 6 decimals, each followed by a blank and a
  newline (armawrap's printing of a column vector), and after the unwrapping "Removed linear
  ramps" again (FSL's text).
- **The labels** (*read*, *validated*). Inside the mask each voxel gets its phase bin,
  `(int)((phase + pi) / binsize) + 1` clamped to `[1, n]` with `binsize = 2pi / n` in float; the
  6-connected components of equal bins (4-connected in a slice for 2D labels) are the regions,
  numbered in scan order (x fastest); 0 outside the mask.
- **The constraints** (*read*, *validated*). For each labelled voxel (z, then y, then x) and its
  neighbours at x-1, y-1 and z-1 with another nonzero label, the pair (smaller label, larger
  label) counts one more voxel pair (`N`) and adds the smaller label's phase minus the larger's
  (`P`, float sums in that order). Then `K = -P / (2pi N)` and `C = N (0.5 - |round(K) - K|)`,
  in double, stored as float (`round`: half away from zero).
- **The merging** (*read*, *validated*). Repeatedly the constraint with the largest `C` (the
  first in the order of the pairs among equal ones; FSL starts from the first constraint's `C`
  and replaces it only by a larger one, so a NaN first `C` stays chosen and NaN values elsewhere
  are passed over) merges its larger label `s` into its smaller `r`: every member of `s` gets
  `round(K)` subtracted from its offset, and each constraint of `s` with a third label `t` is
  folded into `r`'s with `t`: `P_rt + P_st - 2pi N_st round(K)` (seen from `r`), `N_rt + N_st`,
  then `K` and `C` again. With `-v`: "<count> constraints left" at the 1st, 1001st, ... merge
  and "Did while loop <count> times". The offsets are then taken relative to the label with the
  largest total interface (the sum of its constraints' `N`, the first such label), and the
  unwrapped phase is `phase + 2pi offset` (double, then float) inside the labels, 0 elsewhere.
- **2D unwrapping** (*read*, *validated*). With `-s` each slice is unwrapped on its own (its own
  constraints and reference label: "SLICE NUMBER <z>" with `-v`); then each slice from the
  second on is moved by `2pi m`, `m` the rounded median (over 2pi) of its differences from the
  slice below over the voxels labelled in both (the upper median: the value at half the count of
  the sorted differences).
- **The outputs** (*read*, *validated*). The unwrapped phase: one volume per processed volume,
  float, with the phase image's properties and its time step; `--savemask`: the mask (float);
  `-l`: the labels (int32); `-r`: the raw phase. Every file through newimage's `save_volume`
  (`FSLOUTPUTTYPE`, newimage's `descrip`); an invalid `FSLOUTPUTTYPE` stops at the first save
  with FSL's message (exit 1). prelude runs on one thread and draws no random numbers.

## The replica's structure

- **The merging is restructured for speed, with FSL's arithmetic** (*validated*). FSL scans
  every label for every merge (O(L²) for L labels: 6.5 s on fMRIPrep's ds005454 call, 14,678
  labels); the replica keeps each label's neighbours and members and the constraints ordered by
  `C` (largest first, then the smallest pair), so each merge touches the merged labels'
  constraints only: 0.15 s. The constraints visited, their arithmetic and its order are FSL's;
  the unit test `unwrap_matches_fsl_loops` compares the result with a statement-by-statement
  translation of FSL's loops, and every suite case is bit-identical.
- One volume's work after its mask is a function, so that a correction whose notice needs FSL's
  result computes both (FSL-268, FSL-269); `-v`'s messages keep FSL's order. The thresholds'
  two percentiles sort the values once (FSL sorts the same values twice: the same order).

## Defects (D13; the register's FSL-261 to FSL-271)

`LARMORX_UPSTREAM_DEFECTS=reproduce` gives FSL's result bit for bit (the suite runs the replica so
and compares every file); the default `fix` corrects each, with a notice where the correction
changed the result. Decided by the rules A-D (CLAUDE.md, D13).

| ID | What FSL does (*read*; evidence) | Rule | Default | Notice |
|---|---|---|---|---|
| FSL-261 | `--removeramps` loops `x < maxx`, `y < maxy`, `z < maxz` in the estimate, the removal and the restoration: the last column, row and slice never count in the ramps and are unwrapped with their ramp while the rest is unwrapped without it (and restored after); an image of one slice gets no ramp removed at all (suite `opt/removeramps*`, `opt/removeramps-one-slice`) | A | every column, row and slice | the outputs or `-v`'s lines differ (both loops computed where a mask voxel lies in the last column, row or slice; random testing, 2026-11-06: a voxel there need not change the result) |
| FSL-262 | `--savemask` and `-l` with several volumes write each volume's mask and labels to the same file in turn: only the last volume's are kept (`opt/series-saved`) | A | one series of every processed volume's | more than one volume saved (both modes) |
| FSL-263 | `-c` with an image that is not complex takes the file's bytes as floats (right only for float32 data, without its scaling; an int16 image gives other values and is read past its end) and every imaginary value from memory never written (`opt/complex-real`, `opt/complex-int16`; zeros in practice, bit-identical) | B | refused: "the complex input <name> is a <type> image, not a complex one" (exit 1) | reproduce: the values beyond the buffer and the imaginary parts named, filled with zeros |
| FSL-264 | `-c` with a big-endian complex64 image: NewNifti swaps each complex value's 8 bytes as one, so the real and imaginary parts change places and the phase is wrong (`opt/complex-big-endian`) | A | each float swapped on its own | the file was of the other byte order |
| FSL-265 | `-c` with a complex128 image: NewNifti gives the datatype no width, the buffer is empty and every value comes from memory past it (`opt/complex128`; zeros in practice) | A | read as complex128, converted to float | complex128 read (reproduce: zeros, the undefined values named) |
| FSL-266 | `-v` with `--removeramps` saves the ramp-free phase as `grot` in the working directory, replacing any file of that name (`opt/removeramps-verbose`) | A | not written | `-v` with ramps (both modes) |
| FSL-267 | the range message says "Allowable range is 6.283 radians" while the check is 3 pi; "Removed linear ramps" is printed again after the ramps are restored | (f) message only: kept as FSL prints it, no switch | - | - |
| FSL-268 | NaN values in the magnitude (or mask) image enter the sort of the percentiles that set the default threshold (their place undefined: libstdc++'s sort may read outside the values), and a NaN first voxel makes the maximum NaN, so the mask is empty and the unwrapped phase all zeros (`opt/nan-magnitude`, `opt/nan-first-magnitude`) | A | NaN left out of the threshold and the maximum | FSL's mask and the corrected one differ (both computed); reproduce: a sort outside the values is refused (memory FSL does not own) |
| FSL-269 | NaN phases inside the mask are labelled (their bin is 1: `(int)NaN` is INT_MIN, clamped); their constraints are NaN and merged last with `round(NaN)` = INT_MIN as the offset, so a region labelled together with a NaN voxel, or merged into one through a NaN constraint, moves by about 2^31 cycles (about 1.35e10 written in 2,608 voxels: `nan_phase_region.py`); with `-s` NaN differences between slices enter the median's sort (undefined order) (`opt/nan-phase*`) | A | NaN voxels left out of the mask; NaN put back in the unwrapped phase | the outputs or `-v`'s lines differ (both computed; random testing, 2026-11-06: a NaN phase in a one-voxel mask changes only the size `-v` prints); reproduce: a sort outside the values (`-s`) is refused |
| FSL-270 | `--end` before `--start` processes no volume and writes no unwrapped output, yet exits 0 (the raw phase, if asked for, is written) (`opt/series-start-end`) | B | refused: "no volume to process: --end (<e>) is before --start (<s>)" (exit 1), after FSL's own exception for a `--start` outside the series | reproduce: no volume processed, no output (named) |
| FSL-271 | a magnitude (or mask) image of another size than the phase is not refused: it is read by the phase's voxel indices, outside its own extent by its extrapolation, so the mask does not correspond to the phase (reproduced bit for bit, three suite cases) | B | refused: "the magnitude (mask) image has another size than the phase image" (exit 1) | reproduce: read by the phase's indices (named) |

Refusals print `larmorx-nc: error: prelude: <what> (FSL-<n>)` on standard error and write
nothing.

## Not defects, or unreachable

- **2D labels after an empty slice** (*read*): `find_phase_labels2D` numbers each slice on from
  the previous slice's largest label, which it takes from that slice itself (0 for a slice
  without labels), so the slices after an empty one would reuse earlier numbers. prelude's masks
  are one 3D-connected component, so no empty slice lies between labelled ones: unreachable
  through prelude (a unit test keeps FSL's numbering). No ID.
- **Disconnected groups of labels** (*read*): `unwrap` assumes the constraint graph connects
  every label; the mask's single component guarantees it (each label touches its neighbours).
  Latent only. No ID.
- The automatic 2D labels for images wider than 64 voxels and the 3 pi range check are
  deliberate designs (Jenkinson 2003); kept.

## Validation (2026-11-06)

- **Parity** (`python -m larmorx_validation parity fsl-prelude-replica`, standard tier, relay 1 at
  324c51f): 79 cases, 60 bit-identical (exit status, standard output and error, every file), 19
  the same errors, 0 differences. fMRIPrep 25.2.6's seven prelude calls (sdcflows'
  phase-difference workflow on ds005454-sub-16, ds002603-sub-01, ds002422-sub-02 (two
  estimators) and ds001600-sub-1 (three)) are bit-identical to FSL 6.0.7.23 and to fMRIPrep's own
  outputs (its container's FSL). The D13 pass: each defect named exactly where the default's
  outputs differ. The record `fsl-prelude-replica` is made from a clean commit in sub-task 2.
- **Golden** (`crates-nc/larmorx-nc-cli/tests/golden.rs`, recorded from fb86dc8): 15 cases and 4
  failures on a 24 x 20 x 12 grid of a wrapped quadratic phase (plain arithmetic): fMRIPrep's
  form, a threshold with every output, `-s`, `--labelslices -n 6`, the ramps with `-v` (FSL-261,
  FSL-266, both modes), complex64 (gzip output), complex128 (FSL-265), big-endian complex64
  (FSL-264, both modes), a range of volumes with masks and labels saved (FSL-262, both modes),
  NaN phases (FSL-269, both modes), NaN magnitudes (FSL-268); the range error (255) and the
  refusals FSL-263, FSL-270, FSL-271.
- **Speed**: fMRIPrep's ds005454 call, 0.15 s against FSL's 6.5 s (the merging above).
