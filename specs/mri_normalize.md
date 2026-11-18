# `mri_normalize` (FreeSurfer 8.2.0): behaviour spec

Status: part 1 of 2 (queue item 3.2-24: the options and the control-point steps); part 2 (the
program's order of steps, its outputs and exit statuses, 3.2-25) comes with the program. Written
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

## 3. Validation plan

The open probe for the clean room comes with part 2 (the program as a black box: every option
of §1 through the official program, the steps of §2 through the options that reach them). The
replica's checks (closed): `validation/probes/fs-normalize/` (each step against the official
code, case by case, and on three recorded heads).
