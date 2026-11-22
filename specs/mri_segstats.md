# `mri_segstats` (FreeSurfer 8.2.0): behaviour spec

Status: complete for queue item 3.2-20 (the volume options, the subject options that read the
subject's cached brain-volume statistics, the surface options `--annot`, `--slabel` and `--euler`,
the four stand-alone options, the outputs and exit statuses, and the validation plan with the open
probe, §7). Computing the brain-volume statistics from the subject's surfaces (no cache, or
`--no-cached`) belongs to queue item 3.5-36 and is not specified here. Written by the replica's
author (track D) as the spec writer: behaviour, maths, conventions and messages, for the clean
room (`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-*.md`) and black-box runs of the official program only.

Conventions: volumes are FreeSurfer volumes (`specs/freesurfer-mri.md`,
`specs/freesurfer-volume.md`); a voxel's value is read as a `float` and written from a `float`;
a label is the `int` C's `(int)` gives of a voxel's `float` value; voxels are visited with the
column index outermost and the slice index innermost. Messages go to standard output unless
"standard error" is said; `%g`, `%f`, `%lf` and their kin are C's formats (`%f` prints `inf`,
`-inf`, `nan` and `-nan`). Numbers on the command line are read as C's `sscanf` reads them (`%d`,
`%f`, `%lf`): a number that cannot be read leaves the setting as it was. `float` arithmetic is
IEEE binary32 with round to nearest, `double` binary64; an operation on a NaN gives that NaN
(quieted) and an invalid one (0/0) the NaN with the sign bit set (printed `-nan`).

## 1. The command line

`mri_segstats [options]`: the version options first (`specs/freesurfer-core.md`, the version
report: `--version` and `--all-info` print their report; with only version options the program
ends with status 0); then, with no argument left, the usage (§1.4) and status 1. Every other
argument is an option, read with the arguments after it. An argument that is no option prints
`ERROR: Option <arg> unknown` on standard error (and, for an argument of two or more characters
starting with one dash not followed by another, `       Did you really mean -<arg> ?`) and ends
with status 255. An option short of its arguments prints `ERROR: <option> flag needs <n>
argument` (`arguments` for more than one) on standard error and ends with status 255. The options
are compared with case, but for `--help`, `--usage`, `-h`, `-u`, `--version`, `--debug`,
`--newprint`, `--no-newprint`, `--dontrun`, `--nonempty`, `--non-empty`, `--empty`, `--no-cached`,
`--robust`, `--maskinvert`, `--gtm-default-seg-merge`, `--gtm-default-seg-merge-choroid`,
`--replace` and `--seed`, compared without case. Before the first option the random numbers are
seeded with 4321 (`specs/freesurfer-core.md`'s `setRandomSeed`). With `--debug` set, each later
option is first printed as `<remaining> <option>` (the count of arguments left, this one
included).

### 1.1 The options

Outputs: `--sum f` (or `--o f`): the summary table (§4). `--avgwf f`: the waveforms' table;
`--sumwf f` the same with sums (§5); `--sfavg f`: each segmentation's mean over space and frames;
`--avgwfvol f`: the waveforms as a volume; the three set "frame averages". `--ctab-out f`: the
reported entries as a colour table. Flags: `--debug`, `--dontrun` (no counting: each id's count is
its index in the list), `--nonempty`/`--non-empty` (the default: empty ids removed), `--empty`
(kept), `--no-cached` (§3.5), `--newprint` (the default) and `--no-newprint` (§4.2), `--snr`,
`--abs`, `--sqr`, `--sqrt`, `--acc`/`--accumulate` (the mean times the count), `--segborder` (the
default) and `--no-segborder` (§3.8), `--etiv`, `--etiv-only`, `--old-etiv-only`,
`--brain-vol-from-seg`, `--subcortgray`, `--supratent`, `--totalgray`, `--surf-ctx-vol`,
`--surf-wm-vol`, `--euler`, `--no-global-stats` (clears the surface white-matter and cortex
volumes, `--supratent`, `--brain-vol-from-seg`, `--subcortgray`, `--totalgray` and `--euler`),
`--regheader`,
`--maskinvert`, `--seg-from-input` (the segmentation is 1 everywhere, the id list 1: §3.4; it sets
the first id to 1 and the id count to 1, so earlier `--id` values beyond the first are dropped
from the count but stay in the list's places), `--excl-ctxgmwm` (excludes 2, 3, 41 and 42),
`--ctab-default` (`$FREESURFER_HOME/FreeSurferColorLUT.txt`, `(null)` for an unset
`FREESURFER_HOME`; prints `Using defalt ctab <name>`).

With values: `--seg f`; `--in f` (or `--i f`); `--pv f`; `--mask f`; `--brainmask f` (only named in
the summary); `--ctab f`; `--ctab-gca f` (a GCA atlas, §3.6); `--frame n` (the input's frame);
`--subject s`; `--sd d` (sets `SUBJECTS_DIR`); `--talxfm f`; `--in-intensity-name s`,
`--in-intensity-units s` (default `unknown`); `--masksign s` (`abs`, `pos` or `neg` by their first
three letters in any case, else `ERROR: mask sign = <s>, must be abs, pos, or neg` on standard
error and status 1; default `abs`); `--maskthresh x` (`%f`, default 0.5); `--maskframe n`;
`--maskerode n`; `--robust p` (`%f`, sets robust statistics with `p` percent, default 5);
`--seg-erode n`; `--segbase n` (default -1000: §3.3); `--surf name` (default `white`);
`--label-thresh x` (`%lf`); `--synth n` (read, unused); `--avgwf-norm-mean x`; `--stiv x` (a file
that opens for reading gives its first number, `fscanf("%lf")`, else the argument as a number;
the summary's sTIV); `--seed n` (seeds the random numbers with `atol(n)` and prints `setting seed
for random number genererator to <atoi(n)>`); `--mul x` and `--div x` (the input's factor,
multiplied and divided in `double` in order; FS-1103 for a number that cannot be read); `--reg f`
(read where it stands as `specs/freesurfer-io.md`'s registration matrix: a `register.dat`, or an
`.lta` converted to its tkregister matrix; unread: status 1); `--id a b ...` and `--excludeid a b
...` (or `--exclude`): every following argument that is not a flag (`--...`); an unreadable `--id`
value takes the place's previous value (0 at first), an unreadable exclusion 0; `--vox c r s` (the
segmentation replaced by one voxel; with no argument status 255 as needing 3 arguments; FS-1104
with one or two); `--annot subject hemi annot` (FS-1105: with fewer than three, FreeSurfer's
message says 1 argument); `--slabel subject hemi label` (FS-1110); `--replace a b` (appends a
replacement; an unreadable number leaves the place as it was); `--replace-file f` (§1.3);
`--gtm-default-seg-merge` (appends the 18 replacements 4, 5, 14, 15, 72, 43, 44 into 24; 251-255
into 192; 1000, 2000, 85, 30, 62, 80 into 0); `--gtm-default-seg-merge-choroid` (the same, then
FreeSurfer drops the last two again: FS-1107). FreeSurfer keeps 1,000 ids, exclusions and
replacements (FS-1109 past that).

### 1.2 The stand-alone options

Run where they stand (options before them have taken effect), they end the program:
- `--xfm2etiv xfm outfile|nofile` (needs 2; FS-1105's message): eTIV of the transform with scale
  1948.106 (`specs/freesurfer-volume.md`'s `MRIestimateTIV`), printed `%12.4lf`, and written so
  into `outfile` unless it is `nofile`; status 0 (1 when the file does not open).
- `--rescale-by-seg invol seg target outvol id ...` (needs 5): reads both volumes (status 1 when
  one does not read); the ids are the arguments after the fourth up to the first flag (`ERROR: no
  segids provided`, status 1, without any); prints `MRIrescaleBySeg() targetmean = <%g>, segids =
  ` and each id as `%2d `, then a new line; over the voxels whose label in the segmentation's frame
  0 is one of the ids, every frame of the input summed in `double` (`ERROR: nhits = 0`, status 1,
  without any); `nhits = <n>, sum = <%g>, mean = <%g>, targetmean = <%g>, scale = <%g>` with
  `mean = sum / n`, `scale = target / mean`; the output (a `float` volume of the input's extent,
  frames, header and pulse parameters) is `scale * value` of every voxel, written; status the
  write's.
- `--sum-in f`: reads a summary table (`ERROR: cannot open <f>`, status 1): each line (read in
  pieces of at most 999 bytes) not starting with `#` is a row read as `%*d %d %d %f %s %f %f %f %f
  %f` (id, count, volume, name, mean, deviation, minimum, maximum, range; fields not read 0);
  prints `Found <n>` and each row as `%3d  %8d %10.1f  %-30s ` then the minimum, maximum, range,
  mean and deviation as `%10.4f `; status 1.
- `--qa-stats subject outfile` (needs 2): §6.

### 1.3 The replacement file

`--replace-file f`: a file that does not open prints `ERROR: could not open <f> <errno>` and the
error's text, status 1. Each line (pieces of at most 999 bytes, counted from 0 with the comment
lines) not starting with `#` must hold exactly two words separated by blanks (spaces or tabs: a
line's first word may not start with a new line or carriage return), else `ERROR: line <n> in <f>
has the wrong format (has <k> elements, expecting 2)`, status 1 (an empty line has 0 elements);
each is read as `%d %d` into the next replacement (a number not read leaves the place as it was).
None read: `ERROR: could not find any replacement items in <f>`, status 1; else `Read in <n>
replacement items from <f>`. FS-1108: FreeSurfer counts replacements given before the file twice.

### 1.4 The checks and the help

After the options, in order, each printing `ERROR: <message>` and ending with status 1: no
`--seg`, `--annot`, `--slabel`, `--etiv-only`, `--old-etiv-only` or `--seg-from-input`: `must specify
a segmentation volume`; `--seg-from-input` without `--in`: `must specify an input volume with
--seg-from-input`; no `--sum`, frame-average table, `--avgwfvol`, `--etiv-only` or
`--old-etiv-only`: `must specify an output table file`; frame averages without `--in`: `cannot do
frame average without input volume`; `--etiv` without `--subject`: `need subject with --etiv`;
`--supratent` without `--surf-ctx-vol`: `need --surf-ctx-vol  with --supratent` (two spaces); a
colour table and `--ctab-gca`: `cannot specify ctab and gca`; `--surf-ctx-vol` without a subject:
`need --subject with --surf-ctx-vol`; `--totalgray` without `--surf-ctx-vol`: `need --surf-ctx-vol
with --totalgray`; `--ctab-out` without a colour table: `need an input ctab to create output
ctab`; the same name for both: `output ctab is the same as input`; then the mask sign defaults to
`abs`; `--euler` without a subject: `need subject with --euler`. The usage (no argument, `--help`,
`--usage`, `-h`, `-u`) is the help text of FreeSurfer's `mri_segstats.help.xml` as
`specs/freesurfer-core.md`'s help renderer prints it; status 1. `--version` (when not taken as a
version option) prints `8.2.0`, status 1.

## 2. The report

After the checks: an empty line; `8.2.0`; `cwd ` (always empty: the program never fills it); `cmdline
<arguments>` (the program's name and every argument left after the version options, each followed
by a blank); `sysname  <s>`, `hostname <h>`, `machine  <m>` (`uname`'s); `user     <name>` (the
user's name, else `UNKNOWN`); `whitesurfname  <name>`; `UseRobust  <0|1>`.

## 3. The program

The steps in order. A volume is read as `specs/freesurfer-io.md` reads one (its not-a-number
voxels made 0); a volume that does not read prints its reader's messages, then (for the named
volumes) `ERROR: loading <name>`, status 1.

### 3.1 The subject and eTIV

With a subject (`--subject`, `--annot`, `--slabel`, `--qa-stats`): `SUBJECTS_DIR` must be set
(`ERROR: SUBJECTS_DIR not defined in environment` on standard error, status 1). Names built from
it below are `<SUBJECTS_DIR>/<subject>/...`, with `(null)` for each that is unset. With `--etiv`,
`--etiv-only` or `--old-etiv-only`: eTIV (`MRIestimateTIV`, scale 1948.106) of `--talxfm`'s file or
`mri/transforms/talairach.xfm`; with `--old-etiv-only` instead `mri/transforms/
talairach_with_skull.lta` with scale 2150; printed `atlas_icv (eTIV) = <(int) eTIV> mm^3    (det:
<%3f> )`; `--etiv-only` and `--old-etiv-only` then end with status 0. Names of 1,000 bytes or more:
FS-1101.

### 3.2 The outputs' first opening

The summary file is opened for writing (created or emptied) and removed again: when it does not
open, `ERROR: could not open <f> for writing` and `Errno: <the error's text>`, status 1. Then the
frame-average table likewise (`ERROR: could not open <f> for writing`, status 1). So an existing
summary file is gone if the program ends before writing it.

### 3.3 Euler numbers and the segmentation

`--euler`: the surfaces `surf/lh.orig.nofix` and `surf/rh.orig.nofix` read (`specs/freesurfer-
surfio.md`); both read: `Computing euler number`, the Euler numbers `V - E + F` of each (its
unripped vertices and faces, the edges between unripped vertices), `orig.nofix lheno = <%4d>,
rheno = <%d>` and `orig.nofix lhholes = <%4d>, rhholes = <%d>` with holes `1 - eno / 2` (C's
integer division); otherwise `Warning: error reading<which>, not computing euler number`
(`lh.orig.nofix and rh.orig.nofix`, `lh.orig.nofix` or `rh.orig.nofix`, no space before it) and
no holes in the summary.

The segmentation, the first that applies:
- `--seg f`: `Loading <f>`, read; with replacements `Replacing <n>` and each voxel's label
  replaced by the first matching replacement's target (`MRIreplaceList`, a new `int` volume);
  with `--seg-erode n` (not 0) `Eroding seg <n> times` and `MRIerodeSegmentation(seg, n, 0)`; with
  `--vox c r s` `Replacing seg with a single voxel at <c> <r> <s>` and the segmentation 1 at that
  voxel, 0 elsewhere (frame 0).
- `--annot subject hemi annot`: `Constructing seg from annotation`; the surface
  `surf/<hemi>.<surf>` read (status 1 when it does not); the annotation `annot` itself when it
  opens for reading, else `label/<hemi>.<annot>.annot`, printed as an empty line and `Reading
  annotation <name>`; read (`specs/freesurfer-surfio.md`), and when that fails ` ... trying local
  annot` and `annot` itself (` Successfully read local annot` and an empty line when it works);
  failing: status 1. Without `--segbase`: 1000 for `aparc` on `lh`, 2000 on another hemisphere,
  1100 and 2100 for `aparc.a2005s`, else 0; `Seg base <n>`. The segmentation is the surface's
  annotation as a `<vertices> x 1 x 1` `int` volume (its colour-table index plus the base,
  `MRISannot2seg`, which prints `MRISannot2seg(): nhits = <n>`). The surface's colour table, with
  that base, is written to a new temporary file `tmp.XXXXXX.ctab` in the temporary folder and
  becomes the colour table (`--ctab`). FS-1114: an annotation without a colour table.
- `--slabel subject hemi label`: `Constructing seg from label` (and ` Label Threshold = <%g>` with
  `--label-thresh`); the label read (status 1 when it does not); the surface `surf/<hemi>.<surf>`;
  an `int` volume of `<vertices> x 1 x 1`, 1 at each label point's vertex (points whose value is
  below the threshold skipped with `--label-thresh`).
- otherwise (`--seg-from-input`): `Creating a segmentation of all 1s from <input>`, the input's
  header read (FS-1115 when it does not), a `float` volume of its extent and frames, 1 everywhere.

### 3.4 The colour table

`--ctab` (or the annotation's, or `--ctab-default`): read (`specs/freesurfer-io.md`: `ERROR:
reading <f>`, status 1). Else a colour table embedded in the segmentation becomes it: `Using
embedded color table (and excluding seg 0)`, and 0 joins the exclusions.

### 3.5 The brain-volume statistics

With the surface volumes (`--surf-wm-vol`, `--surf-ctx-vol`), `--supratent`, `--brain-vol-from-seg`
or `--subcortgray`: when `mri/ribbon.mgz` opens for reading, `Getting Brain Volume Statistics`
and the subject's cache `stats/brainvol.stats` read (`specs/freesurfer-volume.md`'s
`ReadCachedBrainVolumeStats`: its 16 measures; without the file it computes and caches them,
queue item 3.5-36); with `--no-cached` `Computing Brain Volume Statistics` and the computation
(3.5-36). Without the ribbon: `Warning: cannot find <name>, not computing whole brain stats` and
those options (and `--totalgray`) are cleared. FS-1113: no subject.

### 3.6 `--ctab-gca`

The atlas read (`ERROR: reading <f>`, status 1); `GCAcolorTableCMA: using ctab
$FREESURFER_HOME/FreeSurferColorLUT.txt` (that table read, `ERROR: reading <f>` and status 1 when
it does not); a colour table of `max label + 1` entries made as `specs/freesurfer-io.md`'s
`CTABalloc` makes one (its random colours drawn), then for each label the atlas's nodes hold, in
increasing order, three more draws (`nint(randomNumber(0, 255))`: red, green, blue), alpha 255,
the float colours `(float)i / 255.0`, and the name of FreeSurferColorLUT.txt's entry; every other
entry removed. It becomes the colour table. FS-1117.

### 3.7 The input, the partial-volume volume and the mask

`--in f`: `Loading <f>`, read; `--frame` at or past its frames: `ERROR: input frame = <f>, input
volume only has <n> frames`, status 1. With `--reg` or `--regheader`: `Input Volume
Registration:` and the matrix (with `--reg`; `MatrixPrint`'s `% 2.5f` elements, two spaces
between, `;` after each row), the voxel-to-voxel matrix of the registration from the segmentation
to the input (`MRIvoxToVoxFromTkRegMtx`, the identity's for `--regheader`) as `Input Volume
vox2vox:` and the matrix, `Allocating <frames> frames`, `Vol2Vol`, and the input resampled onto
the segmentation's grid with the nearest neighbour (`MRIvol2VolVSM`; its failure: status 1).
Extents unlike the segmentation's: `ERROR: dimension mismatch between input volume and seg`,
`  input <w> <h> <d>`, `  seg   <w> <h> <d>`, status 1. Then `--mul`/`--div` (`Multiplying input
by <%lf>`: each voxel times the factor in `double`), `--abs` (`Computing absolute value of
input`), `--sqr` (`Computing square of input`: `v * v` in `double`), `--sqrt` (`Computing square
root of input`: `sqrt(fabs(v))` in `double`), each in place.

`--pv f`: `Loading <f>`, read; extents unlike the segmentation's: `ERROR: dimension mismatch
between PV volume and seg`, `  pvvol <w> <h> <d>`, `  seg   <w> <h> <d>`, status 1.

`--mask f`: `Loading <f>`, read; `--maskframe` at or past its frames: `ERROR: mask frame = <f>,
mask volume only has <n> frames`, status 1; extents unlike: `ERROR: dimension mismatch between
brain volume and seg`, status 1. Binarised in place (`specs/freesurfer-volume.md`'s
`mri_binarize`: `NOT INVERTING` or `INVERTING`, then each value against the threshold by the sign);
no voxel set: `WARNING: no voxels in mask meet thresholding criteria.`, `The output table will be
empty.`, `thresh = <%g>, sign = <s>, inv = <0|1>`; `There were <n> voxels in the orginal mask`;
`--maskerode n` (above 0): `Eroding <n> voxels in 3d` and `n` 3-D erosions; then every voxel whose
mask value in `--maskframe` is 0 as `(int)` becomes 0 in the segmentation.

### 3.8 The ids and their statistics

The unit: without a surface `Voxel Volume is <%g> mm^3` with `xsize * ysize * zsize` (`float`); on
a surface `Vertex Area is <%g> mm^3` with the surface's group-average area (when above 0) or its
total area divided by its vertex count (`float`). Then `Generating list of segmentation ids` and
the segmentation's distinct labels (sorted). The ids: with neither a colour table nor `--id`, the
segmentation's labels; with a colour table and no `--id`, every valid entry of the table in index
order (with its name, at most 999 bytes, and colour); with `--id` and a colour table, the given
ids, each of which must be a valid entry (`ERROR: cannot find seg id <id> in <ctab>`, status 1;
FS-1116, FS-1118 for ids outside the table); with `--id` only, the given ids. `Found <%3d>
segmentations`; none: `ERROR: no segmentations to report`, status 1. `Computing statistics for
each segmentation`. With `--pv` and the border (the default): the segmentation's border volume
(`MRIsegBorder` with 6 neighbours: prints `nhits = <n>, maxlabel=<m>`); without it the largest
label is 0.

For each id not excluded and present in the segmentation (the others keep a count and statistics
of 0): without `--dontrun`, on a volume the count is the voxels of frame 0 with that label and the
volume `count * unit` (`float`), or with `--pv` the partial-volume corrected volume
(`MRIvoxelsInLabelWithPartialVolumeEffects(seg, pv, id, -, -, maxlabel + 1, border)`); on a
surface the count is the vertices whose value equals the id (compared as `float`) and the volume
the sum of their areas (group-average areas when loaded; `float`, in vertex order). With
`--dontrun` the count is the id's index in the list and the volume 0. With an input and not
`--dontrun`: for a count above 0 the input's statistics within the id (`MRIsegStats`, or with
`--robust` `MRIsegStatsRobust` with its percent: minimum, maximum, range, mean, deviation) and the
signal-to-noise ratio `mean / std` (`float`); 0 for all without voxels; with `--acc` the mean
times the count (`float`). The ids are independent: any order or number of threads gives the same
numbers.

With `Gdiag_no` above 1 each id not excluded is printed `%3d   %3d  %33s  %6d  %10.3f` (index, id,
name, count, volume); then an empty line. The empty ids (unless `--empty`) and the excluded ones
are removed; `Reporting on <%3d> segmentations`. With `--debug` each row as `%3d  %8d %10.1f  `
(id, count, volume), with a colour table its name `%-30s `, with an input `%10.4f ` for minimum,
maximum, range, mean, deviation and with `--snr` the ratio.

## 4. The summary file

### 4.1 The header

`# Title Segmentation Statistics `, `# `, `# generating_program mri_segstats`, `# cvs_version
8.2.0`, `# cmdline <the report's>`, `# sysname  <s>`, `# hostname <h>`, `# machine  <m>`, `# user
<name>`, `# anatomy_type volume` (or `surface`), `# `; with a subject `# SUBJECTS_DIR <dir>` and `#
subjectname <s>`; with `--robust` `# RobustPct <%g>`; with brain-volume statistics `#
BrainVolStatsFixed see surfer.nmr.mgh.harvard.edu/fswiki/BrainVolStatsFixed` when the unit differs
from 1 by more than 0.01, else `# BrainVolStatsFixed-NotNeeded because voxelvolume=1mm3`. Then the
measures `# Measure <names>, <%f>, mm^3` (FreeSurfer's names and descriptions, the cache's
measures by number): with `--brain-vol-from-seg` BrainSeg (0) and BrainSegNotVent (1); with any
statistics VentricleChoroidVol (15); with `--surf-ctx-vol` lhCortex (5), rhCortex (6), Cortex (7);
with `--surf-wm-vol` lhCerebralWhiteMatter (9), rhCerebralWhiteMatter (10), CerebralWhiteMatter
(11); `--subcortgray` SubCortGray (4); `--totalgray` TotalGray (8); `--supratent` SupraTentorial (2)
and SupraTentorialNotVent (3), both described as `Supratentorial volume`; with `--brainmask` and
statistics Mask (12); with `--etiv` and `--brain-vol-from-seg` `BrainSegVol-to-eTIV` (0 / eTIV)
and `MaskVol-to-eTIV` (12 / eTIV) as `unitless`; with holes lhSurfaceHoles, rhSurfaceHoles and
SurfaceHoles (`%d`, `unitless`); with `--etiv` EstimatedTotalIntraCranialVol (`eTIV`); with
`--stiv` SegmentedTotalIntraCranialVol (`sTIV`). Then, each when given: `# SegVolFile <f> ` and `#
SegVolFileTimeStamp  <time> ` (the file's modification time, local, `YYYY/MM/DD hh:mm:ss`; `(null)`
for a file that cannot be examined), `# Annot <subject> <hemi> <annot>`, `# Label <subject> <hemi>
<label>`, `# ColorTable <f> ` and `# ColorTableTimeStamp <time> `, `# ColorTableFromGCA <f> ` and
`# GCATimeStamp <time> `, the mask's `# MaskVolFile  <f> `, `#   MaskVolFileTimeStamp  <time> `,
`#   MaskThresh <%f> `, `#   MaskSign   <s> `, `#   MaskFrame  <n> `, `#   MaskInvert <n> `
(FS-1106), the input's `# InVolFile  <f> `, `# InVolFileTimeStamp  <time> `, `# InVolFrame <n> `,
`# PVVolFile  <f> ` and `# PVVolFileTimeStamp  <time> `; with `--excl-ctxgmwm` `# Excluding
Cortical Gray and White Matter`; with exclusions `# ExcludeSegId ` and each `<id> `; with the
empty ids removed `# Only reporting non-empty segmentations`; `# VoxelVolume_mm3 <%g> ` (or `#
VertexArea_mm2 <%g> `).

### 4.2 The table

The default (`PrintSegStat`): the column descriptions `# TableCol <%2d> ColHeader <h>`, `FieldName
<f>`, `Units     <u>` for Index (`NA`), SegId (`Segmentation Id`, `NA`), NVoxels (`Number of
Voxels`, `unitless`) and Volume_mm3 (`Volume`, `mm^3`) — on a surface NVertices (`Number of
Vertices`) and Area_mm2 (`Area`, `mm^2`) — and StructName (`Structure Name`, `NA`); with an input
`<name>Mean`, `<name>StdDev`, `<name>Min`, `<name>Max`, `<name>Range` (`Intensity <name><what>`, the
input's units; the Mean header and the first two columns' with a trailing blank, exactly as the
official files show); with `--snr` `SNR` (`Intensity SNR`, `none`), even without an input; `# NRows
<n> `, `# NTableCols <n> `, `# ColHeaders  ` and each header with a blank, then a blank and a new
line; each row `%3d %3d  %8d %10.1f  ` (index from 1, id, count, volume), the name (`%-30s `, or
`Seg%04d ` without a colour table), with an input the mean, deviation, minimum, maximum and range
as `%10.4f ` and with `--snr` the ratio. `Using PrintSegStat` on standard output. With
`--no-newprint` (`Not using PrintSegStat`): the same first four descriptions, StructName's only
with a colour table, the intensity columns', no SNR description; `# NRows`, `# NTableCols`; `#
ColHeaders  Index SegId NVoxels Volume_mm3 ` (or the surface's), `StructName `, with an input the
five headers and two blanks, with `--snr` `<name>SNR `; the rows as above.

`--ctab-out f`: each reported row `%d %-30s %3d %3d %3d 0` (id, name, colour); FS-1111 when it does
not open.

## 5. The waveforms

With frame averages: `Computing spatial average of each frame`; for each reported id (with
`--debug` its index `%3d`, a new line after every 20th) the input's mean of each frame over the id's
voxels (`MRIsegFrameAvg`, `double`), times the voxel count with `--sumwf`; the mean over the
frames (`double`, divided by the frame count); `--avgwf-remove-mean` subtracts it from each frame;
`--avgwf-norm-mean x` multiplies each by `x / mean`. An empty line. `--sfavg f`: `Writing to <f>`,
the means `%g` a line each (FS-1111 when it does not open), and on standard output `<index> <%g>`
each. The table (`--avgwf`, `--sumwf`): `Writing to <f>`, a row per frame with `%11.5f ` per id.
`--avgwfvol f`: no ids: `ERROR: no voxels found in segmentation`, status 1; else `Writing to
<f>` and a `float` volume of `<ids> x 1 x 1` with the input's frames and header (`MRIcopyHeader`),
each voxel the id's waveform (`double` to `float`), written. Then `mri_segstats done`, status 0.

## 6. `--qa-stats subject outfile`

`(null)` for an unset `SUBJECTS_DIR` in the names. The control points `tmp/control.dat` counted
when it opens; `mri/wm.mgz` (status 1 when it does not read): voxels of 1 (erased) and 255
(filled); when `mri/brainmask.auto.mgz` opens, it and `mri/brainmask.mgz` (status 1 when one does
not read): voxels that differ, erased where the mask is 1, cloned elsewhere; `mri/aseg.mgz` and
`mri/aseg.auto.mgz` (status 1): voxels that differ. The holes of `surf/?h.orig.nofix` (`1 - eno /
2`, 0 for a surface that does not read). eTIV of `mri/transforms/talairach.xfm`; the brain-volume
cache's measure 12 divided by it. The white matter (labels 2, 41, 7, 46, 251-255, 77, 78, 79 of
`mri/aparc+aseg.mgz` set to 1, others 0), eroded 3 times in 3-D (`Eroding 3 voxels in 3d`), and
`MRIsegStatsRobust` of `mri/norm.mgz` within it at 2 percent (FS-1112 when a volume does not read).
For `lh` then `rh`: without `surf/<hemi>.w-g.pct.mgh` `Cannot find <name> ... skipping`; else it,
`surf/<hemi>.white` and `label/<hemi>.cortex.label` read (status 1 when one does not), the label's
vertices 1 in a `<vertices> x 1 x 1` `int` volume, `MRIsegStats` of the contrast within it; ` <hemi>
cnrstats: <%6.3f> <%6.3f> <%6.3f>` (mean, deviation, `mean / std` in `float`); the means summed
and the squared deviations summed from `1e-10` (`double`). The mean is the sum over 2 and the
deviation `sqrt(sum / 2)` (2 even for a skipped hemisphere). Printed: `<subject> nc <%3d>, nWMErase
<%3d>, nWMFill <%3d>, nBMErase <%3d>, nBMClone <%3d>, nASegChanges <%3d>, lhholes <%4d>, rhholes
<%4d>, MaskVolToETIV <%7.5f>`, `wmstats: ` and the five statistics `%6.2f` (mean, deviation, minimum,
maximum, range), `cnrstats: <%6.3f> <%6.3f> <%6.3f>`; `outfile` gets `%s %3d    %4d %4d    %4d %4d
%4d  %4d %4d %4d   %7.5f %6.2f %6.2f %6.2f %6.2f %6.2f %6.2f %6.3f %6.3f %6.3f` (subject, the counts,
holes, total holes, the ratio, the five statistics and mean / deviation, the contrast's three); status
0 (FS-1111 when it does not open).

## 7. Defects (larmorx_core::defects)

FS-1101 (names cut by FreeSurfer's buffers), FS-1102 (names written past them), FS-1103 (numbers
not read), FS-1104 (`--vox`), FS-1105 (`--annot`, `--xfm2etiv` messages), FS-1106 (`MaskInvert`),
FS-1107 (`--gtm-default-seg-merge-choroid`), FS-1108 (`--replace-file` after replacements), FS-1109
(lists past 1,000), FS-1110 (`--slabel`), FS-1111 (unopened outputs), FS-1112 (`--qa-stats`
without its volumes), FS-1113 (no subject or `SUBJECTS_DIR` where the cache is read), FS-1114
(`--annot` without a colour table), FS-1115 (no volume where one goes on), FS-1116 and FS-1118 (ids
outside the colour table), FS-1117 (`--ctab-gca`). Their texts are in `crates/larmorx-core/src/
defects.rs`; the default corrects them and `reproduce` gives FreeSurfer's result where it is defined.

## 8. Validation plan

The open probe `validation/investigations/freesurfer-mri-segstats-probe/` (its README) runs the
official program and a candidate on 145 command lines over seeded synthetic inputs (every option,
the subject options on two synthetic subjects, the stand-alone options, the failures) and compares
exit status, messages and every file written; `reproduce` must give FreeSurfer's on every case but
those noting a defect of its `NOT_COMPARABLE` (counted); `fix` may differ only with a notice. The
replica (observed, 2026-11-22): seeds 1 and 2, 138 of 145 identical in `reproduce`, 7 counted, 0
different. A clean room is also checked against the 24 recorded steps of the reference runs
(recon-all's `aseg.stats` and `wmparc.stats`; the replica's record,
`docs/validation/freesurfer-mri-segstats.md`), and its golden cases (32^3, both modes).
