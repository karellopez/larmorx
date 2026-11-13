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
  the same form. 49 commands: every reached function of `utils/histo.cpp` (with the in-place forms
  and given outputs), `MatrixAlloc`, `MatrixCopy`, `MatrixMultiplyD` (real and complex),
  `include/affine.h`, `utils/dmatrix.cpp`'s operations (the x87 `long double` products
  included, with NaNs and infinities among the inputs), `utils/utils.cpp`'s `DListStats`; since
  item 3.1-24 the float matrices' operations, vectors, printing, primes and draws; since 3.1-23's
  minimizers `powell`, `powell2`, `dfpmin` (`OpenPowell`, `OpenPowell2`, `OpenDFPMin` on four seeded
  float costs passed as C function pointers, with the iteration callback, the step function and
  the user callback; every evaluation and callback hashed; standard output and error captured
  while FreeSurfer runs and hex-encoded into the line; `OpenDFPMin` in a child process, its line
  `CRASH` when FreeSurfer dies of a signal, FS-37; `robust` likewise); since item 3.1-24's last
  piece `meig`, `msvd`, `msvdinv`, `mpinv` (the eigensystems, the SVD and the functions over it,
  each in a child process with a 2-second alarm: `TIMEOUT` where VNL never returns on a non-finite
  matrix, FS-47, FS-48, ITK-112; `CRASH` for FS-44 to FS-46) and `mrdtxt`, `mrdasc`, `mrdbin`, `mwr`
  (the matrix files: generated files written to `fsmaths-file.tmp` in the cases' directory, which
  `fsmaths` makes its working directory, so the messages name the same file; `errno` and the bytes
  read are part of the line). VNL's messages name their source file: the unit oracle's ITK path is
  written as the official build's (`official_itk`).
- `variation.py <cases> [--runs N]`: the official code's own variation on a case file (runs `fsmaths`
  N times, never cached: crashes and the commands whose lines differ between runs).
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
  hash `tests/fs_maths_official.rs` records (seed 1, 490 cases).

- `scanf/`: black-box measurements of glibc 2.35's `%f` and `%d` (its README), the behaviour the
  port's `cscan` follows.

Run (the unit oracle built):

```
validation/probes/fs-maths/build.sh
scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-maths/check.py --seed 7 --n 67000
```

Measured 2026-11-12: seed 7, 49,000 cases: 48,185 of 48,185 comparable lines identical with
`reproduce`, 815 FS-22; the default differs on 2,348 lines, all with a defect's notice but 13
entropies (the correctly rounded `log`). Seed 1, 490 cases: 481 identical, 9 FS-22, hash
`394127d7bcefb0a2849cdac19b2465fb05c1ace98069d89983a22fb95e224054`.

Measured 2026-11-12 (item 3.1-24, first piece): seed 7, 62,000 cases of 62 commands: 61,138 of
61,138 comparable lines identical with `reproduce`, 862 not comparable (855 FS-22, 7 FS-16); the
default differs on 4,035 lines, all with a defect's notice but 14 rotations, 3 Gaussian draws and 6
entropies (the correctly rounded `sinf`, `cosf`, `log`). Seed 1, 620 cases: 607 identical, 13 not
comparable, hash `ed7e3de20c30f3c02cf8425fc20f8b5ab530fa45a21c0e22b524ee62e36a3b28`.

Measured 2026-11-13 (with the inverse, determinant and QR factors): seed 7, 64,000 cases of 64
commands: 63,173 of 63,173 comparable lines identical with `reproduce` (827 not comparable: 817
FS-22, 10 FS-16); the default differs on 4,271 lines, all with a defect's notice but 19 rotations,
4 Gaussian draws and 10 entropies. Seed 1, 640 cases: 627 identical, 13 not comparable, hash
`bb0da3dafb4cc3d5a3810a49f2e624a30912bf5302523e52a0b814c4af82ef04`. Complex matrices given to
`minv` are invertible: FreeSurfer crashes on a singular one (FS-33).

Measured 2026-11-13 (3.1-23's minimizers; NaNs and infinities in the double matrices): seed 7,
67,000 cases of 67 commands: 66,042 of 66,042 comparable lines identical with `reproduce`, 958 not
comparable (827 FS-22, 131 ITK-106: `OpenDFPMin`'s verbose L-BFGS printout of the vectors, which
prints memory before them); the default differs only with a defect's notice, but 29 rotations, 1
Gaussian draw and 3 entropies (the correctly rounded `sinf`, `cosf`, `log`). FreeSurfer crashes in 29
of the 1,000 `OpenDFPMin` cases (FS-37: the line `CRASH`, the port's `reproduce` gives the same).
Seed 1, 670 cases: 657 identical, 13 not comparable, hash
`642c41982a861dcf793433c0d73274d9672611a0e36612a0312a31cd2c71c3b9`, checked with AArch64's NaN
emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` **with**
`NAN_TRAP_DIR` set: without it the trap is not armed) in debug and release builds. The official code
varies between runs only in FS-22 cases (`robust`, heap-dependent reads; one of 8 runs crashed in
one): `robust` and `dfpmin` run in a child process.

Measured 2026-11-13 (item 3.1-24's last piece: the decompositions and the files): seed 7, 75,000 cases of 75
commands: 73,122 of 73,122 comparable lines identical with `reproduce`, 1,878 not comparable (memory
FreeSurfer does not own or never wrote, and 313 non-finite matrices the port refuses: 97 of them
`TIMEOUT` in FreeSurfer, 174 equal all the same, 42 that FreeSurfer finishes); the default differs
only with a defect's notice but for 8 rotations, 1 Gaussian draw and 10 entropies. Seed 1, 750 cases:
723 identical, 27 not comparable, hash `6cebc7a8e9a749ae619547d19bba0d991eb2c10efbbaf1cf46b65f5c0d5a6470`,
checked with AArch64's NaN emulated in debug and release builds.
