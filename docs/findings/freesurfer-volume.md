# FreeSurfer 8.2.0's volume functions (`fs.volume`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines and the official build's code; the behaviour is in `specs/freesurfer-volume.md`). Written by
the replica author of queue items 3.1-35 to 3.1-40 (FreeSurfer's foundations, track C, from relay
11). Defect IDs FS-285 to FS-330, decided by rules A to D (CLAUDE.md, D13); the register's entries
are in the orchestrator's handoff.

## 1. `readVolGeom` (`utils/transform.cpp` 467-532): FS-285 to FS-288

`readVolGeom` keeps `char line[STRLEN + 16]`, `char param[64]`, `char eq[2]`, `char buf[STRLEN]` on
its stack and runs `sscanf(line, "%s %s %*s", param, eq)` on every line, then, for a recognised
`param`, `sscanf(line, "%s %s %d \n", ...)`, `"%s %s %1023s%*s\n"`, `"%s %s %d %d %d\n"` or
`"%s %s %f %f %f\n"`. Nothing bounds the first two `%s`.

**The official build's frame** (GCC 8.4.0 `-O3` with Ubuntu's default `-fstack-protector-strong`;
`objdump -dr` of the unit oracle's `libutils.a` `transform.cpp.o`, the same flags and compiler as
the official binaries, `docs/freesurfer/variation.md`): `eq` at `rsp+0x2e`, `param` at `rsp+0x30`
(right after `eq`), `buf` at `rsp+0x70`, `line` at `rsp+0x470`, the canary at `rsp+0x888`. So:

- **FS-286** (rule: memory written outside a buffer; the official build's behaviour is deterministic
  and `reproduce` follows it): a second word of `n >= 2` bytes writes `n - 2` bytes and a NUL into
  `param` after `sscanf` wrote `param` (the conversions assign in order), so `strcmp(param, ...)`
  sees the second word from its third byte on. The longest word a line holds (1,039 bytes) never
  reaches `line` (`0x2e + 1,040 < 0x470`) or the canary. Observed in the probe: every FS-286 case
  identical with `reproduce` (seed 7: 176 of 2,000 cases).
- **FS-285**: a first word of 64 bytes or more runs into `buf`, which every reader of `buf`
  (`strcpy(vg->fname, buf)` after the `filename` call) writes first in the same call: no effect
  (`0x30 + 1,040 < 0x470`: never `line`). Both modes read the word whole; the notice marks the case.
- **FS-287** (rule A: a blank line in a hand-edited block makes FreeSurfer skip the block's last line
  and read the next one): when the first `sscanf` returns `EOF` (no word), `param` keeps the previous
  line's word, so the line is counted again (`vgRead = 1` for `valid`; the value `sscanf` returns
  `EOF` and assigns nothing). Default: the line neither counts nor is read; `reproduce` counts it.
- **FS-288** (memory never written): before any word, `param` is uninitialised stack. Both modes: no
  key, a notice; the probe skips such cases (in the probe's child they happened to match nothing:
  20 of 20 identical at seed 7).
- Kept (rule C): the loop condition `(p = fgets(...)) && counter < 8` reads a ninth line and gives it
  back with `fseek` (not when `ftell` gave 0 or -1, so not for pipes); `strlen(p) == 0` (a NUL first
  byte) ends the block and also gives the line back; a block with fewer than eight recognised lines
  reads on into whatever follows (the next block's `valid` line overwrites this one's): files
  FreeSurfer writes always have the eight lines.

**`errno`.** The value `readVolGeom` leaves depends on glibc's `sscanf` at the end of a line (track D's
open `validation/investigations/glibc-scanf-probe/`): `cscan::Call` of `larmorx-freesurfer-core`
(before track A's alignment with that probe) latched the end and kept a later `ERANGE`, which the
probe showed wrong (`cras = -168.5 -1e-50`: glibc leaves 0, the value saved when the end was first
met, before the underflow's `ERANGE`). `vgeom.rs` has its own `sscanf` model following the open
probe's observations; seed 7, 2,000 cases: every `errno` identical.

## 2. The rest of `transform.cpp`'s geometry functions (285-603)

`vg_print` (285), `initVolGeom` (305, already `fs.mri`), `getVolGeom` (334), `copyVolGeom` (428),
`writeVolGeom` (452), `vg_i_to_r` (536), `vg_r_to_i` (542), `TkrVox2RASfromVolGeom` (549),
`vg_isEqual` (588), `vg_isNotEqualThresh` (600): no defect. `vg_isNotEqualThresh` stops at the
centre (no shears), unlike `VOL_GEOM::isNotEqualThresh` (`mri.h`), which `operator==` uses:
`vg_isEqual` and `==` can disagree on geometries with shears. `TkrRAS2VoxfromVolGeom` (557) is not
reached by any recorded program (the inventory's core view): not ported.
