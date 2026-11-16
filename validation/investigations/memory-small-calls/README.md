# The peak of a small call through the Python entry point, from run to run

What it shows: why `tests/parity/test_memory_peak.py` measures a call whose first measurement is
within `RECHECK_MIB` (1 MiB) of its allowance, or above it, in five pairs (the entry point, then
the call) and keeps the pair of the median difference (`larmorx_peak`), and that this keeps the
user's rule as it is (larmorx's peak at most 1.10 times the original's plus the entry point's own
peak: CLAUDE.md, "Memory similar to the original's").

A call through `python -m larmorx` pays the interpreter's start-up and larmorx's Rust core
(`python -m larmorx --version`, about 15 MiB here). Both that peak and a small call's vary from
run to run by about 1 MiB in steps of 0.25 MiB, whatever the hash seed (`PYTHONHASHSEED=0`) or the
address-space layout (`setarch -R`): 30 runs each gave the same spread. For FLIRT's `extracttxt`
(FSL's peak 1.75 MiB, so 1.925 MiB allowed beyond the entry point) one pair in about 75 passed the
limit by a quarter of a MiB: the test failed about once in fifteen merges (2026-11-15 and 16)
without any change of larmorx.

`spread.py` runs the entry point and the call alternately, as the test does, and counts how often
the rule fails for blocks of 1, 3 and 5 consecutive pairs with three estimates of a block:

    scripts/orchestration/boxed.sh 8 python -P validation/investigations/memory-small-calls/spread.py \
        150 1.75 --flirt-env -- <venv>/bin/python -m larmorx fsl extracttxt value <text.txt> 3

(`PYTHONPATH=<worktree>/python:<worktree>/validation/src`, `FSLDIR` set, `LARMORX_IMPLEMENTATION=original`,
`LARMORX_UPSTREAM_DEFECTS=reproduce`; `<text.txt>` the test's twelve lines `line <k> value <3k>`.)

Results of 2026-11-16 (the main machine, under the load of a probe at eight threads; log
`<workspace>/oracles/logs/fs-found-c-r36-mem-spread-flirtenv.log`), 150 pairs:

| | values (MiB) |
|---|---|
| the entry point | 14.50 x1, 14.75 x8, 15.00 x42, 15.25 x57, 15.50 x42 |
| the call | 15.50 x1, 16.00 x2, 16.25 x41, 16.50 x52, 16.75 x54 |
| their difference (allowed 1.925) | 0.50 x1, 0.75 x15, 1.00 x27, 1.25 x44, 1.50 x44, 1.75 x17, 2.00 x2 |

| blocks | the least of each side | the median of each side | the median of the differences |
|---|---|---|---|
| 150 of 1 pair | 2 fail | 2 fail | 2 fail |
| 50 of 3 pairs | 1 fails | 0 | 0 |
| 30 of 5 pairs | 0 | 0 | 0 |

The least peak of each side (tried first) does not help: the entry point's low values are more
frequent than the call's. The median of the pairs' differences does: with one pair in 75 above the
limit, three of five are about once in 40,000 measurements. A call whose typical peak is above the
limit is above it in the median too, so the rule is not loosened. In the default environment
(the whole `os.environ`, `spread.py` without `--flirt-env`, 80 pairs) both peaks were higher and
the typical difference 1.5 MiB: the test's own environment is the one that counts.

Why a band and not a size: five pairs for every original under 64 MiB (the first form) made the
memory file much slower (prelude's and BET's calls of 10 to 60 MiB, five times each in both
modes); a call far from its allowance (most of them: 2 MiB and more) is decided by one pair as
before, since a difference of 1 MiB is beyond the spread seen. Only a call within 1 MiB of its
allowance (`extracttxt`, always: its typical margin is about 0.7 MiB) or above it runs five pairs.

With the final rule (5a77c548): `extracttxt`'s test 20 times boxed, 20 passed (margins 0.32 to 1.43
MiB; `fs-found-c-r36-mem-extracttxt-20.log`); the whole memory file once, boxed, with `FSLDIR` set:
121 passed (`fs-found-c-r36-mem-whole.log`; 56 minutes while the Rust gates ran beside it at 12
jobs). Of its 214 measurements through the entry point only `extracttxt`'s two (one per mode) came
within 1 MiB of their allowance and ran five pairs; every other was at least 1.5 MiB from it and
ran one pair, as before.
