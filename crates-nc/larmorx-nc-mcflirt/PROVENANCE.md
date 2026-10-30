# Provenance: larmorx-nc-mcflirt

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of
FSL's `mcflirt` (fsl-mcflirt 2111.0 of FSL 6.0.7.23 since 2026-10-29; first translated from FSL
6.0.7.17, whose fsl-mcflirt is the same source), written to give mcflirt's outputs, messages and
exit statuses bit for bit. FSL 6.0.7.23 rebuilt mcflirt with newer libraries (newimage 2601.0,
miscmaths 2412.6, NewNifti 5.0.0, znzlib 2511.3, armawrap 0.7.0): what that changes, and what the
replica follows, is in `docs/findings/fsl-mcflirt.md`, "FSL 6.0.7.23". It is distributed under the FSL Licence (`../LICENSE`), as part
of the `larmorx-nc` package (`docs/licensing.md`); the FSL files it was translated from are in
`../fsl-source/fsl-mcflirt/`, unchanged. The Apache-2.0 clean-room original of the same tool is
`crates/larmorx-mri` (`lx.mri.hmc`, `larmorx mri hmc`); neither crate uses the other's code.
The libraries mcflirt uses (newimage, miscmaths, NewNifti, znzlib, armawrap) are translated in
`larmorx-nc-core`.

## Files and functions translated

| Rust module | fsl-mcflirt 2111.0 file | Functions, classes |
|---|---|---|
| `options` | `Globaloptions.{h,cc}` | `Globaloptions` (defaults, `parse_command_line`, `print_usage`), the usage text, every option and message |
| `run` | `mcflirt.cc` | `main` (the stages, `-meanvol`'s second pass, `-gdt`, `-init`, the final resampling with each interpolation, `-stats`, the output type), `correct`, `usroptimise`, `optimise_strategy1`, `optimise`, `powell_opt`, `costfn`, `vector2affine`, `affmat2vector`, `set_param_tols`, `initialise_params`, `usrsetscale`, `fix2D`, `double_end_slices`, `decompose_mats`, `run_and_save_stats` |
| `run` | `Log.{h,cc}` | `Log::establishDir` (the `+` names, permissions 0770, its return values) |

## Changes from the original

- **Errors are values.** Where mcflirt throws and the C++ runtime aborts (signal 6), the
  replica prints what mcflirt prints (the `Image Exception` line, `terminate called after
  throwing an instance of ...`, `what(): ...`), then one line of its own, and exits with status
  1 (a process cannot end with a signal portably). The parity suite compares the text up to
  that line.
- **Threads.** mcflirt is single-threaded. In the stages whose volumes start from a fixed
  matrix (all but the first, which chains each volume to the previous one, unless `-fudge`)
  and in the final resampling, the replica registers the volumes in parallel
  (`OMP_NUM_THREADS`, default all logical CPUs); every volume's computation is exactly
  mcflirt's, its messages are printed in mcflirt's order, and the results do not depend on the
  thread count.
- **The `descrip` field** of the images it writes is `larmorx-nc fsl mcflirt` (mcflirt writes
  FSL's build string).
- **The C library's maths** are glibc 2.35's (`larmorx-nc-core::glibc_float`, `fdlibm`), as
  FSL's Linux binaries call them, so the outputs are mcflirt's on Linux x86-64 on every
  platform.
- **D13** (`docs/api/upstream-defects.md`; `LARMORX_UPSTREAM_DEFECTS=fix`, the default, or
  `reproduce`): mcflirt's deterministic defects are corrected by default with the same
  behaviour as the clean-room original (FSL-01, FSL-03, FSL-05 to FSL-10, FSL-12 to FSL-14,
  FSL-20, FSL-21, FSL-65 and FSL-66 (since 2026-10-30, `specs/mcflirt.md` §10.6 and §12.1:
  an unopenable `.par` or `.rms` file, a negative `-bins`); FSL 6.0.7.23's FSL-59, `-sinc_final` and `-nn_final` resampled trilinearly,
  and FSL-60, image names holding `.img` opened under another name; `-gdt`, broken in mcflirt
  (FSL-04), is refused) and reproduced bit for bit
  with `reproduce`; each case prints a notice line `larmorx-nc: upstream defect FSL-nn (mcflirt)
  corrected|reproduced: ...` to standard error. Decided to keep: FSL-02 (rule C, decided
  2026-10-23), FSL-11's `.nii.gz` default when `FSLOUTPUTTYPE` is unset (decided 2026-10-23),
  FSL-15 and FSL-18 (rule C; `docs/findings/fsl-mcflirt.md`). FSL-22 (NaN values in the
  background value's sort, an order C++ leaves undefined): sorted last by default, libstdc++'s
  order with `reproduce` (2026-10-26), each named.
- **Not followed yet (FSL 6.0.7.23).** Armadillo 14.2.1's 4 × 4 inverses and solves through
  LAPACK (OpenBLAS): the replica keeps Armadillo 10.5.1's closed-form inverse (`larmorx-nc-core`'s
  `arma`), so the last bits of values computed from nearly equal matrices (`.rms` of identical
  volumes) and the sign of NaN parameters can differ from FSL 6.0.7.23 (5 of 203 standard-tier
  cases, its record's expected divergences).
