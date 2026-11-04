# bet -S, -B, -A, -A2: FSL against the replica on one call

The probes behind `docs/findings/fsl-bet.md`, "`bet -S`, `-B`, `-A`, `-A2`" (2026-11-04): FSL
6.0.7.23's `bet` script and the replica (`larmorx-nc fsl bet`) on the same call, each in a
directory of its own, then every file and both streams compared.

- `bet.sh <tag> <input> <options...>`: runs `bet in out <options>` with FSL (`fsl/`) and the
  replica (`rep/`, or `$REPDIR`) under `runs/<tag>/` (`$PROBE_RUNS`), `PATH=/usr/bin:/bin`,
  `FSLDIR` set (`$LARMORX_FSL_DIR`, default `~/fsl-6.0.7.23`); `WHO=fsl|rep|both`,
  `MODE=reproduce|fix` (the replica's D13 mode), `T2=<image>` copied in as `t2.nii.gz` for the
  word `T2` among the options, `OT=` the `FSLOUTPUTTYPE`.
- `fsl_batch.sh`: the first seven calls (`-S -d`, `-S -m -v`, `-S -n`, `-B -d`, `-B -s -m`, `-A
  -d`, `-A2 <T2> -d` on ds000005's and ds000256's T1w, from the test-data cache), in parallel;
  run it inside `scripts/orchestration/boxed.sh` (FSL and the replica's FAST need about 2 GiB
  each).
- `cmp.py <tag>...`: the comparison (statuses, times, the messages with the scripts' paths read
  as their names and larmorx's own lines set aside, every file: images by data and header
  without `descrip`).
- `phantoms.py <dir> <name>...`: copies the BET suite's generated phantoms (`head-nan`,
  `head-large`, ...) for a probe (`PYTHONPATH=<checkout>/python:<checkout>/validation/src`).

Results of 2026-11-04 (`reproduce`): the seven calls identical to FSL (statuses, both streams,
every file); the default differs where FSL-09 and FSL-202 are named.
