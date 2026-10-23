# Final report: A2 core: ImageMath scaffold + thresholds

Agent `a4da5d293b8ff7930`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## A2 scaffold and the threshold/statistics/arithmetic group: final report

The shared scaffold is in place, and so is my group: ImageMath arithmetic and intensity operations, ThresholdImage and MultiplyImages. Every compared output is bit-identical to the ANTs 2.6.5 binaries except ImageMath `^` (cause below), and every output header matches ANTs' 348 bytes exactly.

All gates pass on the final tree (fmt, clippy `-D warnings`, cargo test, `maturin develop --release` + pytest: 193 python + 154 parity tests, ruff, SPDX). The branch contains the latest `main`; the GPL lockfile is still valid (`crates-gpl` only uses larmorx-core/-io, whose dependencies I did not change). The venv's editable larmorx now points at my worktree (`maturin develop`).

### The scaffold

**1. `larmorx-io`: ITK's NIfTI writer, and two reader fixes**
- `nifti::itk::{itk_header, write_itk_image, ItkMeta, is_itk_nifti_name}`: a port of ITK's `WriteImageInformation`/`SetNIfTIOrientationFromImageIO` and nifti_clib's float orthogonalisation and quaternion.
  - Both xforms are code 1, `xyzt_units` 10, `pixdim[4..7]` = 0.
  - `descrip` and `aux_file` come from the image's metadata.
  - `antsApplyTransforms` now writes through it too. Its standard tier is unchanged: 83 cases, 63 of 79 bit-identical.
- `ItkGeometry` gained:
  - `flipped`, the axes whose stored spacing was negative;
  - `from_grid3()` and `with_axis()`;
  - `stored_in_new_image()`: ITK's `SetDirection`/`SetOrigin` keep the image's `+0.0` where the reader computes `-0.0`. The sign of that zero reaches the written srow, so it decides whether header bytes match.
- `ItkImage.meta` holds the input's `descrip`/`aux_file`.
- **Reader finding:** nifti_clib reads every NaN and ±inf float as 0 (`nifti1_io.c:5013-5049`). `read_itk_image` now does the same; before, it kept them.

**2. `larmorx-image` (the shared crate for the next groups)**
- `FilterError` (`Invalid`, `SizeMismatch`, `Threads`; non-exhaustive; `FilterError::invalid()` is crate-internal).
- `itk_math`: ITK's `FloatAlmostEqual` for f32/f64.
- `statistics`: `Histogram` (Initialize with float width, GetIndex, Quantile), `scalar_image_histogram`, `label_histogram`, `minimum_maximum`.
- `threshold`: binary threshold, the Otsu calculator, threshold labeller, Otsu filter.
- `intensity::rescale_intensity`. The Gaussian group should reuse it; `gaussian.rs`'s doc now points there.
- The gaussian/morphology/components/distance stubs are otherwise untouched. `Volume` is unchanged.
- Convention: voxel-wise filters take flat Fortran-order slices; spatial filters take `Volume`. All parallel work runs inside `larmorx_core::parallel::with_threads(n_threads, …)` with fixed chunks.

**3. `larmorx-ants`**
- `image.rs`:
  - `AntsImage<T>{data, geometry, meta}`. `derived(data)` is a filter or `AllocImage` output (empty meta). `with_data(data)` is an image changed in place (keeps `descrip`). `to_volume()` gives a `Volume` for 2D/3D and `None` for 4D.
  - `Pixel` (f32, f64, i32, u8: C++ casts with x86-64 truncation).
  - `ReadError`, carrying ANTs' messages (`ants_message()`).
  - `OutputImage` (F32/F64/I32/U8). To add a pixel type: a variant, a `From` impl, and arms in `geometry()`, `FileStore::save` and `output_to_py`.
  - The `ImageStore` trait (`load`/`save`; `read`/`write` apply ANTs' `ReadImage`/`WriteImage` rules, including identity direction when a file has more dimensions than `D`), `FileStore`, and free functions `read_with`/`write_with`/`from_itk`.
- `cli/cstd.rs`: `atof` (glibc strtod, including inf/nan/hex), `stoi`, `from_string_f32` (libstdc++ `istringstream >> float`), `format_g` (`cout << float`).
- `cli/image_math/mod.rs`, the dispatcher:
  - `Operation {name, dims, usage, run: fn(&mut Context) -> Result<(), OpError>}`.
  - `GROUPS` (registry) and `KNOWN` (every ANTs op per dimension table).
  - `Context`: `argv` numbered as in `ImageMath_Templates.hxx` (2 = output, 3 = op, 4.. = operands), `arg/required/output/op/argc`, `try_read::<T>`/`read::<T>` (ReadImage semantics, prints ANTs' message), `write`, `dim`, `n_threads`, `out`, `err`.
  - `OpError::{Reported → exit 0, Crash → exit 1, Unsupported → exit 1}`.
  - ANTs' usage, dimension and "not found" behaviour. A known op always exits 0. Ops ANTs has but larmorx lacks answer "not supported yet".
- `threshold_image.rs`, `multiply_images.rs`, `image_math/{arithmetic,intensity}.rs`: the Rust APIs.
- `cli/{threshold_image,multiply_images}.rs`: `main(args, out, err)` plus `run(args, store, n_threads, out, err)`. `cli/mod.rs`: `TOOLS` and `run`.

**How to add an ImageMath op group** (also in the module docs):
1. Rust API in `larmorx-ants/src/image_math/<group>.rs` (or ITK filters in `larmorx-image`).
2. `larmorx-ants/src/cli/image_math/<group>.rs` with one `fn(&mut Context)` per op and `pub const OPERATIONS: &[Operation]`.
3. In `cli/image_math/mod.rs`: `mod <group>;` and add `<group>::OPERATIONS` to `GROUPS`.
4. Parity cases (below) and a Python wrapper.

A separate program (SmoothImage, ResampleImageBySpacing): a `cli/<prog>.rs` with `main`/`run`, an entry in `TOOLS`/`run`, and a match arm in larmorx-py's `ants_program`.

**4. The Python pattern**
- `larmorx-py/src/ants_filters/mod.rs`:
  - `ants_program(program, args, inputs, outputs, n_threads)` runs the command-line code on in-memory images through a `MemoryStore`.
  - Helpers `slice_f32`, `array_like`, `value_err`.
  - Each group is a file (`image_math.rs`, `threshold.rs`) with `#[pyfunction]`s (GIL released, explicit `n_threads`) and a `register(m)` called from `mod.rs`.
- Stubs go in `python/larmorx/_core.pyi`.
- `python/larmorx/ants/_filters.py`: `float_input` (paths read as ITK reads them, then cast to float32), `mask_input`, `result` (keeps the input's affine and header).
- Wrappers, exported from `ants/__init__.py`:
  - `image_math.py`: generic `lx.ants.image_math(op, *operands, dimension=3, n_threads=1)` and `image_math_operations()`; `image_arithmetic`, `add_to_zero`, `negative_image`, `truncate_image_intensity`, `normalize_image`, `rescale_image`.
  - `threshold.py`: `threshold_image`, `otsu_threshold` (returns `OtsuResult`), `multiply_images`.

**5. Parity harness: `validation/src/larmorx_validation/parity/ants_programs.py`**
- `AntsCase(id, category, description, program, args, tier="smoke", expect="pass"|"error"|"divergence", tolerance_ulps=None, reason="", stdout=False, outputs=("{out}",), files=())`.
- Placeholders: `{out}`, `{out2}`…, `{in:<catalog path>}`, `{gen:<name>}`, `{missing:<name>}`, `{tmp}`.
- `make_suite(name, title, tool, all_cases, notes)`.
- Oracle: `LARMORX_ANTS_BIN`, else `oracles/ants-2.6.5/bin`. ANTs runs as a binary; larmorx in-process. Both get `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`.
- Each case compares every header field from the raw bytes, dtype, shape, affine, values bit for bit (or within the declared ulps, with the reason), and stdout when asked.
- Generated inputs: `ants_inputs.py`, registered with `@builder("name")`.
- ImageMath cases: package `parity/ants_image_math/`. Add a module with `CASES` and list it in `GROUPS` (now dispatch, arithmetic, intensity); shared paths are in `common.py`.
- New suites register in two places: `__main__.py` `PARITY_MODULES`, and `tests/parity/test_ants_programs.py` `SUITES` (which skips without the oracle).
- Benchmarks: `bench/ants_programs.py` (`JOBS` list; `BENCH_MODULES`).

### Results (standard tier; records in `docs/validation/`)

| Program | Cases | Pass | Both reject | Expected divergence | Bit-identical | Header bytes identical |
|---|---|---|---|---|---|---|
| ImageMath | 110 | 99 | 11 | 0 | 95 of 98 with data | 98 of 98 |
| ThresholdImage | 41 | 35 | 5 | 1 (`Kmeans`) | 35 of 35 | 35 of 35 |
| MultiplyImages | 15 | 12 | 3 | 0 | 12 of 12 | 12 of 12 |

- **ImageMath `^`, the only residual difference:** at most 35 of 35,840 voxels, one ulp. ANTs uses glibc `powf` (verified), which is not correctly rounded. I ported CORE-MATH `cr_powf` into `larmorx_core::math::powf`; it is bit-identical to the C on 1.5 M random inputs and all 460,475 worst cases, and differs from glibc in 0.06 % of calls.
- ImageMath `exp` is verified to be double `exp` rounded to float, so it matches exactly.
- Cases cover: every arithmetic op with image and scalar operands; operand parsing quirks; `/` carry; running sums with stdout compared; `TruncateImageIntensity` (the fMRIPrep call on a raw T1w, masks, outliers, the order-dependent histogram range); `Normalize`; `RescaleImage`; Otsu with and without mask; 2D and 4D; error and crash paths; real fMRIPrep and template images.

### Benchmark (quiet machine, load 1.3 before, no other benchmark; `docs/benchmarks/ants-programs.md`)

| Job | ANTs 1 / 12 threads | larmorx 1 / 12 threads |
|---|---|---|
| Truncate raw T1w | 388 / 175 ms | 234 / 83 ms |
| Otsu 3 with mask | 558 / 315 ms | 205 / 125 ms |
| ThresholdImage 0.5 1 | 115 / 103 ms | 100 / 49 ms |
| MultiplyImages | 157 / 158 ms | 128 / 62 ms |
| addtozero | 204 / 200 ms | 75 / 40 ms |
| ImageMath 4 on 28 MB BOLD | 447 / 438 ms | 392 / 225 ms |

All outputs bit-identical. Gzip decompression dominates.

### Commits (not pushed, no attribution)
`17a3d2c`, `35df791`, `ee0aaa1`, `2960b32`, `9a1e340`, plus merges of main `039f29c` and `793b46e`.

### Left undone or uncertain
- **Test data were not added to larmorx-testdata.** The sandbox blocks writing to that repo, so I could not add or commit there. The suites build their 19 inputs on the fly (`gen:`).
  - A drop-in generator and docs page are staged in my scratch area: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/a4da5d/larmorx-testdata-staging/` (`generators/ants_filters_synthetic.py`, `docs/sources/synthetic-ants-filters.md`). Its files are verified identical to `ants_inputs.py`'s.
  - To publish them: copy, run the generator and `python -m larmorx_testdata index`, commit. Then switch the cases' `gen:` inputs to catalog paths if wanted.
  - `/tmp` may be cleaned; the builders are also committed in `ants_inputs.py`.
- `ThresholdImage Kmeans` is not ported; it is documented and reported as not supported.
- Where ANTs crashes or aborts (missing inputs, `stoi` errors, ITK exceptions), larmorx exits 1 with a message. `Otsu 0` hangs ANTs (read, not run); larmorx reports an error.
- Not supported: vector/tensor images; MultiplyImages in 1D; non-NIfTI formats.
- Minor edge cases: int64/uint64 voxels are converted through f64 (possible double rounding); uint32 masks above `i32::MAX` saturate instead of wrapping.
- **For the spatial groups:** 2D and 4D need D-dimensional neighbourhoods. ITK's 4D morphology also acts across time, and a 2D image is not a one-slice 3D image. `Volume` is 3D, and `to_volume()` returns `None` for 4D. Details are in `docs/findings/ants-image-programs.md`.
- CLAUDE.md's A2 status line is yours to update.
