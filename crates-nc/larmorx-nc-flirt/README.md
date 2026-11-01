# larmorx-nc-flirt

The bit-exact replica of FSL 6.0.7.23's FLIRT (fsl-flirt 2111.4): `flirt`, affine registration
with every cost function (`corratio`, `mutualinfo`, `normmi`, `normcorr`, `leastsq`, `labeldiff`,
`woods`, and boundary-based registration, `bbr`, with its field-map form), the rotation search,
the multi-resolution pyramid and the schedule language (every command, FSL's shipped schedules
and the default 3D and 2D ones), `-applyxfm` and `-applyisoxfm`, and `convert_xfm`, translated from
FSL's source: options, messages, exit statuses and failure cases. Part of the `larmorx-nc`
package, under the FSL Licence's terms (non-commercial use only; `docs/licensing.md`); FSL's
libraries (newimage's cost functions, BBR, filters and resampling, miscmaths' optimiser and
transform algebra, NewNifti) are the shared core's (`../larmorx-nc-core`). larmorx (Apache-2.0)
runs the `larmorx-nc` program as a separate process (`larmorx fsl flirt ...`, `lx.fsl.flirt`,
`lx.fsl.convert_xfm`); it never links this crate.

```
larmorx-nc fsl flirt [options] -in <inputvol> -ref <refvol> [-out <outputvol>] [-omat <matrix>]
larmorx-nc fsl convert_xfm [options] <input-matrix>
```

Settings come from the environment: `FSLOUTPUTTYPE` and `FSL_LOAD_NIFTI_EXTENSIONS` (FSL's),
`LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or `reproduce`: FSL's result bit for bit),
`OMP_NUM_THREADS` (the boundary points' smoothing; results do not depend on it).

- What it translates and from where: `PROVENANCE.md`; FSL's original source: `../fsl-source/`.
- The behaviour, for the clean-room original: `specs/flirt.md`.
- Validation: `docs/validation/fsl-flirt-replica.md` (parity with FSL); golden cases: the FLIRT
  and convert_xfm cases of `../larmorx-nc-cli/tests/golden.rs`.
- Not replicated yet (part 2, refused with a message): `-anglerep quaternion`, the schedule command
  `alignpaxes`, parameter-subset masks other than FLIRT's own and FSL's shipped schedules'
  (columns of single ones, the search's mask, `sch2D_4dof`'s; `validation/probes/fsl-flirt-pinv`),
  `-debug` with the BBR cost (its debug volume). `-debug`'s other volumes are written but not yet
  compared with FSL's.
