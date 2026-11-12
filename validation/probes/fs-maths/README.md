# fs.maths against FreeSurfer 8.2.0's official code

**Closed to FreeSurfer clean-room implementers**: `fsmaths.cpp` and `check.py` name FreeSurfer's
functions. (Running `fsmaths` is a black-box run of FreeSurfer's code, like running one of its
programs; reading these files is not.)

What it shows: the replica of FreeSurfer's histograms, float matrices (allocation, copy, the
double-accumulated product, complex ones included), 4 x 4 affine helpers and double matrices
(`crates-freesurfer/larmorx-freesurfer-core`: `histo`, `matrix`, `affine`, `dmatrix`; queue item
3.1-23) gives the official code's bits with `--upstream-defects reproduce`, case by case, and its
default differs only where a defect it corrects changed the result (`docs/findings/freesurfer-maths.md`).

- `fsmaths.cpp`: reads a case file (one case per line: a command and its inputs, every float and
  double as the hexadecimal of its bits), calls FreeSurfer's functions, writes one line per case in
  the same form. 47 commands: every reached function of `utils/histo.cpp` (with the in-place forms
  and given outputs), `MatrixAlloc`, `MatrixCopy`, `MatrixMultiplyD` (real and complex),
  `include/affine.h`, `utils/dmatrix.cpp`'s double-precision operations.
- `build.sh [<workspace>]`: compiles it with the unit oracle's compiler and flags and links it with
  the unit oracle's libraries as the unit oracle's harness is linked
  (`validation/investigations/freesurfer-unit-oracle/`: the official build's `utils` and packages,
  ITK 4.13.2), into `<workspace>/oracles/freesurfer-8.2.0-probes/fs-maths/fsmaths` (with
  `BUILD_INFO.txt`: the program's and the source's SHA-256, the compiler).
- The cases and the port's lines: the example `fs_maths_probe` of `larmorx-freesurfer-core`
  (`cases <seed> <n>`, `run <cases> fix|reproduce`; its code is `tests/fs_maths/cases.rs`, shared
  with the test `tests/fs_maths_official.rs`). The cases are seeded integers and exact divisions
  only, the same on every platform.
- `check.py --seed S --n N [--json out.json]`: builds the example, writes the cases to
  `<workspace>/oracles/inputs/fs-maths/`, runs `fsmaths` through the oracle-output cache (one
  thread), runs the port in both modes and prints the counts: lines identical with `reproduce`,
  cases where FreeSurfer reads outside its arrays (FS-22: the port notes it; not comparable),
  lines where the default differs and the defects noted there; and `official_sha256_skip`, the
  hash `tests/fs_maths_official.rs` records (seed 1, 470 cases).

Run (the unit oracle built):

```
validation/probes/fs-maths/build.sh
scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-maths/check.py --seed 7 --n 47000
```

Measured 2026-11-12: seed 7, 47,000 cases: 46,138 of 46,138 comparable lines identical with
`reproduce`, 862 FS-22; the default differs on 2,362 lines, all with a defect's notice but 8
entropies (the correctly rounded `log`). Seed 1, 470 cases: 461 identical, 9 FS-22, hash
`658b0ffaf12d685e225df01da0b23bf7e959d9d050b6ffc8457ae29764aebadb`.
