# fs-dicom: larmorx-io's DICOM reader against CTN 3.0.6 and FreeSurfer 8.2.0's copy

**Closed to FreeSurfer's clean-room implementers** (it builds and calls FreeSurfer's copy of CTN,
and names its changes). What it shows: `larmorx_io::dicom` (CTN 3.0.6's `DCM_OpenFile` for files,
its conditions, condition stack and the process state its calls leave) gives the originals'
results byte for byte: every condition, every stack text, every field of the object and every
element's bytes, standard output and standard error, on generated files.

- `shim.c`: one entry point, `fdp_run(name, ops, n, out, cap)`, running a script of operations on
  one object handle (1 `DCM_OpenFile(name, arg)` and the object walked through CTN's private
  structures; 2 `DCM_CloseObject`; 3 the stack, bottom first; 4 `COND_PopCondition(arg)`; 5
  `COND_DumpConditions`; 6 `COND_TopCondition`), writing typed records (`src/port.rs` writes the
  same from the port; `src/render.rs` shows them as text). The element item's allocated length is
  dumped as its low 32 bits (FreeSurfer's copy computes an undefined length's room in 64 bits,
  CTN 3.0.6 in 32; nothing reads it back).
- `build.sh <out>`: `libup.so` (CTN 3.0.6's `dcm.c`, `dcmdict.c`, `dcmcond.c`, `dcmsupport.c`,
  `condition.c`, `lst.c` from `<workspace>/tags/ctn-3.0.6` with `up-include/dicom_platform.h`) and
  `libfs.so` (the unit oracle's `libdicom.a`), both with the unit oracle's GCC 8.4.0 and
  FreeSurfer's flags (`-O3 -DNDEBUG -fopenmp -fPIC -DLITTLE_ENDIAN_ARCHITECTURE -DLinux`), and the
  driver; `BUILD_INFO.txt` records the compiler, the flags and the checksums.
- `gen_dict.sh <scratch>`: generates `crates/larmorx-io/src/dicom/dict_table.rs` from CTN 3.0.6's
  `dcmdict.c` (`gen_dict.c` includes it) and checks that FreeSurfer's copy gives the same table.
- `fs-dicom <out> [<scale>]`: the fixed edge cases (every defect's, the special elements', a
  missing file, a directory, a 1,156-byte name, nesting of 8, 128, 129 and 2,000 levels) and
  2,000 x scale generated files (`src/make.rs`), each run by the originals in a forked child
  (errno 0 at its start, a 20-second limit) and by the port in `reproduce` (CTN's dialect for
  CTN 3.0.6; for FreeSurfer's copy the probe's `FsDialect`: its longer "Data Element ... longer"
  message and the 2-byte-pair swap of FS-94). A difference counts as explained only when the port
  names a defect that leaves the original's result undefined or that the port bounds (FS-90 to
  FS-93, FS-95, FS-97, FS-98); the last line is `VERIFY-PASS` or `VERIFY-FAIL`.
  `LXPROBE_ONLY_CASE=<n>` shows one case in full, `LXPROBE_SHOW_EXPLAINED=1` every explained
  case's notices, `LXPROBE_SEED=<n>` another set.

Run (boxed, about 9 minutes at scale 5 on the main machine):

```
validation/probes/fs-dicom/build.sh <out>
scripts/orchestration/boxed.sh 12 bash -c "cd <out> && ./fs-dicom <out> 5"
```

Results: `docs/findings/ctn-dicom.md` ("The numbers"); scale 5 (2026-11-13): 10,030 files, 11
opens each, against both originals 0 different (CTN 3.0.6: 9,429 identical, 601 explained, 360 of
them its crash; FreeSurfer's copy: 9,429 identical, 601 explained, 361 crashes).
