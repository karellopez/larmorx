# FLIRT's clean-room original: development probes (part 1, 2026-11-02)

The tools behind [`docs/findings/fsl-flirt-original.md`](../../../docs/findings/fsl-flirt-original.md)
and the original's record ([`docs/validation/fsl-flirt.md`](../../../docs/validation/fsl-flirt.md)).
They run FSL 6.0.7.23 as a black box (through the oracle cache, under the `fsl-flirt-replica`
suite's keys) and the clean-room original (`LARMORX_FLIRT_BIN`, else `python -m larmorx`); each
inside `scripts/orchestration/boxed.sh`, with `PYTHONPATH=python:validation/src`.

| File | What it shows | How to run |
|---|---|---|
| `probe.py` | one suite case (or any command line) with FSL and the original into two kept directories: the streams, the matrices (displacement over the reference's brain), the images' headers and differing values | `probe.py <dir> --case real/dof6`, `probe.py <dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat"` |
| `costs.py`, `costs.sch`, `costs2.sch` | the costs of two matrices at each pair, every cost function, FSL against the original (the findings' "The costs") | `costs.py [--schedule costs2.sch] [-- -noresample]` |
| `sinc_kernel.py` | which construction of the windowed sinc reproduces FSL's `-applyxfm -interp sinc` values (the findings' "The pyramid and the outputs") | `sinc_kernel.py <probe dir of real/interp-sinc> [voxels]` |
