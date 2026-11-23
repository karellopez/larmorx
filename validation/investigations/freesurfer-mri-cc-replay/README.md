# freesurfer-mri-cc-replay: `mri_cc` through recon-all's recorded `seg2cc` steps

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

recon-all runs `mri_cc` only inside the tcsh script `seg2cc` (`mri_cc -aseg aseg.auto_noCCseg.mgz -o
aseg.auto.mgz -lta transforms/cc_up.lta <subject>`, both streams). The recordings
(`<workspace>/oracles/freesurfer-8.2.0/runs/`) hold `seg2cc` as one step; until `seg2cc` itself is
ported (queue item 3.2-28), `replay.sh` replays each recorded step with FreeSurfer's own script and a
`PATH` on which `mri_cc` is the port: a shim that runs `larmorx-freesurfer --upstream-defects <mode>
freesurfer mri_cc`, put first on the step's `PATH` by a wrapper given to the replay tool as
`--command '<wrapper> {program}'`. The replay tool compares every file the step writes with the
recording (`aseg.auto.mgz`, `transforms/cc_up.lta`, `scripts/seg2cc.log`; FreeSurfer's provenance and
the log's dates, times and memory figures aside).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 validation/investigations/freesurfer-mri-cc-replay/replay.sh \
        crates-freesurfer/target/release/larmorx-freesurfer reproduce <scratch> [run-pattern]

## Results (relay 53 of track D, 2026-11-23)

The 12 recorded steps (6 heads x 2 streams): `reproduce` 12 reproduced, `fix` 12 reproduced; memory
(`tests/parity/test_memory_peak.py` `MRI_CC_CASES`): 328.7 and 329.6 MiB against FreeSurfer's 344.5
(`docs/findings/freesurfer-mri-cc.md` §4).
