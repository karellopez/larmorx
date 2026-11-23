# fs-vol2vol: `mri_vol2vol` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_vol2vol.md`, whose §9 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_vol2vol/`)
gives the official program's results bit for bit with `--upstream-defects reproduce` (exit status,
standard output, standard error, the files written), case by case, and differs by default only
where a defect it corrects is noted. Queue item 3.2-38, part 1 (track A, relay 53): every option's
reading and checks and every linear path; the morphs, `--gcam`, `--spm-warp`, `--map-point`,
`--disp` and `--cost` end as not ported yet (counted, not compared).

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_vol2vol_probe` generates the cases (the
  fixed ones: the command line's oddities, every check, every registration, method, path and
  option, the v8 scripts' and fMRIPrep's forms; then seeded command lines), each with its files:
  moving and target volumes of several types, geometries and formats, transforms of every kind
  and their failures, and the case's own `fsh/` (`FREESURFER_HOME`: the Talairach subfov and
  MNI152 registrations), `subjects/` (`SUBJECTS_DIR`: `s1`'s `orig.mgz`, `brain.mgz`,
  `talairach.xfm`, `reg.mni152.2mm.dat`) and `fsl/` (`FSLDIR`); the official `mri_vol2vol`
  (`$FREESURFER_HOME/bin`, by name, `FS_DISABLE_LANG=1`, the licence from `FS_LICENSE`) runs each
  through the oracle-output cache at one thread, **every case in the same folder**
  (`<inputs>/fs-vol2vol/case`: names built from `SUBJECTS_DIR` or `FREESURFER_HOME` are absolute,
  and the cache stores output files as they are; the layout is keyed as an input, `layout.txt`),
  with a 30 s limit; the noise seed it prints (`SynthSeed`, from the clock without `--seed`) and
  `VmPeak` are volatile for the cache (`Policy.volatile`) and masked in the comparison. The port
  runs the cases in both modes; compared line by line with the folder as `<DIR>`, the transforms'
  `# created by` line masked. `SKIP`: `CRASH` (the port's status is one FreeSurfer's build reaches
  by a signal: FS-1178's abort, FS-1179's division by 0, FS-1180's and FS-1181's crashes: the
  official run's unflushed standard output is lost) and `NOTPORTED` (part 2's paths).
  `--golden`: the golden cases' official SHA-256 (`tests/mri_vol2vol_golden.rs`). `--dump <dir>`:
  every line of the three sides. No case sets `DIAG_NO`, and none `--gdiagno` above 0: FreeSurfer's
  licence check prints the licence's lines when the diagnostic number is above 0 (the script
  refuses such a case).
- `real.py --port <larmorx-freesurfer>`: the v8 stream's nine calls per head and fMRIPrep's
  (sMRIPrep's `ApplyVolTransform`) on the six recorded v8 heads, on their own files (the scripts'
  temporary inputs made again by the official `mri_vol2vol`, `mri_mask` and `lta_convert`, in
  `<inputs>/fs-vol2vol-real/`).
- `record.py`: `docs/validation/freesurfer-mri-vol2vol.{json,md}`.

## Results (relay 53)

Seed 1, 600 cases: 587 identical with `reproduce`, 13 `SKIP` (5 crashes, 8 paths of part 2), 0
different; `fix` differs on 113, each with a notice. Seed 2, 1,500 cases: see the record. The
recorded calls and fMRIPrep's: 72 on 6 heads, all identical in both modes. Recorded test:
`tests/fs_vol2vol_official.rs` (seed 1, 600 cases, 13 `SKIP`, `27d88053...`). Golden: 8 cases,
`be32774d...` (official and `reproduce`), `c84fa0a2...` (`fix`).

    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-vol2vol/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-vol2vol/check.py --golden
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-vol2vol/real.py --port <larmorx-freesurfer>
