# The version update's part 3: the numerical libraries (2026-10-29)

The tools behind the third part of the version update (next-steps item 3): the FSL replicas
follow FSL 6.0.7.23's Armadillo 14.2.1, whose 4 × 4 inverses, solves and determinants go
through LAPACK (OpenBLAS 0.3.33), and the one-shot BOLD resampler and numpy's 4 × 4 arithmetic
are checked against SciPy 1.18.1, numpy 2.5.4 and the OpenBLAS 0.3.34 numpy's wheels bundle.
Findings: `docs/findings/fsl-mcflirt.md` ("Armadillo 14.2.1's LAPACK path", restricted),
`fsl-bet.md`, `scipy-ndimage.md` ("SciPy 1.18.1"), `numpy-blas.md` ("numpy 2.5.4 and
OpenBLAS 0.3.34").

Sources read (permissive; kept in `<workspace>/tags/`): OpenBLAS 0.3.33 (the release tarball from
github.com/OpenMathLib/OpenBLAS/releases/download/v0.3.33, SHA-256
`6761af1d9f5d353ab4f0b7497be2643313b36c8f31caec0144bfef198e71e6ab`, with the reference LAPACK
3.12.0 it bundles in `lapack-netlib/`), OpenBLAS 0.3.30 and 0.3.34, SciPy 1.15.2 and 1.18.1,
numpy 2.3.5 and 2.5.4; Armadillo 14.2.1 and 10.5.1 as FSL installs them
(`~/fsl-6.0.7.23/include/armadillo_bits`, `~/fsl/include/armadillo_bits`).

| Tool | What it shows | Run |
|---|---|---|
| `lapack_probe.py` | FSL 6.0.7.23's own `dgetrf`, `dgetri`, `dtrtri`, `dgesv` and Armadillo's `inv()`, `inv(A)·B`, `det()` (its dispatch emulated in Python from Armadillo 14.2.1's source) through ctypes, against larmorx-nc-core's `lapack.rs` and `arma.rs` (the `lapack-probe` tool), every output word bit for bit, on 18 seeded families of 4 × 4 and 3 × 3 matrices; and FSL's library under each OpenBLAS core this CPU runs, against its Haswell core | `OPENBLAS_CORETYPE=Haswell python -P lapack_probe.py --out <dir> --tool crates-nc/target/release/lapack-probe --count 100000 --cores Haswell,Cooperlake,SkylakeX,Zen,SandyBridge,Prescott` |
| `blas_nan_probe.py` | which operand's NaN FSL's `dgemv` passes on, per code path and row (GCC's operand order; the port follows it) | `OPENBLAS_CORETYPE=Haswell python -P blas_nan_probe.py` |
| `openblas_kernels.py` | the cores an OpenBLAS library holds and the fused multiply-adds in each kernel larmorx reproduces (FSL's 0.3.33; numpy 2.3.5's 0.3.30, numpy 2.5.4's 0.3.34) | `python -P openblas_kernels.py <libopenblas.so>` |
| `compare_cores.py` | mcflirt 6.0.7.23's 137 recorded runs on OpenBLAS's own core for this CPU (Cooperlake) against the Haswell core: 1 differs (`identical-copies/reffile-fudge`, its relative rms) | `python -P compare_cores.py <workspace>` |
| `make_permanent_cases.sh` | the permanent cases of the FSL version trial's BET cases 80 (drawn) and 141 (shrunk), which agree with FSL 6.0.7.23 now | `make_permanent_cases.sh <findings/fsl-bet> <scratch>` |
| `make_python_newest.sh`, `requirements-newest.txt` | the oracle environment of the newest releases, `<workspace>/oracles/python-newest` (numpy 2.5.4, SciPy 1.18.1, the shared environment's other packages), with `ENVIRONMENT.txt` (pip freeze, the build configuration, the OpenBLAS cores) | `make_python_newest.sh <workspace>` |
| `svd_nonfinite.py` | numpy 2.5's SVD of a matrix with ±∞ raises where 2.3's returned NaN; nibabel's `io_orientation` raises in both | `python -P svd_nonfinite.py` under each environment |

Also: `validation/probes/fsl-mcflirt-affine-trace/nan_rms.cc` (FSL's `rms_deviation` on seeded
pairs, or on given pairs from standard input; built against `FSLDIR=~/fsl-6.0.7.23` by its
`build.sh`) and `nan_decompose.cc` rebuilt against 6.0.7.23, replayed through the replica by
`crates-nc/larmorx-nc-core/tools/affine-trace-replay` (which reads `RMS` lines too).

The harness: `--python newest|shared` (`LARMORX_PYTHON`,
`validation/src/larmorx_validation/parity/python_versions.py`) for `parity`, `evidence` and
`difftest`, the suites of the Python originals defaulting to `newest`; the step from the shared
environment's releases to the newest per case (the resample-series record); FSL's programs on
OpenBLAS's Haswell core (`fsl_versions.OPENBLAS_CORE`, part of the oracle cache's key for FSL).
