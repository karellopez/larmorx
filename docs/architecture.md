# Architecture

How the code is organised and the conventions every tool follows. The plan behind it is [PLAN.md](../PLAN.md) §2 and §4–§5; decisions are in [decision-log.md](decision-log.md).

## Layers

```
 Python users / larmorprepx
        │  lx.Image, lx.load/save, lx.<family>.<tool>(...)        python/larmorx (pure Python)
        ▼
 larmorx._core (PyO3, abi3)   numpy arrays in and out, no copies, GIL released    crates/larmorx-py
        ▼
 Rust crates                                                                       crates/larmorx-*
   larmorx-core      Affine, Grid3, Image<T, D>, DynArray/DynImage, element types, thread pools,
                     small linear algebra (incl. vnl's SVD inverse, bit-exact with ITK)
   larmorx-io        file formats: NIfTI (nibabel and ITK semantics), ITK transform files
   larmorx-transform ITK transforms: matrix-offset family, displacement fields, composites, -t chains
   larmorx-interp    the ITK interpolators (linear, nearest, B-spline, Gaussian, label, windowed sinc)
   larmorx-ants      ANTs tools (antsApplyTransforms) and their original command lines
   larmorx-afni      AFNI-compatible tools, clean-room (3dTshift), AFNI's NIfTI rules, an FFT
   larmorx-cli       the multicall `larmorx`/`lx` command line (original tool syntax)
   (next)            larmorx-image, -optim, -mesh, -mri
```

- **The Rust crates know nothing about Python.** Bindings live only in `larmorx-py`, so the same code serves the standalone CLI and Rust users.
- **No Rust objects cross into Python.** The extension takes and returns numpy arrays and plain dicts; the Python layer turns them into dataclasses (`Image`, `NiftiHeader`). This keeps the separate `larmorx-freesurfer` wheel interoperable (PLAN.md §2.2) and keeps Python objects picklable and inspectable.
- **Format-independent types in `larmorx-core`; formats in `larmorx-io`.** An `Image` is data plus an affine; format headers (`NiftiHeader`) live with their format, and the Python `Image` carries the header only as metadata.

## Data conventions

| Topic | Convention |
|---|---|
| World space | RAS+ millimetres (x → Right, y → Anterior, z → Superior), as NIfTI and nibabel. LPS+ (ITK/ANTs) only at the `lx.ants` boundary: `Affine::ras_to_lps` |
| Indexing | `data[i, j, k, t, ...]`, as nibabel; arrays read from files are Fortran-ordered (i fastest), so they map to disk without copies |
| Affines | f64, row-major `[[f64; 4]; 4]` in Rust, `(4, 4)` float64 read-only arrays in Python |
| Element types | `u8 i8 u16 i16 u32 i32 u64 i64 f32 f64 c64 c128`; returned in native byte order |
| Images in Rust | `Image<T, D>` over an ndarray (`Image3`, `Image4`, or dynamic); `DynImage` when the type is known only at run time |

## Behaviour conventions

- **Threads:** every entry point takes `n_threads` (0 = all logical CPUs); there is no global or environment setting. Work runs in a pool of exactly that size (`larmorx_core::parallel::with_threads`; pools are cached per size). Results never depend on the thread count: reductions use fixed chunking, and compressed output uses fixed block boundaries.
- **GIL:** released for all file and compute work.
- **Errors:** typed in Rust (`thiserror`), mapped to Python exceptions: missing files → `FileNotFoundError`, other I/O → `OSError`, bad arguments → `ValueError`, malformed files → `larmorx.io.NiftiError` (a `ValueError`).
- **Safety:** `#![forbid(unsafe_code)]` in every crate; pure-Rust dependencies only (CLAUDE.md rule 5).
- **Compatibility:** a re-implementation reproduces its reference's results (nibabel for I/O; ANTs, AFNI, FreeSurfer for the tool families). Deliberate differences are listed in the tool's docs page and in its validation record, with the reason.

## Testing and validation

| Level | Where | Runs |
|---|---|---|
| Rust unit and property tests (round trips, fuzzed inputs, thread-count invariance) | `crates/*/src/**` | `cargo test`, all six platforms |
| Python API tests (no external data) | `tests/python/` | pytest, all six platforms |
| Parity with reference tools, case by case on the test-data catalog | `validation/` suites, `tests/parity/` | locally (smoke or standard tier); in CI on all platforms once the test data are published |
| Benchmarks against reference tools | `validation/` (`bench`) | on demand; reports in `docs/benchmarks/` |

Test data live in a separate repository, `larmorx-testdata` (next to this one in the workspace; not published yet): a catalog of every file (source, licence, SHA-256, properties), small generated edge cases committed in the repo, and a downloader that verifies hashes into a content-addressed cache. larmorx never commits image data.

## Adding a tool (the per-tool contract, PLAN.md §4)

1. Rust API in the family crate, on `Image`/`Affine` types, with an explicit `n_threads`.
2. Binding in `larmorx-py` and an idiomatic wrapper in `python/larmorx/<family>/` (typed, dataclass results), with `.pyi` stubs.
3. CLI parser for the original program's arguments in `larmorx-cli`.
4. Docs page in `docs/api/` with the option-mapping table and the deliberate differences.
5. A parity suite in `validation/` against the original tool, its report in `docs/validation/`, and the tool's status (`experimental` → `validated` → `stable`).
6. `PROVENANCE.md` entries for ported code.
