# FLIRT's cost functions, bit for bit

What it shows: that `larmorx-nc-core`'s translation of a FLIRT cost function gives FSL 6.0.7.23's
own bits for the same volumes and matrices. It found the label-difference cost's last-bit
difference of 2026-11-01 (the C types of FSL's trilinear weights: `dx*dy` is a float product,
`docs/findings/fsl-flirt.md`, restricted).

`cost_probe.cc` calls one cost function of FSL's `libfsl-newimage` (`p_labeldiff`,
`p_labeldiff_smoothed`, `p_leastsquares_smoothed`) on a reference and a test volume, for matrices
read from standard input (16 numbers per line, row by row), and prints each cost as a C99
hexadecimal float. The replica's side is the ignored test `cost_probe` of
`crates-nc/larmorx-nc-flirt/tests/cost_probe.rs`, which prints the same lines from the core's
translation (`COST_PROBE_REF`, `COST_PROBE_TEST`, `COST_PROBE_COST`, `COST_PROBE_SMOOTH`,
`COST_PROBE_MATS`).

```bash
bash build.sh <scratch>/cost_probe
<scratch>/cost_probe ref.nii test.nii labeldiff < mats.txt > fsl.txt
cd crates-nc && COST_PROBE_REF=ref.nii COST_PROBE_TEST=test.nii COST_PROBE_COST=labeldiff \
    COST_PROBE_MATS=mats.txt cargo test -p larmorx-nc-flirt --test cost_probe -- --ignored \
    --nocapture > replica.txt
diff fsl.txt replica.txt
```
