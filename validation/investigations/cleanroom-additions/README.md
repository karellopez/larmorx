# The clean-room additions of 2026-10-30: black-box tools

The tools the clean-room implementer of FSL-20, FSL-21, FSL-65 and FSL-66 in the head-motion
correction (`crates/larmorx-mri`) and of AFNI-75's guard in 3dUnifize's original
(`crates/larmorx-afni`) used, from `specs/mcflirt.md` §10.6, §12.1, §14.1 and `specs/3dUnifize.md`
§9.1, §10. Black-box only: FSL 6.0.7.23's mcflirt, AFNI 26.3.05's 3dUnifize, larmorx's original
and the replica program are run and their outputs read; nothing here reads FSL's, AFNI's or the
replicas' source, nor the spec writer's probes (`../specs-memory/`). What they showed is in
[docs/findings/mri-hmc-original.md](../../../docs/findings/mri-hmc-original.md) and
[docs/findings/afni-masks-original.md](../../../docs/findings/afni-masks-original.md) (sections of
2026-10-30). The cases of §14.1 themselves are in the `mri-hmc` suite
(`validation/src/larmorx_validation/parity/mri_hmc_failures.py`, `failures/<case>`), those of
§10 in the `afni-unifize` suite (`guard/epi-<voxels>`).

Run them from the repository with `PYTHONPATH=python:validation/src` (larmorx is
`target/release/larmorx`).

| Tool | What it shows |
|---|---|
| `unifize_guard.py <work> [--larmorx <program>] [--afni <AFNI bin dir>]` | `-EPI` on series of 3 µm, 1 µm, 0.05 mm, 0.3 mm and 3 mm voxels (and one volume of 3 µm): the original's and the replica's AFNI-75 lines side by side (identical), their exit status and output digests, AFNI's where it runs |
| `hmc_bins_matrices.py --larmorx <program> <work> -- <mcflirt options>` | mcflirt's and larmorx's (`reproduce`) matrices on the §14.1 phantom, volume by volume: RMS deviation and text identity (few bins: one bin in stage 1 gives a cost of rounding noise) |
