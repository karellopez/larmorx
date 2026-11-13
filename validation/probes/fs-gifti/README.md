# fs-gifti: GIFTI files and the XML parser under them, against gifticlib and expat

The check behind FreeSurfer's foundations' items 3.1-09 to 3.1-12 (`larmorx_io::gifti`,
`larmorx_io::xml`, `larmorx_core::cscan::sscanf_f32`). Findings: `docs/findings/gifti.md` (the
upstreams) and `docs/findings/freesurfer-permissive.md` (FreeSurfer's copies; this probe, whose
`src/dialect.rs` models them, is closed to FreeSurfer's clean-room implementers as that file is).

```
bash build.sh <out>        # needs the unit oracle (oracles/freesurfer-8.2.0-unit) and tags/gifticlib-1.12,
                           # tags/expat-2.0.1, tags/nifticlib-2.0.0; GCC 8.4.0 from the unit oracle, the machine's zlib
scripts/orchestration/boxed.sh 12 <out>/fs-gifti <out> [scale] [--only scan|xml|read]
```

- `libup.so`: gifticlib 1.12 (`gifti_io.c`, `gifti_xml.c`), expat 2.0.1 (`xmlparse.c`, `xmlrole.c`,
  `xmltok.c`, configured by `up-include/expat_config.h` as FreeSurfer configures its copy) and
  nifticlib 2.0.0, with `shim.c`.
- `libfs.so`: FreeSurfer 8.2.0's `libgifti.a`, `libexpat.a`, `libnifti.a` from the unit oracle's
  build, with `shim.c` compiled against FreeSurfer's `packages/` headers (read by the compiler
  only).
- Every call of an original runs in a forked child (`src/ffi.rs`): the libraries' state is fresh
  each time, a crash or a hang (killed after 20 s) is recorded as such.
- Families: `scan` (`sscanf("%f")` against glibc's: 20,000 strings per scale unit: halfway values,
  subnormals, hexadecimal, NaN and infinity forms, junk); `xml` (4,000 generated documents per
  scale unit, after the 20 fixed ones of larmorx-io's `events_digest` test: every construct expat
  reports to gifticlib's handlers, internal subsets, entities, the four encodings, mutated and cut,
  fed in gifticlib's buffers, in random pieces or whole; every event and its bytes, the error code
  and line, against both libraries); `read` (3,000 generated GIFTI files per scale unit, against
  both libraries through their dialects: every data type in every encoding and byte order,
  external files, label tables, coordinate systems, metadata, CDATA, odd values, misplaced
  elements, bad Base64, corrupt and cut zlib streams, cut and mutated files, files in UTF-16 and
  ISO-8859-1, every setting (verbosity 0 to 7, `b64_check`, buffer sizes, `update_ok`, `zlevel`,
  `indent`, DataArray lists, two reads in a row); every field and data byte read, the validity
  check, the display, every message, and the file written back in its encoding or another,
  GZipBase64Binary data compared by content).
- A case whose original reads memory it never wrote or crashes is compared where it is defined
  and counted as not run where it is not (the port's notices FS-80, FS-84 to FS-88 say which).
- `LXPROBE_SHOW=<n>` prints up to n differences per family; `LXPROBE_KEEP=1` keeps the files of
  differing cases in `<out>/work`; `LXPROBE_READ_N=<n>` sets the read family's count;
  `LXPROBE_ONLY_CASE=<k>` runs one read case and writes both sides' dumps and messages;
  `LXPROBE_XML_SIMPLE=1` generates UTF-8 documents without internal subsets only.
- `gen_nametab.py` writes `crates/larmorx-io/src/xml/nametab.rs` from expat's `nametab.h`.

Prints `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). Single-threaded, about 200 MB.

## Results

- 2026-11-13, at 9b9c4990, scale 1 (log `oracles/logs/fs-found-c-r6-gifti-all-1.log`): scan 20,000
  strings 0 differ; xml 4,000 documents x 2 libraries, 0 differ (4 not run: FS-88); read 6,000
  (3,000 files x 2 libraries), 13,796 comparisons, 4 differences, all from the probe's own state
  (fixed in the next build: each run now starts from the case's own external files) and a check
  reading memory never written (FS-80; masked since); 2,670 not run (FS-80, FS-84 to FS-88: memory
  never written or a crash in the original). GZipBase64Binary files written: 272, 198 of them
  byte-identical to zlib's.
- Scale 5: see `oracles/logs/fs-found-c-r6-gifti-all-5.log` (the numbers are in the findings).
