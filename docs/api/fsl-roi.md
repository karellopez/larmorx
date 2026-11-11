# fslroi and standard_space_roi (regions of an image)

`lx.fsl.fslroi` (command line `larmorx fsl fslroi`): FSL 6.0.7.23's `fslroi`, a box of an image
(`xmin xsize ymin ysize zmin zsize`), a range of its volumes (`tmin tsize`) or both, in the
image's own data type, its header's transforms moved to the box's corner; a size of -1 is the
rest of the axis, and the box may reach outside the image (zeros there). The indices are NIfTI
voxel indices, from 0.

`lx.fsl.standard_space_roi` (command line `larmorx fsl standard_space_roi`): FLIRT's script
(fsl-flirt 2111.4), which registers the input to a standard-space reference with FLIRT
(`MNI152_T1_2mm` unless `-ssref` gives another), takes the reference's field of view (less its
outer voxel) or a standard-space mask back into the input's space with the inverse matrix, and
masks the input with it and/or cuts the input down to its bounding box. `bet -S` and `bet -B`
run it (`-b -d`, `-maskFOV -roiNONE -d`).

**Implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)):
- the **clean-room original** (since 2026-11-05; `crates/larmorx-fsl`, `larmorx_fsl::cli::fslroi`
  and `::ssroi`, Apache-2.0, written from
  [`specs/standard_space_roi.md`](../../specs/standard_space_roi.md), FSL's documentation and
  black-box runs of FSL 6.0.7.23: [PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)), in this
  process; `standard_space_roi` runs the original's own FLIRT, `convert_xfm`, `fslmaths`,
  `fslstats` and `fslroi`, and fslpy's `imcp`, in the same process: no FSL installation is
  needed;
- the bit-exact **replica** (2026-11-04), `larmorx-nc fsl fslroi` and `larmorx-nc fsl
  standard_space_roi` (package `larmorx-nc`, under the FSL Licence's terms:
  [PROVENANCE](../../crates-nc/larmorx-nc-flirt/PROVENANCE.md)), which runs every program in the
  replica's own process.

`implementation="auto"` runs the replica when its program is found, else the clean-room
original; `"clean-room"` (`"original"`, 0.1.0rc1's spelling, still works) and `"replica"` choose. The command line follows
`--implementation` (before the family) or `LARMORX_IMPLEMENTATION` the same way.

**Built for speed** (the original): the registration to the reference is almost all of
`standard_space_roi`'s time; besides it, without `-d` the temporary images are written
uncompressed (they are removed at the end), and when the field of view is both the mask and the
region it is resampled into the input's space once. `fslroi` converts the stored values once (or
moves them, where the type is kept) and copies the box row by row in parallel.

**Status: `experimental`** (the records come with BET's options, in the same part).

## FSL's standard images

`standard_space_roi` reads `MNI152_T1_2mm` (the default reference) and, for `-b`,
`MNI152_T1_2mm_brain_mask_dil`, from FSL's `data/standard` (FSL 6.0.7.23's `fsl-data_standard`
2208.0; their checksums in [`specs/bet.md`](../../specs/bet.md) §5.4). They are FSL's data
(the FSL Licence: non-commercial use; the MNI152 template also under its MNI notice): the
`larmorx` package (Apache-2.0) never ships them. `-ssref` (and masks given with `-maskMASK`,
`-roiMASK`) need none of them.

**The original** looks for each, in this order (`specs/bet.md` §5.4):
1. the directory the call names: `fsl_data=` in Python, `LARMORX_FSL_DATA` otherwise (laid out
   as `$FSLDIR/data`: `<dir>/standard/MNI152_T1_2mm.nii.gz`, any of FSL's image extensions); when
   it is given, nothing else is searched;
2. larmorx-nc's copy, when its program is found (`LARMORX_NC_BIN`, the Python environment's
   scripts directory, `PATH`): `<prefix>/share/larmorx-nc/fsl-data/standard/`, read as data only
   (nothing of larmorx-nc is run), each file checked against FSL's SHA-256;
3. `$FSLDIR/data/standard/`;
4. larmorx's data cache: `LARMORX_DATA_DIR`, else the user cache directory (`$XDG_CACHE_HOME` or
   `~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%\larmorx\cache` on Windows,
   `larmorx` in it), under `fsl/fsl-data_standard-2208.0-0/standard/`. When the image is not
   there, FSL's package is downloaded once from FSL's site
   (`https://fsl.fmrib.ox.ac.uk/fsldownloads/fslconda/public/noarch/fsl-data_standard-2208.0-0.tar.bz2`,
   196 MB, with the platform's `curl`, else `wget`), checked against its size and SHA-256, and
   only its three standard images kept (each checked against its SHA-256, with a
   `MANIFEST.tsv`); a message names FSL's Licence when it downloads. `larmorx fetch
   fsl-standard [<package>]` fills the cache ahead and prints its directory. For a machine
   without the network, copy the package file there and name it (`larmorx fetch fsl-standard
   <file>`, or `LARMORX_FSL_DATA_PACKAGE=<file>`, read instead of the download), or copy a filled
   cache directory.

Files from larmorx-nc's copy and from the cache must match FSL's checksums (a damaged file is
refused, naming it); files from the named directory or `$FSLDIR` are used as they are (another
FSL release's images give that release's results, as in FSL). When none has the image, the
original refuses before running anything (exit 1), naming the image and every place looked
(`larmorx fsl standard_space_roi: FSL's standard image MNI152_T1_2mm was not found (looked in:
...)`).

**The replica** looks in `LARMORX_FSL_DATA` (alone when set), then `$FSLDIR/data`, then its
own copy (`crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/`, installed by its wheel); it
refuses likewise (`larmorx-nc: error: standard_space_roi: FSL's data image
standard/MNI152_T1_2mm was not found: ...`, exit 1).

## Options

| `fslroi` | Python (`lx.fsl.fslroi(image, ...)`) |
|---|---|
| `<tmin> <tsize>` | `t=(tmin, tsize)` |
| `<xmin> <xsize> <ymin> <ysize> <zmin> <zsize>` | `x=`, `y=`, `z=` (an axis not given: `(0, -1)`) |
| all ten numbers | `x=`, `y=`, `z=`, `t=` |

| `standard_space_roi` | Python (`lx.fsl.standard_space_roi(image, ...)`) |
|---|---|
| `-maskFOV`, `-maskMASK <mask>`, `-maskNONE` | `mask="fov"`, `mask=<image>`, `mask="none"` (`None`: the script's default) |
| `-roiFOV`, `-roiMASK <mask>`, `-roiNONE` | `roi="fov"`, `roi=<image>`, `roi="none"` |
| `-ssref <ref>` | `reference=` |
| `-altinput <image>` | `alt_input=` |
| `-b` | `betpremask=True` (given before `mask` and `roi`) |
| `-d` | always (the matrices are returned: `to_standard`, `from_standard`) |
| FLIRT's options after the script's | `flirt_args=[...]` |

The script's own rules hold: the last `-mask*` and `-roi*` given win, `-b` sets the mask, its 0.5
threshold and `-roiNONE` (a later `-maskMASK` keeps the threshold), every option of the script
comes before FLIRT's, and an option the script does not know ends its options (FLIRT gets it and
the rest).

## Deliberate differences (D13; `LARMORX_UPSTREAM_DEFECTS=reproduce` gives FSL's)

- **FSL-11** (`fslroi`, `standard_space_roi`): with `FSLOUTPUTTYPE` unset FSL's fslroi stops with
  FSL's message at its output (exit 1), and standard_space_roi's script goes on after its FLIRT
  stops there (exit 0, no output). Both implementations, the clean-room original and the replica,
  write the images as `NIFTI_GZ` (`.nii.gz`: the bytes `FSLOUTPUTTYPE=NIFTI_GZ` gives) and name
  FSL-11 on one line per command, in the same words (the replica's line starts `larmorx-nc:`);
  `reproduce` stops as FSL does. Command line only: the Python functions set the type themselves.
- **FSL-211** (`standard_space_roi`): the script expands its arguments unquoted; by default each
  name is one name, whatever its blanks; `reproduce` splits them as the shell does (with the
  shell's `[: too many arguments`). A name with `*`, `?` or `[` in `reproduce` is refused: the
  shell would expand it as a file-name pattern, which is not reproduced.
- **FSL-212**: the cleanup (`rm ${out}_tmp*`) removes only this run's temporary files by default.
- **FSL-213**: a variable `maskthr` in the environment is ignored by default (the script applies
  it to the transformed mask unless `-b` sets it).
- **FSL-214**: by default the script stops at the first program that fails (exit 1, its
  temporary files removed unless `-d`); FSL's goes on and exits with its cleanup's status.
- **FSL-215** (`fslroi`): fslroi exits 1 after the errors it reports (FSL's exits 0).
- **FSL-217** (`fslroi`): an int8 input is written as int8 with its values (FSL writes the same
  bytes as uint8, so a negative value becomes 128 to 255); named whenever the input is int8, with
  the number of negative values; `reproduce` writes FSL's uint8 (named where a value is negative).

**imcp's conversion.** `standard_space_roi -maskNONE -roiNONE` copies its input with fslpy's
`imcp`, which converts an input of another file type than `FSLOUTPUTTYPE` with nibabel. The
original converts it (the same header and data in the output's file layout and compression: a
`.nii.gz` input becomes FSL's `.nii` byte for byte, the record's `ssroi/phantom-none-convert`);
the replica refuses that case clearly (exit 1) in both modes.

The validation: the BET records (`docs/validation/fsl-bet.md` for the original,
`fsl-bet-replica.md` for the replica), categories `ssroi/` and `fslroi/` and the `defects/`
cases of FSL-211 to FSL-215. The original's `standard_space_roi` is FSL's bit for bit where its
FLIRT gives FSL's matrix (an adult's T1w, a Philips T1w and a T2w with the defaults); elsewhere
its matrices are within FLIRT's 12-dof threshold and, given FSL's matrix, every file is FSL's
(`docs/findings/fsl-bet-original.md`, "standard_space_roi and fslroi").
