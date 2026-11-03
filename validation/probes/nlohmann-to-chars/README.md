# nlohmann/json's numbers (fslstats -json)

`fslstats -json` prints its numbers through nlohmann/json 3.12.0 (FSL 6.0.7.23's
`include/nlohmann/`), whose Grisu2 gives a short decimal that reads back to the same double but
is not always the shortest or the nearest one. `larmorx_nc_core::json::to_chars` translates it.
This probe prints nlohmann's text of 2 million doubles; the replica's side
(`crates-nc/larmorx-nc-core/tests/json_probe.rs`, an ignored test) prints its own and counts
the lines that differ.

```
PATH=/usr/bin:$PATH bash validation/probes/nlohmann-to-chars/build.sh <scratch>/json_probe
<scratch>/json_probe > <scratch>/json.txt
JSON_PROBE=<scratch>/json.txt cargo test -p larmorx-nc-core --release --test json_probe -- --ignored
```

Result (2026-11-03): 0 of 1,897,170 lines differ (the 2 million values less the non-finite ones, which nlohmann writes as `null`).
