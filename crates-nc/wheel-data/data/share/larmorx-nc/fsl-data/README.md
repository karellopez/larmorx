# FSL's standard images and topup's settings in larmorx-nc

The three FSL standard-space images that FSL's `bet` script (its options `-S`, `-B`, `-A`,
`-A2`) and `standard_space_roi` read, so that larmorx-nc's replicas of those scripts run without
an FSL installation (the user's decision of 2026-11-04, `docs/decision-log.md`, "The data the
programs read"). They are the one exception to the repository's rule that no image data is
committed (CLAUDE.md, "Working on this machine").

Beside them, FSL's six `topup` settings files (`topup --config=<name>`), so that `--config=b02b0`
and the others work without FSL, and the three that sdcflows 2.17.1 chooses among for every
PEPOLAR estimate (`b02b0_1`, `_2`, `_4`, by the image's sizes) are there for larmorx's port of
sdcflows (the user's decision of 2026-11-09, `docs/decision-log.md`, "FSL's `topup` settings
files, a replica and a clean-room original"). They are this replica's part of that decision: the
main package's own settings, a clean-room original written from `specs/topup.md`, come later.

| File | Read by | Bytes |
|---|---|---|
| `standard/MNI152_T1_2mm.nii.gz` | `bet -S`, `-B` (standard_space_roi's reference), `-A`, `-A2` (FLIRT's reference); `standard_space_roi` (its reference unless `-ssref`) | 1,406,977 |
| `standard/MNI152_T1_2mm_brain_mask_dil.nii.gz` | `bet -S`, `-B`; `standard_space_roi -b` | 11,132 |
| `standard/MNI152_T1_2mm_eye_mask.nii.gz` | `bet -S` | 2,788 |
| `etc/flirtsch/b02b0.cnf` | `topup --config=b02b0` | 997 |
| `etc/flirtsch/b02b0_1.cnf` | `topup --config=b02b0_1`; sdcflows 2.17.1's PEPOLAR estimate when a size of the image is odd (`lx.sdcflows.select_topup_config`) | 1,002 |
| `etc/flirtsch/b02b0_2.cnf` | `topup --config=b02b0_2`; sdcflows 2.17.1's when every size is even, one not divisible by 4 | 997 |
| `etc/flirtsch/b02b0_4.cnf` | `topup --config=b02b0_4`; sdcflows 2.17.1's when every size is divisible by 4 | 997 |
| `etc/flirtsch/b02b0_7T.cnf` | `topup --config=b02b0_7T` | 1,072 |
| `etc/flirtsch/test_acqp.cnf` | `topup --config=test_acqp` | 1,101 |

1,427,063 bytes in all (the images 1,420,897, the settings 6,166); nothing else is here. Each
image is a byte-for-byte copy of `${FSLDIR}/data/standard/<file>` from FSL 6.0.7.23's data
package `fsl-data_standard` 2208.0 (`fsl-data_standard-2208.0-0.tar.bz2`, FSL's conda channel,
SHA-256 `2faefb4d366b6427cbf4e5c3e54c63c040ec9bcfe11b1f10848f89a1571b828a`); the package's own
record (`conda-meta/fsl-data_standard-2208.0-0.json`) gives the same size and SHA-256 for each
file. Each settings file is a byte-for-byte copy of `${FSLDIR}/etc/flirtsch/<file>` from FSL
6.0.7.23's `fsl-topup` 2203.5 (`fsl-topup-2203.5-ha905fa9_6.conda`, FSL's conda channel,
SHA-256 `24381a8ab0d7410bc86c440a57666a9c166aef3a48fcf0fb1080a81a60d8909f`): the files taken
from the package itself, identical to the installation's, and its record
(`conda-meta/fsl-topup-2203.5-ha905fa9_6.json`) gives the same size and SHA-256 for each
(`b02b0.cnf` and `b02b0_2.cnf` are the same bytes; sdcflows 2.17.1's sdist carries
`b02b0_1.cnf`, `_2` and `_4` byte for byte too). `MANIFEST.tsv` lists them (file, bytes, SHA-256,
package, version, the programs that read it); `crates-nc/larmorx-nc-cli/tests/fsl_data.rs`
checks that the directory holds exactly those files, unchanged, with this directory's `LICENSE`
identical to `crates-nc/LICENSE`.

## Licence

FSL's Licence (`LICENSE`): FSL's data package and `fsl-topup`'s settings files name no licence
of their own and are distributed as part of FSL, so its terms apply (non-commercial use, without financial return, the conditions
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
anything, naming the image and every place looked. `topup --config=<name>` looks for the file as
FSL's topup does (as named, then with `.cnf`, then, for a name without a directory, in
`${FSLDIR}/etc/flirtsch/`) and then, for such a name, in this copy's `etc/flirtsch/` (as named,
then with `.cnf`). larmorx's port of sdcflows (`lx.sdcflows.topup_config_file`) reads
`b02b0_1.cnf`, `_2` and `_4` from the FSL installation it is given, else from this copy where
larmorx-nc is installed, else refuses, naming both.

## Updating

From a newer FSL release: copy the files byte for byte from its `fsl-data_standard` and
`fsl-topup` packages (the same names, nothing else), update `MANIFEST.tsv` (sizes, SHA-256s, the
versions) and the packages' checksums above and in `NOTICE`, update the settings' tables in
`specs/topup.md` (§2.5) if a value changed, and re-make the records of the programs that read
them.
