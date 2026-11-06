# PRELUDE (FSL 6.0.7.23's `prelude`): probes behind the findings

The replica (`larmorx-nc fsl prelude`) is bit-identical to FSL in `reproduce` mode on every case
of the `fsl-prelude-replica` suite, so these probes run the replica in both modes on the suite's
derived inputs (`oracles/inputs/fsl-prelude-replica/v<N>`, made by the suite) instead of FSL.

| Script | What it shows | Run |
|---|---|---|
| `nan_phase_effect.py` | NaN phases in a few voxels inside the mask (`s_nanph`): in 3D, `-s` and `--labelslices` only those voxels are NaN in FSL's output, nothing else moves (the NaN voxels form a label of their own) | `python -I nan_phase_effect.py <larmorx-nc> <inputs dir>` |
| `nan_phase_region.py` | FSL-269: a NaN phase inside a region of the first phase bin (`s_nanph_bin1`): the region shares the NaN's label, its NaN constraints are merged last with the most negative integer as their offset, and FSL writes about 1.35e10 (2^31 cycles) in its 2,608 voxels; the corrected run leaves the NaN voxel out of the labels (NaN in the output) | `python -I nan_phase_region.py <larmorx-nc> <inputs dir>` |

Both need `nibabel` and `numpy` (the workspace's `.venv`).
