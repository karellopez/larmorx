# FAST's clean-room original: development probes (relays 1 to 4, 2026-11-03)

The tools behind [`docs/findings/fsl-fast-original.md`](../../../docs/findings/fsl-fast-original.md)
and the original's suite (`validation/src/larmorx_validation/parity/fsl_fast.py`). They run FSL
6.0.7.23's `fast` as a black box (through the oracle cache in `use` mode, under the
`fsl-fast-replica` suite's keys; with correctly rounded maths through the CORE-MATH preload,
`oracles/core-math-preload/libcrmath.so`, whose content is part of the key) and the clean-room
original (`LARMORX_FAST_BIN`, a `larmorx` binary, else `python -m larmorx`); each inside
`scripts/orchestration/boxed.sh`, with `PYTHONPATH=python:validation/src`, run with `python -P`.
`@name` in a command line names a derived input of the replica suite
(`oracles/inputs/fsl-fast-replica/`).

| File | What it shows | How to run |
|---|---|---|
| `probe.py` | one suite case, any command line or fMRIPrep's call of an oracle run with FSL and the original into two kept directories: the streams and each image's agreement (Dice and identical voxels for the labels, mean and 99.9th-percentile differences for the rest) | `probe.py <dir> --case opt/type2-verbose`, `probe.py <dir> --args "-v -o seg @t1_half.nii.gz"`, `probe.py <dir> --fmriprep ds000005-sub-01-noreconall` (`--fsl-only`) |
| `fsl_streams.py` | FSL's exit status, standard output and error and files on several command lines (the findings' "The command line and the messages") | `fsl_streams.py err/zeros "args:-v -o seg @const.nii.gz" env:FSLOUTPUTTYPE=BOGUS "args:..."` |
| `band.py` | FSL against FSL with correctly rounded maths: §11's band of a last-bit change (the findings' "FSL's own last-bit band"); `--original` also compares the original with the latter | `band.py fmriprep/ds005003-sub-RS001-noreconall opt/2d [--original]` |
| `verbose_cr.py` | `-v`'s lines of FSL with correctly rounded maths against the original's (the findings' "The class variances", "MRFWeightsTotal") | `verbose_cr.py [--full] "-v -N -I 1 -O 0 -W 2 -o seg @t1_half.nii.gz"` (`--full`: both outputs whole) |
| `files_cr.py` | each image of the two runs above: identical voxels, largest relative difference (one bias field estimate: 99.95 % identical, 1 unit in the last place elsewhere) | `files_cr.py <dir> "-b -I 0 -O 0 -W 1 --nopve -o seg @t1_half.nii.gz"` |
| `conv_model.py` | the bias field's 5-tap convolution modelled in numpy from the original's `W` and `R` against FSL's `_bias` (with correctly rounded maths): only the normalised taps kept in double reproduce it (the findings' "The bias field's kernel") | apply `dump.patch` in a scratch copy, build, run `files_cr.py` with `LXFAST_DUMP=<dump>` on `"-l 5 -b -I 0 -O 0 -W 1 --nopve -o seg @t1_half.nii.gz"`, then `python -I conv_model.py <dump> <run>` |
| `dump.patch` | the development hook `conv_model.py` reads: `LXFAST_DUMP=<dir>` makes the original write the first bias estimate's `W`, `R` and x kernel (never committed into the crate) | `git apply validation/investigations/cleanroom-fast/dump.patch` in a scratch copy |
| `analyze_scale.py` | FSL's `flirt -applyxfm` of FSL's standard CSF prior (big-endian ANALYZE, scale field 1/255) against larmorx's FLIRT: FSL does not apply the scale field (the findings' "FSL's standard priors") | `analyze_scale.py` (`LARMORX_FAST_BIN` a `larmorx` binary) |
| `self_band.py` | the original against itself with one rounding change in the covariances (`LXM_COV`, `self_band.patch` applied): how far an ill-conditioned case moves (the findings' "Five channels") | apply `self_band.patch` in a scratch copy, build, `self_band.py multi/s5` |
| `self_band.patch` | the development switch `LXM_COV` (never committed into the crate) | `git apply validation/investigations/cleanroom-fast/self_band.patch` in a scratch copy |
| `segrule.py` | with `-p`, the voxels where the two runs' `_seg` differ, with their probabilities (near-ties of the posteriors) | `segrule.py "-p -o seg @t1_2d.nii.gz"` |
| `threads_exact.py` | the speed work (relay 3): two builds' outputs, standard output and error byte for byte, the reference build on 1 thread against the new one on each thread count, on one command per path the speed work touched (no FSL run) | `threads_exact.py --ref <old larmorx> --new target/release/larmorx --threads 1 3 14` |
| `bench.py` | the benchmark report `docs/benchmarks/fsl-fast.md`: fMRIPrep's call on the standard tier's three oracle runs, FSL (its time from the oracle cache's stored run: never run again), the replica (checked byte for byte against FSL) and the original on each thread count | `LARMORX_FAST_BIN=target/release/larmorx boxed.sh 8 python -P bench.py --threads 1 14 --json docs/benchmarks/fsl-fast.json` (`LARMORX_NC_BIN` the replica's binary) |
| `applyxfm_tr.py` | the TR FLIRT's `-applyxfm` writes (relay 3; FLIRT's findings, "`-applyxfm`'s TR"): FSL (through the oracle cache) and larmorx's FLIRT on inputs of each format, `dim[0]` and TR; prints each output's `pixdim[4..7]` | `LARMORX_FLIRT_BIN=target/release/larmorx boxed.sh 8 python -P applyxfm_tr.py` |
| `applyxfm_tr_inputs.py` | the images of FLIRT's `-applyxfm` TR probe (relay 3; FLIRT's findings, "`-applyxfm`'s TR"): a 3D and a 4D input with their own `pixdim[4]`, a reference with another, the identity | `python -I applyxfm_tr_inputs.py <dir>`, then `validation/investigations/cleanroom-flirt/probe.py <out> --args "-in @@<dir>/in.nii.gz -ref @@<dir>/ref.nii.gz -applyxfm -init @@<dir>/ident.mat -out out.nii.gz"` (`LARMORX_FLIRT_BIN` a `larmorx` binary) |
| `replay_findings.py` | relay 4: every finding of a random-testing trial replayed (`difftest --replay`, FSL's runs served by the oracle cache), at most 8 at a time, memory findings skipped; one line per finding (the findings' "Random testing" section) | `LARMORX_FAST_BIN=target/release/larmorx boxed.sh 24 python -P replay_findings.py <trial>/findings/fsl-fast-original [--only 161,441]` |
| `probe.py --finding`, `--diff` | relay 4: a finding's shrunk (or, with `--drawn`, drawn) recipe run by FSL (through the cache, its `never` policy kept) and the original into kept directories; `--diff` prints the streams' differences, each image's agreement shows the voxels that differ outside the mask | `probe.py <out> --finding <finding .json> --diff` |
| `glibc_default.py` | the clean-room catch-up (relay 2, 2026-11-05): the original's default with glibc's functions (`glibc_default.patch`, `LXM_GLIBC=1`) against the replica's default (`larmorx-nc`), exit status, standard output and every image's voxels bit for bit, on suite cases (no FSL run): where they agree, the default's corrections (FSL-202's variance, FSL-203's extrema, the rest) are the replica's exactly, and the two defaults differ only by the correctly rounded functions' last bits (the findings' "The catch-up of 2026-11-05") | apply `glibc_default.patch` in a scratch copy (or the worktree, reverted after), build, `LARMORX_FAST_BIN=<that larmorx> LARMORX_NC_BIN=<larmorx-nc> boxed.sh 24 python -P glibc_default.py "fmriprep/\|opt/\|priors/\|multi/" --jobs 8` |
| `trial_catchup_categories.py`, `trial_catchup_triage.json` | the clean-room catch-up (relay 9, 2026-11-05): every finding of the catch-up's trial (`oracles/difftest/2026-11-05-catchup-fast`, 50) in one category by its failed checks and relay 8's triage (the default against the replica's default only, 39 of them the replica's bit for bit with glibc's functions and 02117 not measured; a class covariance singular in exact arithmetic, FSL or the original aborting; several channels where the reproduce run parts, two with noise-free channels); the JSON is its output, which FAST's page reads for the trial's triage table (`evidence/fsl_fast.py`, `trial_catchup`) | `python -P trial_catchup_categories.py <workspace>/oracles/difftest/2026-11-05-catchup-fast --json trial_catchup_triage.json` |
| `glibc_default.patch` | the development switch `LXM_GLIBC` (never committed into the crate): the default's functions glibc's, as `reproduce`'s | `git apply validation/investigations/cleanroom-fast/glibc_default.patch` |
| `variance_model.py` | numpy models of the initialisation's class statistics against FSL's printed lines: the arithmetic of the variance (the findings' "The class variances") | `python -I variance_model.py <workspace>/oracles/inputs/fsl-fast-replica/t1_half.nii.gz` |

glibc's `rand` (the MRF's random posteriors, `specs/fast.md` §8.1) was checked through ctypes
(`srand(0xFFFFFFFF)`: the first values and the millionth; `srand(0)`); the values are in
`crates/larmorx-fsl/src/fast/rng.rs`'s test.

## `math::exp_in_place` against the scalar `exp` (relay 4, 2026-11-03)

The batch `exp` the PVE likelihoods use (`larmorx_core::math::exp_in_place`, relay 3: `exp`'s fast
path four values at a time, compiled with AVX2 and FMA where the CPU has them) must give the scalar
`exp`'s bits on every input, or Linux x64 (the AVX2 path) and the platforms without it (the baseline
path: aarch64, x86-64 CPUs without AVX2) part. The probe is the ignored test
`math::exp::tests::exp_in_place_randomized` in `crates/larmorx-core/src/math/exp.rs` (inside the
crate so that it reaches both builds of the batch, the fast path's lane test and the accurate path's
database):

    boxed.sh 4 cargo test --release -j 14 -p larmorx-core --lib exp_in_place_randomized -- --ignored --nocapture

(`LARMORX_EXP_CHECK_N` sets the count, 2^32 by default; `LARMORX_EXP_CHECK_THREADS` the threads, 14.)
Every value goes through the baseline batch and the dispatched batch, in buffers of 4,093 values (the
scalar tail runs too) where every kind below is mixed, so each lane meets every kind; each result is
compared bit for bit, NaN payloads included, with the scalar `exp` and the dispatched `math::exp`.

The run on the main machine (AMD, AVX-512; the dispatched batch on AVX2 and FMA), 20 s on 14 threads,
**4,294,967,296 values, 0 differences**:

| values | kind |
|---:|---|
| 536,814,387 | uniform bit patterns over all 2^64 (NaNs and infinities included) |
| 805,275,751 | uniform bit patterns over the finite doubles |
| 805,337,402 | uniform in [-746, 710] (the fast path's range and both thresholds beyond it) |
| 536,942,013 | uniform in [-60, 1] (the range FAST's likelihoods use) |
| 536,874,988 | every binade from 2^-64 to 2^10, both signs |
| 268,395,820 | within 2^20 ulps of each branch threshold, both signs: `abs(x) = 2^-54` (`1 + x`), overflow, the subnormal results, underflow, infinity |
| 268,429,078 | within 8 ulps of `(k + 1/2) / S` (the rounding of `x S` to the table's index), k over the whole range |
| 268,434,649 | within 2^12 ulps of each database input (the accurate path's hard cases) |
| 268,463,208 | subnormal inputs, ±0, ±inf, NaNs of random payloads, both signs, quiet and signalling |

Of the lanes of the 4-wide path, 2,408,075,977 kept the fast path's result and 17,222,731 were sent by
its rounding test to the scalar `exp` (its accurate path); the rest were outside the fast path's range.
