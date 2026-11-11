# fMRIPrep's confound estimation: the inventory's tools

The tools behind the confounds' part (the port of nipype's `algorithms/confounds.py`,
niworkflows' confound interfaces and fMRIPrep's `init_bold_confs_wf` steps, with the numpy,
SciPy, OpenBLAS/LAPACK, nitime and pandas arithmetic underneath). The findings they produced go
into `docs/findings/confounds-*.md`. Sources read (permissive, `<workspace>/tags/`): fMRIPrep
25.2.7 and 25.2.6, nipype 1.12.0, niworkflows 1.15.4, nitime 0.12.1 and 0.11, nitransforms
25.1.0, SciPy 1.18.1, 1.15.3 and 1.15.2, numpy 2.5.4, 2.3.5 and 2.2.6, pandas 3.0.6 and 2.2.3,
OpenBLAS 0.3.34 and 0.3.30.

The survey tools read the fMRIPrep 25.2.6 oracle runs under
`<workspace>/oracles/fmriprep-25.2.6/`; set `LARMORX_WORKSPACE=<workspace>` (by default the
folder that holds the repository's checkout). The probes run in whichever Python environment
starts them: run each in the kept environments (the shared `.venv`: numpy 2.3.5, SciPy 1.15.3;
`oracles/python-sdcflows-2.17`: numpy 2.5.4, SciPy 1.18.1, pandas 3.0.6) and, for fMRIPrep
25.2.6's own libraries, inside its image (`apptainer exec <image> python -I <tool>`).

| Tool | What it shows | Run |
|---|---|---|
| `survey_runs.py` | every oracle run's confounds files (rows, columns), the confounds workflow's nodes in its working directory, the BOLD series' shape, TR and dummy scans | `python -I survey_runs.py` |
| `survey_sizes.py` | for every confounds workflow (36 in 21 runs): the series length, the skipped volumes, the voxel count of each CompCor mask, the `dgesdd` path numpy's SVD takes (`MNTHR = int(min(m, n) · 11 / 6)`: 3t, 5t, 5, 3), whether LAPACK blocks (`min(m, n) > 128`), the cosine regressors' count `K`; so which paths fMRIPrep's runs reach | `python -I survey_sizes.py` (needs nibabel, numpy) |
| `node_inputs.py` | the execution inputs (and, with `--outputs`, outputs) of every node of `bold_confounds_wf` in one run's working directory, from nipype's `_report/report.rst`, with whether each input file still exists (`+`/`-`) | `python -I node_inputs.py <run> [node ...] [--outputs]` |
| `survey_column_order.py` | the order of each expanded variable's four columns in all 36 confounds files: niworkflows' `parse_formula` builds it from a `set` of strings, so it follows Python's per-process hash seed (169 groups `_power2` first, 155 `_derivative1_power2` first) | `python -I survey_column_order.py` |
| `probe_numpy_simd.py` | numpy's float64 `cos` on the cosine basis' arguments `(π/n)(t + 0.5)k` (n 2 to 1200, k 1 to 40) against `math.cos` (glibc's, equal on all 4,131,440) and the correctly rounded value (glibc 2.35 is not correctly rounded on 0.15 %); and numpy's complex64 multiply against the plain formula | `python -I probe_numpy_simd.py`, then again with `NPY_DISABLE_CPU_FEATURES` set (see its header) |
| `probe_cmul.py` | which formula numpy's complex64 multiply follows (candidates with one fused multiply-add, evaluated exactly and rounded once): `re = fma(a, c, -(b·d))`, `im = fma(a, d, b·c)` on 20,000 of 20,000 with the x86-64 V3 dispatch (AVX2 and FMA), the plain formula only with it disabled (26 % of products differ) | `python -I probe_cmul.py`; `NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL" python -I probe_cmul.py` (numpy 2.5.4; 2.2.6 names `AVX2 FMA3`) |
| `probe_libm.py` | how often glibc's double `atan2` and `hypot` (SciPy's `Rotation.as_euler` calls them through libc) differ from the correctly rounded result: 96 and 564 of 100,000 uniform pairs, 12 and 10 of 100,000 quaternion-like ones | `python -I probe_libm.py [n]` (needs mpmath) |
| `probe_pandas_parse.py` | how often pandas' default float parser (`read_csv`, `precise_xstrtod`, which stops at 17 digits counting leading zeros) gives another double than the correctly rounded one on the strings fMRIPrep's chain writes: Python's `repr` (most motion-like and component-like values), `%.10f`, `%.18e`, `%0.6f` | `python -I probe_pandas_parse.py [n]` (pandas 3.0.6 and 2.2.3) |
| `fetch_sdist.py` | fetches a package's sdist from PyPI's JSON API and checks its SHA-256 against PyPI's (the pins under `<workspace>/tags/`: pandas 3.0.6 and 2.2.3, nitime 0.12.1 and 0.11, numpy 2.2.6, fMRIPrep 25.2.6) | `python -I fetch_sdist.py <name> <version> <folder>` |
