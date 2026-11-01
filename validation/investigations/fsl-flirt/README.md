# FLIRT replica: investigation helpers

`compare_images.py` compares NIfTI images the way the parity suite `fsl-flirt-replica` does (the
decompressed bytes with the header's `descrip` blanked) and, when they differ, prints the largest
data difference and the number of differing voxels. It was used while the replica was brought to
bit-identity (2026-11-01) to compare FSL's and the replica's outputs of one case by hand.

```bash
python -P compare_images.py <fsl/out.nii.gz> <replica/out.nii.gz> [more pairs]
```

The record itself is made with `python -m larmorx_validation parity fsl-flirt-replica`; the
probes beside it are in `validation/probes/fsl-flirt-costs` and `validation/probes/fsl-flirt-pinv`.
