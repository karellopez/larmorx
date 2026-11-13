# fs-json: larmorx-core's JSON reader and writer against nlohmann/json 3.2.0 and FreeSurfer's copy

**Closed to FreeSurfer's clean-room implementers** (it compiles FreeSurfer's `include/json.h` and
models its change). What it shows: `larmorx_core::json` gives nlohmann/json 3.2.0's values,
exceptions and output bytes on every path: `json::parse` and `operator>>` (the value tree, the
bytes the stream gave, or the exception's id and `what()`), `dump` (indents -1, 0, 2, 4 with
spaces, 4 with a tab, ASCII-only or not), `operator<<` (widths 0, 4 and 3 with a `.` fill; what
the stream holds when an exception stops it), the accessors (`operator[]` with keys and indices,
`get<double>`, `get<float>`, `get<int>`, `get<int64_t>`, `get<std::string>`, `size()`,
range-for) and doubles printed (`to_chars`).

- `shim.cpp`: the entry points `fj_parse`, `fj_dump`, `fj_stream`, `fj_float`, `fj_access`,
  writing typed records (`src/main.rs` writes the same from the port).
- `build.sh <out>`: `libup.so` (against `<workspace>/tags/nlohmann-json-3.2.0`'s
  `single_include/nlohmann/json.hpp`) and `libfs.so` (against `<workspace>/tags/freesurfer-8.2.0`'s
  `include/json.h`), both by the unit oracle's g++ 8.4.0 with FreeSurfer's C++ flags (`-O3 -DNDEBUG
  -fopenmp -fno-builtin -fPIC -std=c++11`), and the driver.
- `fs-json <out> [<scale>]`: 10 fixed documents (an empty one, a duplicate key, a lexer error,
  trailing values, the number formats' edges, nesting of 130 and 300 levels) and 3,000 x scale
  generated ones (random trees with random white space, numbers of every form and their errors,
  strings with escapes, surrogates, valid and invalid UTF-8 and controls; a third mutated: cuts,
  replaced and removed bytes, byte-order marks whole and broken, trailing garbage), and 200,000 x
  scale doubles (random bits, random mantissas, scaled integers, subnormals); the originals run in
  the driver's process; the port in `reproduce` (nlohmann's texts for 3.2.0, the probe's
  `FsDialect` for FreeSurfer's copy). The last line is `VERIFY-PASS` or `VERIFY-FAIL`;
  `LXPROBE_ONLY_CASE=<n>` shows one document.

Run (about 5 minutes at scale 5 on the main machine):

```
validation/probes/fs-json/build.sh <out>
scripts/orchestration/boxed.sh 8 bash -c "cd <out> && ./fs-json <out> 5"
```
