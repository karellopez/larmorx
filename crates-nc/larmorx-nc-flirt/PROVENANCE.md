# Provenance: larmorx-nc-flirt

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's FLIRT (the conda package fsl-flirt 2111.4, FSL's source in `~/fsl-6.0.7.23/src/fsl-flirt`),
written to give `flirt`'s and `convert_xfm`'s outputs, messages and exit statuses bit for bit. It is
distributed under the FSL Licence (`../LICENSE`), as part of the `larmorx-nc` package
(`docs/licensing.md`); the FSL files it was translated from are in `../fsl-source/fsl-flirt/`,
unchanged (`../fsl-source/MANIFEST.tsv`). The clean-room original is to be written from
`specs/flirt.md` by an implementer who has not read FSL's source; neither crate will use the
other's code. The libraries FLIRT uses are translated in `larmorx-nc-core` (its `PROVENANCE.md`:
`costfns` with the weighted costs, `labeldiff` and the unsmoothed entropies, `bbr`, `newimagefns`'
blur, halving, clamping and binarising, `optimise`'s Powell method), shared with mcflirt and BET.

FSL 6.0.7.17's fsl-flirt 2111.2 differs from 2111.4 only in `configure_interpolation` (a function
of its own in 2111.4, called before the final transform; 2111.2 set the interpolation inside
`final_transform`): the same settings in the same order, so the same results (observed: FSL
6.0.7.17 and 6.0.7.23 give the same matrices and images byte for byte on ds005040's default
and fMRIPrep BBR calls; `docs/findings/fsl-flirt.md`).

## Files and functions translated

| Rust module | fsl-flirt 2111.4 file | Functions, classes |
|---|---|---|
| `options` | `globaloptions.{h,cc}` | `globaloptions` (defaults, `parse_command_line`, `print_usage`, `print_version`), every option and message |
| `schedule` | `defaultschedule.h`, `flirt.cc` | `setdefaultschedule`, `set2Ddefaultschedule`; `stripleadingspace`, `striptrailingspace`, `parseline`, `setmatvariable`, `setscalarvariable`, `parsematname` |
| `pair` | fsl-newimage 2601.0 `costfns.{h,cc}` | `Costfn` as FLIRT holds it (references to the volumes resolved at each evaluation), `set_no_bins`, `cost` and the weighted `cost`, the member wrappers, `is_bbr_set` |
| `run` | `flirt.cc` | `main`, `do_applyxfm`, `set_basescale`, `FLIRT_read_volume(4D)`, `get_testvol`, `get_refvol`, `double_end_slices`, `scalemat`, `qsform_init_mat`, `set_initmat`, `vector2affine`, `affmat2vector`, `default_nonlin_params`, `setcostfntype`, `setup_costfn`, `costfn` (three forms), `estimate_scaling`, `set_param_tols`, `optimise`, `params12toN`, `paramsNto12`, `subset_costfn`, `find_cost_minima`, `set_rot_sampling(s)`, `search_cost`, `measure_cost`, `aligncog`, `optimise_strategy0/1/3`, `sorted_posn`, `add2list`, `set_perturbations`, the filters (`filter_blur`, `filter_resamp_blur`, `filter_subsample_by_2`, `filter_weight`, `filter_image`), `resample_refvol`, `output_dtype`, `configure_interpolation`, `final_transform`, `safe_save_volume`, `save_matrix_data`; the schedule commands `usrcopy`, `usrclear`, `usrprint`, `usrsave`, `usrread`, `usrsetrow`, `usrsetrowqsform`, `usrsetrowparams`, `usrsetoption`, `usraligncog`, `usrsort`, `usrdualsort`, `usrsearch`, `usrreadparams`, `usrsaveparams`, `usrprintparams`, `usrmeasurecost`, `usrgridmeasurecost`, `usroptimise`, `usrsetscale`, `interpretcommand` |
| `convert_xfm` | `convert_xfm.cc` | `globaloptions`, `print_usage`, `parse_command_line`, `vector2affine`, `affmat2vector`, `main` |

## Changes from the original

- **Errors are values.** Where FLIRT throws and its `main` catches (status 0, the message on
  standard error), the replica prints the same and returns 0; where FSL aborts (an uncaught
  exception in `convert_xfm`, glibc's heap check after FSL-21), it prints what FSL prints, then one
  line of its own, and exits with status 1.
- **Undefined behaviour is refused** (D13 class a, both modes; `docs/findings/fsl-flirt.md`): a
  schedule's matrix name FSL cannot interpret (an uninitialised pointer), `U[` (a 28th list),
  `setoption bbrstep` below 1 (a loop that never ends), `-pedir` without a field map read (an empty
  field map interpolated), the schedule command `alignpaxes` (not translated yet).
- **The pair's references.** FSL's `Costfn` holds references to `main`'s volumes; the replica's
  pair names which volume it refers to and the run resolves it at every evaluation, so a later
  change of that volume (each `setscale` refreshes the input; a scale with no pyramid level replaces
  the 1 mm reference) reaches an older pair as in FSL.
- **The pseudo-inverse** of a parameter-subset mask (`pinv`, Armadillo's divide-and-conquer SVD on
  FSL's LAPACK) is computed in closed form for masks of zeros and ones with at most one non-zero
  per row, which gives FSL's bits (`validation/probes/fsl-flirt-pinv`: FLIRT's search masks and
  every shipped schedule's); other masks use the normal equations (not FSL's last bits; part 2).
- **Threads.** FLIRT is single-threaded; the replica runs the boundary points' Gaussian smoothing
  on `OMP_NUM_THREADS` threads, with the same result at any count.
- **The `descrip` field** of the images it writes is `larmorx-nc fsl flirt`.
- **The C library's maths** are glibc 2.35's (`larmorx-nc-core`: `exp`, `tanh`, `logf`), as FSL's
  Linux binaries call them, so the outputs are FLIRT's on Linux x86-64 on every platform.
