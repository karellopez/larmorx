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
