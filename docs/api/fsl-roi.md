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
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)). Both programs
have their bit-exact **replica** (2026-11-04), `larmorx-nc fsl fslroi` and `larmorx-nc fsl
standard_space_roi` (package `larmorx-nc`; `crates-nc/larmorx-nc-avwutils` and
`crates-nc/larmorx-nc-flirt`, the region itself and fslpy's `imcp` in `larmorx-nc-core`, under the
FSL Licence's terms: [PROVENANCE](../../crates-nc/larmorx-nc-flirt/PROVENANCE.md)). Their
clean-room originals are to be written from their specs (the clean-room catch-up); until then
`implementation="auto"` and `"replica"` need `larmorx-nc` (otherwise `ReplicaNotFoundError`) and
`"original"` raises `NotImplementedError`. The script runs every program in the replica's own
process (FLIRT, `convert_xfm`, `fslval`, `fslmaths`, `fslstats -w`, `fslroi`, fslpy's `imcp`, GNU
`dc` and `rm`): no FSL installation is needed.

**Status: `experimental`** (the records come with BET's options, in the same part).

## FSL's standard images

`standard_space_roi` reads `MNI152_T1_2mm` (the default reference) and, for `-b`,
`MNI152_T1_2mm_brain_mask_dil`, from FSL's `data/standard`. The replica looks for each, in this
order: `LARMORX_FSL_DATA` (a directory laid out as `$FSLDIR/data`; when it is set nothing else is
searched), `$FSLDIR/data`, and the copy larmorx-nc carries (`<prefix>/share/larmorx-nc/fsl-data`
beside its program, which the wheel installs: FSL 6.0.7.23's files byte for byte, with FSL's
Licence, MNI's notice and their manifest; `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/`). When none has the image it refuses before running anything, naming the
image and every place looked (`larmorx-nc: error: standard_space_roi: FSL's data image
standard/MNI152_T1_2mm was not found: ...`, exit 1). `-ssref` (and masks given with `-maskMASK`,
`-roiMASK`) need none of them. The Python wrapper takes `fsl_data=`.

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

**Not replicated yet:** fslpy's `imcp` converts an input of another file type than
`FSLOUTPUTTYPE` with nibabel (`standard_space_roi -maskNONE -roiNONE` on such an input); the
replica refuses that case clearly (exit 1) in both modes.
