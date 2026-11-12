# Provenance: larmorx-gpl-tedana

**Replica: translated from tedana's source.** This crate is a step-by-step Rust translation of
tedana 26.0.3's `t2smap` (the T2* and S0 maps of multi-echo fMRI and their optimal combination),
written to give tedana's output bit for bit with the numpy and SciPy it runs on. It is part of the
`larmorx-gpl` package (`docs/licensing.md`, "A replica package is a program"); its clean-room
original, from `specs/t2smap.md`, will be `crates/larmorx-tedana` in the main package and will use
none of this crate's code.

## The upstream

- tedana 26.0.3 (released 2026-04-03): git tag `26.0.3`, commit
  `e0a18e331abd6323c926522999a1c4fd3e695d1c`; PyPI sdist `tedana-26.0.3.tar.gz`, SHA-256
  `3d767a09c2ae3a1e8fc0c62e8c86655de677ebd6424af1f7818af0e3c3aa5b2f` (extracted in the workspace's
  `tags/tedana-26.0.3/`). fMRIPrep 25.2.6's image has tedana 25.0.1 (`tags/tedana-25.0.1/`), the
  reference of the final comparisons only.
- Licence: the GNU Lesser General Public License version 2.1 (tedana's `LICENSE`, the licence's
  verbatim text, SHA-256 `d716613abed076e2db04e79e7d402d69a10bd57c0d990321a568db528b5795ad`; no
  source file carries a notice; `__about__.py`: "Copyright 2023, The ME-ICA Developers"). This
  translation is distributed under the GNU General Public License version 3 only, as section 3 of
  the LGPL 2.1 allows (`lib.rs`); every file says so in its header (SPDX `GPL-3.0-only`).

## Files translated

| larmorx | tedana 26.0.3 |
|---|---|
| `src/parser.rs` | `tedana/workflows/t2smap.py` (`_get_parser`, `_main`'s use of it), `tedana/workflows/parser_utils.py` (`is_valid_file`) |
| `src/log.rs` | `tedana/utils.py` (`setup_loggers`, `teardown_loggers`: the stream format, levels) |
| `src/data.rs` | `tedana/io.py` (`load_ref_img`, `load_data_nilearn` and its `_mask_4d`, `_convert_to_nifti1`), `tedana/utils.py` (`load_mask`, the user-mask branch) |
| `src/adaptive.rs` | `tedana/utils.py` (`make_adaptive_mask`) |
| `src/decay.rs` | `tedana/decay.py` (`monoexponential`, `_fit_single_voxel`, `fit_monoexponential`, `fit_loglinear`, `fit_decay`, `fit_decay_ts`) |
| `src/maps.rs` | `tedana/decay.py` (`_apply_t2s_floor`, `modify_t2s_s0_maps`, `rmse_of_fit_decay_ts`, `monoexponential`), `tedana/utils.py` (`unmask`) |
| `src/combine.rs` | `tedana/combine.py` (`make_optcom`, `_combine_t2s`, `_combine_paid`) |
| `src/outputs.rs` | `tedana/io.py` (`OutputGenerator`: `_determine_extension`, `get_name`, `save_file`, `save_img`, `save_json`, `save_tsv`, `save_self`, `register_mask`; `CustomEncoder`, `prep_data_for_json`), `tedana/resources/config/outputs.json` (the names of t2smap's files), `tedana/utils.py` (`get_system_version_info`'s fields) |
| `src/data.rs`'s `compute_mask` | `tedana/utils.py` (`load_mask`, the branch without a mask), `tedana/io.py` (`load_ref_img`: the reference nilearn masks) |
| `src/workflow.rs` | `tedana/workflows/t2smap.py` (`t2smap_workflow`, `_main`), `tedana/utils.py` (`check_te_values`, `parse_volume_indices`), `tedana/io.py` (`OutputGenerator.__init__`, `_infer_prefix`), `tedana/resources/config/outputs.json` (the confounds table's names) |
| `examples/t2smap_stages.rs`, `examples/t2smap_parse.rs` | (larmorx's own: the validation's programs) |

The line numbers the log prints (`%(module)s:%(funcName)s:%(lineno)d`) are those of tedana
26.0.3's files.

larmorx's own addition (2026-11-11, the user's decision that a tool's own thread option wins
only when it is given): without t2smap's `--n-threads`, the fits and the files run on the run's
thread count (`workflow::Env::threads`: larmorx's `--n-threads` before the family, else
`OMP_NUM_THREADS`, else every logical CPU), not tedana's default of one; `parser::Settings::
n_threads_given` tells the option given apart from its default (argparse's actions seen,
`larmorx_core::python::argparse::Parser::parse_given`). The results do not depend on it.

## What other libraries contribute

The numerics tedana calls are translated in the main package's Apache-2.0 crates, which this
crate uses: numpy 2.5.4 and 2.3.5's reductions, percentiles, float32 `log` and array printing
(`larmorx_core::numpy`), `lstsq` with LAPACK's `dgelsd` and OpenBLAS's kernels
(`larmorx_transform::numpy_linalg::lstsq`), SciPy 1.18.1 and 1.15.3's `curve_fit`
(`larmorx_optim`), nibabel 5.4.2's NIfTI reading and scaling (`larmorx_io`), CPython 3.12.16's
`argparse`, `textwrap` and number parsing and printing (`larmorx_core::python`). Re-expressed here,
beside tedana's code: nilearn 0.14.1's `masking.apply_mask` path that tedana's loader falls back
on (`check_niimg_3d`'s squeeze of a one-volume 4D mask, `load_mask_img`'s empty-mask error,
`apply_mask_fmri`'s affine and shape checks and their messages, `smooth_array`'s zeroing of
non-finite values; and, for the files, `masking.unmask` and `image.new_img_like(...,
copy_header=False)`: a fresh header from the mask's affine, `load_mask_img`'s empty-mask error;
BSD-3-Clause, "Copyright (c) The nilearn developers"), nibabel 5.4.2's fresh `Nifti1Image` header
(`SpatialImage.update_header`'s affine test, `_affine2header`, `set_zooms` and its errors; MIT),
pandas 3.0.6's `to_csv` of a float frame (numpy's `astype(str)`, `na_rep`; BSD-3-Clause), numpy's
`average` with its two summation orders (the weights of `fitmode ts` are echo-innermost in
memory: pairwise), numpy's layout of a
boolean index on the last axis (the excluded-volume copy, whose temporal mean numpy sums in
order), and nibabel's choice of image class by extension and magic. Since relay 7b (random
testing, 2026-11-08): nibabel's `img.affine` through `larmorx_io`'s `best_affine_nibabel` (its
qform in x87 extended precision); numpy's floating-point warnings of `_methods.py`'s `_mean`
and `_var` (reduce, subtract), `_nanfunctions_impl.py`'s `nanvar` (subtract) and
`_function_base_impl.py`'s `_lerp` (subtract, multiply, add) on infinite values, with their
lines in numpy 2.5.4 and 2.3.5 (`decay::Libraries`); numpy 2.3.5's `percentile` dividing `q` by
`a.dtype.type(100)` (a float32 rank for float32 data); pandas' `DataFrame` shape error of an
empty table (BSD-3-Clause). Since relay RS2 of the clean-room part (2026-11-12): nibabel
5.4.2's loading of ANALYZE 7.5 pairs (`Spm2AnalyzeHeader`, `Spm99AnalyzeImage`'s `.mat` affine)
and MGH/MGZ images (`MGHHeader`, `MGHImage`) through the main package's Apache-2.0 reader of that
loading, `larmorx_tedana::formats` (the clean-room original's module, called as a library; this
crate edits none of it), with nibabel's own choices this crate adds (`data.rs`: the classes'
order, the reading where `_convert_to_nifti1` reads, `from_matvec`'s float32 MGH translation,
`array_from_file`'s file name; MIT).

## Verification (relay RS2 of the clean-room part, 2026-11-12)

- The parity suite's `formats/` cases (`validation/investigations/t2smap/replica/check_formats.py`,
  20: ANALYZE and MGH inputs, masks and z-concatenated files, nibabel's failures): bit for bit
  with tedana in both environments; `rs2_replayed_formats.py`: the clean-room part's 104 replayed
  random cases with ANALYZE or MGH inputs, every file identical; `rs2_descriptor_formats.py`: the
  descriptor's ANALYZE and MGH draws, 30 of 30 agree.
- TED-46 under the curve fit (`specs/t2smap.md` §7.3, measured by
  `validation/investigations/t2smap-spec/ted46_curvefit.py`): the default's exact solution, unit
  test `decay::tests::equal_samples_take_the_curve_fits_exact_solution_by_default`.
- Golden (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`): 16 cases (6 added: TED-46 in the
  log-linear fit and the curve fit in both modes, TED-31, an ANALYZE 7.5 pair, an MGH image).
- The record (`docs/validation/tedana-t2smap-replica.md`): 102 cases from d5288693 (standard tier): 62 bit-identical in both environments, 40 rejected by both alike, 0 failures; fMRIPrep's calls from the oracle runs made again with OpenBLAS's Haswell kernels; the 20 `formats/` cases.

## Verification (relay 7b, 2026-11-08)

Random testing (`difftest --tools tedana-t2smap`; `validation/difftest/triage.toml`): the trial
`oracles/difftest/2026-11-08-t2smap-replica` (3,886 cases) and its fixes; every case of the trial
that did not agree agrees with the final build (70 of 70,
`validation/investigations/t2smap/recheck_trial.py`); 17 permanent cases in
`validation/difftest/cases/tedana-t2smap/`.

## Verification (relay 5a, 2026-11-08)

`validation/investigations/t2smap/replica/` (its README):

- `check_stages.py`: tedana's own adaptive mask and fit (`make_adaptive_mask`, `fit_decay`,
  `fit_decay_ts` as they return) against the replica's (`LARMORX_UPSTREAM_DEFECTS=reproduce`), bit
  for bit, and the messages up to "Computing T2* map" line by line: 18 cases, all identical, in
  the newest environment (tedana 26.0.3, numpy 2.5.4, SciPy 1.18.1) and in the shared one (numpy
  2.3.5, SciPy 1.15.3, with `LARMORX_TEDANA_LIBRARIES=shared`) — fMRIPrep's four multi-echo
  calls (ds003592 sub-280, ds000210 sub-02 rest and cuedSGT, ds000258 sub-21262; curvefit, and
  loglin for ds003592) and 13 synthetic cases (float32 and scaled int16 echoes, TEs in seconds and
  milliseconds, three and four echoes, `--fitmode ts` with both fits, every `--masktype`,
  `--exclude` with `--dummy-scans`, `--n-independent-echos`, z-concatenated data, a 4D mask (the
  nilearn path), an infinite sample and a NaN).
- `check_parser.py`: the command line against tedana's parser in-process: 109 cases (help at 19
  widths, every error argparse and `is_valid_file` give, abbreviations, `=` forms, clusters,
  negative numbers, `--`), all identical (exit status, standard output and error, values).
- `check_failures.py`: 24 failure cases (the TED defects' and tedana's own clear errors; 13 added
  by the clean-room part's relay RS1, 2026-11-12: TED-31, TED-46, the chained tracebacks, the
  temporal means' and the cast's warnings, the thresholds' printing, `--fitmode ts` with no
  volume): with `reproduce` tedana's exit status and last line, by default the defect named.

## Verification (relay 5b, 2026-11-08)

`validation/investigations/t2smap/replica/check_outputs.py`: tedana 26.0.3's `t2smap` program
against `larmorx-gpl tedana t2smap` (`LARMORX_UPSTREAM_DEFECTS=reproduce`), every output file
(images' decompressed bytes: every header field and every value bit for bit; the confounds
table's text; the registry; `dataset_description.json` but for the host and Python fields, D6;
`t2smap_call.sh`), the logger's lines, the exit status and the last error line: 33 cases, all
identical, in the newest environment (numpy 2.5.4, SciPy 1.18.1) and in the shared one (numpy
2.3.5, SciPy 1.15.3): fMRIPrep's four multi-echo calls (curvefit; ds003592 also loglin), the 13
synthetic cases of `check_stages.py`, and 15 more (no mask: nilearn's `compute_epi_mask` on
float32, scaled int16 and z-concatenated data; PAID in both fit modes; `--verbose` with the
`orig` convention and a prefix, and per volume; nine echoes per volume and over all volumes;
one volume; constant voxels with PAID (TED-15); an image nilearn finds no mask in (TED-03); an
existing output without and with `--overwrite`). Logs: `oracles/logs/t2smap-1-relay5b-outputs-*.log`.
