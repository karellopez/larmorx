# Validation: FreeSurfer's `mri_segment` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `3cc0205e3bc5` with `validation/probes/fs-segment/record.py` (queue item 3.2-31; the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_segment/) on larmorx-freesurfer-filter (src/polv.rs, classify.rs, nonpar.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_segment ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_segment) and its recon-all runs' recorded mri_segment steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's call (both streams: `mri_segment -wsizemm 13 -mprage antsdn.brain.mgz wm.seg.mgz`) replayed by the replay tool on each recorded run (`wm.seg.mgz` and `segment.dat` compared, FreeSurfer's provenance aside):

- `reproduce`: 12 steps of 12 runs, **12 reproduced**, 0 not; at one thread 782 s in all (recorded 512 s).
- `fix`: 12 steps of 12 runs, **12 reproduced**, 0 not; at one thread 781 s in all (recorded 512 s).

## The whole program against the official binary (the open probe)

`validation/investigations/freesurfer-mri-segment-probe/compare.py` with the replica as the candidate: every option, given last, by its first letter, its value forms; inputs of bytes, `float` with NaNs and infinities, shorts, another extent, not coronal, all 0 and constant; masks and segmentations of the input's extent and of others, a segmentation that loses its white matter, `-keep`, unwritable outputs; exit status, standard output and error (the time masked) and every file written compared:

- seed 1: 139 cases; `reproduce` 130 identical, 0 different, 9 counted (FreeSurfer reads or writes memory it does not own or never wrote: FS-498, FS-1216 to FS-1218, FS-1222, FS-1223, FS-1228, FS-1231); `fix` 84 identical, 46 different with a notice, 0 without.
- seed 2: 139 cases; `reproduce` 130 identical, 0 different, 9 counted (FreeSurfer reads or writes memory it does not own or never wrote: FS-498, FS-1216 to FS-1218, FS-1222, FS-1223, FS-1228, FS-1231); `fix` 85 identical, 45 different with a notice, 0 without.
- With `--heads`, five options recon-all does not give (`-noauto`, `-seg`, `-fillbg -fillv`, `-thicken 0`, `-keep`) on two recorded heads (10 calls): `reproduce` 10 identical, 0 different; `fix` 10 identical, 0 with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_segment_golden.rs`: eight cases on 32^3 volumes made with integer arithmetic (recon-all's call, a `float` input with NaNs and infinities by their bits on `-thickenonly`, `-noauto -thicken 0`, `-seg`, `-fillbg -fillv` (FS-1227), `-keep` with `-b 0.5 -polvlen 2` (FS-1220), an output into a missing folder (FS-1226), an option without its value (FS-1219)), on every platform: `reproduce` `17f6257bbee705bc739dffa5d80c6545aefb99cf282f9a0f0c6009546cb38501`, the official program's value on the same inputs (`prog.py --golden`); the default `e3ff2e21bef7d31c481dda27bcfdcb2b0384bd58ea95392a86745eec8ae22207`.

## Memory

`tests/parity/test_memory_peak.py` (`MRI_SEGMENT_CASES`: recon-all's call and `-seg`'s path on ds000005 sub-01's classic files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- mri_segment recon-all (reproduce): 134 MiB, FreeSurfer 139 MiB.
- mri_segment recon-all (fix): 134 MiB, FreeSurfer 139 MiB.
- mri_segment seg (reproduce): 331 MiB, FreeSurfer 346 MiB.
- mri_segment seg (fix): 331 MiB, FreeSurfer 346 MiB.
