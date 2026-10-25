# The mcflirt replica: replays and the C library's maths

Tools behind the replica's findings (`docs/findings/fsl-mcflirt.md`, "The replica"). The
replica's record is made by the parity suite (`python -m larmorx_validation parity
fsl-mcflirt-replica`); these are the quick checks used while translating.

| Tool | What it shows | Run |
|---|---|---|
| `replay.py` | every file, exit status and message of recorded mcflirt runs (`oracles/fsl-6.0.7/mcflirt/runs/`) against the replica: `137/137 identical` (2026-10-24, glibc maths) | `python replay.py syn/ real/ [--mode fix] [--keep]` |
| `crm_compare.py` | a recorded case run again with FSL's mcflirt and the CORE-MATH preload (`oracles/core-math-preload`), compared with mcflirt as built and with a replay kept by `replay.py --keep`: with correctly rounded maths the replica gave the preloaded mcflirt's bytes in all 12 synthetic cases where it differed from mcflirt as built | `python crm_compare.py syn/motion/small/sinc_final` |
| `show_runs.py` | a recorded run's arguments, exit status and messages | `python show_runs.py syn/cli/dof5` |
| `crates-nc/larmorx-nc-core/tools/glibc-float-verify` (Rust, in the FSL Licence's workspace) | glibc 2.35's `sinf`, `cosf`, `sincosf`, `logf` and `atanf` against the replica's translation on all 2^32 inputs (0 differences), `atan2f` on 1,444 pairs of special values and 3 × 2^30 seeded random pairs (0 differences, since the `aT[0]` correction of 2026-10-25), and how often glibc is not correctly rounded at mcflirt's arguments (the sinc kernels: 24 `sinf` arguments, 20 Blackman samples) | `cd crates-nc && cargo run --release -p glibc-float-verify -- --threads 3` |
| `validation/probes/fsl-mcflirt-affine-trace` | LD_PRELOAD interposers on mcflirt's `decompose_aff` and `Costfn::cost`, a cost probe and a runner for fMRIPrep's calls: how the `atanf` constant behind two fMRIPrep calls' differences was found (its README) | `bash build.sh <dir>`, then `python -P run_call.py trace|compare ...` |

They need FSL 6.0.7.17 at `~/fsl` (never on the `PATH`; `crm_compare.py` runs it with a clean
environment), the recorded runs, the test data and a release build of `crates-nc`
(`larmorx-nc`). The workspace is found from this file's location or `LARMORX_WORKSPACE`.
