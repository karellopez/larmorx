# Validation: FreeSurfer's `mri_label2vol` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `880c66875e97` with `validation/probes/fs-label2vol/record.py` (queue item 3.2-40 part 1, track A relay 56: every option, message, exit status and failure case but `--new-aseg2vol` and `--fill-ribbon`, which end with a message, status 1: part 2).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_label2vol/), fs.surf (defects2seg.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_label2vol ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_label2vol) and its recon-all runs (oracles/freesurfer-8.2.0/runs/: defect2seg's calls).

## The recorded calls, on the recorded heads

`validation/probes/fs-label2vol/real.py`: `defect2seg`'s two calls per run (the left hemisphere's defects into `orig.mgz` with the cortex label, the right's merged into the left's volume) on the six classic and six v8 runs, and, on the v8 runs, `seg2cc`'s form (`--seg <seg> --temp <vol> --regheader`: the head's `aseg.mgz` onto its native grid `rawavg.mgz` and back onto `norm.mgz`), each run's own files (ds000005-sub-01-classic, ds000005-sub-01-v8, ds000228-sub-pixar008-classic, ds000228-sub-pixar008-v8, ds002422-sub-02-classic, ds002422-sub-02-v8, ds004173-sub-603309-classic, ds004173-sub-603309-v8, ds005003-sub-RS001-classic, ds005003-sub-RS001-v8, ds005454-sub-16-classic, ds005454-sub-16-v8; by kind: defects-lh 12, defects-rh 12, seg-to-conformed 6, seg-to-native 6): 36 calls, `reproduce` 36 identical to the official program (exit status, both streams, every file byte for byte, the command lines a volume records masked), the default 36; the official defect volume's header and voxels equal to the run's recorded `mri/surface.defects.mgz` in 12 of 12 (its colour table's random colours left out: recon-all seeds them from the clock, the probe with `FREESURFER_SEED=1234`).

## The whole program against the official one

`validation/probes/fs-label2vol/check.py`: the command line (no argument, the version and help options, unknown words, every value-taking option without its value, the case rules, `--debug`), every check and refused combination, templates of four voxel types and five grids (one of a NaN voxel size), registrations by `register.dat`, `.lta`, headers, the identity, inverted and singular; volume labels (NaN, infinite and huge coordinates, scanner coordinates with and without `--tkr-template`, an empty and a broken label), several labels with `--hits`, `--fillthresh`, `--labvoxvol`, `--offset` (to the int range's end), `--label-stat`, `--stat-thresh`, `--pvf`; surface labels projected along the normals (`abs`, `frac`, `none`, an infinite stop, NaN, a step of 2e-5 that passes 32,767 hits in one voxel, points beyond the surface); an annotation with its colour table (offsets, hits, a projection, no labels, a short file); segmentations of three grids by headers, a registration or the identity, with `--fillthresh`, `--upsample`, `--ttype`, `--ttype+head`, `--hits`, `--pvf`, `--aparc+aseg`; `--defects` into templates of three types (merged and not, a label, no volume geometry, NaN and infinite defect numbers, negative ones, numbers near the int range, a short overlay, offsets and merge flags that are no numbers, a negative offset, missing files); then seeded command lines. Each case in one folder; exit status, both streams and every file written, case by case; counted apart: FreeSurfer's crashes (CRASH: statuses 134, 136, 139), results it takes from memory contents (MEMORY: FS-53, FS-249, FS-1252, FS-1256, FS-1258, FS-1259) and a projection that never ends (HANG: FS-1257):

- seed 1: 600 cases; `reproduce` 589 identical, 0 different, 11 counted (CRASH 4, HANG 1, MEMORY 6); `fix` 119 different with a notice, 0 without.
- seed 2: 1500 cases; `reproduce` 1489 identical, 0 different, 11 counted (CRASH 4, HANG 1, MEMORY 6); `fix` 292 different with a notice, 0 without.


## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_label2vol_golden.rs`: 9 cases on volumes up to 12^3 and a surface of 66 vertices (`defect2seg`'s two calls, defect numbers with NaN and infinities from their bits, `seg2cc`'s segmentation onto another grid and upsampled with its hits, two labels with their hits, a label with NaN, infinite and huge coordinates through a `register.dat` with its stat volume, a projected surface label, an annotation), on every platform: `reproduce` `aeabe265b849ca1002d79d2e07cc640fc12d97ca20468cc7dd9be3fe21d7e717` (the official program's value on the same inputs, `check.py --golden`), `fix` `9cb289bfca40e71fb04cc2f6ecd364930c72794d207a8fd168347cccf27caa2c`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_label2vol_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's v8 files read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical, its command lines masked):

- defects (reproduce): 242.4 MiB, FreeSurfer 226.1 MiB.
- defects (fix): 242.2 MiB, FreeSurfer 226.1 MiB.
- defects-merge (reproduce): 228.4 MiB, FreeSurfer 214.4 MiB.
- defects-merge (fix): 228.3 MiB, FreeSurfer 214.4 MiB.
- seg (reproduce): 838.0 MiB, FreeSurfer 841.8 MiB.
- seg (fix): 838.5 MiB, FreeSurfer 841.8 MiB.
- label (reproduce): 76.0 MiB, FreeSurfer 77.4 MiB.
- label (fix): 76.4 MiB, FreeSurfer 77.4 MiB.
- proj (reproduce): 230.9 MiB, FreeSurfer 214.1 MiB.
- proj (fix): 230.9 MiB, FreeSurfer 214.1 MiB.
