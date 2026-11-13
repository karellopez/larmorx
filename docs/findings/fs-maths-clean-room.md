# fs.maths' clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's mathematics (`crates/larmorx-fs-core/src/maths/`, queue item
C3.1-02) was written from [`specs/freesurfer-maths.md`](../../specs/freesurfer-maths.md) and
checked only against black boxes: the prebuilt probe `fs_maths_probe` (FreeSurfer's compiled
mathematics, its VNL and the C library, glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-maths-probe/`](../../validation/investigations/freesurfer-maths-probe/README.md).
Nothing here comes from FreeSurfer's, VNL's build or glibc's source, nor from Numerical Recipes' code;
this file is open to everyone. The comparisons and the tools are in
[`validation/investigations/freesurfer-cleanroom-maths/`](../../validation/investigations/freesurfer-cleanroom-maths/README.md).

## The comparison

- **What:** all 24 groups of the spec's §23.2 as amended on 2026-11-13 (28,168 calls, seed 7; the
  fixed batches `mmake-sizes`, `mmake-sizes-old`, `affine-nan`, `eigen-nonfinite`, `svd-nonfinite`,
  `mfiles-cut` and `mfiles-end` included), on the probe (plainly, again, and in two guard runs) and on
  the clean room's candidate, in both modes. `reproduce`: 26,676 identical, 1,121 where FreeSurfer
  did not finish, 361 memory lines, 10 lines of `svd` that differ in the bits of NaNs only (§17.4:
  the official build's LINPACK kernels), none other. `fix`: 22,898 identical, 1,121 not finished, 502
  memory, 3,647 corrected by a noted defect, none other.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-13, relay 6).

## The minimizers (§13)

- **What:** Powell's method with given directions (`OpenPowell`, `OpenPowell2`, its printed lines
  and VNL's counters) and the L-BFGS driver behind `OpenDFPMin` give the official build's results
  bit for bit on the 181 calls of the `minimizers` group (60 `dfpmin`, 61 `powell`, 60 `powell2`,
  the probe's four test costs, their FNV-1a hash of every evaluation): `reproduce` 179 identical and
  2 where FreeSurfer crashed (FS-34: no directions; FS-37: a successful pass without improvement
  leaves empty parameters); `fix` no unexplained line (10 noted lines corrected: FS-37, FS-38; 9
  memory lines: ITK-106's printout).
- **Confirmed:** the line minimization hands VNL's Brent minimizer the bracket's three points
  without the middle point's known value (it is evaluated again, as VNL's own Powell does): the hash
  of every evaluation matches.
- **How we know:** *validated* (`compare.py compare`, both modes).

## Operand orders measured where the spec gives none

Which NaN a result keeps depends on the order of the operands of the official build's code; these
were fitted on the probe (`mproduct`, `affine`, `mvectors`, `harith` groups), and every line of
those groups is identical:

- `AffineMV` and `AffineMM` (now the spec's §3): in every product the vector's number (the second
  matrix's) first, summed left to right. The first fit (`A[i]·x0` and `A[12+i]·x3`, the matrix's
  number first in the first and last products) matched every line of the first `affine` batch but
  differs on 6 of the 33 lines of the fixed batch `affine-nan` (two NaNs of different payloads in each
  product), which the spec's order matches.
- `MatrixMultiplyD`'s complex sums: the term first (`term + (double) re`); `MatrixMultiply`'s float
  sums in blocks of four (`v + p0`, `p1 + v`, `p2 + v`, `v + p3`, then `v + p`; the spec's §16).
- `VectorTripleProduct`'s order (a fit over its NaN cases).
- The peak searches take a bin when `!(count <= pct × largest)` (a NaN count is a candidate);
  `HISTOinit`'s bin size `<= 0` becomes 1 (a NaN stays); `HISTOfindLinearFit` starts at
  (a, b) = (1, 0); `HISTOfindCurrentPeak` clips a start beyond the last bin; `hp.plt` holds Z's bins
  with the fitted curve.

## FS-49's `reproduce` (§18.2)

- **What:** FreeSurfer reads each number of a binary matrix file into one stack slot that it then
  byte-swaps in place: a short read keeps the slot's other bytes from the previous number, swapped
  (not zeros, as the spec first said; §18.2 amended to this at `e3acdd40`).
- **How we know:** *validated* (`mfiles` identical in `reproduce`, the fixed batch `mfiles-cut` too:
  every byte of the header, of the first element and of later ones; a fit over every short-read case).

## `errno` at the end of the input (§18.1)

- **What:** the first rule implemented (an item that meets the end sets `errno` to 0 unless an
  earlier item of the call ended there) differs on 7 of the 24 lines of the fixed batch `mfiles-end`
  (the ASCII read's last element overflowing at the file's very end: the probe keeps the `errno` from
  before it, 0). The amended rule (the call keeps `errno`'s value at its first meeting of the end, 0
  when a conversion's skip of white space meets it, and every later meeting sets it back; an item
  meets the end before its own `ERANGE`) gives every line; the ASCII read is one call per element
  (`%f` then white space, complex `%f %f` then white space) and one call for each row's white space.
- **How we know:** *validated* (`mfiles` identical in `reproduce`, 2026-11-13).

## FS-13's sizes (§2.2)

- **What:** the fixed batches `mmake-sizes` and `mmake-sizes-old` (372 calls each: every row count
  around -16 and -9, every product around -2, real, complex and an unknown type, both allocation
  forms, both allocators) give the amended rule's outcomes in `reproduce`: the matrices of no element
  FreeSurfer makes (the probe prints `max(0, k rows cols + 2)` floats of their arrays), the record's
  type, rows and columns as the first floats of an array under rows -9 to -16 (its type plus 65,536
  in a caller's buffer), the buffer's block with every size modulo 2^64, the old allocator's two
  lines and exit; FreeSurfer's crashes refused. 74 lines are memory lines (the record's two
  addresses, 0 in the clean room).
- **How we know:** *validated* (`mmake` identical in `reproduce` but the memory lines and those
  where FreeSurfer crashed, 2026-11-13).

## FS-30 cannot occur with the C library's generator

- **What:** FS-30 (the Gaussian draw's `r2 = 0`) needs two consecutive `drand48` draws of exactly
  0.5, i.e. the 48-bit state 2^47 twice in a row. POSIX's recurrence `x' = (a·x + c) mod 2^48`
  (`a = 0x5DEECE66D`, odd; `c = 11`) maps 2^47 to 2^47 + 11, so the next draw is 0.5 + 11 × 2^-48:
  no seed reaches FS-30. Both modes implement the spec's switch; no test can meet it through
  `srand48`.
- **How we know:** *proved* from POSIX's definition.

## VNL's decompositions on NaN inputs (the main package's ports)

- **What:** with NaN or infinite entries, `MatrixInverse`, `MatrixDeterminant` and
  `MatrixQRdecomposition` (VNL's fixed-size routines and QR in float) gave results whose NaN bits
  differed from the official build's (`minverse`: 34 lines in `reproduce`, 27 in `fix`, every value
  identical) and between x86-64 and the AArch64 NaN emulation, while the main package's ports
  followed the C++ source's operand order. Since the ports follow the official build's order with
  x86-64's NaN rules (main `c9ab2a4e`), every `minverse` line is identical in `reproduce` (852 and
  10 not finished) and none is unexplained in `fix`; the golden cases give these three commands
  special values again, identical on every build and under the emulation.
- **What remains:** `OpenNonSymmetricEigenSystem` of a matrix with a NaN entry that FreeSurfer
  finishes (`oeigenns 4 41700000 3f800000 4106db6e 42752492 40a00000 40e00000 c0dc0000 42b8db6e
  41200000 41200000 7fc00000 40400000 41980000 c2740000 41a00000 3f800000`): the official build's
  eigenvectors are `0 0 0 0 0 0 NaN NaN …`, the clean room's (through
  `larmorx_core::vnl_eigen::real_eigensystem`) all NaN, and their signs differ between larmorx's
  debug and release builds (an optimisation changes a NaN's sign inside the port). The golden cases
  give `oeigen` and `oeigenns` finite arguments until that port follows the official build too.
- **How we know:** *measured* (`compare.py`, the probe on that line; the golden cases under
  `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`'s emulation, debug and release).

## FS-47 and FS-48: the matrices VNL never finishes (§17, amended)

- **What:** with an infinite or NaN entry, the clean room asks the main package's ports whether VNL
  would never return (`symmetric_eigensystem_f32_full(..).never_returns()`, `vnl_svd_f32_full(..)
  .never_returns()`): there it refuses in both modes (the eigensystem's error; the SVD's failure lines
  with `info` min(rows + 1, cols)), and every line where FreeSurfer never returned (`TIMEOUT`) carries
  that refusal; elsewhere `reproduce` gives VNL's result (a 2 x 2 with an infinity, the 1 x 1
  infinity of `MatrixNSConditionNumber`'s product, LINPACK's failures with their `info`) and `fix`
  refuses, noted. Before the amendment (both modes refused every such matrix) 5 lines of `eigen` and
  8 of `svd` differed in `reproduce`; now none, the batches `eigen-nonfinite` (236 lines) and
  `svd-nonfinite` (360) included. 10 lines of `svd` differ in the bits of NaNs in U only (the
  allowance of §17.4: the port's BLAS kernels do not yet follow the official build's order).
- **How we know:** *validated* (`compare.py compare`, both modes).

## Golden cases and platforms

- **What:** `tests/golden_maths.rs`, every command of the probe's grammar on seeded arguments in both
  modes (16 SHA-256, 0.08 s in a debug build; since relay 6 also negative sizes through `malloc2` and
  §18.1's end of the input on memory streams), identical in debug and release builds on x86-64 and
  under the AArch64 NaN emulation of both (202 to 280 NaN signs flipped by the emulation, none
  reaching a result).
- **How we know:** *measured* (2026-11-13, again after relay 6's changes).

## Speed (indicative)

`speed.py`: best of three batch times minus a baseline batch of the same shape, per call, one
thread, 2026-11-13; the probe forks a child for every line, so differences below about 0.5 ms are
noise (the 128 x 128 products varied between runs from 0.2 to 4.9 ms on the probe).

| Calls | Probe (ms a call) | Clean room (ms a call) |
|---|---|---|
| `mmul 128x128` | 3.210 | below the noise |
| `mmuld 128x128` | 4.936 | 1.035 |
| `mmul 256x256` | 15.634 | 8.088 |
| `mmuld 256x256` | 15.965 | 9.484 |
| `minverse 64x64` | 0.364 | 0.196 |
| `minverse 128x128` | 2.725 | 1.496 |
| `msvd 64x64` | 1.122 | 0.718 |
| `msvd 128x128` | 5.837 | 3.818 |
| `hsmooth 8192 bins` | 0.371 | 0.759 |
| `hcount 4000 samples` | 88.270 | 31.328 |
| `hrobust 1024 bins` | 0.635 | 1.905 |

- Products: each output row summed with plain arithmetic in each element's own order, vectorised
  across columns; only a NaN result is summed again with x86-64's NaN rules (a NaN is absorbing
  under + and ×, so every other result has the same bits).
- `HISTOsmooth`: the same for the kernel sums. `HISTOsmooth` and `HISTOrobustGaussianFit` remain
  slower than the probe by under 1.5 ms a call on these sizes (not yet profiled: `perf` is not
  available to this account).
