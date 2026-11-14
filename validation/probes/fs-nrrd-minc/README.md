# fs-nrrd-minc: NrrdIO's NRRD reader and MINC's volume input

The check behind `larmorx_io::nrrd` (NrrdIO r3991's `nrrdLoad`) and `larmorx_io::minc` (MINC 1.5.1's
volume input) for FreeSurfer's foundations, track C (relay 26). Findings: `docs/findings/nrrdio.md`
(the upstream) and `docs/findings/freesurfer-permissive.md` (FreeSurfer's copies; this probe
compiles against FreeSurfer's `packages/` headers and is closed to FreeSurfer's clean-room
implementers as that file is).

```
bash build.sh <out>    # needs the unit oracle (oracles/freesurfer-8.2.0-unit), tags/NrrdIO-r3991
                       # (and tags/minc-1.5.1 for the MINC part); GCC 8.4.0 from the unit oracle
<out>/fs-nrrd-minc <out> [count] [seed] [--only nrrd|minc]
```

- `libnrrd_up.so`: NrrdIO r3991's files built as FreeSurfer builds its copy (`TEEM_DIO=0
  TEEM_QNANHIBIT=1 TEEM_32BIT=0 TEEM_ENDIAN=1234`, no `TEEM_ZLIB`, no `TEEM_BZIP2`; GCC 8.4.0,
  `-O3 -DNDEBUG -fopenmp -w -fPIC`), with `shim_nrrd.c` and an empty `up-include/itk_NrrdIO_mangle.h`
  (NrrdIO r3991 includes ITK's mangling header; FreeSurfer's copy drops the include).
- `libnrrd_fs.so`: FreeSurfer 8.2.0's `libnrrdio.a` from the unit oracle's build, with `shim_nrrd.c`
  compiled against its header.
- `shim_nrrd.c` runs `nrrdLoad` in a forked child and writes the whole `Nrrd` (every field, every
  double's bits, the data, comments, key/values) and biff's messages as a dump on a pipe, with the
  child's standard error; the parent classifies a child that exits, dies of a signal or runs past
  the time limit (300 ms for a case the port says never returns, 5 s otherwise).
- The driver (`src/nrrd.rs`) generates files of three kinds: random headers (every field in valid
  and broken spellings and values, the five encodings, attached and detached data, templates,
  lists, skips, `\r\n`, NUL bytes), well-formed files (every type, every encoding NrrdIO has, space
  information, kinds sized to their axes, detached pieces), and cases aimed at the undefined paths
  (long messages, 17 sizes, template conversions, long ASCII values, sizes past 2^64, long lines).
  The port reads each with `reproduce`: its dump must equal both originals'; a case where the port
  notes a defect whose original behaviour is undefined (FS-761, FS-762, FS-765 to FS-768, FS-771) is
  not compared, and the originals' outcomes there are counted. Generated sizes stay small: NrrdIO
  clears its whole array before reading, so a header announcing gigabytes makes the original's
  memory, not its result, the measure.
- `LXPROBE_SHOW=<n>` prints up to n differences; a differing case's files are kept in
  `<out>/work-<seed>/keep-<seed>-<case>`.

Prints the counts and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). Run inside a memory box
(`scripts/orchestration/boxed.sh 8 ...`); single-threaded, each case two forks per original.

## Results (2026-11-14, relay 26)

| seed | cases | comparisons identical | of them successful reads | differ | not compared (undefined) |
|---|---|---|---|---|---|
| 7 | 1,500 (first generator) | 2,880 | | 0 | 120 |
| 13 | 4,000 (with the undefined cases) | 7,352 | 196 | 0 | 648 |
| 17 | 4,000 (with the well-formed files) | 7,402 | 1,138 | 0 | 598 |
| 101 | 20,000 | 36,926 | 6,070 | 0 | 3,074 |
| 202 | 20,000 | 36,992 | 6,250 | 0 | 3,008 |
| 303 | 20,000 | 36,918 | 6,266 | 0 | 3,082 |

(Each case is compared with both originals; seeds 101 to 303 ran three at a time in a 12 GiB box,
logs `<workspace>/oracles/logs/fs-found-c-r26-nrrd-seed*.log`.)

On the undefined cases both originals behaved alike: FS-761 never returned; FS-762, FS-768 and all
but one FS-767 case aborted (signal 6: FreeSurfer's build has `_FORTIFY_SOURCE` and the stack
protector); FS-771 crashed (signal 11); FS-765 and FS-766 returned.
