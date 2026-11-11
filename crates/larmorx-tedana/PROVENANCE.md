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

Added in relays 2b and 2c (2026-11-11), same rules:

| Input | What it gave |
|---|---|
| SciPy's documentation of `scipy.optimize.least_squares` (`method="trf"`) and `curve_fit`, Branch, Coleman and Li (1999), and the main package's port of SciPy 1.18.1's optimizers (`crates/larmorx-optim`: `least_squares`' trust-region reflective method, `curve_fit2`; Apache-2.0, a permissive port, spec §13.9) | the curve fit's method (`src/curvefit.rs`): Coleman-Li scaling, the exact trust-region subproblem, the reflected and anti-gradient steps, the radius rules, the tolerances, SciPy's failures in order |
| SciPy 1.15.3 and 1.18.1 run as black boxes (`curve_fit` and `least_squares` on one voxel's samples, with wrappers printing the arguments and results of its internal step functions; SciPy's files not opened) and numpy probed (`np.percentile`'s rank in 2.3.5 and 2.5.4; a float32-by-int32 3 × 3 product) | the start-edge rules and the infinite start's failure (`validation/investigations/t2smap-original/scipy_edges.py`), the percentile's rank in both numpy releases |
| nibabel 5.4.2's source (MIT, a permissive upstream: `analyze.py`, `spm99analyze.py`, `spm2analyze.py`, `freesurfer/mghformat.py`, `arrayproxy.py`, `volumeutils.py`, `imageclasses.py`, `loadsave.py`) | the ANALYZE 7.5 and MGH/MGZ readers as nibabel loads them (`src/formats.rs`): SPM2's byte-order guess, header fixes, data codes and errors, SPM2's scaling in float64, the origin affine and `.mat` sidecars, MGH's header, affine, frames and footer |
| Further black-box runs of tedana 26.0.3 and `larmorx-gpl` 0.1.0rc1 (the curve-fit, reader and message cases of `validation/investigations/t2smap-original/compare.py`) | the curve fit's messages and images (SciPy's OptimizeWarning once, the DEBUG failure counts), numpy's warnings with overflowing and infinite values, the warnings registry's reset, TED-03's, TED-13's and TED-16's messages |

## Files

| File | What it is |
|---|---|
| `src/lib.rs` | the run in §3's order: every step, message and output, the notices |
| `src/cli.rs` | tedana's command line (argparse's parser, help and errors from `larmorx_core::python::argparse`), the settings table, `run` |
| `src/options.rs` | the typed options and the libraries' choice (`newest`, `shared`) |
| `src/log.rs` | tedana's log lines, Python's warnings once per place, the errors' tracebacks (§12) |
| `src/input.rs` | the echoes, the reference, the user's and nilearn's computed masks, nilearn's masking path (§4) |
| `src/formats.rs` | ANALYZE 7.5 and MGH/MGZ as nibabel 5.4.2 loads them; NIfTI single files and pairs through `larmorx_io` (§4.1) |
| `src/volumes.rs` | `--exclude`'s parser and its errors (§5.2) |
| `src/adaptive.rs` | the adaptive mask: numpy's means and percentiles, the dropout and decay methods (§6) |
| `src/fit.rs` | the log-linear fit per voxel in closed form, its groups, TED-09 (§7.1, §7.2) |
| `src/curvefit.rs` | the curve fit: the trust-region reflective method on the per-echo means, its failures and covariance, `--fitmode ts` (§7.3, §7.4) |
| `src/maps.rs` | the floors, the limited maps, the cap, TED-05 and TED-18 (§8) |
| `src/rmse.rs` | the RMSE map and the confounds table (§9) |
| `src/combine.rs` | the optimal combination: T2*'s and PAID's weights, TED-15 (§10) |
| `src/output.rs` | the files: names, conventions, existence checks, headers, the table, the JSON files (§11) |
| `examples/t2smap_cleanroom.rs` | the original as a program, for black-box runs before it is wired into `larmorx tedana t2smap` |
| `tests/run.rs` | end-to-end tests of the command line and the files |
