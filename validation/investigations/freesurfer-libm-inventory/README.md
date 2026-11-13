# FreeSurfer 8.2.0's C-library maths: the inventory's tools

**Closed to FreeSurfer clean-room implementers**: `inventory.py` reads FreeSurfer's source and its
build, and `libm_rates.c` names an argument set taken from FreeSurfer's source. The results are in
`docs/findings/freesurfer-libm.md` (closed too). The run-time measurement uses the open shim in
`../libm-shim/`.

| File | What it is |
|---|---|
| `inventory.py` | the static inventory: `imports` (every official program's dynamic imports, `nm -D`), `compile` (the default-path programs' own sources compiled as the unit oracle compiles: its GCC 8.4.0 and flags), `objects` (per function of every object, the C-library maths it calls, from `objdump`'s relocations: the unit oracle's `utils` and packages, and the programs'), `join` (with the call graph's reached functions), `scan` (the reached functions whose source names a maths function), `table` (the findings' table). Its docstring has the commands |
| `libm_rates.c` | how often glibc 2.35 differs from the correctly rounded value (CORE-MATH) for `atan2`, `log10`, `log2` (2²⁴ arguments each), `log10f`, `log2f`, `tanhf` (every float), `acosf`, `asinf` (every float in [−1, 1]), and on FreeSurfer's table of `atan2(i, 100000)`, `i` = 0 to 100,000, rounded to float |

Regenerating (2026-11-12, the main machine; the call-graph views and `cmake.json` from
`../freesurfer-inventory/`, code.md §6):

    python -P inventory.py imports <FREESURFER_HOME> docs/freesurfer/programs.toml <s>/imports.json
    python -P inventory.py compile <workspace> <s>/cmake.json <s>/imports.json <s>/obj
    python -P inventory.py objects <workspace> <s>/obj <s>/objcalls.json --jobs 4
    python -P inventory.py join <s>/core.json <s>/objcalls.json <s>/join-core.json
    python -P inventory.py scan <workspace>/tags/freesurfer-8.2.0 <s>/core.json <s>/scan-core.json
    python -P inventory.py table <s>/imports.json <s>/join-core.json <s>/scan-core.json

    ../libm-shim/build_shim.sh <workspace> <s>/shim
    gcc -O2 -fno-builtin -o <s>/libm_rates libm_rates.c <s>/shim/obj/*.o -lm && <s>/libm_rates

Results: 70 default-path compiled programs, 103 of their 104 own sources compiled (`tkregister2.cpp`
needs Tcl's headers), 526 objects, 1,331 functions that call the C library's maths; `libm_rates`'s
numbers are in the findings' §2 (log `<workspace>/oracles/logs/freesurfer-libm-rates.log`).
