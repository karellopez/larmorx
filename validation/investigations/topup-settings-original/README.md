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

`lookup_probe.py <program> <work folder>` shows how a program finds a name (six `--config` words,
FSLDIR unset or an empty folder, both D13 modes, the input missing so that each run stops after
the lookup). 2026-11-11: larmorx-nc 0.1.0rc1 and the original give the same exit statuses and
lines in all 24 runs (a setting found by name with FSLDIR unset or empty; `b02b0_9` FSL-121 with
FSLDIR unset, `Cannot find config-file` with it set; `./b02b0` never looked for among the
settings), but for the wording and order of the FSL-121 line (both larmorx's own messages).

## Result (2026-11-11, relay 1; `results.json`)

larmorx-nc 0.1.0rc1 from PyPI, `larmorx` built from this branch's crates at ed1a5269 (the
settings and their lookup as committed), one thread per run, 8 runs at a time:

- **54 comparisons of 54 identical** (3 pairs × 6 names × 3 checks): in each, the exit status 0,
  standard output and error, and all 12 outputs (`tu_fieldcoef`, `tu_movpar.txt`, the field, the
  corrected images and their mean, both Jacobians, both displacement fields, both matrices, the
  log; `test_acqp`'s `field_test`, `iout_test`, `iout_test_mean`) identical, the compressed bytes
  too. So FSL's topup (its bit-exact replica) gives the same results with larmorx's settings as
  with FSL's files, on real fMRIPrep inputs and on a pair whose movement the first levels
  estimate, and the clean-room original gives the same results from its compiled-in settings, from
  their files and from FSL's files (what changes when both exist: nothing).
- **234 negative controls of 234 change the outputs** (6 names × 13 options × 3 pairs): 233 change
  the images or matrices, one only the log (`test_acqp`'s last knot spacing 14 → 16 mm on the
  synthetic pair's 2.5 mm voxels, where both round to the same knot spacing in voxels; the real
  pairs' images change). Every value of every setting is thus seen by the comparisons.
- Times (one thread): the replica 33 to 89 s per real-data run of the nine- and twelve-level
  settings, the clean-room original 5 to 14 s.
