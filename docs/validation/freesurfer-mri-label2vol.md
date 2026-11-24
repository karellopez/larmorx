# Validation: FreeSurfer's `mri_label2vol` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `6e0ba3763925` with `validation/probes/fs-label2vol/record.py` (queue item 3.2-40, track A relays 56 and 57: every option, message, exit status and failure case, `--new-aseg2vol` and `--fill-ribbon` included).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_label2vol/, its resample.rs: MRIseg2SegPVF, VOXsegPVF2Seg, MRIsurf2VolOpt), fs.surf (defects2seg.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_label2vol ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_label2vol) and its recon-all runs (oracles/freesurfer-8.2.0/runs/: defect2seg's calls).

## The recorded calls, on the recorded heads

`validation/probes/fs-label2vol/real.py`: `defect2seg`'s two calls per run (the left hemisphere's defects into `orig.mgz` with the cortex label, the right's merged into the left's volume) on the six classic and six v8 runs, and, on the v8 runs, `seg2cc`'s form (`--seg <seg> --temp <vol> --regheader`: the head's `aseg.mgz` onto its native grid `rawavg.mgz` and back onto `norm.mgz`) and part 2's paths, which no recorded step takes (`--new-aseg2vol --ttype --resmm 0.5`: the aseg onto `rawavg.mgz`'s grid with 8 samples a voxel and the tissue types' vote; `--fill-ribbon` with `lh.cortex.label` and its stats, and with the atlas annotation `rh.aparc.annot`, whose table lists a structure no vertex has: FreeSurfer crashes, FS-1268), each run's own files (ds000005-sub-01-classic, ds000005-sub-01-v8, ds000228-sub-pixar008-classic, ds000228-sub-pixar008-v8, ds002422-sub-02-classic, ds002422-sub-02-v8, ds004173-sub-603309-classic, ds004173-sub-603309-v8, ds005003-sub-RS001-classic, ds005003-sub-RS001-v8, ds005454-sub-16-classic, ds005454-sub-16-v8; by kind: defects-lh 12, defects-rh 12, fill-ribbon-annot 6, fill-ribbon-label 6, new-aseg2vol 6, seg-to-conformed 6, seg-to-native 6): 54 calls, `reproduce` 54 identical to the official program (exit status, both streams, every file byte for byte, the command lines a volume records masked), the default 48; the official defect volume's header and voxels equal to the run's recorded `mri/surface.defects.mgz` in 12 of 12 (its colour table's random colours left out: recon-all seeds them from the clock, the probe with `FREESURFER_SEED=1234`); 6 calls in which the official program crashes, ended by `reproduce` with the same signal's status (counted as identical; `fix` maps them); `fix` different with a notice in 6.

## The whole program against the official one

`validation/probes/fs-label2vol/check.py`: the command line (no argument, the version and help options, unknown words, every value-taking option without its value, the case rules, `--debug`), every check and refused combination, templates of four voxel types and five grids (one of a NaN voxel size), registrations by `register.dat`, `.lta`, headers, the identity, inverted and singular; volume labels (NaN, infinite and huge coordinates, scanner coordinates with and without `--tkr-template`, an empty and a broken label), several labels with `--hits`, `--fillthresh`, `--labvoxvol`, `--offset` (to the int range's end), `--label-stat`, `--stat-thresh`, `--pvf`; surface labels projected along the normals (`abs`, `frac`, `none`, an infinite stop, NaN, a step of 2e-5 that passes 32,767 hits in one voxel, points beyond the surface); an annotation with its colour table (offsets, hits, a projection, no labels, a short file); segmentations of three grids by headers, a registration or the identity, with `--fillthresh`, `--upsample`, `--ttype`, `--ttype+head`, `--hits`, `--pvf`, `--aparc+aseg`; `--defects` into templates of three types (merged and not, a label, no volume geometry, NaN and infinite defect numbers, negative ones, numbers near the int range, a short overlay, offsets and merge flags that are no numbers, a negative offset, missing files); `--new-aseg2vol` (segmentations of 1, 0.5 and 0.4 mm voxels, labels below 0, a label of 10^6, labels without a tissue type or not in the colour table, a segmentation of 0 only, `--resmm` positive, negative, 0, NaN and infinite, `--upsample` 0, 3 and -2, `--ttype` and `--ttype+head` with two colour tables, the tissue types' vote over many samples); `--fill-ribbon` (the ribbon's 3, 42 and other values; surface labels, stats NaN and infinite, `--label-stat`, `--stat-thresh`, labels of vertices outside the surface, in scanner coordinates with and without `--tkr-template`; annotations with every index and with gaps; `--lta` with the ribbon at the source, the destination and neither end; a ribbon far from the surface; `--sd` other than `SUBJECTS_DIR`, none, a `FREESURFER_HOME` without its colour table; a right hemisphere; outputs that cannot be written); then seeded command lines. Each case in one folder; exit status, both streams and every file written, case by case; counted apart: FreeSurfer's crashes (CRASH: statuses 134, 136, 139), results it takes from memory contents (MEMORY: FS-53, FS-249, FS-441, FS-1252, FS-1256, FS-1258, FS-1259, FS-1263, FS-1265) and a projection that never ends (HANG: FS-1257); `MRIseg2SegPVF(): done t = <seconds>` masked:

- seed 1: 600 cases; `reproduce` 551 identical, 0 different, 49 counted (CRASH 26, HANG 1, MEMORY 23); `fix` 116 different with a notice, 0 without.
- seed 2: 1500 cases; `reproduce` 1381 identical, 0 different, 119 counted (CRASH 73, HANG 1, MEMORY 57); `fix` 264 different with a notice, 0 without.


## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_label2vol_golden.rs`: 15 cases on volumes up to 12^3 and a surface of 66 vertices (`defect2seg`'s two calls, defect numbers with NaN and infinities from their bits, `seg2cc`'s segmentation onto another grid and upsampled with its hits, two labels with their hits, a label with NaN, infinite and huge coordinates through a `register.dat` with its stat volume, a projected surface label, an annotation; `--new-aseg2vol` with no sample per voxel (FS-1266), with the tissue types' vote and through a `register.dat`; `--fill-ribbon` with a label of NaN and infinite stats and its stat volume, through an `.lta` whose destination is the ribbon, and with an annotation), on every platform: `reproduce` `f50d82be5500fd538efeaa58f05453303144525721334ce59d38d3afe5c0b139` (the official program's value on the same inputs, `check.py --golden`), `fix` `17475818fc453f38cbb8061dba1f5e8260aaf3544a1da5844d9c0b008b101953`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_label2vol_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's v8 files read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical, its command lines masked):

- defects (reproduce): 241.4 MiB, FreeSurfer 226.1 MiB.
- defects (fix): 242.0 MiB, FreeSurfer 226.1 MiB.
- defects-merge (reproduce): 228.8 MiB, FreeSurfer 214.4 MiB.
- defects-merge (fix): 228.6 MiB, FreeSurfer 214.4 MiB.
- seg (reproduce): 837.0 MiB, FreeSurfer 841.8 MiB.
- seg (fix): 836.5 MiB, FreeSurfer 841.8 MiB.
- label (reproduce): 75.2 MiB, FreeSurfer 77.4 MiB.
- label (fix): 75.4 MiB, FreeSurfer 77.4 MiB.
- proj (reproduce): 230.6 MiB, FreeSurfer 214.1 MiB.
- proj (fix): 230.3 MiB, FreeSurfer 214.1 MiB.
- new-aseg2vol (reproduce): 134.5 MiB, FreeSurfer 205.6 MiB.
- new-aseg2vol (fix): 135.0 MiB, FreeSurfer 205.6 MiB.
- fill-ribbon (reproduce): 248.1 MiB, FreeSurfer 267.1 MiB.
- fill-ribbon (fix): 248.1 MiB, FreeSurfer 267.1 MiB.
