# FreeSurfer's core services: the clean room against FreeSurfer (black box)

The validation of `crates/larmorx-fs-core`, the clean room of FreeSurfer 8.2.0's core services
(`fs.core`, queue item C3.1-01), written from [`specs/freesurfer-core.md`](../../../specs/freesurfer-core.md)
by implementers who never opened FreeSurfer's source or its replica (the crate's
[`PROVENANCE.md`](../../../crates/larmorx-fs-core/PROVENANCE.md)). Everything here runs FreeSurfer
and the C library as black boxes: the prebuilt probe
`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe` (FreeSurfer's compiled core functions)
through the open comparison loop of
[`validation/investigations/freesurfer-core-probe/`](../freesurfer-core-probe/README.md).

| File | What it is |
|---|---|
| `candidate/` | the candidate of the spec's §20.1 (its own Cargo workspace, program `fs-core-candidate`): the probe's commands and print forms (the probe folder's README) over `larmorx-fs-core`; `LARMORX_UPSTREAM_DEFECTS` chooses the mode, and in `fix` the notices of the defects that changed a result are printed on standard error at the end |

## How to run

```sh
# the candidate (its own workspace; any target directory)
cd validation/investigations/freesurfer-cleanroom-core/candidate
CARGO_TARGET_DIR=<scratch>/target cargo build --release --locked
cd ../../../..
# the comparison of the spec's §20, in each mode (in a memory box)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 4 python -P \
  validation/investigations/freesurfer-core-probe/compare.py compare --workspace <workspace> \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe \
  --candidate <scratch>/target/release/fs-core-candidate --scratch <scratch>/core --mode reproduce
# then --mode fix; --groups a,b limits the groups
```

## Results so far (2026-11-13, the reference machine; seed 7)

The probe's reference hashes (`compare.py refs`) are the spec's §20.2 for all 22 groups.

| Group | Runs (calls) | `reproduce` | `fix` |
|---|---|---|---|
| swaps | 3 (833) | identical | 1 identical, 2 corrected (FS-261, FS-262: the byte reversal, checked by the loop) |
| cmdargs | 1 (230) | identical | identical |
| strings | 1 (4,670) | identical | identical |
| fgetl | 1 (300) | identical | identical |
| numbers | 2 (1,388) | identical | the FS-12 batch corrected; the main batch differs on 4 lines, all the spec's corrections (below) |
| lists | 2 (303) | identical but FS-10's empty lists (memory) | as `reproduce` |
| select | 2 (2,137) | identical but FS-09's empty list (memory) | as `reproduce` |
| atan2 | 3 (120,054) | identical | identical |
| romp | 2 (708) | identical (1 and 8 threads) | identical |
| clib | 1 (370) | identical | identical |
| files | 1 (34) | identical | identical |
| init | 27 | identical | 23 identical, 4 corrected (FS-02) |
| messages | 72 | identical | 31 identical, 41 corrected (FS-01, FS-03, FS-263) |
| random | 36 | identical | 28 identical, 8 corrected (FS-04, FS-05) |
| logic | 7 | identical | identical |
| xdbg | 4 | identical | identical |
| threads | 6 | identical | identical |
| time, version, fsenv, temp, usage | | not yet (the next relay) | |

**The `numbers` batch in `fix`:** compare.py does not tag it with a defect, but it holds two values at
2^31 − 0.5 for `nint` and `nint2` (FS-12: larmorx saturates to 2147483647, FreeSurfer gives
-2147483648) and the finite check's C-style error (FS-01: no NUL byte). Both are the spec's
corrections.
