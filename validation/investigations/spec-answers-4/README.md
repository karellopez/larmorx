# spec-answers-4: the waiting spec questions (PRELUDE's replica part, relay 5, 2026-11-06)

The spec writer's answers to the questions gathered for a spec writer in `docs/next-steps.md`
("The questions for a spec writer, gathered", the clean-room catch-up of 2026-11-05, and the older
ones it names). Each answer is written into its spec, marked "(spec writer, 2026-11-06 ...)",
behaviour only. The scripts here are written from the specs and black-box runs of FSL 6.0.7.23
(through the oracle cache in `use` mode; FSL off the `PATH`), so a clean-room implementer may run
them. Run everything inside `scripts/orchestration/boxed.sh` (8 GiB is plenty), with
`PYTHONPATH=python:validation/src` and `python -P`.

| File | What it shows | How to run |
|---|---|---|
| `jac_scales.py` | `specs/topup.md` §7.3, FSL-273: applytopup `--method=jac`'s Jacobian scales when the field's voxel sizes differ from the images'. Six FSL runs on the same image data: topup's coefficients (made on 6 mm voxels along y) on the data stored with 6, 4 and 3 mm voxels along y, and topup's field image with its header saying 6, 4 and 3 mm on the 6 mm data. FSL's outputs are `S (1 + c g)` (the resampled image `S` the same in every run) with `c = dy / fy` for coefficients and `c = dx / fy` for a field image: `g` solved from the two ratios to the reference run agrees to 5e-6 and 2.4e-6 at the median. With `--larmorx`, a larmorx program's default run in each directory against FSL's reference run (the Jacobian of the mapping applied gives it to single-precision rounding) | `jac_scales.py run <dir>`, then `jac_scales.py check <dir> [--larmorx larmorx fsl applytopup]` |
| `woods4.sch` | `specs/flirt.md` §6.4, the record's `real/weights-woods`: a schedule measuring `woods` on the 4 mm pair (`setscale 8`, `setscale 4`, smoothing 4) at the printed parameters of the trace's parting point (the record's 4 mm stage, evaluation 356) and 39 matrices around it (seeded); FSL's and the original's printed costs agree on all 40 (`0.948010` at the parting point), so the pair and `woods` are as specified and the trace parts on the path's last bits | `validation/investigations/cleanroom-flirt/probe.py <dir> --args "-in @epi.nii -ref @t1.nii -refweight @refweight.nii -cost woods -dof 6 -omat out.mat -schedule @@<this dir>/woods4.sch"` |

What the scripts gave (2026-11-06):

- `jac_scales.py`: coefficients: ratios 4 mm / 6 mm p1 0.980 p50 0.9967 p99 1.029, 3 mm / 6 mm
  p1 0.970 p50 0.9951 p99 1.043, the solved `g` (|g| > 0.02, 3,129 voxels) agreeing to 5.1e-6
  (p50), 2.6e-4 (p99); field image: ratios 1.005 and 1.010 at the median, `g` agreeing to 2.4e-6
  (p50), 9.6e-6 (p99). The topup replica's new default (`larmorx-nc fsl applytopup`) gives FSL's
  reference output bit for bit in 5 of the 6 runs and to 1.5e-5 relative in the sixth (the
  coefficients on 4 mm voxels: the displacement's millimetres converted back with 4 mm).
- `woods4.sch`: FSL and the original print the same 40 costs. The normalised correlation's
  answer (§6.4) came from a replica probe (`validation/investigations/fsl-flirt/normcorr_row_totals.py`,
  closed to a clean-room implementer): FSL's arithmetic with each row's total weight added to the
  count at once gives the original's trace and matrix; the text states what FSL does instead.
