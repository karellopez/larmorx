# FSL's standard images in larmorx-nc

The three FSL standard-space images that FSL's `bet` script (its options `-S`, `-B`, `-A`,
`-A2`) and `standard_space_roi` read, so that larmorx-nc's replicas of those scripts run without
an FSL installation (the user's decision of 2026-11-04, `docs/decision-log.md`, "The data the
programs read"). They are the one exception to the repository's rule that no image data is
committed (CLAUDE.md, "Working on this machine").

| File | Read by | Bytes |
|---|---|---|
| `standard/MNI152_T1_2mm.nii.gz` | `bet -S`, `-B` (standard_space_roi's reference), `-A`, `-A2` (FLIRT's reference); `standard_space_roi` (its reference unless `-ssref`) | 1,406,977 |
| `standard/MNI152_T1_2mm_brain_mask_dil.nii.gz` | `bet -S`, `-B`; `standard_space_roi -b` | 11,132 |
| `standard/MNI152_T1_2mm_eye_mask.nii.gz` | `bet -S` | 2,788 |

1,420,897 bytes in all; nothing else is here. Each is a byte-for-byte copy of
`${FSLDIR}/data/standard/<file>` from FSL 6.0.7.23's data package `fsl-data_standard` 2208.0
(`fsl-data_standard-2208.0-0.tar.bz2`, FSL's conda channel, SHA-256
`2faefb4d366b6427cbf4e5c3e54c63c040ec9bcfe11b1f10848f89a1571b828a`); the package's own record
(`conda-meta/fsl-data_standard-2208.0-0.json`) gives the same size and SHA-256 for each file.
`MANIFEST.tsv` lists them (file, bytes, SHA-256, package, version, the programs that read it);
`crates-nc/larmorx-nc-cli/tests/notices.rs` checks that the directory holds exactly those files,
unchanged, with this directory's `LICENSE` identical to `crates-nc/LICENSE`.

## Licence

FSL's Licence (`LICENSE`): FSL's data package names no licence of its own and is distributed
as part of FSL, so its terms apply (non-commercial use, without financial return, the conditions
passed on to every receiver), which only larmorx-nc meets; larmorx's main package (Apache-2.0)
does not carry these files. The MNI152 average also carries the McConnell Brain Imaging
Centre's notice (`NOTICE`).

## How the wheel installs them

This directory is maturin's wheel data directory (`crates-nc/pyproject.toml`, `data =
"wheel-data"`): `wheel-data/data/` is the wheel's `larmorx_nc-<version>.data/data/`, which pip
installs under the installation's prefix, so the files land in
`<prefix>/share/larmorx-nc/fsl-data/`, beside the program's `<prefix>/bin/larmorx-nc` (or
`<prefix>\Scripts\larmorx-nc.exe`). The replica looks for each image in this order
(`larmorx_nc_core::fsldata`): `LARMORX_FSL_DATA` (larmorx's explicit choice, a directory laid
out as `${FSLDIR}/data`; nothing else is searched when it is set), `${FSLDIR}/data` when `FSLDIR`
is set and holds the image, then this copy; when none has it, the script refuses before it runs
anything, naming the image and every place looked.

## Updating

From a newer FSL release: copy the files byte for byte from its `fsl-data_standard` package
(the same names, nothing else), update `MANIFEST.tsv` (sizes, SHA-256s, the version) and the
package's checksum above and in `NOTICE`, and re-make the records of the programs that read
them.
