# Validation: FreeSurfer's `mris_calc` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `f9a22e16eff5` with `validation/probes/fs-calc/record.py` (queue item 3.2-21; the whole program, its curvature files included, so queue item 3.5-37 adds only checks).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mris_calc/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mris_calc ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mris_calc) and its recon-all runs' files (oracles/freesurfer-8.2.0/runs/).

## The real calls on the recorded heads

`mris_calc` is no step of its own in the reference runs (`mri_nu_correct.mni`, recon-all's vertex areas and `label-cortex` run it; their recorded steps check it when those scripts are ported). `validation/probes/fs-calc/real.py` runs their calls on the recorded files of 6 runs (3 heads, both streams) with the official program and the port, comparing exit status, messages and every file written (FreeSurfer's provenance aside), one thread:

- 30 calls (5 kinds: `mri_nu_correct.mni`'s rescale with each run's recorded ratio on a copy of its `orig.mgz` (N3's output is not kept), the vertex areas of each hemisphere (`add`, then `div 2` in place), `label-cortex`'s mask product on a surface overlay).
- `reproduce`: **30 identical**, 0 different, 0 counted; the vertex areas equal the runs' own `?h.area.mid` in 12 of 12 (and in 12 with the default).
- `fix`: 30 identical, 0 different with a notice, 0 without.
- Time (the calls not served from the cache): official 0.0 s (those run), port 12.8 s (`reproduce`).

## The whole program against the official binary

`validation/probes/fs-calc/prog.py` (the open probe's command lines, `validation/investigations/freesurfer-mris-calc-probe/`): every option, every action on curvature files with NaN, infinite and denormal values, on numbers and on volumes, the statistics, the output's names, labels, the failures; exit status, standard output and error, and every file written compared:

- seed 1: 228 cases; `reproduce` 223 identical, 0 different, 5 counted (FreeSurfer reads memory it never wrote or writes outside its mask: FS-167, FS-1124, FS-1125); `fix` 177 identical, 46 different with a notice, 0 without.
- seed 2: 228 cases; `reproduce` 223 identical, 0 different, 5 counted (FreeSurfer reads memory it never wrote or writes outside its mask: FS-167, FS-1124, FS-1125); `fix` 180 identical, 43 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mris_calc_golden.rs`: ten cases on 32^3 volumes and 1,000-value curvature files made with integer arithmetic (the rescale, a square root of negative values: SSE's default NaN, a volume over another with zeros, NaN operands added, `atan2`, `bcor`, the statistics, `norm` in a label, `ascii`, `andbw`), on every platform: `reproduce` `b458bcb2273de933e2d984c81896ec11a90b8f92d92d2d17145b0bc010f3c608`, the official program's value on the same inputs (`prog.py --golden`); the default `4c2c95918dc4f37bcf540273acd190fac4c1c56c71f6836ee8dea47ae4e6bd7f` (the correctly rounded `atan2f`, `pow` and `log10`, GLIBC-07; FS-17's `andbw`).

## Memory

`tests/parity/test_memory_peak.py::test_mris_calc_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- nu-rescale (reproduce): 197 MiB, FreeSurfer 198 MiB.
- nu-rescale (fix): 197 MiB, FreeSurfer 198 MiB.
- volume-pair (reproduce): 213 MiB, FreeSurfer 214 MiB.
- volume-pair (fix): 213 MiB, FreeSurfer 214 MiB.
- vertex-areas (reproduce): 6 MiB, FreeSurfer 7 MiB.
- vertex-areas (fix): 6 MiB, FreeSurfer 7 MiB.
