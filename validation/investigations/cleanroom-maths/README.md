# fslmaths' clean-room original: black-box probes

Helpers of the clean-room implementer of `crates/larmorx-fsl`'s `maths` (fslmaths' original),
behind [`docs/findings/fsl-maths-original.md`](../../../docs/findings/fsl-maths-original.md). They
run FSL 6.0.7.23's `fslmaths` as a black box, every run through the oracle-output cache (the
replica suite's runner, so a run made once is served after), and print what it writes.

| Script | What it shows |
|---|---|
| `probe_thresholds.py` | the thresholds' far side per computing type (FSL-152: `-thr`/`-uthr` inclusive, `-bin` exclusive, `short` wrapping too), `-save`'s type and display range, `-div`/`-rem`/`-max`/`-min` with an image of other sizes (FSL-159) (findings 1 to 3) |
| `show_fsl.py <case> ...` | FSL's exit status, standard output and error on cases of the replica suite (`fsl-maths-replica`), by name: the messages of findings 4 |

Run from the repository with the validation package on the path, inside a memory box:

```
PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 4 \
  python -P validation/investigations/cleanroom-maths/probe_thresholds.py
```
