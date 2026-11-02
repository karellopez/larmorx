# FAST's command line, probed (FSL 6.0.7.23)

`probe.sh <work dir>` runs FSL 6.0.7.23's `fast` (black box: FSL never on the `PATH`, `FSLDIR` its
installation) on its command-line paths: the usage and the option errors, the checks before any
image is read, missing and degenerate images, the option values without meaning (FSL-99), the
class counts 0 and below, a singular covariance (two identical channels), one class, verbose texts
and the priors' checks (FSL-100). For each call it prints the exit status, the head of standard
output and error, and the files written. Run it in a memory box:

    scripts/orchestration/boxed.sh 8 validation/probes/fsl-fast-cli/probe.sh <scratch>/probe

The inputs are the `fsl-fast-replica` suite's derived inputs (`oracles/inputs/fsl-fast-replica/`,
`python -m larmorx_validation parity fsl-fast-replica` makes them). What it showed (2026-11-02) is
in `specs/fast.md` §2.4, §2.5, §4.1, §4.6 and §9 (the facts marked observed) and in
`docs/findings/fsl-fast.md` (FSL-99, FSL-100).

The probe runs FSL's program directly, outside the oracle-output cache (its calls are a few
seconds each on small inputs and measure messages and exit statuses, not outputs); the suite's
error cases (`err/` in `fsl-fast-replica`) run the same paths through the cache and are the
checked record. Prefer them; rerun this probe only to see a new path by hand.
