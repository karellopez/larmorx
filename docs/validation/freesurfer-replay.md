# FreeSurfer 8.2.0's recorded steps, replayed: the proof of the replay tool

**What this shows.** FreeSurfer 8.2.0's reference runs record every step `recon-all` makes
(`validation/investigations/freesurfer-oracle/`). The replay tool (`larmorx_validation.freesurfer`,
`python -m larmorx_validation freesurfer-replay`) rebuilds a subject directory as it was before
one step, runs a program there and compares its files with the recorded ones. Before it can judge a
port, it must judge FreeSurfer itself: **FreeSurfer's own step, replayed at the recording's
settings, must give the recorded files** (the same result; FreeSurfer's provenance aside), or the
difference must be measured and explained. This page is that check.

Made 2026-11-12 on the main machine (AMD Ryzen 9 7945HX, Ubuntu 22.04), FreeSurfer 8.2.0
(`freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45`, the package's SHA-256 `0c17edb0...1013`),
the same machine and installation that made the recordings; each replay in a 12 GiB memory box,
4 threads as the recordings (`--threads recorded`), three steps also at one thread (the tool's
default), every run of FreeSurfer through the oracle-output cache. The tool was written without
FreeSurfer's source: what it treats as provenance was found in the files themselves
(`larmorx_validation/freesurfer/files.py` lists it).

## How a file is judged

| Verdict | Meaning |
|---|---|
| `identical` | the same bytes (a `.gz` stream decompressed) |
| `provenance` | the same result: only FreeSurfer's provenance differs (the command-line records with their time, user and machine; a surface's "created by" stamp; the run's paths; the provenance lines of LTA and statistics files; temporary names; in logs, times, dates, memory and load figures) |
| `log` | a log whose text still differs after those masks (a message, not a result) |
| `varies` | differs where FreeSurfer itself varies from run to run, measured by replaying the step twice |
| `differs` | anything else, with what differs measured (voxels, vertices, values, labels, lines) |

A step is **reproduced** when every file is `identical` or `provenance` (or a `log`), and its exit
status is the recorded one.

## The result

<!-- proof_table.py: the tables below are generated; the prose around them is written by hand -->

The proof run (batch B, the tool as merged): 68 steps of three recorded runs, both streams, four threads as recorded and three steps at one thread. Made with `proof_table.py` from the replay reports (`python -m larmorx_validation freesurfer-replay run ... --json`).

- **Steps replayed:** 68 (49 programs, 3 runs): 66 reproduced, 1 reproduced where the original is deterministic, 1 reproduced; a log differs
- **Files compared:** 120

| Kind | identical | provenance | log | varies | differs | missing | extra |
|---|---:|---:|---:|---:|---:|---:|---:|
| annot | 4 |  |  |  |  |  |  |
| binary |  | 2 |  |  |  |  |  |
| ctab | 1 |  |  |  |  |  |  |
| curv | 17 |  |  |  |  |  |  |
| dat | 2 |  |  |  |  |  |  |
| label | 5 |  |  |  |  |  |  |
| log | 4 | 10 | 1 |  |  |  |  |
| lta | 4 | 6 |  |  |  |  |  |
| m3z | 1 |  |  |  |  |  |  |
| mgh | 9 | 27 |  | 1 |  |  |  |
| stats | 2 | 4 |  |  |  |  |  |
| surface |  | 13 |  |  |  |  |  |
| text | 5 |  |  |  |  |  |  |
| xfm | 2 |  |  |  |  |  |  |

| Run | Step | Threads | Verdict | Files | Seconds (recorded, replayed) |
|---|---|---:|---|---|---|
| ds000005-sub-01-v8 | 0001-mri_convert | 4 | reproduced | 1 provenance | 0.8, 0.8 |
| ds000005-sub-01-v8 | 0002-mri_convert | 4 | reproduced | 1 provenance | 1.4, 1.3 |
| ds000005-sub-01-v8 | 0003-mri_add_xform_to_header | 4 | reproduced | 1 provenance | 0.6, 0.5 |
| ds000005-sub-01-v8 | 0004-lta_convert | 4 | reproduced | 1 provenance | 0.1, 0.1 |
| ds000005-sub-01-v8 | 0005-mri_synthstrip | 4 | reproduced | 1 identical | 15.6, 11.5 |
| ds000005-sub-01-v8 | 0007-rca-talairach | 4 | reproduced | 5 identical, 2 provenance | 19.8, 14.4 |
| ds000005-sub-01-v8 | 0009-mri_nu_correct.mni | 4 | reproduced | 2 provenance | 147.7, 133.7 |
| ds000005-sub-01-v8 | 0011-mri_normalize | 4 | reproduced | 1 provenance | 54.2, 51.9 |
| ds000005-sub-01-v8 | 0012-mri_em_register | 4 | reproduced | 1 identical, 1 provenance | 174.3, 110.9 |
| ds000005-sub-01-v8 | 0013-mri_ca_normalize | 4 | reproduced | 2 provenance | 38.2, 33.9 |
| ds000005-sub-01-v8 | 0014-seg2cc | 4 | reproduced | 1 identical, 2 provenance | 17.9, 16.4 |
| ds000005-sub-01-v8 | 0016-mri_mask | 4 | reproduced | 1 provenance | 0.5, 0.5 |
| ds000005-sub-01-v8 | 0019-mri_edit_wm_with_aseg | 4 | reproduced | 1 provenance | 0.8, 0.5 |
| ds000005-sub-01-v8 | 0021-AntsDenoiseImageFs | 4 | reproduced | 1 provenance | 21.3, 20.3 |
| ds000005-sub-01-v8 | 0022-mri_segment | 4 | reproduced | 1 identical, 1 provenance | 36.2, 34.0 |
| ds000005-sub-01-v8 | 0024-mri_pretess | 4 | reproduced | 1 provenance | 1.7, 1.6 |
| ds000005-sub-01-v8 | 0027-mri_fill | 4 | reproduced | 1 identical, 1 provenance | 49.4, 32.1 |
| ds000005-sub-01-v8 | 0029-mri_tessellate | 4 | reproduced | 1 provenance | 0.6, 0.6 |
| ds000005-sub-01-v8 | 0030-mris_extract_main_component | 4 | reproduced | 1 provenance | 0.8, 0.4 |
| ds000005-sub-01-v8 | 0034-mris_smooth | 4 | reproduced | 1 provenance | 1.9, 1.4 |
| ds000005-sub-01-v8 | 0036-mris_inflate | 4 | reproduced | 1 provenance | 13.3, 8.3 |
| ds000005-sub-01-v8 | 0042-defect2seg | 4 | reproduced where the original is deterministic | 4 identical, 1 provenance, 1 varies | 22.9, 17.6 |
| ds000005-sub-01-v8 | 0045-mris_remove_intersection | 4 | reproduced | 1 provenance | 4.6, 3.6 |
| ds000005-sub-01-v8 | 0047-mris_autodet_gwstats | 4 | reproduced | 1 identical | 2.2, 1.9 |
| ds000005-sub-01-v8 | 0051-label-cortex | 4 | reproduced | 1 identical, 1 provenance | 15.2, 12.5 |
| ds000005-sub-01-v8 | 0052-mri_label2label | 4 | reproduced | 1 identical | 11.1, 9.1 |
| ds000005-sub-01-v8 | 0059-mris_curvature | 4 | reproduced | 2 identical | 1.4, 1.1 |
| ds000005-sub-01-v8 | 0066-mris_jacobian | 4 | reproduced | 1 identical | 0.9, 0.7 |
| ds000005-sub-01-v8 | 0068-mrisp_paint | 4 | reproduced | 1 identical | 0.8, 0.6 |
| ds000005-sub-01-v8 | 0070-mris_ca_label | 4 | reproduced | 1 identical | 8.8, 7.7 |
| ds000005-sub-01-v8 | 0081-vertexvol | 4 | reproduced | 2 identical | 1.0, 1.2 |
| ds000005-sub-01-v8 | 0088-mris_curvature_stats | 4 | reproduced | 9 identical | 1.3, 1.5 |
| ds000005-sub-01-v8 | 0095-pctsurfcon | 4 | reproduced | 1 identical, 2 provenance | 1.7, 2.0 |
| ds000005-sub-01-v8 | 0097-mri_relabel_hypointensities | 4 | reproduced | 1 identical | 8.8, 8.8 |
| ds000005-sub-01-v8 | 0098-mri_surf2volseg | 4 | reproduced | 1 identical | 3.2, 2.9 |
| ds000005-sub-01-v8 | 0099-mri_brainvol_stats | 4 | reproduced | 1 identical | 2.1, 1.8 |
| ds000005-sub-01-v8 | 0105-mris_anatomical_stats | 4 | reproduced | 1 identical, 1 provenance | 1.6, 1.5 |
| ds000005-sub-01-v8 | 0136-mris_label2annot | 4 | reproduced | 1 identical | 0.5, 0.4 |
| ds000005-sub-01-classic | 0005-mri_nu_correct.mni | 4 | reproduced; a log differs | 1 identical, 1 log | 130.3, 122.0 |
| ds000005-sub-01-classic | 0006-talairach_avi | 4 | reproduced | 2 identical, 2 provenance | 21.1, 14.0 |
| ds000005-sub-01-classic | 0007-talairach_afd | 4 | reproduced |  | 0.1, 0.0 |
| ds000005-sub-01-classic | 0010-mri_normalize | 4 | reproduced | 1 provenance | 61.0, 49.7 |
| ds000005-sub-01-classic | 0011-mri_em_register | 4 | reproduced | 1 identical, 1 provenance | 157.6, 118.6 |
| ds000005-sub-01-classic | 0012-mri_watershed | 4 | reproduced | 1 provenance | 10.3, 9.3 |
| ds000005-sub-01-classic | 0014-mri_ca_normalize | 4 | reproduced | 2 provenance | 36.1, 33.5 |
| ds000005-sub-01-classic | 0016-mri_ca_label | 4 | reproduced | 1 identical, 1 provenance | 1121.7, 1037.5 |
| ds000005-sub-01-classic | 0017-seg2cc | 4 | reproduced | 3 provenance | 18.5, 17.6 |
| ds000005-sub-01-classic | 0022-mri_edit_wm_with_aseg | 4 | reproduced | 1 provenance | 15.6, 15.4 |
| ds000005-sub-01-classic | 0023-mri_pretess | 4 | reproduced | 1 provenance | 1.4, 1.3 |
| ds000005-sub-01-classic | 0024-mri_fill | 4 | reproduced | 1 identical, 1 provenance | 35.4, 33.1 |
| ds000005-sub-01-classic | 0046-mris_place_surface | 4 | reproduced | 1 provenance | 184.9, 135.0 |
| ds000005-sub-01-classic | 0067-mris_ca_label | 4 | reproduced | 1 identical | 11.8, 6.3 |
| ds000005-sub-01-classic | 0101-mri_segstats | 4 | reproduced | 1 provenance | 72.6, 56.0 |
| ds000005-sub-01-v8 | 0038-mris_sphere | 4 | reproduced | 1 provenance | 88.3, 51.0 |
| ds000005-sub-01-v8 | 0040-mris_fix_topology | 4 | reproduced | 3 identical, 1 provenance | 301.7, 266.0 |
| ds000005-sub-01-v8 | 0043-mris_remesh | 4 | reproduced | 1 provenance | 42.3, 19.7 |
| ds000005-sub-01-v8 | 0049-mris_place_surface | 4 | reproduced | 2 provenance | 178.2, 93.1 |
| ds000005-sub-01-v8 | 0090-mris_volmask | 4 | reproduced | 3 identical | 96.9, 70.2 |
| ds000005-sub-01-v8 | 0104-mri_segstats | 4 | reproduced | 1 provenance | 60.5, 49.0 |
| ds000005-sub-01-v8 | 0065-rca-surfreg | 4 | reproduced | 5 provenance | 502.1, 236.4 |
| ds002422-sub-02-v8 | 0011-mri_normalize | 4 | reproduced | 1 provenance | 67.9, 59.9 |
| ds002422-sub-02-v8 | 0029-mri_tessellate | 4 | reproduced | 1 provenance | 0.7, 0.6 |
| ds002422-sub-02-v8 | 0052-mri_label2label | 4 | reproduced | 1 identical | 12.5, 7.3 |
| ds002422-sub-02-v8 | 0070-mris_ca_label | 4 | reproduced | 1 identical | 8.9, 6.0 |
| ds000005-sub-01-v8 | 0011-mri_normalize | 1 | reproduced | 1 provenance | 54.2, 56.2 |
| ds000005-sub-01-v8 | 0021-AntsDenoiseImageFs | 1 | reproduced | 1 provenance | 21.3, 19.5 |
| ds000005-sub-01-v8 | 0024-mri_pretess | 1 | reproduced | 1 provenance | 1.7, 1.7 |
| ds000005-sub-01-classic | 0015-mri_ca_register | 4 | reproduced | 2 identical | 3659.7, 2157.9 |

| Step | File | Verdict | Detail |
|---|---|---|---|
| ds000005-sub-01-v8 0042-defect2seg | `mri/surface.defects.mgz` | varies | the colour table embedded in surface.defects.mgz gives each defect cluster a colour that changes from run to run (two replays differ from each other as from the recording; the voxels are identical); record 4 (number 1 and what follows, 72717 bytes: 'none unknown cluster1 cluster2 i cluster3 m cluste |
| ds000005-sub-01-classic 0005-mri_nu_correct.mni | `mri/mri_nu_correct.mni.log` | log | 1 of 34 lines differ, numbers by at most 5.5994e+08; the first, line 20: 'valid   : 589411376' vs 'valid   : 1149350928' |

## What the first pass found, and what changed in the tool

The first pass (34 steps of the v8 stream, the tool as first written) reproduced 29 steps; the
five others were the comparison's or the replay's own faults, each fixed and then confirmed by
running the step again:

| What the first pass showed | The cause | The fix |
|---|---|---|
| `mris_anatomical_stats`: `# generating_program /home/.../bin/mris_anatomical_stats` instead of `mris_anatomical_stats`; `mri_segment`'s volume: one more provenance run | the launcher ran the program by its full path, and programs write their `argv[0]` (recon-all's `fs_time` runs them by name, found on `PATH`) | the launcher runs the program under its bare name (`exec -a`) |
| `mris_remove_intersection`'s surface: the volume path of the recording where the replay's was expected | a surface keeps the volume path it copies from its input surface, which the recording wrote | both paths of a place are masked on both sides |
| `pctsurfcon`'s statistics: `# ColorTable /tmp/tmp.JBTcX9.ctab` | a colour table its script wrote under a `mktemp` name | temporary names masked |
| `label-cortex`, `rca-talairach`, `seg2cc`, `defect2seg` logs | process numbers in the scripts' temporary directories (`tmpdir.label-cortex.719800`), fs_time's load lines, run times printed with a decimal comma (the run's `LC_NUMERIC`), paths printed with `~` | masked in logs |
| `defect2seg`'s `surface.defects.mgz`: bytes differ in its embedded colour table, the voxels identical | FreeSurfer itself: two replays with the cache off differ from each other as from the recording; each defect cluster's colour changes from run to run | measured and reported as `varies`; such a step is never cached |

## Limits

- **The streams are not compared.** The recorder keeps no step's messages (they go to
  `recon-all.log`); a replay's messages are kept in the oracle cache's entry. Comparing them with
  the recorded log, step by step, is a follow-up.
- **A script is one step.** The programs a script runs (`rca-talairach`'s SynthMorph,
  `mri_nu_correct.mni`'s N4) are checked through the script's step; a port of one of them is
  checked by rebuilding before the script and running it with the arguments the script's log
  shows.
- **Memory.** `mri_synthseg` (about 23 GiB) and `fs-synthmorph-reg`'s deformable step (about
  18 GiB) do not fit this page's 12 GiB boxes and were not replayed here.
- **One machine.** The replays ran where the recordings ran; another CPU may compute differently
  (the cache's key holds the CPU).
