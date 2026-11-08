# sdcflows' field-map estimation (field maps, part A)

The tools behind the port of sdcflows 2.17.1's field-map estimation (`crates/larmorx-sdcflows`,
`lx.sdcflows`). Sources read (permissive; kept in `<workspace>/tags/`, sdists from PyPI with their
SHA-256 checked against PyPI's): sdcflows 2.17.1 and 2.15.1 (fMRIPrep 25.2.6's), scikit-learn 1.9.1
and 1.6.1, SciPy 1.15.3 and 1.18.1, OpenBLAS 0.3.30 and 0.3.34, numpy 2.5.4.

| Tool | What it shows | Run |
|---|---|---|
| `oracle_nodes.py` | the sdcflows nodes fMRIPrep 25.2.6 ran in the oracle runs (phase-difference, phase1/phase2 and TOPUP estimators; fMRIPrep's reconstructions), with their parameters, durations and input grids: every `bs_filter` with spacing (16, 16, 10) mm, median recentring, alpha 1e-4, `zooms_min` 1, extrapolation; 9 to 118 s each | `python -P oracle_nodes.py` |
| `probe_colmat.py` | how sdcflows' collocation matrix is stored: scipy's `kron` stores a right operand densely (explicit zeros) when it fills half its matrix, and `hstack(...).tocsr()` keeps them, rows sorted | `python -P probe_colmat.py` (in each kept environment) |
| `check_weights.py` | `lx.sdcflows.bspline_grid` and `collocation_matrix` against sdcflows' `bspline_grid` and `sparse_hstack([grid_bspline_weights(...)]).tocsr()`, every array byte for byte, and the products `X @ v`, `X.T @ y` against scipy's: 15 cases (orientations, permuted axes, two levels, a single slice and row, dense axes, an oblique grid, float64, a rotated target with sdcflows' warning, a target beyond the knots, four oracle runs' field maps): all identical in the newest environment (SciPy 1.18.1, numpy 2.5.4) and the shared one (SciPy 1.15.3, numpy 2.3.5), 2026-11-08 | `OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P check_weights.py` |
