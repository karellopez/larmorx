# FLIRT's clean-room original: development probes (part 1, 2026-11-02)

The tools behind [`docs/findings/fsl-flirt-original.md`](../../../docs/findings/fsl-flirt-original.md)
and the original's record ([`docs/validation/fsl-flirt.md`](../../../docs/validation/fsl-flirt.md)).
They run FSL 6.0.7.23 as a black box (through the oracle cache, under the `fsl-flirt-replica`
suite's keys) and the clean-room original (`LARMORX_FLIRT_BIN`, else `python -m larmorx`); each
inside `scripts/orchestration/boxed.sh`, with `PYTHONPATH=python:validation/src`, run with
`python -P`.

| File | What it shows | How to run |
|---|---|---|
| `probe.py` | one suite case, any command line or a random-testing finding with FSL and the original into two kept directories: the streams, the matrices (displacement over the reference's brain), the images' headers and differing values | `probe.py <dir> --case real/dof6`, `probe.py <dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat"`, `probe.py <dir> --finding <finding.json> [--whole]` |
| `costs.py`, `costs.sch`, `costs2.sch`, `costs3.sch` | the costs of two matrices at each pair (`costs3.sch`: the 4 mm pair and the two scales that only reset the input), every cost function, FSL against the original (the findings' "The costs") | `costs.py [--schedule costs3.sch] [-- -noresample]` |
| `corratio_synth.py`, `corratio_sparse.py` | `corratio` and `woods` on synthetic images: which bin counts agree (`corratio_synth.py`), and a reference bin holding 1 to 12 points on a 24³ or 10³ image (`corratio_sparse.py <dir> [small]`): the sparse bins' rule of the findings | `corratio_synth.py <dir>`, `corratio_sparse.py <dir> small` |
| `centre.sch` | the image pair's centre after each `setscale`: two rotations' parameters (`printparams`), whose translations give the centre (the findings' "The image pair's centre") | `probe.py <dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat -schedule <path>/centre.sch"` |
| `costs4.sch` | the weighted costs of scaled matrices at 8 mm (the scales of the search's first line search), with `-refweight` and `-inweight` | `probe.py <dir> --args "... -refweight @@w.nii -schedule <path>/costs4.sch"` |
| `bbrcost.sch` | the boundary-based cost of the initial matrix with `bbrstep` 1, 2, 200 and 201 (the findings' "The boundary-based cost") | `probe.py <dir> --args "-in @@epi -ref @@t1 -init @@m -wmseg @@wm -cost bbr -omat o.mat -schedule <path>/bbrcost.sch"` |
| `stages.py` | the default schedule with `print` after the search and every `measurecost` and `optimise`, FSL against the original: where a registration's path leaves FSL's (the findings' "The search and the optimisation") | `stages.py <dir> [flirt args, default -dof 6]` |
| `sinc_kernel.py` | which construction of the windowed sinc reproduces FSL's `-applyxfm -interp sinc` values (the findings' "The pyramid and the outputs") | `sinc_kernel.py <probe dir of real/interp-sinc> [voxels]` |
| `speed.py` | the informal speed table of the API page: FSL, the replica (`LARMORX_NC_BIN`) and the original at 1 and 8 threads, one run each | `speed.py` |

FSL's `flirt -verbose 7` (every cost evaluation with its parameters) was the reference for the
evaluation-by-evaluation comparison of the findings; the original printed its own evaluations
through a development hook that is no longer in the code.
