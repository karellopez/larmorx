# `mri_robust_register` (FreeSurfer 8.2.0): behaviour spec

Status: the robust cost complete (queue item 3.2-13, part 1: the command line, every message of
the robust registration's path, its outputs, the exit statuses, the defects with their switches,
the validation plan, §7); not yet specified: the Powell registration's costs, the entropy images,
the test mode and the debug output (§6). Written by the replica's author (track C, relay 44) as the
spec writer: behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`), which
works from this file, the library modules' specs (`specs/freesurfer-robust.md` §16 for the
registration itself, `freesurfer-volume.md` for the transforms' reading, conversion, writing and
the resampling, `freesurfer-io.md` for reading and writing volumes, `freesurfer-filter.md` for the
Sobel image, `freesurfer-core.md` for the version options, the help renderer and error messages)
and black-box runs of the official program only.

Conventions: as `specs/mri_add_xform_to_header.md`'s. A **number printed as C++ prints it** is
`std::ostream`'s default (`%g` with 6 significant digits); a matrix printed in VNL's long format is
`specs/freesurfer-robust.md` §1's. An **error** is FreeSurfer's error message (with `errno`'s
message first when it is not 0); `ErrorExit(code)` ends with `code`'s status (`ERROR_NOFILE` 255,
`ERROR_BADPARM` 252, `ERROR_BADFILE` 251).

## 1. The command line

1. First, always: `8.2.0` and an empty line on standard output.
2. The version options (`specs/freesurfer-core.md`): when one is given and nothing else, status 0.
3. The options, while the next argument starts with `-` (an empty argument or a word without a dash
   stops them; the rest is ignored). One dash or two are dropped and the rest compared in upper case
   (ASCII); an option taking an argument takes the next one whatever it is. Each option prints its
   line (below) on standard output. Options and lines (`<v>` the argument as given; `<n>` read with
   `atoi`, `<x>` with `atof` and printed as C++ prints a number):
   - `MOV`/`M` `<v>`: `--mov: Using <v> as movable/source volume.`; `DST`/`D` `<v>`: `--dst: Using
     <v> as target volume.`; `LTA` `<v>`: `--lta: Output transform as <v> . ` (a final space).
   - `VOX2VOX`: `--vox2vox: Output transform as VOX2VOX. `; `AFFINE`/`A`: `--affine: Enabling
     affine transform!`; `ISOSCALE`: `--isoscale: Enabling isotropic scaling!`; `INITSCALING`:
     `--initscaling: Enabling initial scale adjustment based on image dimensions!`; `ISCALE`/`I`:
     `--iscale: Enabling intensity scaling!`; `ISCALEONLY` (sets the intensity scale and clears
     affine, isotropic scale, initial scaling, translation only, initial orientation and the initial
     transform): `--iscaleonly: No geometric transform, only iscale!`; `TRANSONLY`: `--transonly:
     Using only translation!`; `TRANSFORM`/`IXFORM` `<v>`: `--ixform: Using previously computed
     initial transform: <v>`; `INITORIENT`: `--initorient: Using moments for initial orientation!`;
     `NOINIT`: `--noinit: Skipping init of transform !`; `LEASTSQUARES`: `--leastsquares: Using
     standard least squares (non-robust)!` (it changes nothing but check 3 below).
   - `COST` `<v>` (compared in upper case): `LS`, `ROB`, `ROBENT` (robust, with entropy images), `MI`,
     `NMI`, `ECC`, `NCC`, `SCR`, `TB`, `LNCC`, `SAD`, `SB`; then `--cost: Using cost function: <V> !`
     (upper case); another word: `ERROR: cost function <V> unknown! ` on standard output, status 1.
   - `ENTRADIUS`/`RADIUS` `<n>`: `--entradius: Using local boxes with radius <n> = <2n+1> sides. `
     (32-bit wrap); `ENTDST` `<v>`: `--entdst: Output entropy dst image as <v>`; `ENTBALL`:
     `--entball: Using ball instead of local boxes. `; `ENTCORRECTION`: `--entcorrection: Using
     'correction' mode for entropy images. `; `ENTMOV` `<v>`: `--entmov: Output entropy mov image as
     <v>`; `SOBEL`: `--sobel: Using Sobel magnitude images.`.
   - `MAXIT` `<n>`: `--maxit: Performing maximal <n> iterations on each resolution`; `HIGHIT` `<n>`:
     `--highit: Performing maximal <n> iterations on highest resolution`; `EPSIT` `<x>`: `--epsit:
     Stop iterations when change is less than <x> . `; `NOMULTI`: `--nomulti: Will work on highest
     resolution only (nomulti)!`; `MAXSIZE` `<n>`: `--maxsize: Largest dimension < <n> . `; `MINSIZE`
     `<n>`: `--minsize: Smallest dimension > <n> . `; `SAT` `<x>`: `--sat: Using saturation <x> in
     M-estimator!`; `WLIMIT` `<x>`: `--wlimit: Using wlimit in satit <x>`; `SUBSAMPLE` `<n>`:
     `--subsample: Will subsample if size is larger than <n> on all axes!` (`<n>` at least 0) or
     `--subsample -1: Will not subsample on any scale!`; without its argument the error `\nERROR:
     specify min side lenght to start subsampling, e.g. --subsample 200.\n`, `ErrorExit(ERROR_BADPARM)`;
     `SATIT`: `--satit: Will iterate with different SAT to ensure outliers below wlimit!`; `SATEST`:
     `--satest: Trying to estimate SAT value!` (no effect); `POWELLTOL` `<x>`: `--powelltol: Using
     tolerance <x> in Powell optimizier!`; `DOUBLEPREC`: `--doubleprec: Will perform algorithm with
     double precision (higher mem usage)!`; `DEBUG`: `--debug: Will output debug info and files!`;
     `VERBOSE` `<n>`: `--verbose: Will use verbose level : <n>`.
   - Outputs: `WEIGHTS` `<v>`: `--weights: Will output weights (in target space) as <v> !`;
     `WARP`/`MAPMOV` `<v>`: `--mapmov: Will save resampled movable as <v> !`; `MAPMOVHDR` `<v>`:
     `--mapmovhdr: Will save header adjusted movable as <v> !`; `HALFMOV` `<v>`: `--halfmov: Will
     output final half way MOV !`; `HALFDST` `<v>`: `--halfdst: Will output final half way DST !`;
     `HALFWEIGHTS` `<v>`: `--halfweights: Will output half way WEIGHTS from last step to <v> !`;
     `HALFMOVLTA` `<v>`: `--halfmovlta: Will output half way transform (mov) <v> !`; `HALFDSTLTA`
     `<v>`: `--halfdstlta: Will output half way transform (dst) <v> !`; `MASKMOV` `<v>`: `--maskmov:
     Will apply <v> to mask mov/src !`; `MASKDST` `<v>`: `--maskdst: Will apply <v> to mask
     dst/target !`.
   - `TEST`: `--test: TEST-MODE ` (§6). `CONFORM`: `--conform: Will conform images to 256^3 and
     voxels to 1mm!` (no effect); `FLOATTYPE`: `--floattype: Use float images internally (independent
     of input)!`; `ONEMINUSW`: `--oneminusw: Will output 1-weights (zero=outlier), as in earlier
     versions!` (it switches the `1 - w` of §3 off); `NOSYM`: `--nosym: Will resample source to
     target (no half-way space)!`; `WHITEBGMOV`: `--whitebgmov: Will assume white background in
     MOV!`; `WHITEBGDST`: likewise `DST`; `UCHAR`: `--uchar: Will convert images to uchar (with
     re-scaling and histogram cropping)!`; `ISCALEOUT` `<v>` (sets the intensity scale): `--iscaleout:
     Will do --iscale and ouput intensity scale to <v>`; `ISCALEIN` `<v>`: `--iscalein: Will use init
     intensity scale`.
   - `HELP`/`H`: the help (`mri_robust_register.help.xml`, the help renderer), status 1.
   - Anything else: `\n\nERROR: Option: <arg> unknown !! \n\n` on standard error, `<arg>` as given
     but upper case after its dashes (`--foo` gives `--FOO`), status 1.
   - An option taking an argument given last: FS-1070 (§5).
4. No argument at all: the help, status 1. Then the checks, each failing with the help (but the
   last) and then `\n\nERROR: <text>\n\n` on standard error, status 1: `--mov` and `--dst` both
   (`Please specify --mov and --dst !  `); `--lta` unless `--iscaleonly` (`Please specify --lta !  `);
   `--satit`, a `--sat` above 0, a cost other than the robust one, or `--leastsquares` (`Please specify
   either --satit or --sat <float> !  `); a `--mapmov` name different from the `--weights` one
   (`Resampled input name (--mapmov) cannot be same as --weights output!`); `--maxit` above 0 (no help:
   `--maxit must be larger than 0 (max iterations on each resolution)`).

## 2. The registration (the robust cost)

The settings (in this order): rigid unless `--affine`; translation only; isotropic scale; the
intensity scale; intensity only (`--iscaleonly`); the verbose level (clipped to 0..2); the debug
level (1 sets verbose 2); the highest level's iterations; the initial transform, orientation and
scaling switches; double precision; symmetry; the saturation and the weight limit; the name (the
`--lta` name without the text from its last dot, unless that dot is its first character); the
subsampling size (when above 0); the minimum and maximum sizes.

Then an empty line and `reading source '<mov>'...`; the source read (FreeSurfer's reader, its
messages); not read: `<progname>: could not open MRI source <mov>.\n`, `ErrorExit(ERROR_NOFILE)`; more
than one frame: `<progname>: only pass single frame MRI source <mov>.\n`, likewise. The image as read
is kept for the outputs. Its mask (`reading source mask '<mask>'...`; not read: `<progname>: could not
open mask volume <mask>.\n`, `ERROR_NOFILE`). With `--sobel`: `Converting mov to sobel magnitute
image ...` and the image replaced by its Sobel magnitude (a `float` image with the source's header).
The mask applied (voxels where the mask is 0 set to 0). The same for the target (`reading target
'<dst>'...`, `MRI target`, `reading target mask`, `Converting dst to sobel magnitute image ...`).

The initial transform (`--ixform`): an empty line, `reading initial transform '<name>'...`, the file
read as FreeSurfer's transform reader reads it (`specs/freesurfer-volume.md`); none:
`<progname>: could not read transform file <name>`, `ErrorExit(ERROR_BADFILE)` (without a final
newline); a morph: FS-1072. Its first transform without a valid source geometry: ` WARNING: no
source geometry (RAS) in transform, assuming movable !!!`, the source's geometry taken (valid);
target likewise (` WARNING: no target geometry (RAS) in transform, assuming destination !!!`). Then
changed to RAS to RAS, its geometries replaced by the images' where they differ (the volume spec's
source-and-destination update, with the kept images), changed to voxel to voxel, and its first
matrix is the registration's initial transform (in the input images' voxels).

The initial intensity scale (`--iscalein`): the file opened for reading; it does not open (a folder
opens and gives 1): `Load Iscale input: no such file ( <name> )` (`errno`'s message first),
`ErrorExit(ERROR_BADFILE)`; otherwise its first number as C++ reads one: white space only gives 1,
no number 0, one too large `+-1.79769e+308`; the rest of the file ignored.

An empty line. `--uchar`: for each image `       -- changing data type from <type> to 0 (noscale = 0)...`
and the image changed to bytes as FreeSurfer's change of type does with the histogram's 0 and 0.999
(`specs/freesurfer-volume.md`; no result: `ERROR: MRISeqchangeType in MyMRI::setTypeUCHAR`, status
1). The white backgrounds: the image's outside value becomes its largest voxel (frame 0, compared as
`float`s). Then the registration's images are set (`specs/freesurfer-robust.md` §16.1; types kept
unless `--floattype`), and: with `--satit`, the saturation search (§16.6) and the multiresolution
registration; with `--nomulti`, the iterations on the full images; otherwise the multiresolution
registration (`--maxit`, `--epsit`).

## 3. The results

1. An empty line, `Final Transform:`, the final voxel transform (§16.7, with its message when the
   images were resampled) as `M` in VNL's long format and an empty line; ` Determinant : <d>` (C++)
   and an empty line. With `--affine` or `--isoscale`: ` Decompose into Rot * Shear * Scale : `, an
   empty line, the 3 x 3 part's polar decomposition (`specs/freesurfer-robust.md` §4.3) as `Rot` and
   `Shear` (long format, each followed by an empty line) and `Scale = diag([ <three long numbers> ])`
   and an empty line. With the intensity scale: `Intenstiy Scale Factor: <s>` (sic). An empty line.
2. `writing output transformation to <lta> ...`; an `--lta` name of 1,024 bytes or more: FS-1071. A
   transform array of one transform from the kept source: RAS to RAS (`converting VOX to RAS and
   saving RAS2RAS...`: the voxel transform, as `float`s, made RAS to RAS between the kept images) or
   voxel to voxel (`--vox2vox`: `saving VOX2VOX...`), the kept images' geometries as its source and
   destination, written by the LTA writer (by the name's extension: `.xfm`, `.dat`, ...; its messages
   if it cannot write; the program goes on).
3. With the intensity scale, a scale above 0 and `--iscaleout`: the file holds the scale as C++
   prints it (no newline); a file that cannot be created is skipped (its `errno` stays for later
   messages).
4. With the intensity scale, a scale above 0 and no entropy images: `Adjusting Intensity of MOV by
   <s>` and the kept source's voxels scaled (`specs/freesurfer-robust.md` §2.1, offset 0).
5. `--mapmov`: an empty line, `mapmov: resampling MOV to DST ...`, ` copy ... `, ` transform ... `, the
   kept source resampled onto the target's grid in the source's type by the transform with cubic
   B-splines (the volume spec's transform resampling; its messages, `MRItoBSpline degree 3`), the
   source's scan parameters copied, written (a failure ignored); an empty line, `To check aligned
   result, run:`, `  freeview -v <dst> <mapmov>`.
6. `--mapmovhdr`: an empty line, `mapmovhdr: Changing vox2ras MOV header (to map to DST) ...`; a copy of
   the kept source whose vox2ras is the RAS transform times its own; written; a failure: `ERROR:
   writing <name>` on standard output, status 1; then an empty line, `To check aligned result, run:`,
   `  freeview -v <dst> <mapmovhdr>`.
7. `--weights`: with weights (the robust cost): an empty line, `Writing out Weights ...`, the weights
   (in the target's space, `float`) written as `1 - w` (`w * -1 + 1` in `float`), or as they are with
   `--oneminusw`; `... overlay the weights:` and `  freeview -v <dst> <mapmov> <weights>:colormap=heat`
   (or with the `--mapmovhdr` name; neither: nothing). Without: `Warning: no weights could be created!
   Maybe you ran with --leastsquares??`.
8. Any half-way output: an empty line; without symmetry `ERROR: no half way space created (symmetry
   was switched off)!`; without weights `ERROR: no half way space created (not implemented for this
   cost function)!`; otherwise `Creating half way data ...`, the two half-way maps (§16.7) on standard
   error as `movhw` and `dsthw` (long format, each followed by an empty line), two transform arrays
   (source to half-way, target to half-way; RAS to RAS or voxel to voxel as above; the half-way space
   the weights' geometry), written for `--halfmovlta` and `--halfdstlta`; `--halfmov` (` creating
   half-way movable ...`) and `--halfdst` (` creating half-way destination ...`): the kept image
   resampled into the half-way space with cubic B-splines, its scan parameters copied, written;
   `--halfweights` (` saving half-way weights ...`): the weights resampled likewise (`1 - w` unless
   `--oneminusw`), written.
9. An empty line; `\nRegistration took <m> minutes and <s> seconds.` (the wall time since the command
   line was read, rounded to whole seconds); then ` Thank you for using RobustRegister! `, ` If you
   find it useful and use it for a publication, please cite: `, an empty line, the citation's four
   lines (` Highly Accurate Inverse Consistent Registration: A Robust Approach`, ` M. Reuter, H.D. Rosas,
   B. Fischl.  NeuroImage 53(4):1181-1196, 2010.`, ` http://dx.doi.org/10.1016/j.neuroimage.2010.07.020`,
   ` http://reuter.mit.edu/papers/reuter-robreg10.pdf`) and an empty line. Status 0.

## 4. Exit statuses

0; 1 (the help, an unknown option or cost, a failed check, the `--mapmovhdr` write, `--uchar`'s
change of type, FS-1070 by default, the paths of §6); 251, 252, 255 (`ErrorExit` above, and the
library's: the registration's own, `specs/freesurfer-robust.md` §16); 134 (FS-1070 and FS-1071 with
`reproduce`).

## 5. The defects (`larmorx_core::defects`)

- **FS-1070** (crash): an option that takes an argument given last: FreeSurfer aborts (a name:
  `terminate called after throwing an instance of 'std::logic_error'` and `  what():
  basic_string::_M_construct null not valid` on standard error, status 134) or dies of a segmentation
  fault (a number). By default `mri_robust_register: option <arg> needs an argument (FreeSurfer
  crashes here)` on standard error, status 1; `reproduce` the two lines and 134 for a name (the
  error for a number), noted.
- **FS-1071** (crash): an `--lta` name of 1,024 bytes or more: FreeSurfer aborts right after `writing
  output transformation to <lta> ...` (`*** buffer overflow detected ***: terminated` on standard
  error, status 134; nothing written). By default the name is used; `reproduce` aborts the same way,
  noted.
- **FS-1072** (memory): `--ixform` of a non-linear transform (a morph): read as an LTA. An error in
  both modes (`<progname>: <name> is not a linear transform (FreeSurfer reads the morph as one here)`,
  `ERROR_BADFILE`), noted.
- The library's: FS-1063, FS-1073 (double precision with the intensity scale), FS-1062, FS-1067,
  FS-1068, FS-1069 (`specs/freesurfer-robust.md` §14 and §16).

## 6. Not specified yet (the replica ends each with its own message and status 1)

The Powell registration (every `--cost` but `ROB` and `ROBENT`: the replica ends before reading the
images; FreeSurfer reads them, registers and writes as above), the entropy images (`--cost ROBENT`:
after `Converting mov to entropy image (radius <r> ) ... (can take 1-2 min)`), the test mode
(`--test`: after its line) and the debug output (`--debug`: the library's intermediate images and key
presses).

## 7. Validation plan (for the clean room)

- fMRIPrep's call (`--satit --iscale --mov T1.mgz --dst <T1w> --lta <out>`) on the six recorded
  heads' `T1.mgz` onto their `rawavg.mgz` (`<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/
  subjects/<subject>/mri/`), against the official program at one thread: the transform byte for byte,
  standard output with the time taken masked (**(observed)**: identical on five heads; on
  ds004173-sub-603309 the final voxel transform's x and z translations differ in their last two
  printed digits, 2e-13 of 210, an open item of the replica); with every output (`--mapmov`,
  `--weights`, the half-way outputs) on two heads: every file byte for byte (**(observed)**).
- Synthetic cases (small head-like images of every type, extent and orientation; every option and
  output, the failures of §1-§3), case by case against the official program in a folder of its own
  (`LC_ALL=C`, `OMP_NUM_THREADS=1`, `FS_DISABLE_LANG=1`, `USER` set and
  `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` fixing the LTA's creation line, no `DIAG_NO`):
  exit status, both streams (the time taken masked) and every file written; with `reproduce`
  identical but FS-1063's, FS-1067's to FS-1072's and the library's crash cases; the default differs
  only where a defect it corrects is noted.
