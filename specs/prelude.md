# Spec: `lx.fsl.prelude` (behaviour of FSL 6.0.7.23's `prelude`: fsl-fugue 2603.0)

The input for a clean-room **original** of FSL's phase unwrapping, PRELUDE (Phase Region
Expanding Labeller for Unwrapping Discrete Estimates), to be written in the main package
(Apache-2.0). FSL is distributed under the FSL Licence (non-commercial use only), so larmorx has
two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code in `larmorx-nc` (done, 2026-11-06: `larmorx-nc fsl prelude`,
  `lx.fsl.prelude(..., implementation="replica")`; `docs/validation/fsl-prelude-replica.md`);
- the original, in the main package (`larmorx fsl prelude`, `lx.fsl.prelude(...,
  implementation="original")`), written from this file, FSL's documentation, the paper and
  black-box runs of FSL's program only (`~/fsl-6.0.7.23/bin/prelude`, run in a clean environment:
  FSL never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`,
`crates-nc/fsl-source/`), the replica crates (`crates-nc/`), the findings written from the source
(`docs/findings/fsl-prelude.md`, the sections of `docs/findings/upstream-defects.md` marked
restricted), the replica author's probes (`validation/investigations/fsl-prelude/`), the
replica's permanent random-testing cases (`validation/difftest/cases/fsl-prelude/`) or
`docs/parts/` (porting guide §1). It may read and run `validation/investigations/prelude-spec/`
(this spec's black-box probe and its model of §4, written from runs of FSL's program and from
this text: §12).

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
FSL's program (`validation/investigations/prelude-spec/blackbox.py`, and the replica's suite,
which compares every case of §11 with FSL bit for bit); the rest come from the analysis of the
program's behaviour. The method: M. Jenkinson, "Fast, automated, N-dimensional
phase-unwrapping algorithm", Magnetic Resonance in Medicine 49:193-197, 2003. FSL's
documentation: the FUGUE user guide ("PRELUDE"), and `prelude -h`.

**Exact by construction.** prelude draws no random numbers, runs on one thread and uses only
IEEE arithmetic, integer conversions and one transcendental function (`atan2f`, for complex
input), all in orders this spec fixes. An original that follows the conventions of §4 and §6
gives FSL's bits in every case; the thresholds of §11 ask for that, and leave the implementer free
in how it computes (§10): the data structures, the parallelism and the memory layout are its own,
as long as the values and orders named here come out the same.

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): FSL-261 to
FSL-271 (§9; FSL-267 is a message only). Questionable behaviours kept as FSL's (rule C) are listed
in §9.2.

---

## 1. Purpose and use

`prelude` unwraps a phase image (radians, wrapped into an interval of length 2π) inside a mask.
The masked phase is cut into regions of similar wrapped phase (each voxel's phase falls into one
of `n` equal intervals of `[-π, π]`; the connected parts of equal interval are the regions);
neighbouring regions are then merged, the best-agreeing pair first, each time moving the merged
region by the whole number of turns (2π) that best joins it to its partner; the result is the
phase plus 2π times each region's number of turns, 0 outside the mask. Large images are labelled
slice by slice (and unwrapped in 3D); with `-s` everything is done slice by slice and the slices
are joined (§4.3, §4.9). The
input is either a wrapped phase and a magnitude image (the mask is made from the magnitude), or a
complex image (its phase and magnitude); a mask image can replace the magnitude's mask.

fMRIPrep (sdcflows' phase-difference workflow) calls `prelude --abs=<magnitude> --mask=<mask>
--phase=<phase difference> --unwrap=<out>`, with the phase difference rescaled to `[0, 2π]` in
float32 (sdcflows' `au2rads`), the brain-extracted first magnitude and its brain mask; it then
converts the unwrapped phase to a field in Hz.

## 2. Command line

### 2.1 Syntax and the parser

`prelude -c <complex> -o <out> [options]` or `prelude -p <phase> -a <magnitude> -o <out>
[options]`. FSL's option parser (the one of `specs/fast.md` §2.1) reads every word from the
second on; each must be an option:
- A word that does not start with `-` is an error, `<word>:  is an unrecognised token!` (two
  blanks) **(observed)**.
- An option's value is the next word, even if it starts with `-` (`-n -3`). One-letter options may
  be joined (`-fv` is `-f -v`; `-vn 3` is `-v -n 3`): each takes its value from the words that
  follow, in turn **(observed)**.
- A long option takes its value only after `=` (`--start=1`); `--start 1` is `--start: Missing
  non-optional argument!`, and so is `--complex=` (an empty value) **(observed)**. A flag given
  as `--verbose=<anything>` is set, the value ignored (`--verbose=no` is verbose) **(observed)**.
- An option given twice keeps the last value (`-n 3 -n 4` is 4) **(observed)**.
- Integers are read as C's `strtol` in base 0 (`010` is 8, `0x3` is 3) and floating-point values
  as C's `strtod` (then rounded to single precision); the whole word must be read (`-n 2.5` is an
  error) **(observed)**. An integer beyond the 32-bit range keeps its low 32 bits (two's
  complement), as C's conversion of the parsed 64-bit `long` does (beyond the 64-bit range
  `strtol` saturates first).

### 2.2 Options

In the order of the usage text:

| Option | Value | Default | Meaning |
|---|---|---|---|
| `-c`, `--complex` | image | | the complex input (§3.2), instead of `-p` and `-a` |
| `-a`, `--abs` | image | | the magnitude (with `-p` only) |
| `-p`, `--phase` | image | | the wrapped phase, radians (with `-a` only) |
| `-o`, `--out`, `-u`, `--unwrap` | name | (compulsory) | the unwrapped phase output |
| `-n`, `--numphasesplit` | integer | 8 | the number `n` of phase intervals (§4.6); at least 2 |
| `--labelslices` | flag | | labels slice by slice, unwraps in 3D (§4.3) |
| `-s`, `--slices` | flag | | masks, labels and unwraps slice by slice (§4.3, §4.9) |
| `-f`, `--force3D` | flag | | everything in 3D: cancels `-s`, `--labelslices` and the automatic 2D labels |
| `-t`, `--thresh` | float | (§4.4) | the mask's threshold |
| `-m`, `--mask` | image | | the mask image (§4.4) |
| `--start` | integer | 0 | the first volume processed (from 0) |
| `--end` | integer | the last | the last volume processed |
| `--savemask` | name | | write the mask (§3.3) |
| `-r`, `--rawphase` | name | | write the raw phase series as prelude leaves it (§3.3) |
| `-l`, `--labels` | name | | write the regions' labels (§3.3) |
| `--removeramps` | flag | | remove linear phase ramps before unwrapping, restore them after (§4.5) |
| `-v`, `--verbose` | flag | | messages on standard output (§4.11) |
| `-h`, `--help` | flag | | the usage text |

### 2.3 The usage text (standard error) (observed)

Exactly, with tabs before and between the option names and their help (`<TAB>` below), a leading
empty line and two trailing empty lines; `2412.6-dirty` is FSL 6.0.7.23's utilities version:

```

Part of FSL (ID: 2412.6-dirty)
prelude (Version 2.0 in c# minor)
Phase Region Expanding Labeller for Unwrapping Discrete Estimates
Copyright(c) 2001, University of Oxford (Mark Jenkinson)

Usage: 
prelude -c <rawphase> -o <unwrappedphase> [options]
prelude -p <phasevol> -a <absvol> -o <unwrappedphase> [options]

Compulsory arguments (You MUST set one or more of):
<TAB>-o,--out,-u,--unwrap<TAB>filename for saving the unwrapped phase output

Optional arguments (You may optionally specify one or more of):
<TAB>-c,--complex<TAB>filename of complex phase input volume
<TAB>-a,--abs<TAB>filename of absolute input volume
<TAB>-p,--phase<TAB>filename of raw phase input volume
<TAB>-n,--numphasesplit<TAB>number of phase partitions to use
<TAB>--labelslices<TAB>does label processing in 2D (slice at a time)
<TAB>-s,--slices<TAB>does all processing in 2D (slice at a time)
<TAB>-f,--force3D<TAB>forces all processing to be full 3D
<TAB>-t,--thresh<TAB>intensity threshold for masking
<TAB>-m,--mask<TAB>filename of mask input volume
<TAB>--start<TAB>first image number to process (default 0)
<TAB>--end<TAB>final image number to process (default Inf)
<TAB>--savemask<TAB>filename for saving the mask volume
<TAB>-r,--rawphase<TAB>filename for saving the raw phase output
<TAB>-l,--labels<TAB>filename for saving the area labels output
<TAB>--removeramps<TAB>remove phase ramps during unwrapping
<TAB>-v,--verbose<TAB>switch on diagnostic messages
<TAB>-h,--help<TAB>display this message


```

(`Usage: ` ends with a blank.) The parity suite compares this text exactly.

### 2.4 The checks of the command line, in order (standard error, exit status 1)

1. A parser error (§2.1): the usage, then an empty line and the parser's message:
   `<option>: Option doesn't exist!`, `<option>: Couldn't set_value! valstr="<value> <next
   word>"!` (the value and the word after it, if any: `-n abc -v` gives `valstr="abc -v"`),
   `<option>: Missing non-optional argument!`, `<word>:  is an unrecognised token!`
   **(observed)**. The first error stops the parsing.
2. `-h` (anywhere, with any other options): the usage **(observed)**.
3. No `-o`: `***************************************************`, `The following COMPULSORY
   options have not been set:`, `<TAB>-o,--out,-u,--unwrap<TAB>filename for saving the unwrapped
   phase output`, `***************************************************` (four lines), then the
   usage **(observed)**.
4. `-c` with `-a` or `-p`: the usage, an empty line, `Cannot specify both --complex AND --phase
   or --abs.` **(observed)**.
5. `-p` without `-a`, or `-a` without `-p`: the usage, an empty line, `Both --phase AND --abs must
   be used together.` **(observed)**.
6. `-n` below 2: the usage, an empty line, `Always set --numphasesplit greater than 1.` and `NOT
   <n>` (two lines) **(observed:** `-n 1`, `-n -3`**)**.

Neither `-c` nor `-p`/`-a` passes these checks: the phase is then read from an empty name (§3.1:
the image layer's exception for `""`, exit 1 in larmorx) **(observed)**.

### 2.5 Exit statuses

| Status | When |
|---|---|
| 0 | the outputs were written (also an empty mask: all zeros; FSL-270's empty volume range with `reproduce`) |
| 1 | §2.4; an output type FSL does not know (§3.3); larmorx's refusals (§9); every abort of FSL's (larmorx exits 1 there, §3.1) |
| 255 | the phase range check (§4.2) |
| 134 (signal 6) | FSL's aborts: an image that cannot be read or written, a volume index outside the series (§4.1), a magnitude series shorter than the phase's (§4.4) |

## 3. Images

### 3.1 Reading `-p`, `-a`, `-m`

Each is read as FSL's image layer reads a series, as `specs/bet.md` §3.1 and §3.2 describe for
FSL 6.0.7.23 (names with or without their extension, the formats and compressions, every real
datatype converted to 32-bit float with `scl_slope`/`scl_inter` applied, voxel sizes the absolute
header values, the radiological storage: an image stored with a positive-determinant matrix is
mirrored along x when read, and every output mirrored back when written), but **whole**: every
volume of a 4D image is kept (no truncation, no warning). A complex image given to `-p`, `-a` or
`-m` stops the image layer: `Image Exception : #8 :: Fslread: DT 32 not supported`, then
`terminate called after throwing an instance of 'std::runtime_error'` and `  what():  Fslread:
DT 32 not supported`, an abort **(observed)**.

A name that matches no image prints the `Image Exception` lines of `specs/bet.md` §3.1 (`#63 ::
No image files match: <name>`, `#22 :: Failed to read volume <name>`, `Error : No image files
match: <name>`), then `terminate called after throwing an instance of 'std::runtime_error'`,
`  what():  Failed to read volume <name>` and `Error : No image files match: <name>` again, and
FSL aborts (signal 6) **(observed:** for the phase, the magnitude and the mask; an empty name
prints them with nothing after the colons**)**. The lines `-v` printed before stay (every line of
standard output is written out as it is printed, §4.11). **Every abort of FSL's** (this one,
§3.3's write failure, §4.1's index, §4.4's short magnitude, §3.1's datatype): larmorx prints FSL's
lines, then one line of its own saying that FSL aborts there (the family's form: `larmorx fsl
prelude: FSL aborts here (signal 6); exit status 1 (larmorx)`), and exits 1.

The images are read in this order: the phase (`-v`: `Loading volumes` before, `Phase loaded`
after), the magnitude (`Magnitude loaded`), the mask (`Mask loaded`); with `-c`, the complex image
(no message), then the mask.

**Every loop of §4 runs in the radiological voxel order** (as held in memory after the mirroring
above): x fastest, then y, then z. For an image stored neurologically, this x is the file's x
reversed: it decides the regions' numbers, the ties of §4.4 and §4.8, the order of the sums of
§4.7, and "the last column" of FSL-261 (§9) **(observed:** the suite's neurological case, bit for
bit**)**.

### 3.2 The complex input (`-c`)

The complex image's header is read as for §3.1 (its matrix decides the radiological mirroring; the
sizes are `dim[1..4]`, at least one volume; higher dimensions are left out), but its data are
taken **as stored, without `scl_slope` and `scl_inter`**, in both modes, and its voxel sizes are
the absolute header values with a zero kept as zero. Each complex value `(re, im)` (single
precision) gives:
- the phase `atan2f(im, re)` (single precision; glibc 2.35's `atan2f`, fdlibm's algorithm, is not
  correctly rounded: §6);
- the magnitude `sqrt(re × re + im × im)` in single precision (each product rounded, the sum
  rounded, the square root correctly rounded).

The phase series takes the complex image's time step; the magnitude is used as `-a` would be.
What FSL reads for other datatypes and byte orders is defective (FSL-263, FSL-264, FSL-265, §9):
by default larmorx reads complex64 (either byte order, each 32-bit float swapped on its own) and
complex128 (each double converted to single precision, rounded to nearest), and refuses every
other datatype.

### 3.3 Writing

**The files**, each through FSL's image layer as `specs/bet.md` §3.1 and §3.3 describe (the type
`FSLOUTPUTTYPE` names; unset or unknown: that section's messages at the first write, exit 1; a
recognised image extension in the name is replaced by the type's: `-o o.nii` writes `o.nii.gz`
under `NIFTI_GZ`, `--savemask=m.hdr` writes `m.nii.gz`, `-o o.nii.gz` writes `o.nii` under `NIFTI`
**(observed)**; `descrip` FSL's image library's build string, `2601.0 2025-12-08T15:20:57Z`
**(observed)**; `xyzt_units` 10; `scl_slope` 1, `scl_inter` 0):

| File | When | Datatype | Values | Header from |
|---|---|---|---|---|
| `-o` | always (not with FSL-270's empty range) | float32 | the unwrapped phase, one volume per processed volume, in order (a 3D image for one volume); 0 outside the mask | the phase image (with `-c`, the complex image): its dimensions, voxel sizes, qform and sform with their codes, intent code and parameters, `cal_min`/`cal_max`, `aux_file`; its time step **(observed)** |
| `--savemask` | given | float32 | the mask (0 or 1) | the magnitude image (with `-m`, the mask image; with `-c`, the complex image): its sform and qform, `cal_min`/`cal_max` (not the phase's) **(observed:** a magnitude with another sform and `cal_max` 999 gives a mask with them**)** |
| `-l` | given | int32 | the regions' labels (§4.6), 0 outside the mask | the phase image (with `-c`, the complex image) **(observed)** |
| `-r` | given | float32 | the whole phase series as prelude leaves it: every volume (processed or not), after the rewrap (§4.2) and, for the processed volumes, the ramps' removal and restoration (§4.5) | the phase image (with `-c`, the complex image) **(observed)** |
| `grot` (in the working directory) | `-v` with `--removeramps`, `reproduce` only (FSL-266) | float32 | the phase without its ramps (§4.5's removal done, before the labels) | the phase image (with `-c`, the complex image) |

**When and in what order.** FSL writes, for each processed volume in turn: the mask (right after
it is made), `grot` (after the ramps' messages), the labels (after `Number of phase splits`);
after the last volume, the unwrapped phase, then the raw phase. So with several volumes FSL's
`--savemask` and `-l` files hold only the last volume's (FSL-262). By default larmorx writes, after
the last volume, the masks as one series (one volume per processed volume, in order; one volume:
a 3D image, as FSL's), then the labels likewise, then the unwrapped and the raw phase (each series
with its source's header and time step: the masks the magnitude's or the mask image's, the labels
the phase's) **(observed:** the replica, `fix`**)**. The first write decides where an unknown
`FSLOUTPUTTYPE` stops the run, so `-v`'s lines before the message differ between the two orders
(`reproduce` with `--savemask`: only the loading lines; the default: every volume's lines)
**(observed)**.

A file that cannot be created (a missing directory): `terminate called after throwing an instance
of 'NiftiIO::NiftiException'`, `  what():  Error: cant open file <name with its extension>`, an
abort (larmorx: §3.1's rule) **(observed:** `-o nodir/o`**)**.

## 4. The work

### 4.1 The series and the volumes processed

The phase series has `T` volumes (with `-c`, the complex image's). The volumes processed are `s`
to `e`: `s` is `--start` (0 when not given), `e` is `--end` (`T − 1` when not given); given
values are used as they are. A volume index outside `[0, T − 1]` (a negative `--start`, a
`--start` or `--end` beyond the series) stops FSL with `Image Exception : #61 :: Invalid t index
in [] operator`, `terminate called after throwing an instance of 'std::runtime_error'`, `
what():  Invalid t index in [] operator`, an abort (larmorx: §3.1's rule) **(observed:**
`--start=5`, `--start=-1`, `--end=7` of 3 volumes; `-v`'s loading lines printed before**)**. Where
it stops: in §4.2's loop at the first index outside, or, when `e < s` (no volume to check), at
the first volume's size in §4.3 (so an `e < s` with `s` outside the series aborts too).

`e < s` with `s` inside the series processes no volume: FSL writes no unwrapped phase (its image
layer writes nothing for an image of no volume), writes `-r`'s raw phase if asked, and exits 0
(FSL-270: refused by default, §9).

### 4.2 The phase range and the rewrap

Before any mask, for each volume `v = s, ..., e` in turn (before the next volume's check):
- its **extrema** as FSL takes them: start both at the volume's first value (in §3.1's order);
  then for each later value, if it is greater than the maximum it becomes the maximum, otherwise
  if it is less than the minimum it becomes the minimum (IEEE comparisons: a NaN never replaces
  nor is replaced, so a NaN first value makes both extrema NaN; NaN values elsewhere are passed
  over) **(observed:** a NaN first voxel skips both rules below**)**. That is FSL's (`reproduce`);
  the **default** starts both at the first value that is **not NaN** and applies the same rule to
  the values after it (every value NaN: both NaN, neither rule below applies), so a NaN first
  value no longer skips the range check or the rewrap (FSL-272, §9.1; spec writer, 2026-11-06);
- the **range** `R = max − min` (single precision); if `R > 3π` (the comparison in double, `3π`
  the double `3 × π`): `ERROR: input phase image exceeds allowable phase range.`, `Allowable range
  is 6.283 radians.  Image range is: <R> radians.` (two blanks after the first full stop; `R` as a
  C++ stream prints a float by default: 6 significant digits, `%g`'s form: `14.1372`, `12.568`,
  `8188`) and `Aborting.` on standard error, exit status 255, nothing written **(observed)**;
  volumes after it are not checked, `-v`'s lines of earlier volumes stay;
- otherwise, if `max > 1.001 π` or `min < −1.001 π` (in double, `1.001 π` the double `1.001 ×
  π`): with `-v` the line `Rewrapping phase range to [-pi,pi]`; every value `φ` of the volume
  becomes `wrap(φ)` (§6.3) **(observed:** a phase from 0 to 2π**)**.

Only the processed volumes are checked and rewrapped; the others stay as read (and are written so
by `-r`).

### 4.3 2D or 3D

Decided once, from the first processed volume `s`:
- **2D labels** when its grid is larger than 64 voxels along x or along y (not z), or with
  `--labelslices` **(observed:** 70 × 10 × 4: 100 regions, `-f`: 24**)**;
- **2D unwrapping** with `-s` (its labels are always per slice, §4.6, whatever the size);
- `-f` turns both off.

So there are three modes: **3D** (3D mask, 3D labels, 3D unwrapping), **2D labels** (3D mask,
labels numbered through the slices, 3D unwrapping) and **slices** (`-s`: the mask, the labels and
the unwrapping slice by slice, then the slices joined, §4.9).

### 4.4 The mask (per volume)

For volume `v`:
- **The source**: with `-m`, the mask image's volume `min(v, M − 1)` (`M` its number of volumes:
  its last volume serves the later ones); otherwise the magnitude's volume `v`. A magnitude series
  shorter than the volume index aborts (§4.1's `#61` exception) after the earlier volumes' work
  and messages (FSL has then written their `--savemask` and `-l` files, if asked; the default,
  which writes those at the end, nothing) **(observed:** three phase volumes, one magnitude**)**.
- **The threshold** `θ`: `-t`'s value if given; else 0.5 with `-m`; else the default threshold of
  the source (recomputed for each volume): sort every value of the source volume ascending (`N`
  values, the whole 3D grid); `i₂ = ⌊fl(fl(N) × 0.02f)⌋` and `i₉₈ = ⌊fl(fl(N) × 0.98f)⌋` (`fl` the
  rounding to single precision; `0.02f`, `0.98f` the floats nearest 0.02 and 0.98; each index
  clamped to `N − 1`); `t₂`, `t₉₈` the sorted values there; `θ = fl(0.1 × fl(t₉₈ − t₂) + t₂)`,
  the subtraction in single precision, the rest in double with the double constant `0.1`. (FSL's
  `-t` thus applies to the `-m` image when both are given.)
- **The candidate set**: the voxels whose source value `x` satisfies `θ ≤ x ≤ top` (single
  precision comparisons), where `top` is the source volume's maximum by §4.2's extrema rule; in
  the slices mode the same `θ` for every slice and `top` each slice's own maximum.
- **The mask**: the largest 6-connected component of the candidate set (in the slices mode, each
  slice's largest 4-connected component: its 6-connected components within the slice); among
  components of equal size the one whose first voxel comes first in §3.1's order (in the slices
  mode, within the slice); none when the set is empty (an all-zero mask; in the slices mode, an
  all-zero slice). So a given `-m` mask is also cut down to its largest component **(observed:**
  the suite's mask with a gap**)**.

The mask holds 1 inside and 0 outside; "inside" below is a mask value above 0.5. NaN values of the
source never satisfy the comparisons; for NaN values in the sort and as the first value
(FSL-268), see §9.

### 4.5 The linear ramps (`--removeramps`)

On volume `v`'s phase `φ` (after §4.2) and its mask, before the labels. The **loop domain** is
every voxel by default; FSL's (FSL-261, `reproduce`) leaves out the last column, row and slice:
`x < nx − 1`, `y < ny − 1`, `z < nz − 1` (an image of one slice: no voxel).
- **The estimate**: for each voxel `p` of the domain inside the mask, in §3.1's order, and each
  axis `a` (x, y, z, in that order, each into its own sum) whose forward neighbour `q = p + eₐ` is
  inside the grid and inside the mask: `Sₐ += wrap(fl(φ(q) − φ(p)))`, `nₐ += 1` (the sums in
  double, the wrapped differences single). The slope `rₐ = Sₐ / fl(nₐ)` (double, the count
  converted to single precision first) when `nₐ > 0`, else 0.
- **The centre** `cₐ = (sizeₐ − 1) / 2` per axis (exact); `dₐ = xₐ − cₐ` (exact).
- **The removal**: for each voxel of the domain inside the mask, `φ ← wrap(fl(((φ − r_x d_x) −
  r_y d_y) − r_z d_z))`, every operation in double (each product `rₐ dₐ` in double).
- **The restoration**, after the unwrapping (§4.8, §4.9), on the unwrapped phase and on the phase
  kept in the series (the one `-r` writes): for each voxel of the domain inside the mask, `u ←
  fl(u + ((r_x d_x + r_y d_y) + r_z d_z))` (the sum of the three products in double, in that
  order, added to `u` in double). Inside the mask the raw phase written by `-r` is therefore
  `wrap(φ − ramp) + ramp`, not the input `φ` **(observed:** values beyond `[-π, π]` in `-r`'s
  output**)**.
- With `-v`: `Removing linear ramps` before the estimate, `Estimated linear ramps` after it,
  `Removed linear ramps` after the removal, then `Ramp values: ` followed by the three slopes, each
  in C's `%.6f` form followed by a blank and a newline, then an empty line (`Ramp values:
  0.000000 `, `0.000000 `, `0.400000 `, ``) **(observed)**; after the restoration `Removed linear
  ramps` again (FSL's text, FSL-267).

### 4.6 The phase intervals and the regions

On the phase as §4.5 leaves it (§4.2's otherwise), inside the mask:
- **The interval** of a voxel: `b = trunc((φ + π) / w) + 1` with `w = fl(2π / n)` (the division
  in double, stored single), the sum and the division in double, `trunc` toward zero as C's
  conversion to a 32-bit integer (a NaN or out-of-range value gives `−2³¹`, x86-64's
  conversion); then `b` is clamped to `[1, n]` (values slightly above π after the rewrap fall
  into interval `n`; a NaN into interval 1).
- **The regions**: the 6-connected components of inside voxels of equal interval (in the 2D labels
  and slices modes: 4-connected within each slice); outside voxels get 0.
- **The numbers** (the `-l` output, and the order every later rule uses):
  - 3D: `1, ..., L` in the order of each region's first voxel in §3.1's order;
  - 2D labels: slice by slice upward, each slice's regions in the order of their first voxel in the
    slice, numbered on from the previous slice's largest number (FSL takes that number from the
    previous slice itself, 0 for a slice without regions; the mask being one 3D-connected
    component, no slice without regions lies between two with regions, so the numbers run
    through the slices without gaps);
  - slices: each slice's regions numbered `1, ..., L_z` on their own (numbers repeat across
    slices).

### 4.7 The constraints

Between each pair of regions that touch, from the phase of §4.6:
- Visit the voxels `p` with a region number `l₁ ≠ 0` in §3.1's order; for each of the three
  backward neighbours `q = p − e_x`, `p − e_y`, `p − e_z`, in that order (in the slices mode only
  the first two exist), that is inside the grid and has a number `l₂ ≠ 0`, `l₂ ≠ l₁`: let `a <
  b` be the two numbers and `φ_a`, `φ_b` the phases of the voxel of region `a` and of region `b`;
  the pair `(a, b)` counts one more voxel pair, `N ← N + 1` (single precision), and adds
  `fl(φ_a − φ_b)` to `P` (`P ← fl(P + fl(φ_a − φ_b))`, single precision, in this visiting order;
  the first pair sets `P = fl(φ_a − φ_b)`).
- Then for each pair: `K = fl(−P / (2π × N))` (double: `2π` the double `2 × π` times `N`, the
  quotient rounded to single) and `C = fl(N × (0.5 − |fl(rnd(K) − K)|))`, where `rnd` is §6.2's
  rounding (`rnd(K) − K` in single precision, `rnd(K)` converted to float; `0.5 − ...` and the
  product with `N` in double). `K` is the estimated number of turns that region `b`'s phase lies
  above region `a`'s; `C` is the merge's merit: `N/2` when `K` is a whole number, 0 when it is
  half-way.

### 4.8 The merging (3D and 2D labels modes; each slice in the slices mode)

Each region `l` starts in its own group with the offset `D_l = 0` (a 32-bit integer of turns); a
group is named by its smallest region number. While any constraint remains:
1. **Choose** the constraint with the largest `C`; among equal `C` the smallest pair `(a, b)` (by
   `a`, then `b`). Exactly, FSL's rule (which also decides NaN values, FSL-269 with `reproduce`):
   go through the constraints in the order of their pairs; the choice starts at the first; a
   later constraint replaces the current choice only when its `C` is greater (an IEEE comparison;
   `−0` and `+0` equal).
2. Let `(r, s)` be its pair (`r < s`) and `k = rnd(K)`; remove it. Every region of group `s` gets
   `D_l ← D_l − k` (32-bit two's-complement arithmetic) and joins group `r`.
3. **Fold** group `s`'s other constraints into `r`'s: for each constraint `(s, t)` (or `(t, s)`),
   `t ≠ r`: take `P_st` as seen from `s` (`P` when `s < t`, `−P` otherwise) and `N_st`; if a
   constraint between `r` and `t` exists, take its `P_rt` seen from `r` and `N_rt` and remove it,
   else `P_rt = 0`, `N_rt = 0`; remove `(s, t)`; insert the constraint `(min(r, t), max(r, t))`
   with `N = fl(N_rt + N_st)` and `P = ±Q`, `Q = fl(fl(P_rt + P_st) − (2π × N_st) × k)` (the sum
   in single precision; `(2π × N_st) × k` in double, two roundings; the difference in double,
   then single), `+Q` when `r < t`, `−Q` otherwise; then its `K` and `C` as in §4.7. The order of
   the `t` does not change any value.
4. With `-v`, at the 1st, 1001st, 2001st, ... choice (before it is removed), the line `<count>
   constraints left` with the number of constraints remaining at that moment.

The loop ends when no constraint remains (`L − 1` choices when every region touches the others
through a chain; with `-v`, `Did while loop <count> times`). **The reference region** is the one
with the largest total `Σ N` over its constraints of §4.7 (before any merge; the totals are exact
integers), the first (smallest number) among equal; region 1 when there are no constraints. The
final offset of region `l` is `o_l = fl(fl(D_l) − fl(D_ref))` (single precision), and the
**unwrapped phase** of each voxel of region `l` is `u = fl(φ + 2π × o_l)` (`2π × o_l` in double,
the sum in double, then single), 0 at every voxel without a region (outside the mask). A mask
without regions gives an all-zero result (`-v`: `Calculating starting matrices (0 by 0)`,
`Finished connection_matrices`, `Did while loop 0 times`) **(observed:** `-t 1e9`**)**; a single
region gives the phase itself (offset 0) **(observed:** a one-voxel mask**)**. Should regions not
all connect (only when FSL-269's correction cuts the mask, §9), each group keeps its own offsets,
all taken relative to the reference region.

### 4.9 The slices mode (`-s`)

Each slice `z` is labelled (§4.6, its own numbers) and unwrapped on its own as in §4.7 and §4.8
(its own constraints and its own reference region), giving `u(·, z)` (0 outside its regions);
with `-v`, `SLICE NUMBER <z>` before each slice's lines (`Calculating starting matrices (<L_z> by
<L_z>)`, ...) **(observed)**. Then the slices are joined, upward: for `z = 0, ..., nz − 2` in turn,
over the in-plane positions (§3.1's order) that have a region in both slice `z` and slice `z + 1`,
the differences `fl(u(·, z + 1) − u(·, z))` (with slice `z` as already joined); if there is at
least one: sort them ascending, take the value at the 0-based position `⌊count / 2⌋` (the upper
median for an even count), `m = rnd(median / 2π)` (double), and every voxel with a region in slice
`z + 1` becomes `fl(u − m × 2π)` (`m × 2π` and the difference in double). A slice with no common
position with the slice below keeps its own offsets. (The order of `+0` and `−0` in the sort does
not matter: either gives `m = 0`.)

### 4.10 The volume's results

In order, for each processed volume: the mask (§4.4), the ramps removed (§4.5), the labels (§4.6),
the unwrapping (§4.8 or §4.9), the ramps restored (§4.5); the unwrapped volume joins the output
series; the phase as left by §4.5 replaces the volume in the series `-r` writes.

### 4.11 Messages of `-v` (standard output)

Each line is written out as it is printed (so the lines before an abort are kept, **observed**).
In order:
1. `Loading volumes`, `Phase loaded`, `Magnitude loaded` (not with `-c`), `Mask loaded` (with
   `-m`);
2. for each processed volume whose values are rewrapped (§4.2): `Rewrapping phase range to
   [-pi,pi]`;
3. for each processed volume: §4.5's ramp lines (with `--removeramps`), then `Number of phase
   splits = <n>`, then `Calculating starting matrices (<L> by <L>)` (`L` the largest region
   number), `Finished connection_matrices`, §4.8's `<count> constraints left` lines and `Did while
   loop <count> times` (in the slices mode, these for every slice after its `SLICE NUMBER <z>`),
   then, with `--removeramps`, `Removed linear ramps`.

## 5. Messages and exit statuses (summary)

| Text | Stream | Exit |
|---|---|---|
| the usage (§2.3) | stderr | 1 |
| the parser's messages after the usage (§2.4) | stderr | 1 |
| the compulsory-options block, then the usage | stderr | 1 |
| `Cannot specify both --complex AND --phase or --abs.`, `Both --phase AND --abs must be used together.`, `Always set --numphasesplit greater than 1.` + `NOT <n>` (after the usage) | stderr | 1 |
| the image layer's `Image Exception` lines and the abort (§3.1, §4.1) | stderr | 134 (larmorx: 1) |
| `terminate called after throwing an instance of 'NiftiIO::NiftiException'` (a file that cannot be written, §3.3) | stderr | 134 (larmorx: 1) |
| `ERROR: input phase image exceeds allowable phase range.` ... `Aborting.` (§4.2) | stderr | 255 |
| `FSLOUTPUTTYPE`'s messages (`specs/bet.md` §3.1), at the first write | stderr | 1 |
| `-v`'s lines (§4.11) | stdout | — |
| larmorx's refusals (§9), `larmorx: error: prelude: <what> (FSL-<n>)` (the replica: `larmorx-nc: error: prelude: ...`) | stderr | 1 |
| larmorx's notices, `larmorx: upstream defect FSL-<n> (prelude) corrected: ...` (`reproduced:` with `reproduce`), and GLIBC-07's (§6.4) | stderr | — |

## 6. Numeric conventions that decide the bits

### 6.1 Precision

Values are IEEE single (`float`) or double precision as each formula says; `fl` rounds a double to
the nearest float (ties to even). Every value read is single precision (§3); every image written
is float32 except the labels (int32). No fused multiply-add (each product rounded as written); no
reassociation of the sums of §4.5 and §4.7 (each in its stated order).

### 6.2 Rounding to an integer, conversions

- `rnd(x)` (§4.7, §4.8, §4.9, §6.3): half away from zero: `trunc(x + 0.5)` for `x > 0`, else
  `trunc(x − 0.5)`, the addition in double (exact for a float `x`), `trunc` C's conversion to a
  32-bit integer toward zero; a NaN or a value outside the 32-bit range gives `−2³¹` (x86-64's
  conversion, on every platform).
- A 32-bit integer converted to single precision (`fl(D_l)`, `rnd(K)` in `C`, `k` in §6.3) is
  rounded to nearest (exact below `2²⁴`); to double, exact.
- Every NaN made from numbers is x86-64's default NaN (sign bit set, quiet) on every platform.

### 6.3 `wrap`

`wrap(φ)` of a single-precision `φ`: `k = rnd(fl(φ × c₁))`, `wrap(φ) = fl(φ − fl(fl(k) × c₂))`,
with `c₁ = fl(1 / (2π))` (`0.15915494f`) and `c₂ = fl(2π)` (`6.2831855f`), every operation in
single precision. It returns a value in about `[-π, π]` (a float slightly beyond π is possible).
Where a double is wrapped (§4.5's removal) it is first rounded to single precision.

### 6.4 The arctangent (`-c`)

FSL's binary calls glibc 2.35's `atan2f`, fdlibm's single-precision algorithm, which is not
correctly rounded. The original follows the main package's rule (CLAUDE.md rule 5;
`larmorx_core::math::choice`'s `Libm`, the user's decision of 2026-11-04): by default the
correctly rounded arctangent (`Libm::atan2f`: the correctly rounded double `atan2` of the two
values, rounded to single precision), with `reproduce` glibc's bits (`atan2f_glibc`, a translation
of fdlibm's under Sun's notice, permissive); prelude's phase is a pointwise computation, so the
witness applies: GLIBC-07 is named, in both modes, where the two arctangents differ on a voxel.
(The replica uses glibc's `atan2f` in both modes, so its default and the original's default may
differ on `-c` inputs in the last bit of a few phases, and through them in a region's numbering;
§11's thresholds.)

### 6.5 Sorts

Two sorts decide values: §4.4's percentiles and §4.9's medians. Without NaN values any correct
ascending sort gives the same values at every position (equal values are equal bits, except `±0`,
whose order changes nothing, §4.4, §4.9). With NaN values (FSL-268, FSL-269 with `reproduce`
only) FSL's result is the order libstdc++'s `std::sort` leaves (its introsort: median-of-three
pivots moved to the front, an unguarded partition, the larger part looped and the other recursed
into while more than 16 values remain, a heap sort when the depth `2⌊log₂ n⌋` runs out, then an
insertion sort guarded over the first 16 values and unguarded after; the main package's clean-room
FLIRT has an implementation of it from its documentation, `std_sort_checked` in
`crates/larmorx-fsl/src/flirt/schedule.rs`); where its unguarded scans would read outside the
values, FSL reads memory it does not own: such a case is refused in both modes (§9).

### 6.6 Constants

`π` is the double nearest π (`M_PI`); `2π`, `3π`, `1.001 π` are the doubles `2 × π`, `3 × π`,
`1.001 × π`; `w = fl(2π / n)`; `0.02f`, `0.98f`, `c₁`, `c₂` as above; `0.1` and `0.5` doubles.

## 7. Python

`lx.fsl.prelude(phase=None, magnitude=None, *, complex_image=None, mask=None, threshold=None,
num_phase_split=8, slices=False, label_slices=False, force_3d=False, start=None, end=None,
remove_ramps=False, save_mask=False, save_labels=False, save_raw_phase=False, verbose=False,
implementation="auto", upstream_defects="fix", n_threads=1) -> PreludeResult(unwrapped, mask,
labels, raw_phase, stdout)` (`docs/api/fsl-prelude.md`, the option-mapping table): the same
wrapper for both implementations (`implementation="original"` runs the original in the process).
Inputs are paths or in-memory images; the wrapper checks its arguments first (`ValueError`:
`complex_image` or `phase` and `magnitude` together, `num_phase_split ≥ 2`, `start` and `end` not
negative); a failed run raises `ReplicaError` with prelude's messages (the original: the
family's error type with the same messages).

## 8. Data types and formats (summary)

- Inputs `-p`, `-a`, `-m`: every real NIfTI-1, NIfTI-2 and ANALYZE datatype, any of the formats
  and compressions of `specs/bet.md` §3.1, 3D or 4D, scaled to float; not complex (§3.1).
- Input `-c`: complex64 (both byte orders) and complex128 by default; any other datatype refused
  (FSL-263); the data unscaled (§3.2).
- Outputs: §3.3 (float32; the labels int32), every `FSLOUTPUTTYPE` FSL 6.0.7.23 accepts.

## 9. Defects (D13): what the default does

### 9.1 The register's entries

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` | Notice |
|---|---|---|---|---|
| FSL-261 | `--removeramps`' estimate, removal and restoration leave out the voxels of the last column, row and slice (§4.5's domain): no difference starts there (a voxel of the last column still ends the x differences of its row), their ramp is never removed, so they are unwrapped with it, and never restored; an image of one slice gets no ramp removed | every voxel (rule A) | FSL's domain | where the correction changes anything the run writes or prints (compute FSL's too when a mask voxel lies in the last column, row or slice: it need not change the result) |
| FSL-262 | `--savemask` and `-l` with several volumes write each volume's to the same file: only the last remains | one series of every processed volume's (§3.3; rule A) | FSL's | whenever more than one volume is saved (both modes) |
| FSL-263 | `-c` with an image that is not complex takes its data bytes as single-precision floats (right only for float32, unscaled; an image of a narrower type is read past its data) and every imaginary part from memory never written | refused before anything is written: `the complex input <name> is a <type> image, not a complex one` (rule B; `<type>` `uint8`, `int16`, `int32`, `float32`, `float64`, `int8`, `uint16`, `uint32`, `int64`, `uint64`, `complex256`, else `datatype <code>`) | the data, as stored and decoded in its own byte order, taken as consecutive little-endian float32 values; values beyond the data and every imaginary part 0 | reproduce: the values from beyond the data and the imaginary parts named |
| FSL-264 | `-c` with a complex64 image of the other byte order: each complex value's 8 bytes are swapped as one, so the real and imaginary parts change places | each float swapped on its own (rule A) | the parts exchanged | whenever such a file is read (both modes) |
| FSL-265 | `-c` with a complex128 image: FSL reads none of its values (an empty buffer, every value from memory past it) | read, each double converted to float (rule A) | every value 0 | whenever such a file is read (both modes) |
| FSL-266 | `-v` with `--removeramps` writes the ramp-free phase as `grot` (`FSLOUTPUTTYPE`'s extension) in the working directory, replacing any file of that name | not written (rule A) | written (§3.3) | whenever `-v` and `--removeramps` are given (both modes) |
| FSL-267 | the range message says `Allowable range is 6.283 radians` where the check is `3π`; `Removed linear ramps` is printed again after the restoration | kept as FSL prints it (a message only; no switch) | — | — |
| FSL-268 | NaN values of the source (§4.4) enter the default threshold's sort (their places undefined) and, as the first value, make the maximum NaN, so the mask is empty and the result all zeros | NaN values left out of the threshold's values (`N` the count of the others; none: `θ = 0`) and of the maximum (`top` the largest other value, 0 when none) (rule A) | FSL's rules (§4.2's extrema, the NaN values in the sort, §6.5); a sort that would read outside its values refused | where NaN values are present and FSL's mask and the corrected one differ (both computed) |
| FSL-269 | NaN phases inside the mask are labelled (interval 1), their constraints get NaN `P`, `K` and `C`, are chosen by §4.8's rule, and their `rnd(NaN) = −2³¹` turns move a region labelled with a NaN voxel, or merged through a NaN constraint, by about `2³¹` turns (about `1.35 × 10¹⁰` rad); in the slices mode NaN differences enter §4.9's sort | the NaN voxels left out of the mask (before the ramps; the saved mask without them, not reduced to a component again), the unwrapped phase NaN there (the raw phase keeps them) (rule A) | FSL's (§4.6, §4.8 with NaN arithmetic); a §4.9 sort with NaN values refused where it would read outside them | where the correction changes anything the run writes or prints (both computed; a NaN in a one-voxel mask changes only `-v`'s `(1 by 1)`) |
| FSL-270 | `--end` before `--start` processes no volume and writes no unwrapped phase, yet exits 0 (`-r` written) | refused: `no volume to process: --end (<e>) is before --start (<s>)`, after the loading lines and FSL's own exception for a `--start` outside the series, before §4.2, nothing written (rule B) | FSL's | reproduce: no volume processed, no output written |
| FSL-271 | a magnitude (or, with `-m`, a mask image) of another spatial size than the phase is not refused: the mask, made on the source's own grid, is read at the phase's voxel indices: a coordinate one voxel beyond the source's grid is moved onto its edge, and a point then inside reads that voxel, any other point outside reads 0 | refused: `the magnitude image has another size than the phase image` (with `-m`: `the mask image has ...`), after §4.2 and §4.3, before any volume (only when a volume is processed; with `-m` the magnitude's size is not compared; the number of volumes is not compared: §4.4) (rule B) | FSL's reading | reproduce: named |
| FSL-272 | §4.2's extrema start at the first value, so a NaN first phase value makes both NaN: the `3π` range check and the `1.001 π` rewrap are both skipped (a phase from 0 to 2π is unwrapped without the rewrap, and some regions come out whole cycles from where they come without the NaN; a phase of any range is processed) | the extrema of the values that are not NaN (§4.2): the range error and its status 255 and the rewrap as without the NaN (rule D measured, then A) | FSL's | when the two extrema decide differently (the range error, the rewrap or neither), both modes: `volume <n>: its first phase value is NaN: the rewrap to [-pi,pi] taken over its other values (prelude skips it)` (`the range check ...`; with `reproduce` `... skipped, as in prelude`) |

Refusals print `larmorx: error: prelude: <what> (FSL-<n>)` on standard error (the replica:
`larmorx-nc: error: prelude: ...`) and exit 1; notices print `larmorx: upstream defect FSL-<n>
(prelude) corrected: <detail> [LARMORX_UPSTREAM_DEFECTS=reproduce gives prelude's result]` (or
`reproduced: ...`), after the run's own messages, and in Python an `UpstreamDefectWarning`. The
refusals of a sort that would read outside its values (FSL-268, FSL-269 with `reproduce`) say
`... (FSL-26x: FSL reads memory it does not own here)`.

### 9.2 Kept as FSL's (rule C, not registered)

- The automatic 2D labels for images larger than 64 voxels along x or y (Jenkinson 2003's design).
- The `3π` range check and the rewrap beyond `1.001 π` themselves (FSL's design). Their extrema's
  NaN rule is FSL-272 (§9.1), decided by rule D (spec writer, 2026-11-06) **(observed** with FSL's
  runs, `validation/investigations/prelude-spec/fsl272_measure.py`: fMRIPrep's seven prelude calls
  and a synthetic map, each phase moved to [0, 2π) with its first value NaN: FSL's unwrapped
  phase differs from its own result for the same map without the NaN in 7 of the 8, by whole
  cycles, in 0.1 % to 100 % of the mask's voxels, with up to five distinct offsets in one map
  (regions unwrapped against each other wrongly, up to 18.8 rad); the map rewrapped beforehand,
  which is what the default does, gives FSL's result without the NaN to 9.5e-7 rad (the rewrap's
  own rounding); a phase of range 4π with a NaN first value is processed, exit 0, where without the
  NaN FSL stops with its range error, 255**)**.
- A given `-m` mask cut down to its largest component; `-t` applied to it; its last volume reused.
- `-r`'s raw phase with the ramps removed and restored (§4.5).
- The complex input unscaled (§3.2); a magnitude series shorter than the phase's aborting (§4.4).
- `--verbose=<value>` setting the flag; the parser's rules (§2.1).
- The upper median of §4.9.

### 9.3 Not reachable through prelude

- §4.6's 2D numbering after a slice without regions (the numbers would restart): the mask is one
  3D-connected component.
- Regions that do not all connect: only after FSL-269's correction (§4.8's last sentence).

## 10. Built for speed

Where FSL spends its time: the merging. For every choice FSL scans every constraint for the
largest `C`, every region for the members of group `s`, and every region number for `s`'s
neighbours: `O(L²)` per volume (fMRIPrep's call on ds005454, 112 × 112 × 70 with 14,678 regions,
6.5 s; the replica, with an ordered set of constraints and per-region neighbour and member lists,
0.15 s with the same bits). The labels, the constraints and the masks are linear in the voxels.

What must stay the same, and what is free:
- **The choice sequence** of §4.8 decides the arithmetic of every fold, so it must be FSL's:
  largest `C`, then smallest pair (§4.8 step 1). Any structure that yields that sequence is
  allowed: a heap or an ordered set keyed by `(−C, a, b)` with lazy deletion of stale entries, an
  indexed priority queue, ... (`C` changes only for the constraints a fold rewrites). With
  `reproduce` and NaN values, the rule of step 1 itself (a NaN first constraint stays chosen; NaN
  elsewhere never replaces) must be followed.
- **The offsets** are integers: a weighted union-find (offsets relative to the group root, path
  compression) gives the same `D_l` as moving every member (two's-complement sums are
  associative). Folding `s`'s constraints into `r`'s may iterate `s`'s neighbours in any order
  (§4.8 step 3), but always folds `s` into `r` (the arithmetic is not symmetric: `P` seen from
  `r`, the `− (2π N_st) k` term).
- **The sums `P`** (§4.7) are single-precision sums in the visiting order, per pair: they may be
  collected in parallel (slabs of slices, each slab's contributions per pair kept in order and
  concatenated in slab order), but each pair's sum must be accumulated in that order.
  The counts `N` and the totals of §4.8 are exact integers (any order).
- **The regions** (§4.6) and the mask's components (§4.4): any connected-component method
  (parallel union-find included), as long as the numbers follow the first voxels' order and the
  ties of §4.4 fall the same way.
- **Volumes** of a series are independent (§4.1 to §4.10), so they may run in parallel; the
  messages and outputs keep the volume order. In the slices mode the slices' unwrapping is
  independent; only the joining (§4.9) is sequential.
- **Memory**: the peak at most 10 % above FSL's plus 8 MiB for the same call (CLAUDE.md; FSL holds
  the phase and magnitude series in float, the mask and the labels of one volume, the constraints
  and the output series). `tests/parity/test_memory_peak.py`'s prelude calls (fMRIPrep's
  ds005454 call as it ran, and with `-s --removeramps --savemask -l -r`) are the model.

## 11. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23/bin/prelude`), through `python -m larmorx_validation parity`:
the replica's suite `fsl-prelude-replica` (`docs/validation/fsl-prelude-replica.md`, 86 cases on
the standard tier) is the model; the original's suite runs the same cases with
`implementation="original"` and the oracle cache's stored FSL runs (`use` mode).
- **Cases** (the record's categories): fMRIPrep 25.2.6's seven calls (sdcflows' phase-difference
  workflow: ds005454-sub-16, 112 × 112 × 70; ds002603-sub-01, 128 × 128 × 128; ds002422-sub-02's
  two estimators, 64 × 64 × 24 and 64 × 64 × 36; ds001600-sub-1's three, 64 × 64 × 44; the inputs
  in each run's `work/.../phdiff_wf/prelude/` node: `command.txt`, the magnitude, the mask and the
  phase difference; checked also against fMRIPrep's own outputs); the data panel's two other
  phase-difference maps (ds000256 sub-CTS203, ds004958 sub-fsm64ps: sdcflows' `au2rads` input,
  prelude's own mask, `-s` and the ramps with every output, the scanner's raw units: the range
  error); every option of §2.2 (the three modes, `-n` 2, 3, 24, `-t`, `-t 0`, `-m` with and without
  `-t`, series with `--start`/`--end` and a shorter mask series, every output, the ramps in every
  mode and on one slice, one slice, a neurological image, an int16 phase with `scl_slope`, a mask
  with a gap, complex64 in both byte orders, complex128, float32 and int16 given to `-c`, NaN
  magnitudes and phases (first voxels, inside a region, `-s`), long names, `-u` and grouped
  flags); every output type; every message and failure of §2.4, §3, §4.1, §4.2 (the errors'
  category); FSL-261 to FSL-272 in both modes.
- **Random testing**: the descriptor `fsl-prelude` (`validation/src/larmorx_validation/difftest/
  tools/fsl_prelude.py`: wrapped smooth phases over a head phantom with a random background, complex
  images, every option, series, the defects' inputs) with the original as the implementation under
  test, a short trial (`difftest --tools fsl-prelude --budget 0.5`, boxed), every difference
  triaged.
- **Thresholds**, each case against FSL's run:
  - exit status, standard output and error (the run directory and FSL's program path replaced;
    larmorx's own lines left out), the files written: identical;
  - with `reproduce`: every file bit-identical (images decompressed; `descrip` left out), on every
    case, `-c` included (glibc's `atan2f`, §6.4);
  - by default, against the replica's default run (bit-identical to FSL wherever no defect is
    named): every file bit-identical, except on `-c` inputs where GLIBC-07 is named; there the
    phases differ by a unit or two in the last place on the named voxels, and the unwrapped phase must
    equal `φ + 2π o` with the original's own phase `φ` and FSL's integer offsets `o` on at least
    99.9 % of the mask's voxels (a changed interval may move a region boundary), the others
    reported with their count;
  - the D13 pass: each defect named exactly where the default's outputs differ from
    `reproduce`'s (§9).
  No tolerance is needed elsewhere: a difference is a mistake of the original (a sum's order, a
  tie, a rounding of §6) and is fixed.
- **Speed**: against FSL and the replica on fMRIPrep's seven calls (one thread, and the
  original's threads), reported, not gated; the memory call of §10 within its allowance.
- **Black-box facts**: `validation/investigations/prelude-spec/blackbox.py` run against FSL and
  against the original gives two reports to compare line for line (messages, statuses, files,
  headers); the parts where larmorx differs by design are the aborts (exit 1 and larmorx's line)
  and the defaults of §9.

## 12. How this text was checked (spec writer, 2026-11-06)

`validation/investigations/prelude-spec/model.py` models §4 and §6 as written here (numpy and
scipy; no FSL code; the merging a plain heap keyed by `(−C, a, b)`), and compares its files and
`-v` lines with FSL's, bit for bit:
- the 22 cases of `blackbox.py` that write an unwrapped phase from real-valued inputs (3D, the
  automatic and the asked-for 2D labels, `-s`, the ramps, series with `--start`, a mask series, a
  one-voxel and an empty mask, `-n 0x3`, a NaN first phase voxel with a 0-to-2π phase and with a
  12.6 rad range, a 70-voxel-wide image with and without `-f`): **22 of 22** identical in every
  file (`-o`, `--savemask`, `-l`, `-r`) and every `-v` line; with `--fix` against the replica's
  default (FSL-261's full domain, FSL-262's series): 22 of 22;
- fMRIPrep 25.2.6's seven calls (§11): **7 of 7** bit-identical to fMRIPrep's own unwrapped phase
  (ds005454: 14,678 regions).

Not checked by the model (the replica's suite checks them against FSL): complex input (§3.2,
§6.4), NaN values inside the mask (FSL-268, FSL-269), the readers of FSL-263 to FSL-265,
FSL-271's reading.
