# Relay 5's handoff texts (for sub-task 6's HANDOFF.toml/.md)

## docs/findings/upstream-defects.md (the register, "FSL" table; after FSL-271's row, before ITK's)

| FSL-272 | prelude | the phase range check (more than 3π refused) and the rewrap to [−π, π] (beyond 1.001π) take the image's maximum and minimum from its first value on: a NaN first value (the first voxel of a masked or divided phase map) makes both NaN, so a phase from 0 to 2π is not rewrapped and is unwrapped with whole cycles wrong in some regions, and a phase of any range is not refused (measured with FSL's runs, `validation/investigations/prelude-spec/fsl272_measure.py`: fMRIPrep's seven prelude calls and a synthetic map moved to [0, 2π) with the first value NaN: 7 of 8 unwrapped whole cycles from FSL's own result without the NaN, in 0.1 to 100 % of the mask, up to 18.8 rad; a 4π range processed, exit 0) | c | none for fMRIPrep's calls (sdcflows' `au2rads` rescales the phase to [0, 2π] from its minimum and maximum, so a NaN anywhere would make every value NaN); other users' phase maps in [0, 2π] with a NaN first voxel | rule D measured, then A: the replica (2026-11-06) takes the extrema of the values that are not NaN by default (the rewrap, or FSL's range error with status 255, as without the NaN), FSL's with `reproduce`, named when the two decide differently; the clean-room original from `specs/prelude.md` §4.2 and §9.1 (suite cases `opt/nan-first-twopi`, `opt/nan-first-range`) | original |
| FSL-273 † | applytopup | `--method=jac` divides the field's derivatives by the field's own voxel sizes (a coefficient field's intent parameters, a field image's header) while the displacement it applies is laid at the images' voxel indices in the images' voxel sizes: a field of the images' shape whose voxel sizes differ from the images' gets a Jacobian determinant scaled along each encoded axis by their ratio (`dx/fx`, `dy/fy`; a field image `dx/fx`, `dx/fy` with FSL-134), not the Jacobian of the displacement applied, so the intensities are modulated wrongly (black box: FSL's outputs on one image's data stored with 6, 4 and 3 mm voxels are `S (1 + c g)` with `c = dy/fy`, `validation/investigations/spec-answers-4/jac_scales.py`) | c | none on fMRIPrep's calls (sdcflows applies its own field; topup's coefficients are on the images' grid) | rule A: the replica (2026-11-06) computes the Jacobian of the mapping applied by default (the same numbers wherever the voxel sizes agree), FSL's scales with `reproduce`, named when an encoded axis' voxel sizes differ (suite cases `at/jac-coef-aniso`, `at/jac-field-aniso`); the clean-room original: a field image already so (its coefficient-field default to follow, `specs/topup.md` §7.3) | original (coefficient fields) |

(FSL-261 to FSL-271's rows: relay 2's items in NOTES.md.)

## docs/api/upstream-defects.md (the user-facing table; the same columns as its FSL-145 row)

- FSL-272 | prelude | the replica `larmorx-nc fsl prelude` (the clean-room original: next part) | a NaN first phase value skips the phase range check and the rewrap to [−π, π] | the extrema of the other values: rewrapped, or FSL's range error (exit 255), as without the NaN; `reproduce` FSL's
- FSL-273 | applytopup | the replica `larmorx-nc fsl applytopup`; the clean-room original for field images | `--method=jac`'s Jacobian scaled by the field's own voxel sizes when they differ from the images' | the Jacobian of the displacement applied; `reproduce` FSL's

## docs/next-steps.md ("The questions for a spec writer, gathered", 2026-11-05 catch-up's follow-ups)

Replace the topup bullet's first two questions, the flirt bullet's "§3/§8.1 step 9's sign of a zero third skew ... first evaluated vector's zero" and "§6.4's weighted normcorr and woods costs' last bits", and "Carried from the brief: specs/fslmaths.md §10.4, §19, §7" with:
"ANSWERED by PRELUDE's spec writer (2026-11-06; validation/investigations/spec-answers-4/): topup §7.3 (FSL-273, rule A: the Jacobian of the mapping applied, in the replica; the clean-room original's coefficient-field default to follow); flirt §6.4 normcorr (the count's running total adds each point's weight as it is visited, not each row's total: the cause of real/weights-normcorr; also specs/mcflirt.md §6.4 and lx.mri.hmc's shared engine); flirt §3/§8.2's zero signs (a skew is never an exact −0: FSL's −0.000000 is a residue of the matrix's last bits; a line search's first evaluation is p + 0·û); fslmaths §10.4, §7, §19. Still open: real/weights-woods (the 4 mm pair and woods are FSL's at 40 matrices; the path's last bits), the third skew's parting (the decomposed matrix's last bits, an earlier step), BET's, FAST's, topup's --splineorder=2 bending and the 1×1×3 smoothing (FSL's smoothing wraps by a true modulo, as the original's: the 3 % lies elsewhere), standard_space_roi's headers (the box through FSL's internal frame: relay 5's analysis in docs/parts/<part>/notes.md), FLIRT's other items."
And add to the clean-room originals' follow-ups: "FLIRT's normcorr count per point (crates/larmorx-mri hmc::cost::correlation's row totals; also lx.mri.hmc); topup's coefficient-field Jacobian default (FSL-273); fslmaths' FFT twiddles with glibc's sincos in reproduce (specs/fslmaths.md §19); PRELUDE's original implements FSL-272 (specs/prelude.md §4.2)."

## CLAUDE.md rule 1 / porting guide §1

- `validation/investigations/fsl-flirt/normcorr_row_totals.py` is a replica probe (closed, as its folder);
  `validation/investigations/spec-answers-4/` and `validation/investigations/prelude-spec/fsl272_measure.py` are open
  (FSL runs, generated inputs, schedules).

## Report to the orchestrator

- The topup replica record changed (177 cases, 107 pass): `tables` to regenerate if `tables --check` fails.
- The PRELUDE replica record re-made (88, 68 pass, 20 both).
