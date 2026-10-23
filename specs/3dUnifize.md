# Spec: `lx.afni.unifize` (behaviour of AFNI 3dUnifize 25.2.09)

The input for a clean-room **original** of larmorx's intensity uniformization (to be written in
`crates/larmorx-afni`, Apache-2.0). AFNI's 3dUnifize program file is NIH public domain, but its
results depend on library routines copyrighted by the Medical College of Wisconsin (GPL-2: the
neighbourhood ball, the sorts and medians, the maximum, the sub-volumes and conversions of the
automask it computes; `docs/licensing.md`, and the licence table of
`docs/findings/afni-unifize.md`). So larmorx has, or will have, two implementations (CLAUDE.md
rule 1):
- a bit-exact replica of AFNI's code, in the separate `larmorx-gpl` package (done:
  `larmorx-gpl afni 3dUnifize`, `docs/validation/afni-unifize.md`);
- the original, in the main package, written from this file, AFNI's documentation and black-box
  runs of the AFNI binary only (`oracles/afni-25.2.09/bin/3dUnifize`, built with OpenMP by
  `scripts/build_afni_oracle.sh --add 3dUnifize`).

**Whoever writes the original must not open** AFNI's source, the replica crates (`crates-gpl/`)
or the findings written from the source (`docs/findings/afni-unifize.md`, the restricted
sections of the defect register). It may read `specs/3dAutomask.md` (the automask and the clip
level used here are 3dAutomask's).

This spec describes behaviour and the numeric conventions that decide the bits; it holds no
code. **(observed)** marks what black-box runs confirmed.

**Defects (D13).** Corrected by default, reproduced with `upstream_defects="reproduce"`: AFNI-45
(`-T2up` at or near 100, §5), AFNI-47 (the half-size grid of a volume with an axis of 2 or 3
voxels, §4.2), AFNI-01 (a negative `scl_slope` in `-EPI`'s median, §7); refused in both modes:
AFNI-40 (`-T2up` without its value, §8.2), AFNI-46 (`-T2` on a 2D image smaller than 16 × 16,
§5); kept as AFNI's until the user decides: AFNI-48 (`-T2` three times, §6); and those of AFNI's
dataset reading and writing (`specs/3dTshift.md` §6, §7.1). §9 lists the defaults.

---

## 1. Purpose

Make the white-matter intensity of a T1-weighted volume uniform across space, scaled to about
1000: a local estimate of the white-matter intensity (WMI) at each voxel divides the image.
With `-T2` the contrast is inverted first and back after; with `-EPI` the scaling of an EPI
series' median volume is applied to every volume. fMRIPrep uses it on the BOLD reference:
`3dUnifize -clfrac 0.2 -rbt 18.3 65.0 90.0 -T2 -prefix uni.nii.gz -input <masked reference>`.

## 2. Inputs and outputs

- **Input:** a NIfTI dataset, read as 3dAutomask reads it (`specs/3dAutomask.md` §2). Without
  `-EPI`, only sub-brick 0 is used, as floats: each stored value as float, non-finite values 0,
  then times the brick factor when it is neither 0 nor 1 (negative included).
- **Output** (`-prefix`): float32, one volume, no time axis, `scl_slope` 0, the input's
  geometry. With `-EPI`: every volume, float32, the input's time axis kept, `scl_slope` 0.
  Without `-prefix`, AFNI writes `Unifized` in its own format (larmorx refuses: NIfTI only).
- `-ssave s`: the white-matter scale factor of each voxel (`1000 / WMI`, or 0), float32, one
  volume. `-amsave a`: the automasked input of §4.1, float32, one volume. Neither carries the
  `-GM` scaling.

## 3. Parameters

| Parameter | Default | Option |
|---|---|---|
| radius `R` (voxels) | 18.3 | `-Urad R` (5 to 99), `-rbt R b t` |
| percentiles `b`, `t` | 70, 80 | `-rbt R b t` (30 ≤ b ≤ 80, b < t ≤ 90) |
| clip level fraction | 0.2 | `-clfrac`, `-mfrac` (0.1 to 0.9; the help says 0.1: AFNI-49, its text only) |
| `-T2` count | 0 | `-T2` (each occurrence adds 1) |
| `-T2up` | 98.5 | `-T2up u` (90 to 100) |
| half-size ("duplo") | on above 1,000,000 voxels | `-noduplo`, `-nodouble` turn it off |
| squash | on | `-nosquash`, `-oink` |

All are read with C's `strtod` and rounded to float. The automask settings are 3dAutomask's
defaults (1 peel, threshold 17, gradual clip level) with the exterior clip (`-eclip`) **on**
and the fraction above.

## 4. The local white-matter intensity

### 4.1 The automasked image

`a` = the float image (a copy, non-finite values 0) with every voxel outside its automask
(`specs/3dAutomask.md` §5 for 3D, §7 for 2D, with the settings of §3 here) set to 0. When the
automask does not exist (a 2D image smaller than 16 × 16, or nothing above the clip level of a
2D image), there is no result (§8.3's fatal error) (observed for 12 × 12).

### 4.2 The half-size image (duplo)

Used when on and the volume has more than 1,000,000 voxels (never with `-EPI`). The half grid
has `max(1, n / 2)` voxels per axis (integer division). Each half voxel `(i, j, k)` is the
**median of 7** values of `a`: at `(2i, 2j, 2k)` and its 6 face neighbours (indices clamped to
the grid), taken in the order centre, x−, x+, y−, y+, z−, z+ and reduced by this sorting network
of compare-and-swap steps (`swap(p, q)` when `p > q`), whose result is the 4th value:
`(0,1) (4,5) (1,2) (5,6) (0,1) (4,5) (0,4) (2,6) (1,3) (3,5) (1,3) (2,3) (3,4) (2,3)`.
(Which value comes out decides the sign of a zero when −0 and +0 meet.)

The ball's radius is then `0.5f * R + 0.001f` (float).

AFNI-47: with an axis of 2 or 3 voxels, the half axis has 1 voxel, and §4.4 keeps it at 1: the
result has another size than the input. AFNI then reads it as if it had the input's size, past
its end: what it finds there depends on the memory (observed: on a 720 × 720 × 2 input AFNI
crashed with signal 11 in every run). The default expands it to the input's grid (§4.4 with the input's sizes); `reproduce`
uses AFNI's smaller image linearly and 0 past its end.

### 4.3 The local mean between percentiles

The ball: every integer offset `(di, dj, dk)` with `di² + dj² + dk² <= r²`, the squares and
their sums in float (exact for these integers) and `r²` the float product `r * r`, including
`(0, 0, 0)`; its half-width per axis is `trunc(r)`. For every voxel of the (half or full) grid,
whether or not it is in the mask:
1. the values of `a` at the ball's offsets that are inside the grid and **not 0**;
2. none: 0; one: that value; otherwise, with `n` values sorted ascending,
   `q1 = trunc(0.01f * b * (float)(n - 1))`, `q2 = trunc(0.01f * t * (float)(n - 1))` (float
   products, left to right: `(0.01f * b) * (n - 1)`), the float sum of the sorted values
   `q1 .. q2` in ascending order, divided (in float) by `(float)(q2 - q1) + 1.0f`.

The order of equal values does not matter, nor where −0 and +0 fall: a float sum starting at +0
gives the same bits. The per-voxel computation is independent of the others (AFNI runs it with
OpenMP; the result does not depend on the thread count) (observed: 1, 6 and 12 threads give the
same bits).

### 4.4 Back to full size

From the half grid `h` (sizes `m`) to the input's: an axis of 1 stays 1; otherwise `2m + 1` when
the input's size is odd, else `2m`. Output voxel `(i, j, k)`: per axis, `lo = i / 2` (clamped to
`m - 1`) and `hi = lo + 1` when `i` is odd (clamped to `m - 1`), else `hi = lo`; the value is
`0.125f * (s)`, `s` the float sum in this order of `h` at `(lo,lo,lo)`, `(hi,lo,lo)`,
`(lo,hi,lo)`, `(hi,hi,lo)`, `(lo,lo,hi)`, `(hi,lo,hi)`, `(lo,hi,hi)`, `(hi,hi,hi)` (x, y, z).

## 5. The T2 inversion (`-T2`, `-EPI`)

The mask of the inversion is the automask (`specs/3dAutomask.md`, with §3's settings) of the
float input before anything else. If it does not exist (a 2D image smaller than 16 × 16), AFNI
crashes (AFNI-46; larmorx refuses in both modes). To invert an image `x` (a float copy, non-finite
values 0):
1. nothing for an image of one row (`ny = 1`);
2. the values inside the mask, sorted ascending: `n` values; fewer than 100: nothing;
3. `U` = the sorted value at index `rintf((float)n * u * 0.01f)` (float products left to right,
   `rintf` half to even), `u` the `-T2up` value clamped to [90, 100]. At `u = 100`, and near it
   for small masks (`n (1 - u/100) < 0.5`), the index is `n`, one past the values: AFNI-45 —
   AFNI reads whatever lies there in memory (observed: 0 in its runs, which makes the output all
   zeros); the default uses the largest value; `reproduce` uses 0;
4. every voxel: inside the mask and positive: `U - x`; then outside the mask, or negative: 0.

## 6. The program

1. Read the input and the float image (§2), or with `-EPI` the median volume (§7).
2. With `-T2` (any count): `I`, the mask of §5, invert.
3. The white-matter scaling: `A`, §4.1; `-amsave` written here; with duplo `D`; `V`, §4.3
   (with progress dots, below); `U`; then for every voxel: `p = (WMI <= 0) ? 0 : 1000.0f / WMI`
   (float), the value times `p` (float), and with squash, a value above 1300 becomes
   `1300 + 200 * tanhf((v - 1300) / 200)` (float operations; `tanhf` **glibc 2.35's**, not
   correctly rounded on some arguments: the replica carries a port of it); `W`, and
   `[s<count>]` when any value was squashed. `-ssave` written here (the factors `p`).
4. `-EPI`: §7, then stop.
5. `-GM`: the grey-matter scaling (§6.1); `G` when it was applied.
6. `-T2` once: `I`, invert the result with the **same mask** (its `U` from the result's values);
   twice: no inversion; bright edges clipped (§6.2); three or more times: neither (AFNI-48: the
   output stays inverted; kept as AFNI's until decided) (observed).
7. The final mask: the automask of the result (§3's settings); `m`; every voxel outside it 0.
   (A 2D result smaller than 16 × 16 cannot reach this point.)
8. Write the result.

### 6.1 `-GM`

On the WM-scaled image `g`: `P` = the median of the values above 1000 (fewer than 111: nothing
happens); `top = 1000 - 1.987654321f * (P - 1000)` (float); `low` = the clip level of `g`
(`specs/3dAutomask.md` §4) for fraction 0.4321; `G` = the median of the values in `[low, top]`
(fewer than 111: nothing); `f = (1000 - 666) / (1000 - G)` (float); `low *= 0.333f`; every value
`>= low` becomes `f * (v - 1000) + 1000` (float), 0 if negative; the others 0.

The medians are AFNI's `qmed_float`: for an odd count the middle value; for an even count
`(float)(0.5 * (double)(x + y))` with `x + y` the float sum of the two middle values; computed by
a partial quicksort whose arithmetic is only that sum, so any exact selection of the two middle
values gives the same bits (ties aside: equal values are equal). Counts 2, 3, 4, 5, 7 and 9 have
special networks with the same results.

### 6.2 Edge clipping (`-T2 -T2`)

With `u1 = 1000.0f * 1.111f` and `u2 = 1000.0f * 1.055f` (float products): repeat until a pass
clips nothing: on a copy `c` of the image at the start of the pass, every voxel with
`c >= u1` and more than 2 of its 6 face neighbours (indices clamped: an edge voxel is its own
neighbour) equal to 0 in `c` becomes 0; then every voxel with `c >= u2` and more than 4 such
neighbours becomes 0. `e` is printed once, before the first pass.

## 7. `-EPI`

- Implies `-T2` (once), `-noduplo`, and turns off `-GM` (`++ -EPI turns off -GM`). The radius
  becomes `u = cbrtf(|dx * dy * dz|)` (the float product of the voxel sizes; `cbrtf` glibc's)
  then `(float)(18.3 / u)` (double quotient), at least 4.14: `++ -EPI changes -Urad to %.1f
  voxels`, whatever `-Urad` or `-rbt` gave.
- A dataset of one volume: `*+ WARNING: -EPI was used, but only 1 volume in the input dataset`,
  and the program goes on as without `-EPI` but with `-T2`, `-noduplo` and the new radius
  (observed).
- The median volume: at each voxel its series as floats (non-finite values 0), each value times
  its sub-brick's factor when that factor is **positive** (AFNI-01: a negative factor is dropped,
  so the median of a negatively scaled series has the wrong sign and the output is all zeros —
  observed; the default applies it), then the median (§6.1's rule) of the `n` values.
- After §6 step 3 (computed on the median), every volume `v` (the float image of that sub-brick,
  §2): invert (§5, the mask of the median), multiply voxel by voxel by the factors `p`, invert
  again (the same mask; its `U` from this volume's values). `E` before, `.` for every volume
  whose index is 47 modulo 100 (printed also with `-quiet`), and a newline after.
- The output keeps the input's time axis and slice timing; no final mask, no `-GM`, no elapsed
  time.

## 8. The command line (`larmorx afni 3dUnifize [options] dataset`)

### 8.1 Options

Scanning stops at the first argument not starting with `-`; that argument is the dataset
(unless `-input` gave it).

| Option | Rule |
|---|---|
| `-prefix p`, `-ssave s`, `-amsave a` | a name `THD_filename_ok` refuses: "Illegal value after -prefix!" (likewise) |
| `-input d`, `-inset d` | opens the dataset at once (its messages come here); a second time: "Can't use '-input' twice" |
| `-Urad R` | outside [5, 99]: "Illegal value %f after option -Urad" |
| `-rbt R b t`, `-param R b t` | fewer than 3 values after it: "Need 3 arguments (R pb pt) after '-rbt'"; out of range: "Illegal values (R pb pt) after '-rbt'" |
| `-clfrac c`, `-mfrac c` | outside [0.1, 0.9]: "-clfrac value %f is illegal!" |
| `-T2up u` | outside [90, 100]: "-T2up value is out of range 90..100 :-(" |
| `-useall` | `*+ WARNING: -useall option is disabled in this version` |
| `-EPI`, `-GM`, `-nosquash`, `-oink`, `-T2`, `-T2up`, `-quiet`, `-verb` | **case-insensitive** (observed: `-t2`, `-gm`, `-NOSQUASH`) |
| `-noduplo`, `-nodouble` | case-sensitive |

Every option taking a value checks that it is there ("Need argument after '-prefix'"), except
`-T2up`: as the last argument AFNI crashes (AFNI-40; larmorx refuses). Unknown options: "Unknown
option: <arg>". No dataset: "No dataset name on command line?"; one that cannot be read: "Can't
open dataset '<name>'". All fatal (exit 1).

### 8.2 Messages

On standard error (observed, byte for byte): AFNI's banner (not printed by larmorx); with
`-EPI`, its two lines (§7); unless `-quiet`, the progress line: ` + Pre-processing: ` followed
by the letters of §6 and §7 (`M`, `I`, `A`, `D`, `V`, dots, `U`, `W`, `[s<n>]`, `G`, `I`, `e`,
`m`) and a newline. A warning during it (`Only processing sub-brick #0 (out of <n>)` for a 4D
input without `-EPI`) is printed right after ` + Pre-processing: ` on the same line. The dots of
§4.3: one for every grid voxel whose storage index is a multiple of 66,666 and that has at least
two values in its ball, printed after `V`. With `-verb` (twice or more), the automask's own
messages (`specs/3dAutomask.md` §8.3, the ` + ` lines) appear among the letters, where each
automask runs. Then `++ Output dataset ./<name>` (also with `-quiet`; not with
`AFNI_QUIET_STARTUP`), and unless `-quiet`, `++ ===== Elapsed = %.1f sec` (the elapsed time of
the run; AFNI 25.2.09 on Linux prints only this, its CPU clock reading 0).

### 8.3 Failures

- No automask for the WM scaling (§4.1): unless `-quiet` a newline, then "Can't compute
  Unifize-d dataset for some reason :-(" (with `-EPI`: "Can't process EPI data: scale image
  missing :("), exit 1.

### 8.4 AFNI's global options and environment variables

As for 3dAutomask (`specs/3dAutomask.md` §8.4). The thread count of AFNI's OpenMP loop comes
from `OMP_NUM_THREADS` (read before the global options, so a `-D` cannot set it; at most 15 when
it is unset); it changes no result.

## 9. Defects and what larmorx does by default

| Defect | AFNI | larmorx's default | `reproduce` |
|---|---|---|---|
| AFNI-01 | `-EPI`'s median drops a negative factor (§7) | applied; named | AFNI's |
| AFNI-40 | `-T2up` as the last argument: a crash | a fatal error, exit 1 | the same |
| AFNI-45 | `-T2up` near 100: reads past the sorted values (§5) | the largest value; named | 0; named |
| AFNI-46 | `-T2` on a 2D image smaller than 16 × 16: a crash | a fatal error, exit 1 | the same |
| AFNI-47 | the half-size grid of an axis of 2 or 3 voxels (§4.2) | expanded to the input's grid; named | AFNI's reading, 0 past the end; named |
| AFNI-48 | `-T2` three times: no re-inversion, no edge clipping | AFNI's (to be decided) | AFNI's |
| AFNI-49 | the help's `-clfrac` default (0.1; the code uses 0.2) | – (text) | – |
| AFNI-02, -07, -11, -12, -16, -18, -20, -22, -23, -26 | dataset reading and writing | as for 3dAutomask | AFNI's |

## 10. Validation

- **Bit-identity** of every output (data and header), exit status and messages with AFNI's
  binary on the parity cases of `validation/.../parity/afni_unifize.py` (phantoms for every
  option, type, geometry and error; fMRIPrep's call on every oracle run's BOLD reference; T1w
  images in the default mode, with the half-size path; `-GM`; `-EPI` on BOLD series), in
  `reproduce` mode. The replica meets it (`docs/validation/afni-unifize.md`). The arithmetic is
  float, in the orders stated; `tanhf` and `cbrtf` must be glibc 2.35's for bit-identity (a
  correctly rounded `tanhf` changes the squashed values in the last bit where glibc's is not
  correctly rounded: about 2.8 % of float arguments).
- **AFNI's own variability**: the record measures OpenMP thread counts and fMRIPrep's build.
- **Speed**: the local mean (§4.3) dominates; it can be computed per voxel in parallel, and only
  the values between the two percentile ranks need sorting.
- Golden cases: the replica's (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`; the half-size path
  needs more than a million voxels, beyond the golden tests' 32³ limit, and is covered by parity
  only).
