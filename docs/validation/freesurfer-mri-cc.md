# Validation: FreeSurfer's `mri_cc` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `1e9f3f4d04be` with `validation/probes/fs-cc/record.py` (queue item 3.2-27; the whole program: the aseg path recon-all runs and the white-matter path, `-a 0`).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_cc/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_cc ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_cc) and its recon-all runs' recorded seg2cc steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's `seg2cc` (both streams: `mri_cc -aseg aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta transforms/cc_up.lta <subject>`) replayed with FreeSurfer's own script and the port as its `mri_cc` (`validation/investigations/freesurfer-mri-cc-replay/`; every file compared, FreeSurfer's provenance and the log's dates, times and memory figures aside):

- `reproduce`: 12 steps of 12 runs, **12 reproduced**, 0 not; the whole `seg2cc` step at one thread 247 s (recorded 362 s).
- `fix`: 12 steps of 12 runs, **12 reproduced**, 0 not; the whole `seg2cc` step at one thread 245 s (recorded 362 s).

## The white-matter path on the recorded heads

`validation/probes/fs-cc/real.py`: each run's `wm.mgz` written as a COR folder `wm` by the official `mri_convert -ot cor`, its `talairach.xfm` beside it; `-a 0` and `-a 0 -d 7 -t 3`; the official program against the port, messages and `cc_org.mgz` compared: 24 calls on 12 runs, `reproduce` **24 identical**, 0 different; `fix` 22 identical, 2 different with a notice (FS-1210: the seed search finds no slice), 0 without.

## The whole program against the official binary (the open probe)

`validation/investigations/freesurfer-mri-cc-probe/compare.py` with the replica as the candidate: every option, given last, by its first letter, its value forms, names of 1,023 and 1,024 bytes, the subjects folder from `-sdir` and the environment, missing inputs, read-only output folders; the aseg path on small segmentations and on a conformed phantom head with each option; the white-matter path on small volumes (NaNs, an unidentified `wm`, voxels of 0.45 mm) and on the phantom (a NIfTI file, a COR folder, voxels of 1.4 mm); exit status, standard output and error and every file written compared:

- seed 1: 112 cases; `reproduce` 107 identical, 0 different, 5 counted (FreeSurfer reads memory past the volume or its tables: FS-53, FS-1201, FS-1206); `fix` 41 identical, 66 different with a notice, 0 without.
  With `--heads`, recon-all's call and the white-matter path on the six recorded heads' two streams (24 calls): `reproduce` 24 identical, 0 different; `fix` 23 identical, 1 with a notice, 0 without.
- seed 2: 112 cases; `reproduce` 107 identical, 0 different, 5 counted (FreeSurfer reads memory past the volume or its tables: FS-53, FS-1201, FS-1206); `fix` 40 identical, 67 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_cc_golden.rs`: twelve cases on 32^3 subjects made with integer arithmetic (the white-matter path with other divisions and thickness and with NaNs kept, without `wm` (FS-1205) and without the transform, more than 50 divisions (FS-1211), the aseg path with `-m 1`, the callosum present, `-lh`, a name of 1,024 bytes (FS-1212), no subjects folder, a norm that does not read), on every platform: `reproduce` `93da69667ef481b6687d4fc483aea5a6998497ae47c0f297f470fcd165e03993`, the official program's value on the same inputs (`prog.py --golden`); the default `c817c0e13c00a567625bbea9e7d087d604958a26242b2d0b629a949349c34412`.

## Memory

`tests/parity/test_memory_peak.py` (`MRI_CC_CASES`: recon-all's `seg2cc` call; `MRI_CC_WM_CASES`: `-a 0`; ds000005 sub-01's v8 files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- mri_cc seg2cc (reproduce): 328 MiB, FreeSurfer 344 MiB.
- mri_cc seg2cc (fix): 329 MiB, FreeSurfer 344 MiB.
- mri_cc -a 0 wm (reproduce): 71 MiB, FreeSurfer 79 MiB.
- mri_cc -a 0 wm (fix): 70 MiB, FreeSurfer 79 MiB.
