# FLIRT replica: investigation helpers

- `compare_images.py` compares NIfTI images the way the parity suite `fsl-flirt-replica` does (the
  decompressed bytes with the header's `descrip` blanked) and, when they differ, prints the
  largest data difference and the number of differing voxels. Used while the replica was brought
  to bit-identity (2026-11-01) to compare FSL's and the replica's outputs of one case by hand.
- `finding_repro.py` reproduces one finding of random testing (`difftest`, tool `fsl-flirt`) by
  hand: the shrunk recipe's inputs (or the drawn one's, `--drawn`) in two directories, FSL's
  `flirt` and the replica (`reproduce`) on each, the output images compared voxel by voxel. It
  showed the two differences part 1 fixed (the blur of `-applyxfm` beyond the image's edges; the
  sign of BBR's NaN cost).
- `recheck_findings.py` re-runs every finding of a random-testing run (shrunk and drawn recipes)
  through the suite's own runner with the program in `LARMORX_NC_BIN` and says which still
  differ: what a fix leaves open.
- `crash_probe.py` runs FSL's `flirt` and the replica with the default options on small and
  constant images, to narrow down the inputs on which FSL corrupts its heap (part 2).

```bash
python -P compare_images.py <fsl/out.nii.gz> <replica/out.nii.gz> [more pairs]
python -P finding_repro.py <workspace>/oracles/difftest/2026-11-01-fsl-flirt/findings/fsl-flirt/trial-flirt-00016-replica.json <scratch>/r16
LARMORX_NC_BIN=<larmorx-nc> python -P recheck_findings.py <run>/findings/fsl-flirt all 16
python -P crash_probe.py <larmorx-nc>
```

Run them with `PYTHONPATH=<checkout>/python:<checkout>/validation/src` and `FSLDIR` unset or
FSL 6.0.7.23's. The record itself is made with `python -m larmorx_validation parity
fsl-flirt-replica`; the probes beside it are in `validation/probes/fsl-flirt-costs` and
`validation/probes/fsl-flirt-pinv`.
