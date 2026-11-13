# Spec: the cubic spline of FreeSurfer 8.2.0's intensity normalisation (`burkardt-spline`)

(spec writer, track D of FreeSurfer's foundations, relay 1, 2026-11-12) The input for a clean-room
implementation, in the main package (`larmorx-core`, Apache-2.0), of the interpolating cubic spline
FreeSurfer 8.2.0 fits and evaluates in `mri_normalize`, **bit for bit** on every input: the second
derivatives of the spline through given knots with given end conditions (FreeSurfer's
`SplineCubicSet`, with the tridiagonal solver `d3_np_fs`), its value and derivatives at a point
(`SplineCubicValue`), and FreeSurfer's two wrappers (`OpenSpline`, `OpenSplint`).

**Why.** These routines are John Burkardt's spline library of about 2006, as FreeSurfer converted
them to single precision and changed their evaluation outside the knots (third-party report TP-04,
placement P-09). The user decided (2026-11-12): a replica in `larmorx-freesurfer` and a clean room in
the main package, both published. **Where FreeSurfer uses it:** every `mri_normalize` run (its
`MRIsplineNormalize`: a spline through the intensity peaks found along the image's rows gives each
row its scale factor), on both of `recon-all`'s streams (`T1.mgz` and the brain normalisation).

**Whoever implements it must not open:** Burkardt's source (any release or copy: `spline.C`,
`spline.cpp`, the downloads of FreeSurfer's preparation), FreeSurfer's source
(`<workspace>/tags/freesurfer-8.2.0`, any other copy; among others `utils/numerics.cpp` and
`utils/mrinorm.cpp`), the replica (in `crates-freesurfer/`, when it exists), the documents written
from FreeSurfer's source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the spec writer's
check (`validation/investigations/freesurfer-spline-spec-check/`), `docs/parts/` and the scratch
folders of FreeSurfer's parts. It may read this spec, the numerical-analysis literature (cubic
splines with first-derivative ends and the tridiagonal elimination are textbook material: de Boor,
*A Practical Guide to Splines*, ch. IV; Burden and Faires, *Numerical Analysis*, §3.5), build and run
the probe `validation/investigations/freesurfer-ranlux-probe/fs_spline_probe.cpp` (calls of
FreeSurfer's functions only) and run the official programs as black boxes (§9). What black-box runs
confirmed is marked **(observed)**.

---

## 1. Notation and arithmetic

- **binary32** (`float`) and **binary64** (`double`): IEEE 754 formats; every operation is one IEEE
  operation rounded **to nearest, ties to even** in the format named. `f(v)` is the rounding of a
  value to binary32, `d(v)` to binary64. A binary32 value converted to binary64 is exact.
- Every operation of §3 and §4 is a **binary32** operation (`f(a − b)`, `f(a / b)`, ...); §5 says
  operation by operation which ones are binary64. No fused multiply-add, no wider intermediate
  format, no reassociation (the official build is SSE2 scalar code without FMA).
- Indices start at 0. The knots `t₀, …, t_{n−1}`, the values `y₀, …, y_{n−1}` and the end values are
  binary32.

## 2. The system

The cubic spline `s` through `(t_i, y_i)` is determined by its second derivatives `M_i = s″(t_i)`,
which solve a tridiagonal system `A·M = B` of order `n`: row `i` holds `A_{i,i−1}` (written `L_i`),
`A_{i,i}` (`D_i`) and `A_{i,i+1}` (`U_i`). The first row comes from the beginning condition, the last
from the end condition, the others from the continuity of `s′` at the interior knots.

## 3. `SplineCubicSet(n, t, y, ibcbeg, ybcbeg, ibcend, ybcend)`: the second derivatives

### 3.1 Checks, in this order (each failure is a fatal error, §7)

1. `n ≤ 1`: error E1.
2. For `i = 0, 1, …, n − 2` in turn: if `t_{i+1} ≤ t_i`, error E2 for that `i` (the first such `i`).
   The test is the binary32 comparison, false for a NaN knot: a NaN passes (FS-103, §8).
3. The beginning condition `ibcbeg` must be 0, 1 or 2, else error E3.
4. The rows of §3.2 for the interior are made, then the end condition `ibcend` must be 0, 1 or 2,
   else error E4 (after E3's test, so a bad `ibcbeg` is reported first).

### 3.2 The rows (binary32)

Write `Δt_i = f(t_{i+1} − t_i)` and `Δy_i = f(y_{i+1} − y_i)`.

| Row | `L` | `D` | `U` | `B` |
|---|---|---|---|---|
| 0, `ibcbeg = 0` (a quadratic over the first interval: `M₀ = M₁`) | | 1 | −1 | 0 |
| 0, `ibcbeg = 1` (the first derivative at `t₀` is `ybcbeg`) | | `f(Δt₀ / 3)` | `f(Δt₀ / 6)` | `f(f(Δy₀ / Δt₀) − ybcbeg)` |
| 0, `ibcbeg = 2` (the second derivative at `t₀` is `ybcbeg`) | | 1 | 0 | `ybcbeg` |
| `i = 1, …, n − 2` | `f(Δt_{i−1} / 6)` | `f(f(t_{i+1} − t_{i−1}) / 3)` | `f(Δt_i / 6)` | `f(f(Δy_i / Δt_i) − f(Δy_{i−1} / Δt_{i−1}))` |
| `n − 1`, `ibcend = 0` (a quadratic over the last interval) | −1 | 1 | | 0 |
| `n − 1`, `ibcend = 1` (the first derivative at `t_{n−1}` is `ybcend`) | `f(Δt_{n−2} / 6)` | `f(Δt_{n−2} / 3)` | | `f(ybcend − f(Δy_{n−2} / Δt_{n−2}))` |
| `n − 1`, `ibcend = 2` (the second derivative at `t_{n−1}` is `ybcend`) | 0 | 1 | | `ybcend` |

The interior diagonal is `(t_{i+1} − t_{i−1}) / 3` computed from the two knots (one subtraction), not
from `Δt_{i−1} + Δt_i`. The divisions by 3 and 6 are binary32 divisions (dividing in binary64 and
rounding to binary32 gives the same bits, since both operands are binary32 values). The system is
the usual one scaled by 1/6 (the right-hand side is the difference of slopes, not six times it).

### 3.3 The solution

- If `n = 2` and `ibcbeg = ibcend = 0`, the result is `M = (0, 0)` (no solve).
- Otherwise `M` is the solution of §4. If §4 refuses (a zero diagonal entry), error E5.

`n` second derivatives are returned (binary32).

## 4. `d3_np_fs(n, A, B)`: the tridiagonal solve, no pivoting (binary32)

1. If any `D_i` (`i = 0, …, n − 1`, the diagonal **before** elimination) equals 0 (+0 or −0; a NaN
   is not 0), the solver refuses (no solution). Nothing else is checked: a diagonal that becomes 0
   during the elimination divides by zero (IEEE: ±∞ or NaN results).
2. `x = B`; `D′₀ = D₀`.
3. For `i = 1, …, n − 1`: `m = f(L_i / D′_{i−1})`; `D′_i = f(D_i − f(m · U_{i−1}))`;
   `x_i = f(x_i − f(m · x_{i−1}))`.
4. `x_{n−1} = f(x_{n−1} / D′_{n−1})`; then for `i = n − 2, …, 0`:
   `x_i = f(f(x_i − f(U_i · x_{i+1})) / D′_i)`.
5. `M = x`.

The products are binary32 (commutative, so their operand order does not matter except for NaN
payloads: §6).

## 5. `SplineCubicValue(n, t, x, y, M)`: the value and derivatives at `x`

FreeSurfer's evaluation **does not extrapolate**: outside the knots it gives the end value.

- If `x ≤ t₀`: the value is `y₀`. The two derivative outputs are **not written** (they keep what the
  caller put there).
- Else, if `x ≥ t_{n−1}`: the value is `y_{n−1}`; the first-derivative output is set to `+0`, the
  second-derivative output is **not written**.
- Else (and for a NaN `x`, which fails both tests): `j` is the smallest `i` in `0, …, n − 2` with
  `x < t_{i+1}`, or `n − 2` if there is none (a NaN `x`). Then, with every binary32 value converted
  exactly to binary64 where a binary64 operation takes it:

  | Step | Format | Value |
  |---|---|---|
  | `δ` | binary32 | `f(x − t_j)` |
  | `h` | binary32 | `f(t_{j+1} − t_j)` |
  | `S` | binary32 | `f(f(y_{j+1} − y_j) / h)` |
  | `I` | binary32 | `f(M_{j+1} − M_j)` |
  | `G` | binary64 | `d(S − d(d(d(M_{j+1} / 6) + d(M_j / 3)) · h))` |
  | `K` | binary64 | `d(I / d(6 · h))` |
  | `N` | binary64 | `d(δ · d(0.5·M_j + d(δ · K)))` (`0.5·M_j` is exact) |
  | **value** | binary32 | `f(d(y_j + d(δ · d(G + N))))` |
  | **first derivative** | binary32 | `f(d(G + d(δ · d(M_j + d(δ · d(0.5·I / h))))))` (`0.5·I` exact, then the binary64 division by `h`) |
  | **second derivative** | binary32 | `f(M_j + f(f(δ · I) / h))` |

  So the value is the spline's cubic on `[t_j, t_{j+1}]` in nested form,
  `y_j + δ·(S − (M_{j+1}/6 + M_j/3)·h + δ·(M_j/2 + δ·(M_{j+1} − M_j)/(6h)))`, with `S`, `h`, `δ` and
  `I` rounded to binary32 first and everything after them in binary64, rounded to binary32 once at the
  end; the second derivative is computed entirely in binary32.

## 6. FreeSurfer's wrappers, and special values

- `OpenSpline(t, y, n, d₀, d₁, out)`: §3 with `ibcbeg = ibcend = 1`, `ybcbeg = d₀`, `ybcend = d₁`;
  `out` receives the `n` second derivatives. FreeSurfer's callers (`mri_normalize`'s spline
  normalisation, for float and for 16-bit images) pass `d₀ = d₁ = 0` and at most 80 knots.
- `OpenSplint(t, y, M, n, x, out)`: `out` receives §5's value at `x`; the derivatives are discarded.
- **NaNs** follow x86-64's SSE rules as in `specs/ranlux.md` §7 (a NaN made from numbers is the
  default NaN, binary32 bits `0xffc00000`; one NaN operand is passed on quieted; of two, the first of
  the pair). Where two NaN operands can meet, the first is the left operand as written in §3 and §4,
  except §4's two products with the super-diagonal: `m · U_{i−1}` (step 3), which is `U_{i−1} · m`,
  and `U_i · x_{i+1}` (step 4), which is `x_{i+1} · U_i` **(observed**, 2026-11-13: `fs_spline_probe
  d3` on a system whose `U₀` and `x₁` are NaNs of different payloads, `2 00000000 40000000 40400000
  7fc00111 40a00000 00000000 40e00000 7fc00222` (`U₀` = `7fc00111`, `b₁` = `7fc00222`), gives `x
  7fc00222 7fc00222`: `x₀` carries `x₁`'s payload, not `U₀`'s; the same on 300 seeded systems with
  NaNs of six payloads in `U`, `L`, `D` and `B`**)**; in §5 the operations after `K` take the nested
  part first: for the value `d(K·δ)`, then `+ 0.5·M_j`, `· δ`, `+ G`, `· δ`, `+ y_j`; for the first
  derivative `d(0.5·I / h) · δ`, then `+ M_j`, `· δ`, `+ G`; for the second `f(δ·I) / h`, then `+ M_j`
  **(observed**: the NaN cases of §9.1, values with two different payloads**)**. A binary64 NaN rounded
  to binary32 keeps its sign and the top 22 bits of its payload, quieted. Infinite inputs follow IEEE
  arithmetic.

## 7. Messages and exit status

Each fatal error writes its lines to **standard output** and ends the program with **exit status 1**
(no partial result). The numbers are written as C's `printf("%d")` for integers and `printf("%g")` of
the binary32 value converted to binary64 for knots (six significant digits: `1`, `1.5`,
`1.4013e-45`, `inf`, `-inf`, `nan`, `-nan`). Each message starts with an empty line:

| Error | Text (each line ends in a line feed) |
|---|---|
| E1 | (empty line) / `SPLINE_CUBIC_SET - Fatal error!` / `  The number of data points N must be at least 2.` / `  The input value is <n>.` |
| E2 | (empty line) / `SPLINE_CUBIC_SET - Fatal error!` / `  The knots must be strictly increasing, but` / `  T(<i>) = <t_i>` / `  T(<i+1>) = <t_{i+1}>` (`i` from 0) **(observed**: `T(1) = 1` / `T(2) = 1` for knots 0, 1, 1**)** |
| E3 | (empty line) / `SPLINE_CUBIC_SET - Fatal error!` / `  IBCBEG must be 0, 1 or 2.` / `  The input value is <ibcbeg>.` |
| E4 | (empty line) / `SPLINE_CUBIC_SET - Fatal error!` / `  IBCEND must be 0, 1 or 2.` / `  The input value is <ibcend>.` |
| E5 | (empty line) / `SPLINE_CUBIC_SET - Fatal error!` / `  The linear system could not be solved.` |

The lines share standard output with the program's own `printf` output, in program order. A library
implementation returns these as errors carrying the same text; the programs (`mri_normalize`'s port)
print it and exit with status 1. Through FreeSurfer's callers only E2 and E5 can occur (E1 is
excluded by `mri_normalize`'s own check of at least two peaks; the conditions are always 1).

## 8. Defects, observations and the `reproduce` switch

- **FS-103: a NaN knot passes the check.** The strictly-increasing test is false for NaN, so a NaN
  knot is accepted and the spline's values become NaN (and `mri_normalize`'s conversion of the
  scaled intensities to integers is then undefined). **Rule B** (a NaN knot makes no sense, and the
  original goes on silently): by default the clean room reports E2 for the first `i` where
  `t_{i+1} > t_i` is not true (so a NaN knot gives E2 with `nan` printed); `reproduce`: the
  original's acceptance and NaN results. No program on `recon-all`'s paths gives a NaN knot (the
  knots are row positions); the case exists for completeness.
- **Kept (rule C), documented:** the evaluation gives the end value outside the knots (FreeSurfer's
  documented choice: "areas outside of where the control points are defined will be interpolated as
  the closest edge point"); the derivative outputs at the two ends (left: none written; right: the
  first derivative 0, the second not written) are used by no caller.
- Burkardt's original routines are binary64 and extrapolate; this spec is FreeSurfer's version, not
  Burkardt's.

## 9. Validation plan (black box)

Through the probe `fs_spline_probe` (built by `build.sh <workspace> <output> fs_spline_probe.cpp`
against the unit oracle's libraries, which reproduce the official programs' code) and the official
programs; suites run them through the oracle-output cache.

**The probe's formats** (every float as the 8 hexadecimal digits of its binary32 bits, fields
separated by blanks, one case per line):

- `fs_spline_probe cases`, standard input: `n ibcbeg ybcbeg ibcend ybcend m t₀ … t_{n−1} y₀ … y_{n−1}
  x₀ … x_{m−1}` (`n`, `ibcbeg`, `ibcend`, `m` in decimal; `ybcbeg`, `ybcend`, the knots `t`, the
  values `y` and the `m` evaluation points `x` as bits). Output per case: a line `ypp` and the `n`
  second derivatives of `SplineCubicSet(n, t, y, ibcbeg, ybcbeg, ibcend, ybcend)`; then one line per
  evaluation point, `val` and three floats: the value, the first and the second derivative written by
  `SplineCubicValue` (the two derivative outputs start as `7fc00bad`, so an output left unwritten
  shows); then a line `wrap` with the `n` second derivatives of `OpenSpline(t, y, n, ybcbeg, ybcend)`
  (always first-derivative ends: `ibcbeg` and `ibcend` do not apply) followed by `OpenSplint`'s `m`
  values at the same points.
- `fs_spline_probe d3`, standard input: `n a₀ … a_{3n−1} b₀ … b_{n−1}` (`n` decimal; the band of
  §2's `A` column by column, three entries per column: `a_{3j} = U_{j−1}`, `a_{3j+1} = D_j`,
  `a_{3j+2} = L_{j+1}`, so `a₀` and `a_{3n−1}` are never read and may be 0; then the right-hand side
  `B`; all as bits). Output: a line `x` and the `n` solution values, or `null` when it
  refuses (a zero diagonal entry).
- `fs_spline_probe set1 n ibcbeg ibcend t… y…` (decimal arguments, the end values 0): one call of
  `SplineCubicSet`, for the messages and the exit status of §7; prints `ypp` and the second
  derivatives when it returns.

1. **`cases`** (every float as its bits): seeded random cases, compared bit for bit (second
   derivatives, value, both derivative outputs with the sentinel `0x7fc00bad` showing unwritten
   outputs, and the wrappers): `n` from 2 to 80; knots strictly increasing (integer row positions as
   `mri_normalize` gives, random floats, nearly equal neighbours one unit in the last place apart,
   widely spread magnitudes); values random and constant; every pair of conditions 0, 1, 2 with
   random end values (and 0, 0 for the wrappers' case); evaluation points below, at and above every
   knot, between knots, ±0, ±∞, NaN; `n = 2` with conditions (0, 0); one case in 50 with two NaN
   values of different payloads and a NaN beginning value, one in 77 with an infinite value (§6).
   **(observed**, 2026-11-12: the spec writer's model of this spec equals the probe on 20,001 seeded cases (400 of them with NaN values), every evaluation point of each, 1,000 systems of item 2 and the nine messages of item 3, bit for bit and byte for byte**)**.
2. **`d3`**: systems with a zero (+0, −0) diagonal entry (refused), with a diagonal that becomes 0
   during the elimination, and random diagonally dominant and non-dominant systems; and systems
   whose super-diagonal, sub-diagonal, diagonal and right-hand side hold NaNs of different payloads,
   so that the products of steps 3 and 4 meet two NaNs (§6's operand order; the example there).
3. **Messages** (`set1`): E1 to E5 (E5 through two knots one subnormal step apart, `2⁻¹⁴⁹`, whose
   `Δt/3` rounds to 0 under condition 1), standard output byte for byte and the exit status.
4. **`mri_normalize`**: the unit oracle's `mri_normalize` (track B: the official program's bytes but
   for the build ID) and the official one, through the replay of the recorded heads
   (`<workspace>/oracles/freesurfer-8.2.0/runs/`, both streams' normalisation steps) once
   `larmorx-freesurfer`'s `mri_normalize` uses the clean room: outputs identical.
5. **Golden cases**: one per path (each condition pair, `n = 2` with (0, 0), the three evaluation
   branches), hashed outputs, no image needed.
