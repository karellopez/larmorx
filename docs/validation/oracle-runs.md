# The fMRIPrep oracle runs

fMRIPrep 25.2.6 run on OpenNeuro datasets through Apptainer, as the user (`scripts/run_fmriprep_oracle.sh`).
Their outputs and working directories (`oracles/fmriprep-25.2.6/<run>/`) are the reference for the stage-by-stage
comparisons and the real-data cases of the parity suites; they are not committed (image data). Each run's
`RUN_INFO.txt` is copied here verbatim (machine paths shown as `<workspace>` and `~`): the image digest, the
arguments, seeds and threads. The data panel's nine runs read BIDS views of hard links (below). The FreeSurfer licence file is mounted read-only, never copied.

## ds000005-sub-01-noreconall

```text
fMRIPrep oracle run: ds000005-sub-01-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds000005
participant: 01
arguments: /data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 8 --omp-nthreads 8
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-11T08:26:05Z
finished: 2026-10-11T09:28:45Z
exit status: 0
note: the machine was shared with a larmorx development agent (builds and parity runs) during this run, so its runtime is not a clean benchmark; outputs are unaffected.
```

## ds001600-sub-1-noreconall

```text
fMRIPrep oracle run: ds001600-sub-1-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds001600
participant: 1
arguments: /data /out participant --participant-label 1 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 8 --omp-nthreads 8
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-11T15:07:34Z
finished: 2026-10-11T16:22:44Z
exit status: 0
```

## ds005040-sub-002-noreconall

```text
fMRIPrep oracle run: ds005040-sub-002-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds005040
participant: 002
arguments: /data /out participant --participant-label 002 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 8 --omp-nthreads 8
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-11T18:43:15Z
finished: 2026-10-11T19:41:07Z
exit status: 0
```

## ds005454-sub-16-noreconall

```text
fMRIPrep oracle run: ds005454-sub-16-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds005454
participant: 16
arguments: /data /out participant --participant-label 16 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 8 --omp-nthreads 8
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-11T19:41:32Z
finished: 2026-10-11T21:06:10Z
exit status: 0
```

## ds005752-sub-ON01016-noreconall

```text
fMRIPrep oracle run: ds005752-sub-ON01016-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds005752
participant: ON01016
arguments: /data /out participant --participant-label ON01016 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 8 --omp-nthreads 8
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-11T21:06:49Z
finished: 2026-10-11T22:01:46Z
exit status: 0
```

## The data panel's nine runs (2026-10-24 and 25)

The data panel (larmorx-testdata `docs/panel.md`) added nine datasets. Their runs read a **BIDS
view** each (`<workspace>/oracles/bids-views/<dataset>`): the panel's files hard-linked from the
test-data cache, so fMRIPrep saw exactly the files the catalog lists and verified
(`validation/investigations/panel-oracle-runs/`: `bids_view.py`, `run_panel_oracles.sh`,
`verify_runs.py`). They ran one after the other at `--nprocs 6 --omp-nthreads 6` (the five
earlier runs used 8) on a machine shared with three development agents, so their times are not
benchmarks; outputs are unaffected. Every run ended with exit status 0, fMRIPrep's end message,
no crash file and every expected output (`verify_runs.py`); every program node of every run is
classified by the pipeline-stage catalogue (`unclassified.py`: 0 left over the fourteen runs).
**ds004884's lesion mask is a derived input** (accepted by the orchestrator, 2026-10-24): the
dataset's manual mask, drawn on the T2w and stored on its grid, resampled onto the T1w's grid
by nearest neighbour through both files' scanner coordinates, no registration, by
`bids_view.py` (`mask_on_grid`), because fMRIPrep reads a lesion mask only in the T1w's space;
the test-data catalog keeps the original. The template registration read it (its node's
`lesion_mask` input). After each run the runner appended the BIDS view and its own commit to
`RUN_INFO.txt` (the lines from `BIDS view:` on), and for ds004884 the derived input.

| Run | What it adds | Minutes | BOLD series | Field maps applied |
|---|---|---:|---:|---|
| `ds000228-sub-pixar008-noreconall` | a 3.5-year-old: small head, oblique field of view | 66 | 1 | none (none acquired) |
| `ds005003-sub-RS001-noreconall` | a 70 mL frontal oligodendroglioma | 78 | 2 | none (none acquired) |
| `ds004173-sub-603309-noreconall` | one MPRAGE, strong instructed head motion (`--anat-only`) | 69 | 0 | (anatomical only) |
| `ds002422-sub-02-noreconall` | 1.5 T, an older adult, float32 T1w | 95 | 3 | two estimators (`wf_auto_00000`, `wf_auto_00001`), 3 runs unwarped |
| `ds000256-sub-CTS203-noreconall` | a 7.5-year-old's three rest runs of graded head motion | 70 | 3 | none (none acquired) |
| `ds004884-sub-M2079-noreconall` | a chronic stroke with its lesion mask (the derived input below) | 73 | 2 | none (none acquired) |
| `ds003592-sub-280-noreconall` | an 82-year-old, three-echo BOLD (the first multi-echo run: `t2smap`, the T2* maps) | 84 | 1 (3 echoes) | none (none acquired) |
| `ds002603-sub-01-noreconall` | Philips, single-band and multiband-4 of the same task | 90 | 2 | one estimator, 2 runs unwarped |
| `ds004958-sub-fsm64ps-noreconall` | multiband-8, 600 volumes, an older adult | 100 | 1 | none: its spin-echo pair and GRE phase difference are not linked (no `IntendedFor`, no `B0FieldIdentifier`), as the panel intends |

### ds000228-sub-pixar008-noreconall

```text
fMRIPrep oracle run: ds000228-sub-pixar008-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000228
participant: pixar008
arguments: /data /out participant --participant-label pixar008 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T19:21:12Z
finished: 2026-10-24T20:27:14Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000228, the panel's files of ds000228 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds005003-sub-RS001-noreconall

```text
fMRIPrep oracle run: ds005003-sub-RS001-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds005003
participant: RS001
arguments: /data /out participant --participant-label RS001 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T20:28:02Z
finished: 2026-10-24T21:46:08Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds005003, the panel's files of ds005003 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004173-sub-603309-noreconall

```text
fMRIPrep oracle run: ds004173-sub-603309-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004173
participant: 603309
arguments: /data /out participant --participant-label 603309 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall --anat-only
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T21:46:35Z
finished: 2026-10-24T22:55:19Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004173, the panel's files of ds004173 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds002422-sub-02-noreconall

```text
fMRIPrep oracle run: ds002422-sub-02-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds002422
participant: 02
arguments: /data /out participant --participant-label 02 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T22:56:07Z
finished: 2026-10-25T00:31:06Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds002422, the panel's files of ds002422 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds000256-sub-CTS203-noreconall

```text
fMRIPrep oracle run: ds000256-sub-CTS203-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000256
participant: CTS203
arguments: /data /out participant --participant-label CTS203 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T00:31:33Z
finished: 2026-10-25T01:41:19Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000256, the panel's files of ds000256 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004884-sub-M2079-noreconall

```text
fMRIPrep oracle run: ds004884-sub-M2079-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004884
participant: M2079
arguments: /data /out participant --participant-label M2079 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T01:41:52Z
finished: 2026-10-25T02:55:12Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004884, the panel's files of ds004884 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
derived input: sub-M2079/ses-217/anat/sub-M2079_ses-217_label-lesion_roi.nii.gz, sha256
  02440da7ebdb5625ce344e4997bd52545a656dc81b544c78c14514574cf962d0,
  the dataset's manual lesion mask (derivatives/lesion_masks/sub-M2079/ses-217/anat/
  sub-M2079_ses-217_acq-spc3_run-4_T2w_desc-lesion_mask.nii.gz, drawn on the T2w's grid)
  resampled onto the T1w's grid (sub-M2079_ses-217_acq-tfl3p2_run-3_T1w) by nearest
  neighbour through both files' scanner coordinates, no registration, by bids_view.py's
  mask_on_grid; the test-data catalog keeps the original mask unchanged
```

### ds003592-sub-280-noreconall

```text
fMRIPrep oracle run: ds003592-sub-280-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds003592
participant: 280
arguments: /data /out participant --participant-label 280 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T02:55:51Z
finished: 2026-10-25T04:19:24Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds003592, the panel's files of ds003592 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds002603-sub-01-noreconall

```text
fMRIPrep oracle run: ds002603-sub-01-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds002603
participant: 01
arguments: /data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T04:20:01Z
finished: 2026-10-25T05:49:29Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds002603, the panel's files of ds002603 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004958-sub-fsm64ps-noreconall

```text
fMRIPrep oracle run: ds004958-sub-fsm64ps-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004958
participant: fsm64ps
arguments: /data /out participant --participant-label fsm64ps -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T05:50:14Z
finished: 2026-10-25T07:30:11Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004958, the panel's files of ds004958 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```
