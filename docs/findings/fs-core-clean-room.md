# fs.core's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's core services (`crates/larmorx-fs-core`, queue items C3.1-00
and C3.1-01) was written from [`specs/freesurfer-core.md`](../../specs/freesurfer-core.md) and
checked only against black boxes: the prebuilt probe `fs_core_probe` (FreeSurfer's compiled core
functions and the C library, glibc 2.35) and the C library itself. Nothing here comes from
FreeSurfer's or glibc's source; this file is open to everyone. The comparisons and the tools are in
[`validation/investigations/freesurfer-cleanroom-core/`](../../validation/investigations/freesurfer-cleanroom-core/README.md).

## The comparison

- **What:** all 22 groups of the spec's §20.2 (368 runs, about 162,700 calls, seed 7), run on the
  probe and on the clean room's candidate. `reproduce`: every run identical but the two whose results
  are memory FreeSurfer never wrote (FS-09's and FS-10's empty lists). `fix`: every run without a
  defect identical; the runs with one differ by its correction.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-13).
- **Two untagged runs** differ in `fix` by the spec's own corrections, and `compare.py` does not tag
  them: the `numbers` batch (FS-12 at 2^31 − 0.5, FS-01's NUL in the finite check's error) and the
  `fsenv` run with `FREESURFER_HOME=/nonexistent` (FS-01's NUL in the colour table reader's error).

## The C library's local time beyond the spec's §14

- **What:** a POSIX `TZ` string with a daylight name and no rule (`ABC5DEF`, `EST5EDTx`, a lone
  trailing `,`) does **not** take `M3.2.0,M11.1.0` when the zone file `posixrules` exists in the zone
  folder (`TZDIR`, else `/usr/share/zoneinfo`; on Ubuntu 22.04 a copy of `America/New_York`): it takes
  that file's transitions (`ABC5DEF` is in daylight time on 1938-04-24, as New York was, and not on
  1990-03-20, before the United States' change of that year). The file's types become the string's two
  types by their daylight flag; each transition time moves by the change of an offset: none when the
  type it goes into has a UT indicator, the daylight offset's change (the string's against the file's
  last daylight type) when it leaves daylight time and the type it goes into has no standard
  indicator, the standard offset's change otherwise. After the file's last (moved) transition its own
  footer rule applies with **its own names and offsets** (`ABC3DEF` shows `EST`/`EDT` after 2037-11-01).
  Only without `posixrules` (`TZDIR=/nonexistent`) does `M3.2.0,M11.1.0` apply, as the spec says.
- **What:** `gmtime` applies the zone's leap seconds too (`TZ=right/UTC`: 1483228826 gives
  2016-12-31 23:59:60 `GMT`), even as the first call of a run.
- **What:** an offset's hours are kept to 24 (`FOO+25` is -86400), its minutes and seconds to 59
  (`EST5:75` is 5:59); a change time may reach 168 hours and be negative; `TZDIR` replaces the
  system's folder, with no second folder tried (`TZDIR=<folder> TZ=Europe/Paris`, the folder holding no
  such file, reads the string as the POSIX name `Europe` at UTC); an empty `TZDIR` is unset; a file
  that is a folder or empty leaves the string to the POSIX reading; a quoted name needs three
  characters, like a plain one; a start rule without an end rule takes `M11.1.0`.
- **What:** malformed rules (an invalid month, week or day, a `/` without a time, characters after a
  rule, a daylight name followed by something other than an offset or a rule) leave glibc with a partly
  filled rule; the clean room reads the default rule there instead (8 such cases differ).
- **How we know:** *verified* (`tz_cases.py`: 58 cases, 50 identical with the probe, 8 the malformed
  rules; the probe's `time` group: 24 zones, 31,320 calls, identical).
- **larmorx:** the clean room follows all of this (`crates/larmorx-fs-core/src/clib/time.rs`); the
  spec's §14 sentence on rules without a rule should name `posixrules` (for the spec writer).

## Smaller facts

- **What:** glibc's `atan2f` with two NaN arguments returns **x's** NaN, quieted (`atan2f(7fc00005,
  ffc00007)` is `ffc00007`, `atan2f(ffa00003, 7fa00009)` is `7fe00009`), the "left one" of SSE's
  `x + y`; FS-06's values for NaN pairs follow it. *Verified* (the probe's `single atan2f` with
  `FREESURFER_fastApproxAtan2f_use_atanf2` set).
- **What:** the speed of the hot services (one thread, indicative): per call, net of each program's
  parsing and printing, the fast arctangent 44 ns in the probe and 5 ns in the clean room, the
  selections and the median of 25 floats 1.4 µs and 0.5 µs, a 4 KiB buffer's byte swap 0.12 ms and
  0.09 ms (both dominated by the batch's hexadecimal reading); the reproducible sum 2.0 ns per term on
  one thread and 0.7 on four, the same bits. *Measured* (`speed.py`).
