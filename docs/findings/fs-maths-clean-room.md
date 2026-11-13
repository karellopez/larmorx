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

- **What:** all 24 groups of the spec's §23.2 (26,707 calls, seed 7), on the probe (plainly, again,
  and in two guard runs) and on the clean room's candidate, in both modes. `reproduce`:
  25,739 identical, 672 where FreeSurfer did not finish, 249 memory
  lines, 47 others: `minverse`'s 34 NaN-bit lines and the 13 lines FS-47 and FS-48 refuse (below). `fix`: 22,533 identical, 672 not finished, 383 memory, 3,092 corrected by a noted
  defect, 27 others: `minverse`'s NaN-bit lines (below).
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-13).

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

- `AffineMV`: `A[i]·x0`, `x1·A[4+i]`, `x2·A[8+i]`, `A[12+i]·x3`; `AffineMM`: `B[k+4j]·A[i+4k]`,
  summed left to right.
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
  (not zeros, as the spec first said; the spec writer amends §18.2).
- **How we know:** *validated* (`mfiles` identical in `reproduce`; a fit over every short-read case).

## FS-30 cannot occur with the C library's generator

- **What:** FS-30 (the Gaussian draw's `r2 = 0`) needs two consecutive `drand48` draws of exactly
  0.5, i.e. the 48-bit state 2^47 twice in a row. POSIX's recurrence `x' = (a·x + c) mod 2^48`
  (`a = 0x5DEECE66D`, odd; `c = 11`) maps 2^47 to 2^47 + 11, so the next draw is 0.5 + 11 × 2^-48:
  no seed reaches FS-30. Both modes implement the spec's switch; no test can meet it through
  `srand48`.
- **How we know:** *proved* from POSIX's definition.

## VNL's decompositions on NaN inputs (the main package's ports)

- **What:** with NaN or infinite entries, `MatrixInverse`, `MatrixDeterminant` and
  `MatrixQRdecomposition` (VNL's fixed-size routines and QR in float) give results whose NaN bits
  differ from the official build's on some lines (`minverse`: 34 lines in `reproduce`, 27 in `fix`,
  every value identical; the main package's ports follow the C++ source's operand order, the
  official build another) and between x86-64 and the AArch64 NaN emulation. The orchestrator's
  decision (2026-11-13): the ports gain the official build's order with x86-64's NaN rules.
- **What else:** `OpenNonSymmetricEigenSystem` of a matrix with a NaN entry that FreeSurfer finishes
  (`oeigenns 4 41700000 3f800000 4106db6e 42752492 40a00000 40e00000 c0dc0000 42b8db6e 41200000
  41200000 7fc00000 40400000 41980000 c2740000 41a00000 3f800000`): the official build's
  eigenvectors are `0 0 0 0 0 0 NaN NaN …`, the clean room's (through
  `larmorx_core::vnl_eigen::real_eigensystem`) all NaN, and their signs differ between larmorx's
  debug and release builds (an optimisation changes a NaN's sign inside the port).
- **So:** the golden cases give those five commands finite arguments until the ports follow the
  official build's NaN rules.
- **How we know:** *measured* (the probe on that line; the golden cases under
  `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`'s emulation, debug and release).

## The defects the spec's rule refuses in both modes

- **What:** `eigen` 5 and `svd` 8 lines differ in `reproduce` only: FS-47 and FS-48 refuse, in both
  modes as the spec says, non-finite matrices that VNL usually never finishes; FreeSurfer finishes
  these rare ones (a 2 x 2 with an infinity, the 1 x 1 infinity of `MatrixNSConditionNumber`'s
  product, two pseudo-inverses of NaN matrices). The orchestrator's decision (2026-11-13): the spec
  writer states which matrices never return and which FreeSurfer finishes (their result under
  `reproduce`).

## Golden cases and platforms

- **What:** `tests/golden_maths.rs`, every command of the probe's grammar on seeded arguments in both
  modes (16 SHA-256, 0.08 s in a debug build), identical in debug and release builds on x86-64 and
  under the AArch64 NaN emulation of both (198 to 205 NaN signs flipped by the emulation, none
  reaching a result).
- **How we know:** *measured* (2026-11-13).

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
