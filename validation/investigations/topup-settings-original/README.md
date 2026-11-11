<!-- SPDX-License-Identifier: Apache-2.0 -->
# topup-settings-original: larmorx's own topup settings against FSL's files

`topup --config=<name>` finds FSL's six shipped configurations by name (`b02b0`, `b02b0_1`,
`b02b0_2`, `b02b0_4`, `b02b0_7T`, `test_acqp`; `specs/topup.md` §2.5). The main package carries
its own settings of these six, a clean-room original written from the spec's tables by an
implementer who never opened FSL's files (the user's decision of 2026-11-09; `t2smap`'s
clean-room part, relay 1, 2026-11-11): `crates/larmorx-fsl/src/topup/settings/<name>.cnf`,
compiled into `larmorx fsl topup`, which looks for them after FSL's own places (as named, with
`.cnf`, `$FSLDIR/etc/flirtsch/`), FSLDIR unset included. The values are necessarily FSL's
(another number gives another field); this check proves it by their effect, every output bit for
bit, without anyone opening FSL's files: the programs read them.

`check.py` runs, on three pairs (a synthetic 40 × 40 × 24 pair with a known field and a small
movement of the second volume; fMRIPrep 25.2.6's topup inputs of ds005040 and ds006736, cropped
about their centre to sizes divisible by 4 so that `b02b0_4` applies; the inputs are made in
`<workspace>/oracles/inputs/topup-settings-original/`), for each of the six names:

| Check | Runs compared |
|---|---|
| `replica-fsl = replica-own` | larmorx-nc's program (the published 0.1.0rc1, a black box) with `--config=<name>`, which finds FSL's file of that name in its own copy (FSLDIR unset), against the same program with `--config=<larmorx's file>` |
| `original-own = original-file` | the clean-room original (`larmorx fsl topup`) with `--config=<name>` (its compiled-in settings, FSLDIR unset) against `--config=<larmorx's file>` |
| `original-own = original-fsl` | the clean-room original with `--config=<name>` and FSLDIR the folder where larmorx-nc installs FSL's files (computed by larmorx's own `larmorx.sdcflows.pepolar._nc_data_roots`, never printed), so it reads FSL's file as data: what changes when both exist |

Every pair must agree in exit status, standard output, standard error and every file written
(images by their decompressed bytes, the others by their bytes; the compressed bytes are
reported). The calls write every output topup has (`--out`, `--fout`, `--iout`, `--jacout`,
`--rbmout`, `--dfout`, `--logout`); `test_acqp`'s own `--fout=field_test --iout=iout_test` are
left to the settings. The log (`--logout`) records every setting in force.

**Negative controls**: for each name and pair, the clean-room original with larmorx's file
changed in one option at its least decisive place (the last level of a per-level list; the last
level that estimates the movements; each single setting to its other value) must change the
outputs, so that the comparisons above see every value.

## How to run

The validation package's environment (`<worktree>/python` with its built `_core` on
`PYTHONPATH`), `LARMORX_NC_BIN` the replica program (here larmorx-nc 0.1.0rc1 from PyPI,
installed with `pip install --no-deps --prefix <dir>`), `LARMORX_BIN` a `larmorx` binary of this
tree (`cargo build --release -p larmorx-cli`), in a memory box:

```
scripts/orchestration/boxed.sh 24 python -P validation/investigations/topup-settings-original/check.py \
    --work <scratch>/topup-settings --jobs 8
```

It prints one line per comparison and per unseen control, writes `results.json` here and exits 1
when a comparison differs or a run fails.

## Result (2026-11-11, relay 1)

See the end of this file (filled in from `results.json`).
