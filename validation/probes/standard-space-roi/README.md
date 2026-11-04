# standard_space_roi and fslroi probes (2026-11-04)

The black-box runs behind `docs/findings/fsl-flirt.md`, "standard_space_roi and fslroi":

- `quick.sh <tag> [options]`: FSL 6.0.7.23's `standard_space_roi` and the replica
  (`LARMORX_UPSTREAM_DEFECTS=reproduce`) on one T1w (`T1=`, default ds000005's from the
  test-data cache), every file compared decompressed with `descrip` left out.
- `q2.sh <tag> <input> <output> [options]`: the same on any input, the messages shown.
- `t.sh`, `t2.sh`: bash's `[` on split words (`too many arguments`, ``syntax error: `-x'
  unexpected``, the integer message), as `shtest.rs` reproduces them.
- `ab.sh`: bash's report of a child aborted by a signal (the padding, no core dump note with
  `ulimit -c 0`).

Run them with FSL off the `PATH` (`FSLDIR_ORACLE` names the installation, `PROBE_DIR` the
scratch directory, `LARMORX` the checkout with `crates-nc/target/release/larmorx-nc` built).
The parity cases themselves are in the BET replica suite (`fsl_bet.py`: `fslroi/`, `ssroi/`).
