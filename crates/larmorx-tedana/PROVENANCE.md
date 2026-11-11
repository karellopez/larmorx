# Provenance: larmorx-tedana

**Clean-room implementation. No tedana code was used.**

tedana is distributed under the GNU LGPL version 2.1 (only), so larmorx's main package carries a
clean-room original of its `t2smap` (CLAUDE.md rule 1, `docs/licensing.md`); the bit-exact
replica is the `larmorx-gpl` package's. This crate was written from 2026-11-11 (the clean-room
part of `t2smap`, relays 2a onwards) by implementers who never opened tedana's source (the
workspace clone `tedana/`, `tags/tedana-*`, any installed copy), the replica crates
(`crates-gpl/`, `crates-nc/`, `crates/larmorx-afni-replica`), the findings written from the
source (`docs/findings/tedana-t2smap.md` and the others of CLAUDE.md rule 1, the sections of
`docs/findings/upstream-defects.md` marked restricted), the replica author's probes
(`validation/investigations/t2smap/`), the replica's parity suite, random-testing descriptor,
permanent cases, record, evidence module and site page, or `docs/parts/`. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/t2smap.md`](../../specs/t2smap.md) (tedana 26.0.3) | the behaviour: the command line, its help and errors, the order of the run and its messages, the readers, the masks, dummy scans and `--exclude`, the adaptive mask, the groups and the fits, the floors, the cap, the RMSE and its table, the combination, the files, the defects TED-01 to TED-18 with their corrections and `reproduce`, the maths notices' one-pass rule (§15.2), what may change for speed (§16), the thresholds (§17.3) |
| tedana's documentation (https://tedana.readthedocs.io: "Multi-echo fMRI", "Outputs of tedana", the `t2smap` page) and the papers the spec names (Posse et al. 1999; Poser et al. 2006; Kundu et al. 2012, 2017; DuPre et al. 2021) | the method |
| The spec's black-box probe and model (`validation/investigations/t2smap-spec/`: `blackbox.py`'s cases and report form, `model.py`'s statement of §§3–11, `check.py`'s §17.3), written from the spec and runs of tedana's program | the probe's inputs and the checks |
| Black-box runs of tedana 26.0.3's program (the launcher `oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap`, through the oracle-output cache) and of `larmorx-gpl` 0.1.0rc1's `tedana t2smap` (PyPI's wheel, run as a program), on the probe's 54 cases and 14 added ones ([`validation/investigations/t2smap-original/`](../../validation/investigations/t2smap-original/README.md)) | the chained tracebacks of a 3D echo file and of nilearn's masking (`Image/mask shape mismatch`, then nilearn's `Mask shape: ...\n is different from img shape:...`), nilearn's mask in Fortran order, a NaN observation of a one-echo voxel (only that voxel NaN: no poisoning of its group, unlike an infinite one), the defaults' error texts, notices and their places, `reproduce`'s notices |
| larmorx's own Apache-2.0 code: `larmorx_core::numpy` (numpy's sums, means, deviations, percentiles, printing), `larmorx_core::math` (correctly rounded `logf`, `exp`, `log`; numpy's float32 `log` kernel and glibc 2.35's `exp`, `log` for `reproduce`), `larmorx_core::python` (argparse, Python's number printing, `json.dump`), `larmorx_core::defects`, `larmorx_io::nifti` (nibabel's reading and new headers), `larmorx_interp::nilearn::compute_epi_mask`, `larmorx_transform::numpy_linalg::LapackRelease` (permissive ports of numpy, SciPy, nibabel, nilearn and CPython, spec §13.9) | the libraries' arithmetic where the spec names them |

The log-linear fit is solved per voxel in closed form (the centred normal equations from the
per-echo sums of the observations, lstsq's rank rule), not by numpy's batched SVD (§13.4, §16):
`reproduce` is held to §17.3 there, as the orchestrator decided on 2026-11-11 (the replica is the
exact one).
