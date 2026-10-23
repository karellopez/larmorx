# The fMRIPrep oracle runs

fMRIPrep 25.2.6 run on OpenNeuro datasets through Apptainer, as the user (`scripts/run_fmriprep_oracle.sh`).
Their outputs and working directories (`oracles/fmriprep-25.2.6/<run>/`) are the reference for the stage-by-stage
comparisons and the real-data cases of the parity suites; they are not committed (image data). Each run's
`RUN_INFO.txt` is copied here verbatim (machine paths shown as `<workspace>` and `~`): the image digest, the
arguments, seeds and threads. The FreeSurfer licence file is mounted read-only, never copied.

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
