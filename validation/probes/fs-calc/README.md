# fs-calc: `mris_calc`'s replica against FreeSurfer 8.2.0 (closed)

**Closed to FreeSurfer clean-room implementers**: these tools run the replica (`prog.py`,
`real.py`, `record.py`). The clean room's own comparison is the open probe
`validation/investigations/freesurfer-mris-calc-probe/`.

- `prog.py --seed <s>`: the whole program, the official `$FREESURFER_HOME/bin/mris_calc` (run by
  name) against the port's `larmorx-freesurfer` in both modes, on the open probe's 228 command
  lines (`compare.py`'s inputs, cases and comparison). `prog.py --golden`: the official program on
  the golden cases' inputs (the example `fs_calc_golden`), hashed as `tests/mris_calc_golden.rs`
  hashes the port's run.
- `real.py`: the real calls of `mri_nu_correct.mni` (the rescale, with each run's recorded ratio,
  on a copy of its `orig.mgz`: N3's output is not kept), the vertex areas of both hemispheres (two
  calls in one folder; their result also against the run's own `surf/?h.area.mid`) and
  `label-cortex`'s mask product (on a copy of a surface overlay of the run) on the recorded heads
  (3 heads, both streams), both programs, every file compared.
- `record.py`: `docs/validation/freesurfer-mris-calc.{json,md}` from `prog.py`'s, `real.py`'s and
  the memory test's results (from a clean commit).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-calc/prog.py --seed 1 --json p1.json
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-calc/prog.py --golden
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-calc/real.py --json real.json

## Results (relay 50 of track D, 2026-11-22)

The open probe's seeds 1 and 2 (228 cases each): 223 identical in `reproduce`, 5 counted, 0
different; `fix` identical or explained by a notice. `real.py`: 30 calls on 6 runs, 30 identical
in both modes; the 12 vertex-area results equal the runs' own `?h.area.mid`. Uncached, the calls
took FreeSurfer 12.3 s and the port 12.6 s. The golden cases: `b458bcb2...` (`reproduce`, the
official value), `4c2c9591...` (the default: the correctly rounded `atan2f`, `pow` and `log10`,
FS-17's `andbw`). Memory: 3 calls on ds000005, the port 0-7 % below FreeSurfer's peak, the files
identical.
