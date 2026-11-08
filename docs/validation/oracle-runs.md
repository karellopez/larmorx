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
## The runs of 2026-11-08 (tedana's `t2smap` and the field maps' part B)

Made by the orchestrator, boxed (`scripts/orchestration/boxed.sh 24`), one after the other at `--nprocs 4 --omp-nthreads 4`
with `--fs-no-reconall`, on a machine shared with two development agents (times are not benchmarks). The two
multi-echo runs are for `t2smap`'s replica (its decision D8); the seven others reach the field-map paths no earlier run
reached (the field maps' part B, its decision D8 (a) to (c)). The views are written by
`validation/investigations/fmap-oracle-runs/derived_views.py`: hard links of the test-data cache, every changed sidecar a
new file with its SHA-256 and the original's in `RUN_INFO.txt`; no image data changed. Every run ended with exit status 0,
and every program node is classified by the pipeline-stage catalogue (the SyN nodes by a roadmap entry until the part
ports them).

| Run | What it adds | Minutes |
|---|---|---:|
| `ds000210-sub-02-noreconall` | three-echo BOLD (`t2smap`): the cued-SGT task and a rest run | 39 |
| `ds000258-sub-21262-noreconall` | three-echo BOLD (`t2smap`; echoes 12, 28, 44 ms) | 35 |
| `ds000228-sub-pixar008-noreconall-syn` | `--use-syn-sdc` on the panel's child (SyN field-map-less estimation) | 46 |
| `ds004884-sub-M2079-noreconall-syn` | `--use-syn-sdc` on the panel's stroke | 68 |
| `ds005003-sub-RS001-noreconall-syn` | `--use-syn-sdc` on the panel's glioma | 73 |
| `ds006736-sub-004-noreconall` | PEPOLAR: two opposite-direction EPIs (view: `IntendedFor` narrowed to the one cached BOLD) | 45 |
| `ds004958-sub-fsm64ps-noreconall-pepolar` | PEPOLAR: the spin-echo AP/PA pair linked to the rest BOLD (view) | 54 |
| `ds002603-sub-01-noreconall-epi` | PEPOLAR from EPI and BOLD: the phase difference unlinked (view), two estimators | 51 |
| `ds005752-sub-ON01016-noreconall-direct` | a direct field map in Hz linked to the two BOLD runs (view) | 39 |

### ds000210-sub-02-noreconall

```text
fMRIPrep oracle run: ds000210-sub-02-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000210
participant: 02
arguments: /data /out participant --participant-label 02 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T12:37:10Z
finished: 2026-11-08T13:16:32Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000210, the catalog collection openneuro-ds000210's cached files hard-linked (larmorx-testdata 52124bc; made by the orchestrator, 2026-11-08, for tedana's t2smap: its two 3-echo runs)
```

### ds000258-sub-21262-noreconall

```text
fMRIPrep oracle run: ds000258-sub-21262-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000258
participant: 21262
arguments: /data /out participant --participant-label 21262 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T13:16:59Z
finished: 2026-11-08T13:51:40Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000258, the catalog collection openneuro-ds000258's cached files hard-linked (larmorx-testdata 52e371a: echoes 1-3 added for this run; made by the orchestrator, 2026-11-08, for tedana's t2smap: one 4-echo run)
```

### ds000228-sub-pixar008-noreconall-syn

```text
fMRIPrep oracle run: ds000228-sub-pixar008-noreconall-syn
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000228
participant: pixar008
arguments: /data /out participant --participant-label pixar008 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall --use-syn-sdc
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T13:52:23Z
finished: 2026-11-08T14:37:54Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000228 (the data panel's view, as its noreconall run); --use-syn-sdc for sdcflows' SyN path (field maps part B, 2026-11-08)
```

### ds004884-sub-M2079-noreconall-syn

```text
fMRIPrep oracle run: ds004884-sub-M2079-noreconall-syn
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004884
participant: M2079
arguments: /data /out participant --participant-label M2079 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall --use-syn-sdc
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T14:38:05Z
finished: 2026-11-08T15:45:47Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004884 (the data panel's view, as its noreconall run); --use-syn-sdc for sdcflows' SyN path (field maps part B, 2026-11-08)
```

### ds005003-sub-RS001-noreconall-syn

```text
fMRIPrep oracle run: ds005003-sub-RS001-noreconall-syn
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds005003
participant: RS001
arguments: /data /out participant --participant-label RS001 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall --use-syn-sdc
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T15:46:00Z
finished: 2026-11-08T16:59:19Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds005003 (the data panel's view, as its noreconall run); --use-syn-sdc for sdcflows' SyN path (field maps part B, 2026-11-08)
```

### ds006736-sub-004-noreconall

```text
fMRIPrep oracle run: ds006736-sub-004-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds006736
participant: 004
arguments: /data /out participant --participant-label 004 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T17:01:31Z
finished: 2026-11-08T17:46:40Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds006736 (field maps part B, 2026-11-08)
BIDS view ds006736: the cached files of ds006736 (sub-004) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-004/fmap/sub-004_dir-AP_epi.json: original 073a417b7c5f1bd37552f144428ff212c84b6486219f6f70432bb5637707e32f, derived 08a27bd436206b9a7df8e503bfaf1aeca746ecd2c41129e8c74c955ba7908335, IntendedFor = ['func/sub-004_task-freeRecall_bold.nii.gz']
sub-004/fmap/sub-004_dir-PA_epi.json: original 6486ded73a7c5856a6cabe9e9e0b306119509ee2cb5de68294171325c9533557, derived ff1cbb027ce505476f51e55877a01e0e454a51646a75cb1d55cc7e2e6e054864, IntendedFor = ['func/sub-004_task-freeRecall_bold.nii.gz']
```

### ds004958-sub-fsm64ps-noreconall-pepolar

```text
fMRIPrep oracle run: ds004958-sub-fsm64ps-noreconall-pepolar
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004958-pepolar
participant: fsm64ps
arguments: /data /out participant --participant-label fsm64ps -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T17:46:54Z
finished: 2026-11-08T18:40:50Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004958-pepolar (field maps part B, 2026-11-08)
BIDS view ds004958-pepolar: the cached files of ds004958 (sub-fsm64ps) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-fsm64ps/fmap/sub-fsm64ps_acq-se_dir-AP_epi.json: original 8f2e0f71e4a28022732d3781da1b74c6ac2143b37713be5825a054eebe624405, derived 1a326fc2f87ff6825e218a1e2f8df88adfba1861e84f16aff4c46285e82b9579, IntendedFor = ['func/sub-fsm64ps_task-rest_bold.nii.gz']
sub-fsm64ps/fmap/sub-fsm64ps_acq-se_dir-PA_epi.json: original ad6fbf2230a58b40003559e17332c29d478a62580efe3a8562580f6a6957594a, derived 51cc4219571aa133b431f0266a7e224789a1bcb9f9ef945bfc627ccfa79a0302, IntendedFor = ['func/sub-fsm64ps_task-rest_bold.nii.gz']
```

### ds002603-sub-01-noreconall-epi

```text
fMRIPrep oracle run: ds002603-sub-01-noreconall-epi
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds002603-epi
participant: 01
arguments: /data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T18:41:03Z
finished: 2026-11-08T19:32:06Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds002603-epi (field maps part B, 2026-11-08)
BIDS view ds002603-epi: the cached files of ds002603 (sub-01) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-01/fmap/sub-01_acq-Mm2_phasediff.json: original 293096011449029bab0d56f4db67c4f0d6ff8a5d1c4651bc8d774ded4a11aba6, derived c5e7451597f19b03c78c12856a722440014599713d85e1339c6cb2a8d93cd1c8, IntendedFor removed
```

### ds005752-sub-ON01016-noreconall-direct

```text
fMRIPrep oracle run: ds005752-sub-ON01016-noreconall-direct
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds005752-direct
participant: ON01016
arguments: /data /out participant --participant-label ON01016 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 4 --omp-nthreads 4 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 4 --omp-nthreads 4
host: Linux 6.8.0-124-generic x86_64, 32 CPUs, 124 GB RAM
started: 2026-11-08T19:32:17Z
finished: 2026-11-08T20:11:16Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds005752-direct (field maps part B, 2026-11-08)
BIDS view ds005752-direct: the cached files of ds005752 (sub-ON01016) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_fieldmap.json: original 783eda358368a9063ac9e017412fbad5c5ced4961b5ad0fef94c6bac03d1aa01, derived 69d15efd91f16c8855792cd282f2079eb7d57a03d67925610d300b6dafc72b74, IntendedFor = ['ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_bold.nii.gz', 'ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_bold.nii.gz']
```

## sdcflows 2.17.1's own runs (2026-11-08)

sdcflows' BIDS app, the newest release, on eight field-map datasets (the field maps' part B, D8 (d)), in
`<workspace>/oracles/python-sdcflows-2.17` (the newest environment's packages with niworkflows 1.15.4), with the
workspace's oracle builds (ANTs 2.6.5, AFNI 26.3.05, FSL 6.0.7.23), boxed, 4 threads, `--no-fmapless`
(`validation/investigations/fmap-oracle-runs/run_sdcflows_oracle.sh`; outputs `<workspace>/oracles/sdcflows-2.17.1/<run>/`).
The app estimates every field map it finds, linked to a BOLD run or not. Every run ended with exit status 0 and no
crash file.

| Run | Estimators | Minutes |
|---|---|---:|
| `ds005040-sub-002` | 1 PEPOLAR | 2 |
| `ds006736-sub-004` | 1 PEPOLAR | 2 |
| `ds004958-sub-fsm64ps-pepolar` | 1 phase difference, 1 PEPOLAR | 7 |
| `ds002603-sub-01` | 1 phase difference, 2 PEPOLAR | 3 |
| `ds001600-sub-1` | 3 phase difference, 1 PEPOLAR | 4 |
| `ds005454-sub-16` | 1 phase difference | 1 |
| `ds002422-sub-02` | 2 phase difference | 1 |
| `ds005752-sub-ON01016-direct` | 1 direct (Hz) | 1 |

### sdcflows ds005040-sub-002

```text
sdcflows oracle run: ds005040-sub-002
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds005040
participant: 002
arguments: ~/.cache/larmorx-testdata/data/openneuro/ds005040 <workspace>/oracles/sdcflows-2.17.1/ds005040-sub-002/out participant --participant-label 002 -w <workspace>/oracles/sdcflows-2.17.1/ds005040-sub-002/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:19:39Z
finished: 2026-11-08T20:21:52Z
exit status: 0
```

### sdcflows ds006736-sub-004

```text
sdcflows oracle run: ds006736-sub-004
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: <workspace>/oracles/bids-views/ds006736
participant: 004
arguments: <workspace>/oracles/bids-views/ds006736 <workspace>/oracles/sdcflows-2.17.1/ds006736-sub-004/out participant --participant-label 004 -w <workspace>/oracles/sdcflows-2.17.1/ds006736-sub-004/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:21:53Z
BIDS view: <workspace>/oracles/bids-views/ds006736
BIDS view ds006736: the cached files of ds006736 (sub-004) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-004/fmap/sub-004_dir-AP_epi.json: original 073a417b7c5f1bd37552f144428ff212c84b6486219f6f70432bb5637707e32f, derived 08a27bd436206b9a7df8e503bfaf1aeca746ecd2c41129e8c74c955ba7908335, IntendedFor = ['func/sub-004_task-freeRecall_bold.nii.gz']
sub-004/fmap/sub-004_dir-PA_epi.json: original 6486ded73a7c5856a6cabe9e9e0b306119509ee2cb5de68294171325c9533557, derived ff1cbb027ce505476f51e55877a01e0e454a51646a75cb1d55cc7e2e6e054864, IntendedFor = ['func/sub-004_task-freeRecall_bold.nii.gz']
finished: 2026-11-08T20:23:41Z
exit status: 0
```

### sdcflows ds004958-sub-fsm64ps-pepolar

```text
sdcflows oracle run: ds004958-sub-fsm64ps-pepolar
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: <workspace>/oracles/bids-views/ds004958-pepolar
participant: fsm64ps
arguments: <workspace>/oracles/bids-views/ds004958-pepolar <workspace>/oracles/sdcflows-2.17.1/ds004958-sub-fsm64ps-pepolar/out participant --participant-label fsm64ps -w <workspace>/oracles/sdcflows-2.17.1/ds004958-sub-fsm64ps-pepolar/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:23:42Z
BIDS view: <workspace>/oracles/bids-views/ds004958-pepolar
BIDS view ds004958-pepolar: the cached files of ds004958 (sub-fsm64ps) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-fsm64ps/fmap/sub-fsm64ps_acq-se_dir-AP_epi.json: original 8f2e0f71e4a28022732d3781da1b74c6ac2143b37713be5825a054eebe624405, derived 1a326fc2f87ff6825e218a1e2f8df88adfba1861e84f16aff4c46285e82b9579, IntendedFor = ['func/sub-fsm64ps_task-rest_bold.nii.gz']
sub-fsm64ps/fmap/sub-fsm64ps_acq-se_dir-PA_epi.json: original ad6fbf2230a58b40003559e17332c29d478a62580efe3a8562580f6a6957594a, derived 51cc4219571aa133b431f0266a7e224789a1bcb9f9ef945bfc627ccfa79a0302, IntendedFor = ['func/sub-fsm64ps_task-rest_bold.nii.gz']
finished: 2026-11-08T20:30:36Z
exit status: 0
```

### sdcflows ds002603-sub-01

```text
sdcflows oracle run: ds002603-sub-01
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: <workspace>/oracles/bids-views/ds002603
participant: 01
arguments: <workspace>/oracles/bids-views/ds002603 <workspace>/oracles/sdcflows-2.17.1/ds002603-sub-01/out participant --participant-label 01 -w <workspace>/oracles/sdcflows-2.17.1/ds002603-sub-01/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:30:36Z
finished: 2026-11-08T20:33:37Z
exit status: 0
```

### sdcflows ds001600-sub-1

```text
sdcflows oracle run: ds001600-sub-1
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds001600
participant: 1
arguments: ~/.cache/larmorx-testdata/data/openneuro/ds001600 <workspace>/oracles/sdcflows-2.17.1/ds001600-sub-1/out participant --participant-label 1 -w <workspace>/oracles/sdcflows-2.17.1/ds001600-sub-1/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:33:38Z
finished: 2026-11-08T20:37:55Z
exit status: 0
```

### sdcflows ds005454-sub-16

```text
sdcflows oracle run: ds005454-sub-16
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: ~/.cache/larmorx-testdata/data/openneuro/ds005454
participant: 16
arguments: ~/.cache/larmorx-testdata/data/openneuro/ds005454 <workspace>/oracles/sdcflows-2.17.1/ds005454-sub-16/out participant --participant-label 16 -w <workspace>/oracles/sdcflows-2.17.1/ds005454-sub-16/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:37:55Z
finished: 2026-11-08T20:39:00Z
exit status: 0
```

### sdcflows ds002422-sub-02

```text
sdcflows oracle run: ds002422-sub-02
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: <workspace>/oracles/bids-views/ds002422
participant: 02
arguments: <workspace>/oracles/bids-views/ds002422 <workspace>/oracles/sdcflows-2.17.1/ds002422-sub-02/out participant --participant-label 02 -w <workspace>/oracles/sdcflows-2.17.1/ds002422-sub-02/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:39:00Z
finished: 2026-11-08T20:39:47Z
exit status: 0
```

### sdcflows ds005752-sub-ON01016-direct

```text
sdcflows oracle run: ds005752-sub-ON01016-direct
sdcflows: SDCFlows 2.17.1
environment: <workspace>/oracles/python-sdcflows-2.17 (nibabel==5.4.2 nipype==1.12.0 niworkflows==1.15.4 numpy==2.5.4 scipy==1.18.1 )
BIDS input: <workspace>/oracles/bids-views/ds005752-direct
participant: ON01016
arguments: <workspace>/oracles/bids-views/ds005752-direct <workspace>/oracles/sdcflows-2.17.1/ds005752-sub-ON01016-direct/out participant --participant-label ON01016 -w <workspace>/oracles/sdcflows-2.17.1/ds005752-sub-ON01016-direct/work --nprocs 4 --omp-nthreads 4 --no-fmapless --notrack -v
threads: --nprocs 4 --omp-nthreads 4
programs:
  topup 0d8ce02a76b99a9d882723e0b6a619b9aa9ab27c786e24910f1c8421903a888a ~/fsl-6.0.7.23/bin/topup
  applytopup e9ba10fb0c311132eb0a05082b3352a072f796433774c0eeb2d5a0b4c6955a52 ~/fsl-6.0.7.23/bin/applytopup
  3dvolreg 95c56fe68dbe412317592d7625db4beacb817f47532a2a75cd19af40136573ab <workspace>/oracles/afni-26.3.05/bin/3dvolreg
  3dAutomask 659c987eb26c1a3857a7db0235a1e4b441d61c9ff58af760cc7392866d978e4d <workspace>/oracles/afni-26.3.05/bin/3dAutomask
  N4BiasFieldCorrection 30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8 <workspace>/oracles/ants-2.6.5/bin/N4BiasFieldCorrection
  antsRegistration 11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c <workspace>/oracles/ants-2.6.5/bin/antsRegistration
  antsApplyTransforms 562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86 <workspace>/oracles/ants-2.6.5/bin/antsApplyTransforms
  DenoiseImage a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28 <workspace>/oracles/ants-2.6.5/bin/DenoiseImage
  ImageMath 32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15 <workspace>/oracles/ants-2.6.5/bin/ImageMath
host: Linux 6.8.0-124-generic x86_64, 4 CPUs, 124 GB RAM
started: 2026-11-08T20:39:48Z
BIDS view: <workspace>/oracles/bids-views/ds005752-direct
BIDS view ds005752-direct: the cached files of ds005752 (sub-ON01016) hard-linked; derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):
sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_fieldmap.json: original 783eda358368a9063ac9e017412fbad5c5ced4961b5ad0fef94c6bac03d1aa01, derived 69d15efd91f16c8855792cd282f2079eb7d57a03d67925610d300b6dafc72b74, IntendedFor = ['ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_bold.nii.gz', 'ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_bold.nii.gz']
finished: 2026-11-08T20:40:27Z
exit status: 0
```

