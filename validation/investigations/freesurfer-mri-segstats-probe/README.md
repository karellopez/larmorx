# freesurfer-mri-segstats-probe: `mri_segstats` as a black box (open)

An **open probe** for `mri_segstats`'s clean room (`specs/mri_segstats.md`): it runs FreeSurfer
8.2.0's official program and a candidate on the same command lines and compares what they give.
It holds no FreeSurfer code: the official program is run, never read. The behaviour it checks is
the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_segstats` of the FreeSurfer 8.2.0 installation
  in the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`),
  found on the `PATH` by its name as the scripts run it (its messages print that name), with
  `FREESURFER_HOME` and `FS_LICENSE` set (`larmorx_validation.freesurfer_unit.freesurfer_env`),
  `OMP_NUM_THREADS=1`, `TMPDIR` a folder of the case's own, through the oracle-output cache
  (`docs/validation/oracle-cache.md`; the synthetic subjects' files are keyed as inputs: the
  subject options read them without naming them).
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give)
  and then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-segstats-prog/<seed>/`: MGZ
segmentations of a head-like pattern with FreeSurfer's labels (`int`, `uchar`, and `float` with
fractional values), intensity volumes (`uchar`, `float`, three `float` frames), masks (one and two
frames), a segmentation of another size, a measure on six vertices; a small colour table;
replacement files (good, wrong format, comments only); a summary table to read back; a
`register.dat` and an `.lta` registration; a Talairach transform; and two subjects (`subj` with the
ribbon, the brain-volume cache, closed and open `?h.orig.nofix` surfaces, white surfaces, an
annotation, cortex labels, the contrast maps and the volumes `--qa-stats` reads; `bare` without
the ribbon). It runs 145 command lines: no arguments, `--help`, `--version`, `--all-info`, unknown
options (with one dash), options short of their arguments; every check; the ids (from the
segmentation, a colour table, `--id` with and without one, ids outside the table first and after
others, unreadable ids), `--empty`/`--nonempty`, exclusions (`--excludeid`, `--excl-ctxgmwm`); the
input's statistics (`--snr`, `--robust`, `--frame`, `--mul`/`--div`, `--abs`, `--sqr`, `--sqrt`,
`--acc`, the intensity names and units), masks (threshold, sign, inversion, erosion, frame, other
sizes), the partial-volume correction with and without the border; the waveforms (`--avgwf`,
`--sumwf`, `--sfavg`, `--avgwfvol`, the mean removed or scaled; unwritable files); `--ctab-out`;
`--seg-erode`, `--vox`, the replacements (`--replace`, `--replace-file`, the GTM default lists
with and without the choroid option); `--seg-from-input`; `--dontrun`; the old table
(`--no-newprint`); `--debug`; the subject options (`--etiv`, `--etiv-only`, `--old-etiv-only`,
`--talxfm`, `--sd`, `SUBJECTS_DIR` unset, the brain-volume measures, `--no-global-stats`,
`--euler` on a closed and an open surface and missing ones, `--stiv` as a number and a file);
`--annot` and `--slabel` (with a base, another surface, a threshold, a missing annotation); the
stand-alone options `--sum-in`, `--xfm2etiv`, `--rescale-by-seg`, `--qa-stats`; registrations
(`--reg` of both formats, `--regheader`); `--ctab-default`, `--ctab-gca` (FreeSurfer's
`average/face.gca`); `--seed`; missing files.

Compared for each case and mode: the exit status; standard output and standard error (the
case's own folder, which a relative name becomes in some messages, `--all-info`'s `TimeStamp` and
the candidate's `larmorx:` lines masked); the names of the files written and each file
(FreeSurfer's provenance, the command line, the user, the machine, dates and the temporary colour
table's name left out: `larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes
a defect of `NOT_COMPARABLE` (FreeSurfer reads memory it does not own or never wrote, writes past
its buffers, crashes or aborts: their texts are in `crates/larmorx-core/src/defects.rs`) is
counted, not compared.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 12 python -P \
        validation/investigations/freesurfer-mri-segstats-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_segstats' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (every case needs less than 300 MiB; `--ctab-gca` reads a 33 MB atlas).

## Results (observed, relay 49 of track D, 2026-11-22)

The replica as the candidate, seeds 1 and 2, 145 cases each: 138 identical in `reproduce`, 7
counted (FS-1103, FS-1104, FS-1111 twice, FS-1115, FS-1116, FS-469), 0 different; `fix` identical or
explained by a notice on every compared case.
