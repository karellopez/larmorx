# fs-dicom: larmorx-io's DICOM reader against CTN 3.0.6 and FreeSurfer 8.2.0's copy

**Closed to FreeSurfer's clean-room implementers** (it builds and calls FreeSurfer's copy of CTN,
and names its changes). What it shows: `larmorx_io::dicom` (CTN 3.0.6's `DCM_OpenFile` for files,
its conditions, condition stack and the process state its calls leave) gives the originals'
results byte for byte: every condition, every stack text, every field of the object and every
element's bytes, standard output and standard error, on generated files.

- `shim.c`: one entry point, `fdp_run(name, ops, n, out, cap)`, running a script of operations on
  one object handle (1 `DCM_OpenFile(name, arg)` and the object walked through CTN's private
  structures; 2 `DCM_CloseObject`; 3 the stack, bottom first; 4 `COND_PopCondition(arg)`; 5
  `COND_DumpConditions`; 6 `COND_TopCondition`; 7 `DCM_GetElement` and the caller's element after
  it; 8 `DCM_GetElementValue` into a buffer of `min(length, file size + 4096)` bytes filled with
  0xa5, and the buffer, `rtnLength` and the context after it; 9 the context set; 10 and 11 the
  caller's tag and representation set; 12 the file cut, `truncate`), writing typed records (`src/port.rs` writes the
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

- The element script (since 2026-11-16, item 3.1-56): every file also runs a second script, five
  opens (the format probe's four, then one of six other options) each followed by requests on the
  object it leaves, built from the port's object (`requests` in `src/main.rs`: FreeSurfer's way,
  pieces, contexts, SQ, large pixel requests, missing tags), the file cut once before its pixel data
  are asked for again (restored before every run). Where the original finished but the case is
  not identical, every `get` and `value` record is compared one by one: a value may differ only
  when the port names it as memory it cannot give (FS-91, FS-951) or FS-953's swap; otherwise the
  case counts `ELEMENT REQUESTS DIFFERENT`. The defects that explain a difference now include
  FS-951 and FS-953.

Results: `docs/findings/ctn-dicom.md` ("The numbers"); scale 5 (2026-11-13): 10,030 files, 11
opens each, against both originals 0 different (CTN 3.0.6: 9,429 identical, 601 explained, 360 of
them its crash; FreeSurfer's copy: 9,429 identical, 601 explained, 361 crashes). Scale 5
(2026-11-16, 10,038 files, the 8 element cases added; log
`<workspace>/oracles/logs/fs-found-c-r35-dicom-probe-5.log`, 2.6 minutes on the main machine): the open script
against CTN 3.0.6 9,412 identical, 626 explained (454 crashes; FS-93's crashes depend on memory and
vary from run to run), against the copy 9,418 identical, 620 explained; the element script against
CTN 3.0.6 8,710 identical, 1,328 explained (410 crashes), 918 checked request by request, against the
copy 8,718 identical, 1,320 explained (408 crashes), 912 checked: 0 different in all four.
