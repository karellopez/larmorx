# FS-1075 measured: the Powell registration's half-way outputs (closed)

**Closed to FreeSurfer clean-room implementers**: the tool runs the replica's example (the probe's
case generator) and names the replica's defect handling. The findings:
`docs/findings/freesurfer-robust-register.md` §2 item 6; the behaviour for the clean room:
`specs/mri_robust_register.md` §2.2 and §5.

FreeSurfer 8.2.0's `mri_robust_register` with a cost other than the robust ones (`--cost` `LS`, `MI`,
`NMI`, `ECC`, `NCC`, `SCR`, `TB`, `LNCC`, `SAD`, `SB`) and a half-way output (`--halfmov`,
`--halfdst`, `--halfweights`, `--halfmovlta`, `--halfdstlta`) reads the half-way space's geometry
from an image it has already freed (FS-1075). Track C's relay 48 measured what that gives before
deciding (rule D, then A).

- `measure.py`: the probe's half-way family (`fs_robreg_probe halfway-cases`, 24 cases of every
  Powell cost with the Powell family's random options and random half-way outputs; two without
  symmetry; some with the finest level skipped, `--highit 0`), each run three times by the official
  program, never served from the oracle-output cache (its result is memory contents), every run in a
  folder of its own (`<workspace>/oracles/inputs/fs1075-halfway/run-<r>/case-<k>/`). Per case: the
  exit status of each run, whether the runs agree (both streams with the time masked, every file),
  and whether the half-way transform's target geometry (`--halfmovlta`'s `dst volume info`) is the
  destination's (`--lta`'s) or another.
  Measured 2026-11-23 (log `oracles/logs/fs-found-c-r48-fs1075-measure.log`): of the 21 cases that
  reach the half-way part, 5 gave the destination's geometry in all three runs, 16 memory contents
  (13 of them different from run to run); of the 63 runs 28 ended with status 1 (`MRI::Shape::Shape():
  volume size must be greater than 0 in every dimension`, `error: could not allocate memory for slice
  ...`), 3 with 252 (`MatrixMultiply(): m1 is null`), 8 were killed by the 16 GiB memory box (sizes of
  about 1.1e9 per axis), the rest wrote garbage geometry with status 0. The case with the finest level
  skipped and no coarser one prints `ERROR: no half way space created (not implemented for this cost
  function)!`; the two without symmetry `... (symmetry was switched off)!`.

The port's comparison is the probe's `check.py --family halfway` (`validation/probes/fs-robust-register/`).

Run from the repository, in a memory box:

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-fs1075-halfway/measure.py \
        --runs 3 --json <out.json>

`--cases 0,1,2` runs some cases only; `--preload <library>` runs the official program with
`LD_PRELOAD` (the probe's `decided_complex.sh` library). No case sets `DIAG_NO` or `DIAG`; the official
side runs with `FS_DISABLE_LANG=1`.
