# `mri_normalize` (FreeSurfer 8.2.0): behaviour spec

Status: complete (part 1, queue item 3.2-24: the options and the control-point steps; part 2,
3.2-25: the program's order of steps, its outputs and exit statuses, §3, and the validation plan
with the open probe, §4). Written
by the replica's author (track D) as the spec writer: behaviour, maths, conventions and messages,
for the clean room (`larmorx-fs-volumes`, queue item C3.2-*), which works from this file, the
library modules' specs (`specs/freesurfer-*.md`) and black-box runs of the official program only.

Conventions: volumes are FreeSurfer volumes (`specs/freesurfer-mri.md`, `specs/freesurfer-volume.md`);
"a control point" is a voxel of a control volume whose value is not 0; voxels are visited with
the column index outermost and the slice index innermost (x, then y, then z), and a step that
changes the control volume while it reads it sees its own earlier changes. Messages go to standard
output unless "standard error" is said; `%2.1f` and its kin are C's formats. `Gx, Gy, Gz` is the
diagnostic voxel (`-d`, default -1: none).

## 1. The command line

`mri_normalize [options] <input volume> <output volume>`: the version options first
(`specs/freesurfer-core.md`, the version report), then every argument that starts with `-` is an
option, read with the arguments after it (below), until the first argument that does not start
with `-`. With fewer than two arguments left the help is printed (§1.3) and the program ends with
status 0. Arguments after the two names are ignored.

### 1.1 How an option is matched

The option's name is the argument without its first `-`, compared without case. The names of the
table below are tried in its order; a name none of them matches is taken by its first letter
alone (upper-cased): `-foo` is `-f`, `-Dx` is `-d`. A letter that is none of the single letters
prints `unknown option <argument>` and ends the program with status 1. `--help` and `--usage`
(their second dash part of the name), `-u`, `-h`, `-?` (and any name starting with those letters
that matches no name) print the help and end with status 0.

### 1.2 The options

Each option prints its message (if any) when it is read; `<n>`, `<f>` are numbers read as C's
`atoi` and `atof` do (`12abc` is 12, `abc` is 0; `atof`'s value stored as a `float` where the
table says so).

| Option | Arguments | Sets | Message |
|---|---|---|---|
| `-no1d` | | no 1-D normalisation | `disabling 1d normalization...` |
| `-T1` | | T1 contrast | `assuming in vivo T1 contrast` |
| `-T2`, `-PD` | | T2 contrast | `assuming T2/PD contrast` |
| `-nonmax_suppress` | `<n>` | non-maximum suppression on (n not 0) or off | `using nonmaximum suppression` / `disabling nonmaximum suppression` |
| `-erode` | `<n>` | erosions | `eroding interior of surface <n> times` |
| `-renorm` | `<name>` | the renormalisation volume | `renormalizing using voxels that are 110 in <name>` |
| `-checknorm` | `<name> <f> <f>` | the check volume, its range (floats) | `removing control points that are not in [%2.0f %2.f] in <name>` |
| `-atlas` | `<atlas> <transform> <n>` | reads the atlas and transform at once; the brain distance | `reading gca and atlas transform from <atlas> and <transform>`, then `reading gca from <atlas>, xform from <transform> and setting brain distance to <n> voxels`; either unread: `<program>: could not read gca (<atlas>) or transform (<transform>)` (standard error, status 255) |
| `-C` | `<name>` | the output control-point volume | `Writing output control points volume to <name>` |
| `-mask` | `<name>` | the mask volume | `using MR volume <name> to mask input volume...` |
| `-noskull` | | skull stripped | `assuming input volume has been skull stripped` |
| `-grad` | `<f>` | the gradient threshold (float) | `using gradient volume thresholded at %2.1f to prevent control points from crossing edges...` |
| `-mask_sigma` | `<f> <f>` | the mask's smoothing and threshold (floats) | `smoothing input volume with sigma = %2.3f mm and thresholding at %2.0f for mask` |
| `-mask_orig` | `<name> <f>` | the original-volume mask and its threshold (float) | `removing control points that are below %2.3f in <name>` |
| `-surface` | `<surface> <transform>` | a surface (at most 10) with its transform, read at once (`identity.nofile` is the identity) | none; an 11th: `too many surfaces (10) specified` (status 254); an unread transform: `<program>:could not load xform from <transform>` (status 255) |
| `-min_dist` | `<f>` | the distance (double) | `retaining  nonmaximum suppressed points that are at least %2.3fmm from the boundary` (with suppression on; else `retaining  points ...`, two spaces either way) |
| `-interior` | `<surface> <surface>` | two white surfaces | `using surfaces <a> and <b> to compute wm interior` |
| `-mgh_mprage`, `-mprage` | | MGH MP-RAGE; intensity below 15 | `assuming input volume is MGH (Van der Kouwe) MP-RAGE` |
| `-washu_mprage` | | WashU MP-RAGE; intensity below 22 | `assuming input volume is WashU MP-RAGE (dark GM)` |
| `-monkey` | | no 1-D normalisation, one 3-D pass | `disabling 1D normalization and setting niter=1, make sure to use -f to specify control points` |
| `-nosnr`, `-snr` | | the SNR normalisation off, on | `disabling SNR normalization`, `enabling SNR normalization` |
| `-sigma` | `<f>` | the bias smoothing (float) | `using Gaussian smoothing of bias field, sigma=%2.3f` |
| `-no-gentle-cp`, `-gentle-cp` | | the gentle pass with control points off, on | none |
| `-conform`, `-noconform` | | conforming on, off | `interpolating and embedding volume to be 256^3...`, `not interpolating and embedding volume to be 256^3...` |
| `-aseg`, `-segmentation` | `<name>` | the segmentation | `using segmentation for initial intensity normalization` |
| `-gentle` | | the gentle 3-D passes | `performing kinder gentler normalization...` |
| `-label` | `<label>` | reads the label at once | `reading control points from label file <label>`; unread: `<program>: could not read label from <label>` (status 255) |
| `-lonly`, `-label_only`, `-labelonly` | `<label>` | as `-label`, and file points only, no 1-D normalisation | `only applying control points from label file <label>` |
| `-file_only`, `-fonly`, `-fileonly` | `<name>` | the control-point file, file points only, no 1-D normalisation | `using control points from file <name>...`, `only using file control points...` |
| `-seed` | `<n>` | the random seed (C's `atol`) | `setting seed for random number genererator to <atoi(n)>` |
| `-d` | `<n> <n> <n>` | the diagnostic voxel | `debugging voxel (x, y, z)` |
| `-v` | `<n> <n> <n>` | the second diagnostic voxel | `debugging alternative voxel (x, y, z)` |
| `-p` | `<n>` | pruning: on for n > 0; 0 is stored as -1 | `turning control point pruning on` (n > 0) / `off` |
| `-r` | `<control> <bias>` | read the bias and control volumes | `reading bias field from <bias> and ctrl points from <control>` |
| `-l` | `<control> <bias>` | longitudinal: those volumes, no 1-D normalisation | the same message |
| `-w` | `<control> <bias>` | write them | `writing ctrl pts to   <control>`, `writing bias field to <bias>` (three spaces) |
| `-f` | `<name>` | the control-point file | `using control points from file <name>...` |
| `-a`, `-b` | `<f>` | intensity above, below (floats; defaults 25, 10) | `using control point with intensity %2.1f above target.` / `below target.` |
| `-g` | `<f>` | the 1-D normalisation's largest gradient (float; default 1) | `using max gradient = %2.3f` |
| `-n` | `<n>` | the 3-D passes (default 2) | `performing 3d normalization <n> times` |

The defaults: contrast unknown, brain distance 1, no conforming, no gentle passes, not skull
stripped, SNR off, distance 2.5, check range 90 to 120, bias sigma 8, mask sigma and threshold 0,
original-mask threshold 150, scan type unknown, pruning 0 (neither on nor the stored off), 2
passes, intensities 25 above and 10 below, gradient threshold -1, the gentle pass with control
points on, suppression on, 0 erosions.

**FS-1016**: an option given fewer arguments than it takes (necessarily the last argument): where
the missing argument is a number, or a file the option reads at once (`-atlas`, `-surface`'s
transform, `-label`, `-lonly`, `-seed`), FreeSurfer crashes; both modes print the messages up to
there and end with `<program>: option <argument> needs more arguments` (standard error, status
1). A missing name the option only keeps prints as `(null)` (FreeSurfer's too, for the first
missing one); for an option of two or three arguments given none, FreeSurfer takes the ones after
the first from the process's environment: both modes use an empty argument (a number 0, a name
`(null)`), noted.

### 1.3 The help

The program's help text (`mri_normalize.help.xml`, FreeSurfer's, rendered as
`specs/freesurfer-core.md`'s help renderer does) on standard output.

## 2. The control-point steps

### 2.1 The bias of `-w` (the output over the input)

A new float volume of the input's extent and header: each voxel the output's value over the
input's (frame 0), or 1 where the input's value is within `FLT_EPSILON` of 0.

### 2.2 Removing white-matter outliers, keeping the medial surface (`-aseg`)

On the control volume C (the segmentation's white-matter labels 2 and 41 copied into a volume of
the segmentation's type) and the intensity volume V, with `b` = intensity below (as an `int`,
truncated):

1. C binarised at 1 (0 and 1); its signed distance transform (label 1, no distance limit) times
   -1; the non-maximum suppression of that (threshold 0, both directions); C replaced by the
   suppression binarised at 1 (0 and 1).
2. The histogram of V over C's label 1 (256 bins), smoothed with sigma 2 into a copy; the highest
   peak in bins 1 to the last; `peak` = that bin's value truncated to `int`; `thresh = peak - b`.
   **FS-1014**: with no control point the histogram has no peak, and FreeSurfer reads the value
   before the bins: 0 in practice; both modes use 0.
3. Each control point (its value rounded to the nearest integer not 0) is removed (set to 0) when
   `val + b` is below the largest value of V among C's label-1 voxels of the 11-voxel cube around
   it (as a `float`) and `val` is below `thresh`; C is that label volume, as it is at the time.
4. `<n> control points removed`. With the diagnostics' write bit (`Gdiag`) the removed points are
   written as 128 in a volume of C's geometry, `out.mgz` (`writing out.mgz outlier volume`); with
   the write and verbose bits C is written to `sc.mgz` before step 1 and `dc.mgz` after step 3,
   and the two histograms plotted to `h.plt` and `hs.plt`.

### 2.3 Removing white-matter outliers (`-l`)

As §2.2 without step 1, with the histogram and the cube's largest value taken over C binarised at
1 (0 and 1) before any removal (the removals do not change it); then `<n> control points removed
(<p>%)` with `p = 100 n / visited` as `%2.1f` (`-nan` when no point was visited).

### 2.4 Adding the white matter's interior (`-interior`)

The interiors of the two surfaces (each surface's filled interior at C's column size, into a volume
of C's geometry), the second's voxels of value 1 copied into the first; eroded once, then
`nint(1 / column size) - 1` more times: the core. Without a segmentation, each voxel of the core
above 0 becomes a control point of its value (truncated); with one, each voxel of C that is 0 and
whose segmentation label is cortex (3, 42) or white matter (`IS_WM`) becomes 1 when its intensity
is within `[110 - below, 110 + above]` (here `below` is 1.25 times the intensity below) and the
core holds it; every voxel of C is written back as its value truncated to `int`. With the write and
verbose bits the joined interiors are written to `i.mgz`.

### 2.5 Removing control points off the white matter (`-surface` with `-aseg`)

Each control point mapped into the segmentation's voxels (the voxel-to-voxel transform of the
two volumes' geometries, each coordinate rounded to the nearest): on a label of the 31 below it is
removed and counted; on any other it is removed when the 3-voxel cube around the mapped voxel
holds a left or right lateral ventricle (4, 43), else set to 1. Then `<n> non wm control points
removed`. The labels: 0, 4, 5, 8, 10, 11, 12, 13, 14, 15, 17, 18, 26, 31, 43, 44, 47, 49, 50, 51,
52, 53, 54, 58, 63, 77, 78, 79, 80, 81, 82. A point mapped outside the segmentation reads the
segmentation as `specs/freesurfer-mri.md`'s voxel reads do there.

### 2.6 The brain mask of `-atlas`

A byte volume of the input's geometry: 1 where the voxel, through the transform (it must be
voxel to voxel, or a morph; else the program ends with `GCAsourceVoxelToPrior: needs vox-to-vox
transform`, status 252) and the atlas's template geometry rounded to the prior grid, falls
outside the grid, or where none of the labels of that prior point is brain (`IS_BRAIN`); then
eroded `brain distance - 1` times.

### 2.7 Removing outliers of a T2-weighted volume (`-T2` with `-surface`)

On C, the distance volume D and the intensity volume V (one geometry): V's Sobel gradient
(3 frames) and magnitude; D's gradient and its non-maximum suppression (threshold 0, both
directions); written to `dg.mgz`, `g.mgz`, `m.mgz` (always, in the working folder); the
magnitude's non-maximum suppression at 2.5 (`nm1.mgz`), each of its voxels not within
`FLT_EPSILON` of 0 set to 0 where the dot product of V's and D's gradients is above 0 (`nm2.mgz`);
C written to `c1.mgz`. Then from each control point a walk: position p = the point (floats), its
voxel q; repeat: the walked distance grows by half D's column size; g = D's gradient at q; stop
(keep) when its length (`float`) is within `FLT_EPSILON` of 0; else p += g / |g| / 2, q = p
rounded; at an edge (the edited suppression above 0 at q) the point is removed and the walk stops;
else the walk stops (keeps) when the dot product of the start's gradient and g / |g| is at most 0,
or at a ridge of D (its suppression above 0 at q); after 10 mm it stops (keeps). `c2.mgz` at the
end. **FS-1015**: with a column size of 0 or NaN a walk that meets nothing never ends in
FreeSurfer; both modes end the program at 10,000,000 steps of one walk (`remove_surface_outliers_T2:
the walk from control point (x, y, z) along the distance gradient does not end (voxel size <s>)`,
status 1).

### 2.8 Thresholds

- The intensity threshold (10, after the surface steps): each control point whose intensity is
  below the threshold is removed (at the diagnostic voxel: `(x, y, z) - val %2.0f lower than
  threshold %2.0f - removing`).
- `-checknorm`'s range: over the check volume's extent, each control point (its value truncated
  to `int`, not 0) whose check-volume value is below the minimum or above the maximum is removed
  and counted (at the diagnostic voxel: `voxel(x, y, z) = %2.1f, removing due to intensity
  outlier`); then `<n> control points removed in intensity range checking`.
- Two steps of FreeSurfer's (an outlier test near the surface, and another of the surface's
  outliers) return before doing anything: they change nothing.

## 3. The program

The steps in order; each prints its messages on standard output when it runs. `<program>` is the
program's name as it was run (`argv[0]`: recon-all runs `mri_normalize`); an error ends the
program with the error code's low 8 bits as its status (`ERROR_NOFILE` -1: 255;
`ERROR_UNSUPPORTED` -3: 253; `ERROR_BADPARM` -4: 252), its message on standard error as
`specs/freesurfer-core.md`'s errors print it (`error: <message>`). The volume functions named
below are the library modules' (`specs/freesurfer-volume.md`, `freesurfer-filter.md`,
`freesurfer-io.md`); `110` is the target white-matter value.

### 3.1 The start

1. The command line is kept for the output's header (`specs/freesurfer-core.md`'s `getAllInfo`
   report); the version options are handled (with only version options, the program ends with
   status 0 after them); the diagnostics are read from the environment (`DIAG`, `DIAGX`, ...).
2. The options (§1); then fewer than two names: the help, status 0.
3. `reading mri_src from <input>...`; the input read (any format the readers take); unread:
   `<program>: could not open source file <input>` (status 255). The command line added to its
   header.
4. With `-atlas`: the mask of the voxels outside the brain (§2.6) from the input; the atlas and
   the transform are then freed. With `-lonly`/`-label`: the label's points are the control
   points to use (in the input's voxel coordinates).

### 3.2 Three programs of their own

- **`-renorm <volume>`**: the volume read (unread: `<program>: could not read renormalization
  file <volume>` and a newline, status 255); when its extent or voxel sizes differ from the
  input's (`MRImatchDimensions`: equal extents and sizes within `FLT_EPSILON`), `resampling
  renormalization volume`, resampled trilinearly onto the input (`r.mgz` written in the working
  folder). A uchar control volume of its geometry: its voxels equal to `renorm_val` (110 unless
  set) marked 1 (`c.mgz`), eroded `-erode` times (`e.mgz`), the file's or label's points added
  (with `-f` or a label); the bias field built from the input and those points (`b.mgz`); with a
  bias sigma above 0, `smoothing bias field with sigma=%2.3f` and the field smoothed by a
  Gaussian of that sigma (in voxels) along the three axes (`bs.mgz`); the input divided by the
  field and scaled to 110; `writing normalized volume to <output>`, written; status 0.
- **`-surface`** (one or more): `-f`'s file or the label's points first, then one gentle 3-D pass
  with those points only into a new volume (else `Copying mri_dst from mri_src`, a copy). For each
  surface in the order given: read (unread: `<program>: could not surface <name>`, status 255),
  its transform inverted and applied unless it is `identity.nofile`; `computing distance
  transform`: the signed distance to the surface (float, the volume's grid; inside negative); the
  first kept, each later one combined voxel by voxel (the second surface's value where it is
  negative and the first's positive, both positive and it is smaller, or both negative and it is
  larger). The combined distance negated; `min_dist = %g`. The control points: with suppression
  on (`computing nonmaximum suppression`), the distance's non-maximum suppression (threshold 0,
  direction 1) at or above `min_dist`; else (with `-erode`) the distance at or above `min_dist`
  eroded `-erode` times; else the distance at or above `min_dist` (all marked 1, others 0). The
  file's or label's points added. `-mask_sigma <s> <t>`: points where the Sobel magnitude of the
  volume smoothed with a Gaussian of `s` (normalised) is at or above `t` removed. `-mask_orig <v>
  <t>`: points where the volume `v` is below `t` removed (unread `v`: FS-1017). `Erasing Border
  planes 4`: the four outermost planes on each face cleared. With `-aseg`: the segmentation read
  (unread: `<program>: could not load aseg from <name>`, status 255), `aseg read with width <w>
  (src width <w>)`, resampled by nearest neighbour onto the control volume when its extent or type
  differs (`resampling aseg...`), `Removing non-wm voxels` (§2.5); else `Removing surface
  outliers` (no change). `-checknorm`'s range (§2.8; unread check volume: `<program>: could not
  load <name>`, status 255); the intensity threshold 10 (§2.8); with `-T2`, §2.7. `Building bias
  image`; the bias smoothed as above (`smoothing bias field with sigma=%2.3f`); `-w`'s control
  volume (`Writing control volume to <name>`) and bias (`Writing bias volume to <name>`);
  `MRIapplyBiasCorrectionSameGeometry()`: the volume divided by the bias and scaled to 110 in
  place, its not-a-number voxels set to 0 (unless `FS_LEAVE_NANS` is set); `writing normalized
  volume to <output>`, written (of the input's type: the surface program never makes the
  input float); `#VMPC# mri_normalize VmPeak  <kB>` (the process's peak
  virtual memory) and `mri_normalize done`; status 0.
- **`-r <control> <bias>`** (after `-conform` and `-mask` below): the bias (unread: `<program>:
  could not read bias volume <name>`, status 252) and control volumes read (`<program>: could not
  read control volume <name>`, 252), the control volume binarised at 1 (0 and 128), the product
  of the bias and the input, its mean over the 128 voxels `m`: `mean in wm is %2.0f, scaling by
  %2.2f` (`m`, `110 / m`), the product scaled by `110 / m` (as a float), written; status 0.

### 3.3 The normalisation

1. `-conform` with an input that is not conformed (256^3, 1 mm, coronal): `unconformed source
   detected - conforming...`, the input conformed (`specs/freesurfer-volume.md`). `-mask`: read
   (unread: `<program>: could not open mask volume <name>.` and a newline, 255), the input's
   voxels where the mask is 0 set to 0.
2. `-l <control> <bias>`: both read (as `-r`'s, status 252), the control volume binarised at 1 (0
   and 1) and made uchar (no scaling), the mean `m` of the input in it (`mean in wm is %2.0f,
   scaling by %2.2f`), the input scaled by `110 / m` into the result; the white-matter outliers
   removed from the control volume (§2.3, `intensity_below / 2` truncated to an integer); the bias
   field built from the result and filled by the soap bubble (50 iterations, change 1), and applied
   to the result in place.
3. `-grad <t>`: the input smoothed by a Gaussian of 0.5 voxels, its Sobel gradient's magnitude
   (`m.mgz`) at or above `t` marked 1 in a uchar volume (`nc.mgz`): the voxels that cannot be
   control points (replacing `-atlas`'s mask).
4. The input made float (no scaling); when conformed, its negative voxels set to 0.
5. `-aseg`: `Reading aseg <name>`; unread: `<program>: could not read aseg from file <name>`
   (255); with `-conform` and a segmentation that is not conformed: `<program>: aseg volume
   <name> must be conformed` (253); `aseg read with width <w> (src width <w>)`; resampled by
   nearest neighbour onto the input when its extent or type differs (`************** resampling
   aseg to account for mismatch with source image ***************`; a resampling that fails:
   FS-1017).
6. `normalizing image...` (verbose, the default). The timer starts. `-f`'s control points read
   (`specs/freesurfer-filter.md`'s control-point file); `-w`'s control-point volume name kept.
7. Without `-l`: with the gentle pass with control points on (the default) and a file or label:
   `Doing gentle normalization with control points/label`, one gentle 3-D pass from the input with
   those points only (intensities `above` and `below / 2`) into a new result; else `NOT doing
   gentle normalization with control points/label` and a copy of the input.
8. With `-aseg`: `processing with aseg`; a clone of the segmentation with its labels 2 and 41
   copied as control points; `removing outliers in the aseg WM...` (§2.2, from the result of 7,
   `intensity_below` truncated); binarised at 1 (0 and 1) and made uchar; the file's or label's
   points added; with `-interior`, §2.4 (`1.25 below`, the segmentation given); `Building bias
   image`; with a bias sigma above 0, `Smoothing with sigma %g` and the field smoothed; `-c`'s
   volume (`Writing output control points volume to <name>`); `Applying bias correction`: the
   result divided by the field and scaled to 110 in place.
9. Without `-aseg`: `processing without aseg, no1d=<0|1>`; unless `-no1d` (or `-l`, `-monkey`,
   `-lonly`, `-fonly`): `MRInormInit(): `, `MRInormalize(): ` and the 1-D normalisation of the
   input into the result (`specs/freesurfer-filter.md`), which, when it fails, prints `1d
   normalization failed - trying no1d...` and continues as `-no1d`. With `-no1d`: with file points
   only and SNR off, or `-gentle` with a file, the result so far (a copy when there is none); else
   with SNR off and `-interior`, `computing initial normalization using surface interiors`: a uchar
   control volume of the input's geometry, §2.4 (no segmentation) on the result so far, the bias
   field built, smoothed (bias sigma above 0) and the input divided by it into the result (after a
   failed 1-D pass: FS-1017); else with SNR off and no `-l`, a copy of the input; with SNR on,
   `computing initial normalization using SNR...` and the high-signal normalisation of the input
   (`specs/freesurfer-filter.md`) into the result. No result: `<program>: could not allocate
   volume` (252).
10. Unless file points only: one gentle 3-D pass of the result in place (intensities `above`,
    `below / 2`).
11. A copy of the result (the original of the 3-D passes); an empty line; `Iterating <n> times`;
    then `n` passes, each (unless file points only, which ends the loop) `---------------------------------`,
    `3d normalization pass <i> of <n>`, then with `-gentle` a gentle pass (`above / 2`, `below / 2`)
    of the result in place, else a 3-D normalisation of the result in place with that copy as the
    original (`above`, `below`, pruning, bias sigma, scan type, the not-control mask). `Done
    iterating ---------------------------------`.
12. `-w`'s control-point volume name kept again; with `-w`'s bias name: the bias (§2.1) of the
    input and the result, `writing bias field to <name>....`, written.
13. A result that is not uchar: when conformed, each voxel below 0 set to 0 and above 255 to 255;
    then made uchar (no scaling: values rounded). `writing output to <output>` (verbose); written
    (a failed write prints its message and does not change the status).
14. `3D bias adjustment took <m> minutes and <s> seconds.`: the seconds since the timer started
    (7), rounded to the nearest second, as minutes and seconds; status 0.

**FS-1017** (rule B): where a step gives no volume and FreeSurfer goes on with none (`-conform`'s
conforming or a resampling that fails, `-mask_orig`'s volume unread, `-no1d` with `-interior`
after a failed 1-D pass, `-r` or `-l` without their names), FreeSurfer crashes; both modes end
with `mri_normalize: <step> gave no volume (FreeSurfer crashes here)` (standard error, status 1).

The working folder receives the files named above (`r.mgz`, `c.mgz`, `e.mgz`, `b.mgz`, `bs.mgz`,
`m.mgz`, `nc.mgz`; with diagnostics, more); every output is written in the input's format rules
(`specs/freesurfer-io.md`), with the input's header and the command line added.

## 4. Validation plan

- **The open probe** (`validation/investigations/freesurfer-mri-normalize-probe/`, its README):
  the official program (the unit oracle's `bin/mri_normalize`) against a candidate on 82 command
  lines over seeded synthetic inputs (every option of §1, the three programs of §3.2, the paths of
  §3.3, the failures), in both modes: `reproduce` must give the official exit status, standard
  output and error, and files (provenance aside) on every compared case; `fix` may differ only
  with a notice.
- **recon-all's two calls** on the recorded heads (`docs/validation/freesurfer-replay.md`): the
  replay tool runs a candidate on each recorded step (`python -m larmorx_validation
  freesurfer-replay run <run> mri_normalize mri_normalize:2 --command '<candidate> ...'`, the 12
  runs `<head>-classic` and `<head>-v8` of 6 heads): every output must be the recorded one,
  provenance aside, in both modes.
- **Speed and memory**: the replica's measures on recon-all's calls (one thread) are the clean
  room's reference: about 27 s for `T1.mgz` and 47 s for `brain.mgz` (observed, at four replays
  side by side), and a peak of about 566 MiB (FreeSurfer 570 and 1,172 MiB) (observed).

The replica's checks (closed): `validation/probes/fs-normalize/` (each step against the official
code, case by case, on three recorded heads, the whole program, the golden cases).
