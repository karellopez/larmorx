# fs-binarize: `mri_binarize`'s replica against FreeSurfer 8.2.0 (closed)

**Closed to FreeSurfer clean-room implementers**: these tools read FreeSurfer's source
(`help_text.py`) or run the replica (`prog.py`, `real.py`, `record.py`). The clean room's own
comparison is the open probe `validation/investigations/freesurfer-mri-binarize-probe/`.

- `help_text.py <freesurfer-8.2.0>`: `mri_binarize`'s usage and help texts as the Rust constants of
  `crates-freesurfer/larmorx-freesurfer-volumes/src/mri_binarize/help.rs` (every `printf` literal
  of `print_usage` and `print_help`, decoded as C decodes it).
- `prog.py --seed <s>`: the whole program, the official `$FREESURFER_HOME/bin/mri_binarize` (run by
  name) against the port's `larmorx-freesurfer` in both modes, on the open probe's 130 command
  lines (`compare.py`'s inputs, cases and comparison). `prog.py --golden`: the official program on
  the golden cases' inputs (the example `fs_binarize_golden`), hashed as
  `tests/mri_binarize_golden.rs` hashes the port's run.
- `real.py`: the real calls of recon-all, `mri_nu_correct.mni`, `mri_mcadura_seg`, `mri_vsinus_seg`,
  `label-cortex` and `seg2cc` (and a hippocampus's surface and topology fix) on the recorded heads'
  files (3 heads, both streams), both programs, every file compared.
- `record.py`: `docs/validation/freesurfer-mri-binarize.{json,md}` from `prog.py`'s, `real.py`'s
  and the memory test's results (from a clean commit).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-binarize/prog.py --seed 1 --json p1.json
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-binarize/prog.py --golden
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-binarize/real.py --json real.json

## Results (relay 53 of track D, 2026-11-23: after FS-268's and FS-269's defaults)

From the clean commit of `docs/validation/freesurfer-mri-binarize.md`: the open probe's seeds 1 and
2 (133 cases each): 125 identical in `reproduce`, 8 counted, 0 different; `fix` 106 identical, 19
explained by a notice (FS-269: `--crop 0`, `--bb 0`, `--bb -2`; FS-268: `--crop-around-ras` of an
`SRA` volume). `real.py`: 81 calls, 78 identical in `reproduce`, 3 counted, 0 different (as below).
The golden cases (seven): `76bdf3d9...` (`reproduce`, the official value), `be04301f...` (the
default). Memory: 4 calls, the port below FreeSurfer's peak (141.0 vs 144.8 MiB the largest).

## Results (relay 48 of track D, 2026-11-21)

The open probe's seeds 1 and 2 (130 cases each): 122 identical in `reproduce`, 8 counted, 0
different; `fix` identical or explained by a notice. `real.py`: 81 calls on 6 runs, 78 identical
in `reproduce`, 3 counted (FS-1021: the hippocampus already correct in the v8 stream's
segmentation; FreeSurfer then crashes, or fails with status 252, writing a freed volume), 0
different; in `fix` 1 differing with FS-1022 (ds000005's classic hippocampus: the fix keeps its
first voxel). Uncached, the 81 calls took FreeSurfer 177 s and the port 204 s. The golden cases: `42a367a2...`
(`reproduce`, the official value), `e2cb236f...` (the default). Memory: 4 calls on ds000005, the
port 2-4 % below FreeSurfer's peak, the files identical.
