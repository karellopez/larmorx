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
   larmorx-transform ITK transforms: matrix-offset family, displacement fields, composites, -t chains;
                     nitransforms chains (RAS) and fMRIPrep's one-shot BOLD resampler
   larmorx-interp    the ITK interpolators (linear, nearest, B-spline, Gaussian, label, windowed sinc);
                     SciPy's map_coordinates and spline prefilter (ndimage)
   larmorx-image     ITK image filters on D-dimensional volumes: thresholds, Otsu, histograms
                     and quantiles, intensity rescaling, recursive and discrete Gaussians
                     (smoothing, Laplacian, gradient magnitude), median, binary and grayscale
                     morphology with ITK's ball, connected components and relabelling, label
                     contours, Danielsson and signed Maurer distance maps, ITK's float
                     comparisons
   larmorx-ants      ANTs tools (antsApplyTransforms, ImageMath, ThresholdImage, MultiplyImages,
                     SmoothImage, ResampleImageBySpacing, ResampleImage), images as ANTs
                     programs read and write them, ITK's identity resampling in 2-4D, the
                     original command lines
   larmorx-afni      AFNI-compatible tools, clean-room (3dTshift), AFNI's NIfTI rules, an FFT
   larmorx-cli       the multicall `larmorx`/`lx` command line (original tool syntax), and
                     `replica`: running a tool's replica from another package as a separate
                     program
   larmorx-mri       clean-room MRI tools named by function: head-motion correction
                     (mcflirt-compatible `hmc`)
   (next)            larmorx-optim, -mesh
```

- **The Rust crates know nothing about Python.** Bindings live only in `larmorx-py`, so the same code serves the standalone CLI and Rust users.
- **No Rust objects cross into Python.** The extension takes and returns numpy arrays and plain dicts; the Python layer turns them into dataclasses (`Image`, `NiftiHeader`). This keeps the separate `larmorx-freesurfer` wheel interoperable (PLAN.md §2.2) and keeps Python objects picklable and inspectable.
- **Format-independent types in `larmorx-core`; formats in `larmorx-io`.** An `Image` is data plus an affine; format headers (`NiftiHeader`) live with their format, and the Python `Image` carries the header only as metadata.
- **Replicas from other licence families are separate programs** (`docs/licensing.md`). `crates-gpl/` (and later `crates-nc/`) build their own command-line program; the main package runs it as a child process and exchanges files with it, and never links or imports it. The registry, the search for the program and the choice between implementations live once in Rust (`larmorx_cli::replica`); the Python wrappers use them through `larmorx._core` (`larmorx/_replica.py`).

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
| Golden tests: every platform reproduces larmorx's own Linux x86-64 output, bit for bit ([validation/golden.md](validation/golden.md)) | `tests/golden/`, `crates-gpl/larmorx-gpl-cli/tests/golden.rs` | pytest on the release wheel and `cargo test` in `crates-gpl/`, all six platforms |
| Parity with reference tools, case by case on the test-data catalog | `validation/` suites, `tests/parity/` | locally (smoke or standard tier); in CI on all platforms once the test data are published |
| Benchmarks against reference tools | `validation/` (`bench`) | on demand; reports in `docs/benchmarks/` |

Test data live in a separate repository, `larmorx-testdata` (next to this one in the workspace; not published yet): a catalog of every file (source, licence, SHA-256, properties), small generated edge cases committed in the repo, and a downloader that verifies hashes into a content-addressed cache. larmorx never commits image data.

## Adding a tool (the per-tool contract, PLAN.md §4)

1. Rust API in the family crate, on `Image`/`Affine` types, with an explicit `n_threads`.
2. Binding in `larmorx-py` and an idiomatic wrapper in `python/larmorx/<family>/` (typed, dataclass results), with `.pyi` stubs.
3. CLI parser for the original program's arguments (in the family crate's `cli` module, dispatched by `larmorx-cli`). ANTs image programs read and write through `larmorx_ants::image::ImageStore`, so the same command-line code also runs on in-memory images from Python (`lx.ants.image_math`).
4. Docs page in `docs/api/` with the option-mapping table and the deliberate differences.
5. A parity suite in `validation/` against the original tool, its report in `docs/validation/`, and the tool's status (`experimental` → `validated` → `stable`).
6. `PROVENANCE.md` entries for ported code.
7. A golden case for each new CLI tool, ImageMath operation and public function: a function in `tests/golden/registry.py` (or `CASES` in `crates-gpl/larmorx-gpl-cli/tests/golden.rs` for a replica), recorded on Linux x86-64 with `scripts/record_golden.py` ([validation/golden.md](validation/golden.md), "Adding a golden case").

## Adding a replica

A tool whose upstream licence keeps its bit-exact replica out of the main package (AFNI's MCW files and Workbench: GPL; FSL: the FSL Licence) has two implementations: the clean-room original in `crates/` and the replica in its licence family's workspace. 3dTshift is the example: `crates/larmorx-afni` and `crates-gpl/larmorx-gpl-afni`.

1. **The replica's program.** Add the tool to the family's command line (`larmorx-gpl <family> <tool> <original arguments>`, `crates-gpl/larmorx-gpl-cli`), with the same arguments, output conventions and exit codes as `larmorx <family> <tool>`. It takes its thread count from `OMP_NUM_THREADS`; with `-verbose` (or the tool's own verbose option) its first line names the implementation, as the original's does. A new licence family also needs its own program, wheel (`bindings = "bin"`) and `Package` entry in `larmorx_cli::replica`.
2. **One line in the registry:** `REPLICAS` in `crates/larmorx-cli/src/replica.rs`, e.g. `Replica { family: "afni", tool: "3dTshift", package: &LARMORX_GPL }`. From then on, `larmorx <family> <tool>` runs the replica when its program is found (`LARMORX_IMPLEMENTATION` overrides), and `larmorx._replica.registry()` lists it.
3. **The Python wrapper** takes `implementation: ImplementationName = "auto"` and asks `_replica.select(family, tool, implementation)`. `None` means: run the original and return `Image(..., implementation=_replica.ORIGINAL)`. Otherwise, in a temporary directory:
   - write in-memory inputs as the original would read them, and pass paths as absolute paths;
   - translate the keyword arguments into the tool's options (use relative file names in the temporary directory: some upstream programs refuse blanks in names);
   - `_replica.run(selected, args, cwd=tmp, n_threads=n_threads)` (raises `ReplicaError` if the program fails; never fall back to the original);
   - read the outputs, and return them with `implementation=selected.implementation()`.
   Validate the arguments that do not need the image before choosing, so both implementations raise the same `ValueError`s.
4. **Tests that run in CI without the replica:** a stub program written by the test (`tests/python/test_replica.py` shows one), and the tool's other tests with the `no_replica` fixture, so that they test the original whatever is installed. Golden cases call the original explicitly (`implementation="original"`); the replica has its own golden cases in its workspace.
5. **Parity through both paths:** the tool's parity suite takes `implementation` (`suite("replica")`), runs the command line with it forced and checks that the Python API reproduces the command line's output; the replica gets its own validation record.
