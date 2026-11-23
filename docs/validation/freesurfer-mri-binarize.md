# Validation: FreeSurfer's `mri_binarize` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `d84bebf77797` with `validation/probes/fs-binarize/record.py` (queue item 3.2-19).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_binarize/) and larmorx-freesurfer-deform (src/tess.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_binarize ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_binarize) and its recon-all runs' files (oracles/freesurfer-8.2.0/runs/).

## The real calls on the recorded heads

`mri_binarize` is no step of its own in the reference runs (recon-all, `mri_nu_correct.mni`, `mri_mcadura_seg`, `mri_vsinus_seg`, `label-cortex` and `seg2cc` run it; their recorded steps check it when those scripts are ported). `validation/probes/fs-binarize/real.py` runs their calls on the recorded files of 6 runs (3 heads, both streams) with the official program and the port, comparing exit status, messages and every file written (FreeSurfer's provenance aside), one thread:

- 81 calls (15 kinds: the white matter's mask of the segmentation, the counts of `wm.mgz`'s edits, the background in place, `mri_nu_correct.mni`'s ones and mask, the cortex and its inverse, the crop around the centre, the corpus callosum, `label-cortex`'s entorhinal labels and the white matter of the v8 stream's segmentation, a hippocampus's surface and its topology fix).
- `reproduce`: **78 identical**, 0 different, 3 counted (the topology fix of a hippocampus that is already correct: FreeSurfer writes a volume it has freed, FS-1021).
- `fix`: 77 identical, 1 different with a notice (FS-1022), 0 without.
- Time (the calls not served from the cache): official 2 s (those run), port 212 s (`reproduce`).

## The whole program against the official binary

`validation/probes/fs-binarize/prog.py` (the open probe's command lines, `validation/investigations/freesurfer-mri-binarize-probe/`): every option, the three stand-alone options, the failures; exit status, standard output and error, and every file written compared:

- seed 1: 133 cases; `reproduce` 125 identical, 0 different, 8 counted (FreeSurfer reads memory it does not own, writes past its lists, crashes there or runs out of address space: FS-53, FS-1020, FS-1021, FS-1023, FS-1025, FS-1026, FS-1027); `fix` 106 identical, 19 different with a notice, 0 without.
- seed 2: 133 cases; `reproduce` 125 identical, 0 different, 8 counted (FreeSurfer reads memory it does not own, writes past its lists, crashes there or runs out of address space: FS-53, FS-1020, FS-1021, FS-1023, FS-1025, FS-1026, FS-1027); `fix` 106 identical, 19 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_binarize_golden.rs`: seven cases on 32^3 inputs made with integer arithmetic (a match with its count file, a threshold in a mask merged as `uchar`, replacements, a dilation, an erosion and the bounding box, a smoothed surface, the topology fix of a shape with a handle, the bounding box without a pad), on every platform: `reproduce` `76bdf3d94f233cad211f17a0e4ca5b0122fe6fc798a292543122ba4d163f0897`, the official program's value on the same inputs (`prog.py --golden`); the default `be04301f13838bbdf18d4ed1d06209755228937f1be1f748ddf8bb3606698216` (the topology fix keeps its first voxel, FS-1022; the box without a pad keeps the mask's last voxels, FS-269).

## Memory

`tests/parity/test_memory_peak.py::test_mri_binarize_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- wm-mask (reproduce): 134 MiB, FreeSurfer 140 MiB.
- wm-mask (fix): 134 MiB, FreeSurfer 140 MiB.
- ones (reproduce): 86 MiB, FreeSurfer 88 MiB.
- ones (fix): 86 MiB, FreeSurfer 88 MiB.
- wm255 (reproduce): 86 MiB, FreeSurfer 88 MiB.
- wm255 (fix): 86 MiB, FreeSurfer 88 MiB.
- surf (reproduce): 140 MiB, FreeSurfer 145 MiB.
- surf (fix): 141 MiB, FreeSurfer 145 MiB.
