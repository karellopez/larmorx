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
| `candidate/` | the candidate of the spec's §20.1 (its own Cargo workspace, program `fs-core-candidate`): the probe's commands and print forms (the probe folder's README) over `larmorx-fs-core`; `LARMORX_UPSTREAM_DEFECTS` chooses the mode, and in `fix` the notices of the defects that changed a result are printed on standard error at the end; `bench <op> <n>` times the library's hot services in process |
| `tz_cases.py` | the C library's reading of `TZ` beyond the `time` group (58 cases: strings without rules and `posixrules` from another zone, offsets out of range, Julian and zero-based days, change times beyond a day, leap seconds, lookups), probe against candidate |
| `speed.py` | the spec's §20.3: the hot services' batch commands timed on both programs, net of each one's own parsing and printing, and the library's in-process times |

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
# then --mode fix; --groups a,b limits the groups (a subset draws other random cases: the
# generator is shared by the groups, so only a run of all of them matches the spec's §20.2)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 2 python -P \
  validation/investigations/freesurfer-cleanroom-core/tz_cases.py --probe <probe> \
  --candidate <scratch>/target/release/fs-core-candidate --scratch <scratch>/tz
scripts/orchestration/boxed.sh 2 python -P validation/investigations/freesurfer-cleanroom-core/speed.py \
  --probe <probe> --candidate <scratch>/target/release/fs-core-candidate --scratch <scratch>/speed
```

The crate's own tests: `cargo test -p larmorx-fs-core` (the unit tests, `tests/golden.rs`: the
spec's §20.6 golden cases, 19 SHA-256 over 16 paths, recorded with `FS_CORE_GOLDEN_RECORD=1 ...
-- --nocapture`, and `tests/defects.rs`: the §20.1 cases the probe cannot show). They also run under
the AArch64 NaN emulation of `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c` (generic C):

```sh
gcc -O2 -shared -fPIC -o <scratch>/libnan_trap.so validation/probes/fsl-mcflirt-affine-trace/nan_trap.c -lm
cargo test -p larmorx-fs-core --no-run [--release]       # then run each test binary directly:
LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1 NAN_TRAP_DIR=<scratch>/nt <test binary>
```

## Results (2026-11-13, the reference machine; seed 7, all 22 groups in one run)

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
| time | 24 (31,320) | identical (24 zones: zone files, POSIX rules, unreadable names) | identical |
| files | 1 (34) | identical | identical |
| init | 27 | identical | 23 identical, 4 corrected (FS-02) |
| messages | 72 | identical | 31 identical, 41 corrected (FS-01, FS-03, FS-263) |
| random | 36 | identical | 28 identical, 8 corrected (FS-04, FS-05) |
| version | 139 | identical | 108 identical, 31 corrected (FS-07) |
| fsenv | 18 | identical | 15 identical, 2 corrected (FS-08), 1 differs by FS-01 (below) |
| logic | 7 | identical | identical |
| temp | 37 | identical | identical |
| usage | 5 | identical (formats) | identical |
| xdbg | 4 | identical | identical |
| threads | 6 | identical | identical |

**Two untagged runs in `fix`** differ by the spec's own corrections; compare.py does not tag them with
a defect. The `numbers` batch holds two values at 2^31 − 0.5 for `nint` and `nint2` (FS-12: larmorx
saturates to 2147483647, FreeSurfer gives -2147483648) and the finite check's C-style error (FS-01:
no NUL byte); the `fsenv` run with `FREESURFER_HOME=/nonexistent` shows the colour table reader's
C-style error without FS-01's NUL byte.

**`tz_cases.py`:** 50 of its 58 cases identical; the 8 others are malformed rules (an invalid month,
week or day, a `/` without a time, characters after a rule, a daylight name followed by something
other than an offset or a rule), for which glibc keeps a partly filled rule: the clean room reads the
default rule there instead (`crates/larmorx-fs-core/src/clib/time.rs`). What these cases showed of the
C library, beyond the spec's §14: a daylight name without a rule takes the transitions of the zone
file `posixrules` beside the others when it exists (its types renamed to the string's, each
transition moved by the change of the offset its indicators name: none for a UT indicator, the
daylight one after daylight time given in wall-clock time, else the standard one, against the file's
last standard and daylight types), and its own footer rule after its last transition; the rule
`M3.2.0,M11.1.0` only without that file. `gmtime` applies the zone's leap seconds too. An offset's
hours are kept to 24, its minutes and seconds to 59; `TZDIR` replaces the system's folder (no second
folder tried); a name without an offset is that name at UTC.

**The golden cases** (`tests/golden.rs`, 19 hashes) and the defect tests (`tests/defects.rs`, 6)
pass in debug and release builds, natively and under the AArch64 NaN emulation (694 NaNs made by the
hardware in the debug run had their sign changed; no result changed: the crate builds its NaNs as
x86-64's SSE does, explicitly).

**Speed** (`speed.py`, one thread, the machine shared with other work: indicative). Per call, net of
each program's own parsing and printing: the fast arctangent 44 ns (probe) and 5 ns (clean room);
the k-th smallest, the quick selection and the median of 25 floats 1.4 µs and 0.5 µs; the byte swap
of a 4 KiB buffer 0.12-0.13 ms and 0.09 ms (both dominated by the batch's hexadecimal reading);
the reproducible sum below the batch's resolution in both. In process (the `bench` command): the
arctangent 14 ns per call on random pairs (the table's 400 KB read at random), the selections and the
median 0.22-0.25 µs per 25-item list, the swaps 3-40 µs per MiB, the reproducible sum 2.0 ns per term
on one thread and 0.7 on four (the same bits on any number of threads).
