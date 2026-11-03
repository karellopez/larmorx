# FAST's clean-room original: development probes (relays 1 to 3, 2026-11-03)

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
| `applyxfm_tr_inputs.py` | the images of FLIRT's `-applyxfm` TR probe (relay 3; FLIRT's findings, "`-applyxfm`'s TR"): a 3D and a 4D input with their own `pixdim[4]`, a reference with another, the identity | `python -I applyxfm_tr_inputs.py <dir>`, then `validation/investigations/cleanroom-flirt/probe.py <out> --args "-in @@<dir>/in.nii.gz -ref @@<dir>/ref.nii.gz -applyxfm -init @@<dir>/ident.mat -out out.nii.gz"` (`LARMORX_FLIRT_BIN` a `larmorx` binary) |
| `variance_model.py` | numpy models of the initialisation's class statistics against FSL's printed lines: the arithmetic of the variance (the findings' "The class variances") | `python -I variance_model.py <workspace>/oracles/inputs/fsl-fast-replica/t1_half.nii.gz` |

glibc's `rand` (the MRF's random posteriors, `specs/fast.md` §8.1) was checked through ctypes
(`srand(0xFFFFFFFF)`: the first values and the millionth; `srand(0)`); the values are in
`crates/larmorx-fsl/src/fast/rng.rs`'s test.
