# Spec: glibc 2.35's double `sin`, `cos` and `sincos` with glibc's bits (x86-64), and the black-box record of `tan`, `asin`, `acos` and `atan`

(spec writer, 2026-11-04) The input for a clean-room implementation, in `larmorx_core::math`
(Apache-2.0), of the double-precision `sin`, `cos` and `sincos` that FSL's programs get from the
C library on Linux x86-64 (glibc 2.35, Ubuntu 22.04, the oracle machines), bit for bit.

**Why.** glibc's double `sin`, `cos`, `sincos`, `tan`, `asin`, `acos` and `atan` are not
correctly rounded (GLIBC-07, `docs/findings/upstream-defects.md`; the shares are in §10.3). The
default of every larmorx implementation stays correctly rounded (`larmorx_core::math`'s CORE-MATH
ports); the user decided on 2026-11-04 that `reproduce` gives glibc's own bits (decision log,
"glibc's exact maths for `reproduce`") and then, once this spec had measured the tables (decision
log, "GLIBC-07's tables"), that this holds for `sin`, `cos` and `sincos`, while `tan`, `asin`,
`acos` and `atan` stay correctly rounded in `reproduce` too (§9). glibc's code for these seven functions is IBM's Accurate
Mathematical Library, LGPL-2.1-or-later: it is never copied or translated. This spec describes
the behaviour as mathematics, from which another agent writes the functions (the `cbrtf` route,
`specs/cbrtf.md`).

**What this spec covers.** `sin`, `cos` and `sincos` completely (§1 to §8), with the table of §5
generated from its mathematical definition, as the user decided (§5.3: 18 of its 440 numbers are
not glibc's, a documented gap that changed no result tested). `tan`, `asin`, `acos` and `atan` are
**not specified**: their tables are data that no definition reproduces, and the user decided that
they stay correctly rounded in `reproduce` too, so that no glibc data enters larmorx (§9). The
black-box record (§10) covers all seven functions: for the four, it measures how often
`reproduce`'s correctly rounded result differs from glibc's (§10.3).

**Whoever implements it must not open:** glibc's source (`<workspace>/tags/glibc-2.35/`, any
other copy), its disassembly, the spec writer's model (`crates-gpl/tools/glibc-dbl-model`, written
by someone who had read glibc's source), `crates-gpl/` and `crates-nc/` in general, `docs/parts/`.
It may read this spec, `validation/investigations/glibc-dbl/README.md` and `families.rs`, the
record (`glibc-dbl-2.35-x86_64-blocks.json`), the numerical literature (Cody and Waite's
reduction, Payne and Hanek's, table-driven sine and cosine), and run the system's functions, the
black-box tool (`blackbox/`) and `tables.py` (without `--print`, which lists glibc's numbers) as
black boxes. What black-box runs confirmed is marked **(observed)**.

---

## 1. Notation and arithmetic

- Every value is an IEEE 754 **binary64** (double); `fl(v)` is the rounding of a real `v` to the
  nearest double, **ties to even**. Every operation written `fl(a + b)`, `fl(a − b)`,
  `fl(a · b)`, `fl(a / b)` is one IEEE operation.
- `FMA(a, b, c)` is the **fused multiply-add** `fl(a · b + c)`: one rounding of the exact
  `a · b + c`. Where a step writes `FMA(−a, b, c)`, it is `fl(c − a · b)`, rounded once.
- **The two readings of `FMA`.** glibc has two builds of the method (§2):
  - in `sin` and `cos` (the build glibc runs on a CPU with FMA), every `FMA` is fused, as above;
  - in `sincos` (its only build), every `FMA(a, b, c)` is read as `fl(fl(a · b) + c)`: the
    product rounded, then the sum. Everything else is the same.
  
  Operations not written as `FMA` are never fused, in either build.
- No wider intermediate format (no x87 extended precision), no reassociation: the operations
  are those listed, in the order listed.
- `|x|`, `−x`, `copysign(v, x)` (the magnitude of `v` with the sign of `x`) are exact sign
  operations; a multiplication by a power of two that stays in the normal range is exact.
- The computation is in round to nearest whatever the caller's rounding mode (glibc sets it for
  the call; larmorx always computes in round to nearest). Flush-to-zero and denormals-are-zero are
  off.
- **The limits are tests on the magnitude.** "`|x| < L`" compares the double `|x|` with the
  double `L` (all limits below are doubles; each is a test on the high 32 bits of `|x|`'s pattern
  with the low 32 bits of `L` zero, which is the same thing).
- **No `errno`.** glibc sets `errno` to `EDOM` for an infinite argument; larmorx's functions only
  return the value.

## 2. Which build glibc runs (observed)

On x86-64, glibc 2.35 chooses the build of `sin` and of `cos` when the program starts: on a CPU
where FMA and AVX2 are usable (every x86-64 CPU since 2013, the oracle machines too) it runs a
build with fused multiply-adds; elsewhere one without. `sincos` has **one** build on every x86-64
CPU, without fused operations. "glibc's bits" are those of the FMA build of `sin` and `cos`
(the orchestrator's decision, 2026-11-03) and of `sincos`'s single build:

- the record (§10) was made on a CPU with FMA and AVX2 (`"cpu_fma_avx2": true`);
- with glibc's FMA builds switched off (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`), `sin`
  and `cos` are this spec's method with the second reading of `FMA` (§1) on every argument of
  the record's families (§11.2); `sincos` is that build's `sin` and `cos` except in
  `[0.85546875, L1)`, where it takes the head-and-tail pair of §7.4 (the same results on
  2^26 arguments of each of two families, `validation/probes/glibc-double-maths`, 2026-11-03
  **(observed)**).

## 3. The constants

The constants are doubles; each is given by its bits and, where it has one, its definition
(proved by `validation/investigations/glibc-dbl/tables.py`, §11.1).

**Multiples and parts of π/2:**

| | Definition | Bits | Hexadecimal |
|---|---|---|---|
| `H0` | `fl(π/2)` | `0x3ff921fb54442d18` | `0x1.921fb54442d18p+0` |
| `H1` | `fl(π/2 − H0)` | `0x3c91a62633145c07` | `0x1.1a62633145c07p-54` |
| `G` | `fl(2/π)` | `0x3fe45f306dc9c883` | `0x1.45f306dc9c883p-1` |
| `M1` | `π/2` rounded to nearest with 26 significant bits | `0x3ff921fb58000000` | `0x1.921fb58p+0` |
| `M2` | `π/2 − M1` rounded to nearest with 27 significant bits | `0xbe4dde973c000000` | `-0x1.dde973cp-27` |
| `M3` | `π/2 − M1 − M2` rounded to nearest with 26 significant bits | `0xbc8cb3b398000000` | `-0x1.cb3b398p-55` |
| `M4` | `fl(π/2 − M1 − M2 − M3)` | `0xbacd747f23e32ed7` | `-0x1.d747f23e32ed7p-83` |
| `M2L` | `π/2 − M1` rounded to nearest with 23 significant bits | `0xbe4dde9740000000` | `-0x1.dde974p-27` |

(The differences in the definitions are exact real numbers: `π/2 − M1` is the real `π/2` minus
the double `M1`.)

**Shifters and scales:** `K52 = 1.5 · 2^52` (`0x4338000000000000`), `K54 = 1.5 · 2^54`
(`0x4358000000000000`), `C27 = 2^27 + 1` (134217729), and the powers of two `2^-600`, `2^576`,
`2^-24`.

**The digits of 2/π.** `D_j`, `j = 0 … 74`: the `j`-th base-2^24 digit of the fraction of 2/π,
`D_j = ⌊(2/π) · 2^(24(j+1))⌋ mod 2^24`, each held exactly by a double (`D_0 = 10680707`,
`D_1 = 7228996`, `D_74 = 343717`). `tables.py` generates them.

**The polynomial coefficients**, bit for bit: `T3` … `T11` multiply `x^3, x^5, …, x^11` in the
sine of a small number (§6.1); `U3`, `U5` are the table-driven sine's (§6.2, §6.3), `V2`, `V4`,
`V6` the table-driven cosine's. They are near the Taylor coefficients but (except `T3 = fl(−1/6)`
and `V2 = 1/2`) not equal to their rounded values: use these bits.

| Name | Bits | Hexadecimal | Near |
|---|---|---|---|
| `T3` | `0xbfc5555555555555` | `-0x1.5555555555555p-3` | −1/6 |
| `T5` | `0x3f81111111110ece` | `0x1.1111111110ecep-7` | 1/120 |
| `T7` | `0xbf2a01a019db08b8` | `-0x1.a01a019db08b8p-13` | −1/5040 |
| `T9` | `0x3ec71de27b9a7ed9` | `0x1.71de27b9a7ed9p-19` | 1/362880 |
| `T11` | `0xbe5addffc2fcdf59` | `-0x1.addffc2fcdf59p-26` | −1/39916800 |
| `U3` | `0xbfc5555555555515` | `-0x1.5555555555515p-3` | −1/6 |
| `U5` | `0x3f811110e829872f` | `0x1.11110e829872fp-7` | 1/120 |
| `V2` | `0x3fe0000000000000` | `0x1p-1` | 1/2 |
| `V4` | `0xbfa5555555555535` | `-0x1.5555555555535p-5` | −1/24 |
| `V6` | `0x3f56c16bedd9e239` | `0x1.6c16bedd9e239p-10` | 1/720 |

**The limit `τ`** of §6.1: the double nearest 0.126, `0x3fc020c49ba5e354`
(`0x1.020c49ba5e354p-3`, exactly 1134907106097365 / 2^53).

## 4. The paths

`x` is the argument. For `sin` and `cos`:

| `|x|` | `sin(x)` | `cos(x)` |
|---|---|---|
| NaN or ±∞ | `fl(x / x)` (§4.1) | `fl(x / x)` |
| `< 2^-27` | `x` | `1` |
| `[2^-27, 2^-26)` | `x` | §7.1 |
| `[2^-26, 0.85546875)` | §7.1 | §7.1 |
| `[0.85546875, L1)` | §7.2 | §7.2 |
| `[L1, L2)` | §7.3 with the reduction of §8.1 | §7.3 with §8.1 |
| `[L2, ∞)` | §7.3 with the reduction of §8.2 | §7.3 with §8.2 |

with `L1 = 0x1.368fd00000000p+1` = 2.4262638092041015625 (bits `0x400368fd00000000`) and
`L2 = 0x1.921fb00000000p+26` = 105414336 (bits `0x419921fb00000000`). 0.85546875 is
`0x1.b6p-1`. `sincos` has its own paths (§7.4).

### 4.1 Special values

- `±0`: `sin(±0) = ±0`, `cos(±0) = 1`, `sincos(±0) = (±0, 1)`.
- `±∞`: every result is the x86-64 **default NaN**, bits `0xfff8000000000000` (the result of the
  IEEE division ∞/∞ on x86-64) **(observed)**.
- NaN: the result is the argument with its quiet bit (bit 51) set, sign and payload kept
  (`0x7ff4000000005678` gives `0x7ffc000000005678`, `0xfff8000000001234` gives itself)
  **(observed)**: the x86-64 result of `x / x` for a NaN `x`. Both outputs of `sincos` alike.
- Subnormal and tiny arguments: `sin(x) = x`, `cos(x) = 1` (the path `< 2^-27`); no other path
  meets a subnormal.

## 5. The table `R` of sines and cosines of N/128

### 5.1 Definition

110 rows, `N = 0 … 109`; row `N` holds four doubles:

- `SH[N] = fl(sin(N/128))`, `SL[N] = fl(sin(N/128) − SH[N])`;
- `CH[N] = fl(cos(N/128))`, `CL[N] = fl(cos(N/128) − CH[N])`.

(`SH[N] + SL[N]` and `CH[N] + CL[N]` are the correctly rounded double-double values of `sin`
and `cos` of `N/128`; row 0 is `(0, 0, 1, 0)`.)

### 5.2 Generator

Any arbitrary-precision library: with 400 bits of working precision, compute `s = sin(N/128)`,
round it to the nearest double (ties to even) for `SH`, then round `s − SH` to the nearest double
for `SL`; the same for `cos`. `tables.py`'s `sincos_table()` does it with `mpmath`
(`python -P validation/investigations/glibc-dbl/tables.py`).

### 5.3 What the definition reproduces (decided: the definition)

Compared with glibc's table, bit for bit (`tables.py`; its source text and the bytes of the
system's `libm.so.6`, which agree with each other): **422 of the 440 numbers agree**. Every
`SH` and `CH` agrees; **18 of the 220 low parts differ** (in 17 rows): 17 by one or two units in
their own last place, one by 39 units (a low part 2^-11 of its high part's last place, so with a
finer last place of its own). In every row, glibc's `high + low` is within
0.31 × 2^-53 of the high part's last place from the exact value: glibc's table was made with
about 108 bits, not 106 correctly rounded. **111 other rules were tried** (the value rounded to
`p` bits for `p` = 100 to 130, to nearest, toward zero and downward, then split; the value
rounded to 4 to 9 base-2^24 digits, the same three ways): none misses fewer than 17 numbers
(`tables.py --search`).

So the table of §5.1 is **not glibc's table** in 18 numbers. What that changes: a low part
enters the result through one fused operation of the correction (§6.2, `ρ`); a change of one
unit in its last place (about 2^-107 of the result) changes the result only where the exact
correction lies within that distance of a rounding boundary, about one evaluation in 2^53 of
those that use those 18 rows. **On the record's 3.2 × 10^9 arguments of `sin`, `cos` and
`sincos`, the method with §5.1's table gives glibc's bits on every one** (§11.2); and the method
gives the same results with §5.1's table as with glibc's own 18 numbers in their place on those
arguments and on 1.8 × 10^10 more of each function chosen next to the 17 rows (§11.3).

**Decided** (the user, 2026-11-04, decision log "GLIBC-07's tables", of three options: §5.1's
table, glibc's 18 numbers in its place, or `sin`, `cos` and `sincos` correctly rounded in
`reproduce` too): **the implementation generates table `R` from §5.1's definition** (§5.2), and no
number from glibc's code enters larmorx. The gap this leaves is documented, not hidden:

- in 18 low parts (17 rows) the table is not glibc's, so a result can differ from glibc's where
  the exact correction of §6.2 lies within about 2^-107 of a rounding boundary: an estimated one
  evaluation in about 2^53 (about 10^16) of those that read these rows;
- **none is known**: 0 differences on the record's 3.2 × 10^9 arguments and on 1.8 × 10^10 more
  per function chosen next to the 17 rows (§11.3);
- the implementation's documentation says so (`reproduce` gives glibc's bits on every argument
  tested, with this one theoretical exception); a difference from glibc found later is reported
  with its argument (§12), and a validation agent tells with the spec writer's model (§11.3)
  whether it is this gap (the model gives glibc's result with glibc's 18 numbers and the
  implementation's with §5.1's) or a mistake to fix.

## 6. The three kernels

Each kernel evaluates the sine or the cosine of a number given as a head `x` and a tail `d`
(their exact sum is the argument; `|d|` is at most about half a unit in the last place of `x`,
and `d` may be zero). `FMA` has the reading of §1 for the build in question.

### 6.1 The sine of a small number, `S0(x, d)` (for `|x| < τ`)

1. `z = fl(x · x)`
2. `P = FMA(z, FMA(z, FMA(z, FMA(z, T11, T9), T7), T5), T3)` (the innermost first)
3. `h = fl(0.5 · d)`
4. `e = FMA(z, FMA(P, x, −h), d)`, the inner `FMA(P, x, −h)` first
5. the result is `fl(x + e)`

### 6.2 The table-driven sine, `S(x, d)`

If `|x| < τ`, `S(x, d) = S0(x, d)`. Otherwise:

1. `d' = d` if `x > 0`, `d' = −d` otherwise; `y = |x|`
2. `N` = the integer nearest to `128 · y`, ties to even (`128 · y` is exact; `N ≤ 109`
   wherever `S` is used); `X = y − N/128` (exact)
3. `(σ, σ', κ, κ') = (SH[N], SL[N], CH[N], CL[N])`
4. `z = fl(X · X)`
5. `p = FMA(z, U5, U3)`
6. `s = fl(X + FMA(fl(X · z), p, d'))`
7. `q = FMA(z, FMA(z, V6, V4), V2)`
8. `c = FMA(X, d', fl(z · q))`
9. `ρ = FMA(s, κ, FMA(−c, σ, FMA(s, κ', σ')))`, the innermost first
10. the result is `copysign(fl(σ + ρ), x)`

### 6.3 The table-driven cosine, `C(x, d)`

1. `d' = −d` if `x < 0`, `d' = d` otherwise; `y = |x|`
2. `N` = the integer nearest to `128 · y`, ties to even; `X = fl((y − N/128) + d')` (the
   difference is exact; the sum is one rounding)
3. `(σ, σ', κ, κ') = (SH[N], SL[N], CH[N], CL[N])`
4. `z = fl(X · X)`
5. `p = FMA(z, U5, U3)`
6. `s = FMA(fl(X · z), p, X)`
7. `q = FMA(z, FMA(z, V6, V4), V2)`
8. `c = fl(z · q)`
9. `ρ = FMA(−s, σ, FMA(−c, κ, FMA(−s, σ', κ')))`, the innermost first
10. the result is `fl(κ + ρ)`

(`C` has no small-argument branch: it is used for every `|x| < 0.8555` it meets, `x = 0`
included.)

## 7. `sin`, `cos` and `sincos`

### 7.1 `|x|` below 0.85546875

`sin(x) = S(x, +0)` (from `2^-26`), `cos(x) = C(x, +0)` (from `2^-27`). The tail is `+0`; with
`x ≤ 0` the sine's step 1 makes it `−0`, which is what the steps then use.

### 7.2 `|x|` in `[0.85546875, L1)`: near π/2

- `sin(x)`: `t = fl(H0 − |x|)`; `sin(x) = copysign(C(t, H1), x)`.
- `cos(x)`: `y = fl(H0 − |x|)`, `a = fl(y + H1)`, `da = fl(fl(y − a) + H1)`;
  `cos(x) = S(a, da)`.

### 7.3 From `L1`: by quadrant

The reduction (§8.1 below `L2`, §8.2 from `L2`) gives `x ≈ k π/2 + (a + da)` with `|a + da|`
at most about π/4, and the quadrant `n = k mod 4` (0 to 3). Then:

- `sin(x)`: `v = C(a, da)` if `n` is odd, `v = S(a, da)` if even; `sin(x) = −v` if `n` is 2 or
  3, `v` otherwise.
- `cos(x)`: the same with `m = (n + 1) mod 4` in place of `n`: `v = C(a, da)` if `m` is odd,
  `S(a, da)` if even; `cos(x) = −v` if `m` is 2 or 3.

### 7.4 `sincos(x)`: its own paths (every `FMA` read as `fl(fl(a · b) + c)`)

| `|x|` | sine, cosine |
|---|---|
| NaN or ±∞ | `fl(x / x)`, `fl(x / x)` (§4.1) |
| `< 2^-27` | `x`, `1` |
| `[2^-27, 0.85546875)` | `S(x, +0)`, `C(x, +0)` |
| `[0.85546875, L1)` | `y = fl(H0 − |x|)`, `a = fl(y + H1)`, `da = fl(fl(y − a) + H1)`: `copysign(C(a, da), x)`, `S(a, da)` |
| `[L1, ∞)` | below |

From `L1`: `(n, a, da)` from §8.1 if `|x| < L2`, from §8.2 otherwise. If `n` is 1 or 2,
`a = −a` and `da = −da`. Then with `s = S(a, da)` and `c = C(a, da)` (the negated `a`, `da` where
they were negated):

| `n` | sine | cosine |
|---|---|---|
| 0 | `s` | `c` |
| 1 | `c` | `s` |
| 2 | `s` | `−c` |
| 3 | `−c` | `s` |

Note the differences from `sin` and `cos`: in `[0.85546875, L1)` the sine is `C(a, da)` with the
head-and-tail pair `(a, da)`, where `sin` takes `C(t, H1)`; and the whole of `sincos` reads every
`FMA` as two roundings.

## 8. The reductions

### 8.1 Below `L2` (in `sin` and `cos` with fused `FMA`, in `sincos` with two roundings)

For `x` with `L1 ≤ |x| < L2` (the signed `x`):

1. `t = FMA(x, G, K52)`
2. `k = fl(t − K52)`: an integer (exact); `n = k mod 4` (0 to 3; for a negative `k` the
   non-negative remainder)
3. `y = FMA(−k, M1, x)`, then `y = FMA(−k, M2, y)`
4. `t2 = FMA(−k, M3, y)`; `e = FMA(−k, M3, fl(y − t2))`
5. `a = FMA(−k, M4, t2)`; `da = fl(e + FMA(−k, M4, fl(t2 − a)))`

The result is `(n, a, da)`.

### 8.2 From `L2` to the largest double (no fused operation in any build)

For finite `x` with `|x| ≥ L2` (the signed `x`; every operation one rounding, in this order):

1. `x' = x · 2^-600` (exact).
2. Split: `w = fl(x' · C27)`, `x1 = fl(w − fl(w − x'))`, `x2 = fl(x' − x1)` (so `x1 + x2 = x'`
   exactly, `x1` with at most 26 significant bits).
3. For each part `p` of `x1`, `x2` in turn, compute `(b_p, bb_p, Σ_p)`:
   1. `E` = the exponent field of `p` (bits 52 to 62 of its pattern, 0 for `p = 0`);
      `j0 = ⌊(E − 450)/24⌋` if `E ≥ 450`, else `j0 = 0`.
   2. For `i = 0 … 5`: `r_i = fl(fl(p · D_{j0+i}) · 2^(576 − 24 (j0 + i)))` (both products
      exact).
   3. `Σ = 0`; for `i = 0, 1, 2`: `u = fl(fl(r_i + K52) − K52)` (the integer nearest `r_i`,
      ties to even), `Σ = fl(Σ + u)`, `r_i = fl(r_i − u)`.
   4. `t = 0`; for `i = 5, 4, …, 0`: `t = fl(t + r_i)`.
   5. `bb = fl(fl(fl(fl(fl(fl(r_0 − t) + r_1) + r_2) + r_3) + r_4) + r_5)`.
   6. `u = fl(fl(t + K52) − K52)`; `Σ = fl(Σ + u)`; `t = fl(t − u)`.
   7. `b = fl(t + bb)`; `bb = fl(fl(t − b) + bb)`.
   8. `u = fl(fl(Σ + K54) − K54)` (the multiple of 4 nearest `Σ`); `Σ = fl(Σ − u)`.
   9. `(b_p, bb_p, Σ_p) = (b, bb, Σ)`.
4. `Σ = fl(Σ_1 + Σ_2)`; `b = fl(b_1 + b_2)`;
   `bb = fl(fl(b_1 − b) + b_2)` if `|b_1| > |b_2|`, else `bb = fl(fl(b_2 − b) + b_1)`
   (subscript 1 for `x1`'s, 2 for `x2`'s).
5. If `b > 0.5`: `b = fl(b − 1)`, `Σ = fl(Σ + 1)`; else if `b < −0.5`: `b = fl(b + 1)`,
   `Σ = fl(Σ − 1)`.
6. `s = fl(b + fl(fl(bb + bb_1) + bb_2))`; `t = fl(fl(fl(b − s) + bb) + fl(bb_1 + bb_2))`.
7. Split `s`: `w = fl(s · C27)`, `s1 = fl(w − fl(w − s))`, `s2 = fl(s − s1)`.
8. `B = fl(s · H0)`;
   `BB = fl( fl( fl( fl(fl(s1 · M1) − B) + fl(s1 · M2L) ) + fl(s2 · M1) ) + fl( fl( fl(s2 · M2L) + fl(s · H1) ) + fl(t · H0) ) )`.
9. `a = fl(B + BB)`; `da = fl(fl(B − a) + BB)`.
10. `n` = the integer `Σ` (a whole number of magnitude at most 4) modulo 4, non-negative.

The result is `(n, a, da)`.

## 9. `tan`, `asin`, `acos`, `atan`: correctly rounded in `reproduce` too (decided)

Each of the four takes, besides constants, a table of numbers. A behaviour spec can give a table
only by a definition that regenerates it (the brief; never glibc's numbers). `tables.py` tested
the natural definitions against glibc's tables **(observed**, 2026-11-04**)**:

| Function | The table | What a definition reproduces |
|---|---|---|
| `tan` | 186 rows: a point `x_i` near `i/256` (`i` = 16 to 201), then `F_i`, `FF_i`, `G_i` | `F_i = fl(tan x_i)`, `FF_i = fl(tan x_i − F_i)`, `G_i = fl(1/tan x_i)` on all 186 rows; but the points `x_i` are searched points (accurate-table points: `tan x_i` within 2^-10.4 of a double), up to 6.6 × 10^8 units in the last place from `i/256`: no rule found |
| `asin`, `acos` | 216 subintervals of [0.125, 0.96875), each a point and a polynomial (6 to 10 coefficients, a constant term in two parts) | the first-derivative coefficient is `fl(asin'(x0))` in 209 of 216, the second in 99, the higher ones in none (they are fitted, up to 10^13 units in the last place from the Taylor coefficients); the constant term approximates `asin(x0)` only to 2^-64: fitted data |
| `atan` | 241 rows on (1/16, 1): a point and 6 coefficients | `fl(atan x0)` and `fl(atan'(x0))` on all 241; the points are searched points and the higher coefficients are fitted |

So `tan` would need its 186 points, `asin` and `acos` their 2,568 numbers and `atan` 241 points
and 964 coefficients from glibc's code (about 4,000 numbers in all), data that only glibc's
LGPL source holds.

**Decided** (the user, 2026-11-04, decision log "GLIBC-07's tables"): **`tan`, `asin`, `acos`
and `atan` stay correctly rounded in `reproduce` too** (`larmorx_core::math`'s CORE-MATH ports `tan`,
`asin`, `acos`, and `atan(x)` as `atan2(x, 1)`), so no glibc data enters larmorx. Their behaviour
is therefore not specified here, and nothing is to be implemented for them. What it costs: in
`reproduce`, these four differ from glibc's only in the last bit, on the shares of §10.3 (0.05 to
0.25 % of results; `tan` 0.19 %, `asin` 0.08 %, `acos` 0.05 %, `atan` 0.07 % over all families),
and they are reached only by fslmaths' double `-tan`, `-asin`, `-acos` and `-atan`, never by
fMRIPrep. Their black-box record stays (§10): it measures those shares, and it would check an
implementation without reading glibc if the decision were ever revisited.

## 10. The black-box record

### 10.1 The arguments

Defined in `validation/investigations/glibc-dbl/families.rs` (Apache-2.0, plain integer and IEEE
arithmetic, the same on every platform), and here in words.

**Random numbers.** SplitMix64: for a 64-bit state `s`, `z = s + 0x9e3779b97f4a7c15`,
`z = (z XOR (z >> 30)) · 0xbf58476d1ce4e5b9`, `z = (z XOR (z >> 27)) · 0x94d049bb133111eb`,
output `z XOR (z >> 31)` (all modulo 2^64). The `i`-th number of family `f` of function `g` is
that output for the state `(g << 56) | (f << 44) | i`; `g` = 1 `sin`, 2 `cos`, 3 `sincos`,
4 `tan`, 5 `asin`, 6 `acos`, 7 `atan`. Each family has `2^27` arguments, `i = 0 … 2^27 − 1`.

**Maps from a random number `r` to an argument:**

- *bit patterns*: the double whose bits are `r`;
- *uniform on [a, a + w)*: `fl(a + fl(w · u))`, `u = (r >> 11) · 2^-53` (exact);
- *log-uniform magnitudes 2^e0 … 2^(e0+n−1)*: sign bit and the 52 low bits of `r`, exponent
  `e0 + (((r >> 52) AND 0x7ff) mod n)`;
- *float values*: the float (binary32) with `r`'s bit 31 and 23 low bits and the exponent
  `e0 + (((r >> 23) AND 0xff) mod n)`, as a double;
- *near the limits*: the limit `L[r mod m]` of the function's list (bit patterns) plus
  `((r >> 8) mod 2^25) − 2^24` added to its pattern as an integer, with `r`'s sign bit;
- *near the multiples of π/2*: `k = 1 + (r mod 2^28)`, `x0 = fl(k · H0)`, the pattern of `x0` plus
  `((r >> 28) mod 2^17) − 2^16`, with `r`'s sign bit.

**The families** (`id`: map):

| Function | Families |
|---|---|
| `sin`, `cos`, `sincos`, `tan` | 1 bit patterns; 2 log-uniform 2^-40 … 2^39 (`e0 = −40`, `n = 80`); 3 uniform on [−2π, 2π) (`a = −fl(2π)`, `w = 2 fl(2π)`); 4 uniform on [−2^27, 2^27); 5 near the multiples of π/2; 6 near the limits; 7 float values `e0 = −10`, `n = 27`; 8 log-uniform 2^26 … 2^1023 (`e0 = 26`, `n = 998`) |
| `asin`, `acos` | 1 bit patterns; 2 log-uniform 2^-60 … 2^-1; 3 uniform on [−1, 1); 4 `±(1 − m 2^e)`, `e = −53 … −3`, `m` in [1, 2) from `r`'s 52 low bits (`e = −53 + (((r >> 52) AND 0x7ff) mod 51)`, the difference rounded once, `r`'s sign bit); 5 near the limits; 6 float values `e0 = −24`, `n = 24` |
| `atan` | 1 bit patterns; 2 log-uniform 2^-60 … 2^59; 3 uniform on [−1, 1); 4 uniform on [−16, 16); 5 near the limits; 6 float values `e0 = −10`, `n = 27` |

The limits (positive bit patterns): `sin`, `cos`, `sincos`: `0x3e40000000000000`,
`0x3e50000000000000`, `0x3fc020c49ba5e354`, `0x3feb600000000000`, `0x400368fd00000000`,
`0x419921fb00000000`, `0x7ff0000000000000`; `tan`: `0x3e4b096c00000000`, `0x3faf212d00000000`,
`0x3fe92f1a00000000`, `0x4039000000000000`, `0x4197d78400000000`, `0x7ff0000000000000`; `asin`,
`acos`: `0x3c88…`, `0x3e50…`, `0x3fc0…`, `0x3fd0…`, `0x3fe0…`, `0x3fe8…`, `0x3fed8…`,
`0x3fee8…`, `0x3fef0…`, `0x3ff0…` (zeros after the digits shown); `atan`: `0x3e4bb67a00000000`,
`0x3fb0000000000000`, `0x3ff0000000000000`, `0x4030000000000000`, `0x43349ff200000000`,
`0x7ff0000000000000`.

**The special arguments** (`families.rs`, `special()`): 184 doubles, both signs of: 0, ∞, NaNs
(quiet and signalling, with payloads), the smallest and largest subnormals, the smallest normal,
the largest double, 1, 0.5, `fl(π/2)`, `fl(π)`, `fl(π/4)`, `fl(3 fl(π)/2)`, `fl(2π)`, 10^22,
6381956970095103 · 2^797, 2^52, 2^53, 2^1023, and every limit above with its two neighbours.

### 10.2 The hashes

For each family, the results in argument order are hashed in **blocks of 2^22**: start
`h = 14695981039346656037`; for each result `h = (h XOR bits) · 1099511628211 mod 2^64`, the
result's 64 bits as they are (NaN results included, no canonical NaN); for `sincos`, the sine's
bits, then the cosine's, for each argument. A family's hash is the same hash over its 32 block
hashes; a function's, over its families' hashes. The special arguments' results are listed one by
one.

The record: `validation/investigations/glibc-dbl/glibc-dbl-2.35-x86_64-blocks.json`
**(observed**, 2026-11-04, glibc 2.35 on x86-64 with FMA and AVX2, the system's `libm.so.6`
through `dlopen`, `blackbox/`, 14.8 s on 28 threads; 1,600 block hashes, 6.7 × 10^9
arguments**)**. The functions' hashes:

| Function | Hash | Families × arguments |
|---|---|---|
| `sin` | `f56e61144bf7fe10` | 8 × 2^27 |
| `cos` | `40834730f664179a` | 8 × 2^27 |
| `sincos` | `0c685a3d03758470` | 8 × 2^27 |
| `tan` | `6cc64fe9da26e23f` | 8 × 2^27 |
| `asin` | `f75145e4d880c002` | 6 × 2^27 |
| `acos` | `a0f0782edc53b8af` | 6 × 2^27 |
| `atan` | `65e6b0b4fdc4569c` | 6 × 2^27 |

`blackbox list <function> <family> <first> <count>` prints arguments and glibc's results inside
a block, to find where an implementation differs.

### 10.3 How often glibc differs from the correctly rounded value (GLIBC-07)

Against `larmorx_core::math`'s correctly rounded functions (`atan(x)` as `atan2(x, 1)`), results
that differ (NaN results counted equal), of 2^27 arguments per family **(observed**, the same
run**)**:

| Family | `sin` | `cos` | `sincos` (sine, cosine) | `tan` |
|---|---|---|---|---|
| 1 bit patterns | 0.0695 % | 0.0693 % | 0.0752 %, 0.0742 % | 0.1254 % |
| 2 log-uniform 2^-40 … 2^39 | 0.0803 % | 0.0714 % | 0.0865 %, 0.0765 % | 0.1423 % |
| 3 uniform on [−2π, 2π) | 0.1383 % | 0.1379 % | 0.1487 %, 0.1480 % | 0.2502 % |
| 4 uniform on [−2^27, 2^27) | 0.1378 % | 0.1379 % | 0.1485 %, 0.1479 % | 0.2496 % |
| 5 near multiples of π/2 | 0.0001 % | 0.0001 % | 0.0001 %, 0.0001 % | 0.0001 % |
| 6 near the limits | 0.1152 % | 0.0877 % | 0.1218 %, 0.0933 % | 0.2705 % |
| 7 float values (intensities) | 0.1208 % | 0.0951 % | 0.1302 %, 0.1019 % | 0.2139 % |
| 8 log-uniform 2^26 … 2^1023 | 0.1381 % | 0.1383 % | 0.1486 %, 0.1480 % | 0.2491 % |
| all | 0.1000 % | 0.0922 % | 0.1075 %, 0.0987 % | 0.1876 % |

| Family | `asin` | `acos` | | Family | `atan` |
|---|---|---|---|---|---|
| 1 bit patterns | 0.0009 % | 0.0001 % | | 1 bit patterns | 0.0009 % |
| 2 log-uniform 2^-60 … 2^-1 | 0.0309 % | 0.0038 % | | 2 log-uniform 2^-60 … 2^59 | 0.0152 % |
| 3 uniform on [−1, 1) | 0.1588 % | 0.0707 % | | 3 uniform on [−1, 1) | 0.2015 % |
| 4 near ±1 | 0.0090 % | 0.0383 % | | 4 uniform on [−16, 16) | 0.0685 % |
| 5 near the limits | 0.1922 % | 0.1955 % | | 5 near the limits | 0.0329 % |
| 6 float values 2^-24 … 2^-1 | 0.0722 % | 0.0088 % | | 6 float values (intensities) | 0.0715 % |
| all | 0.0773 % | 0.0529 % | | all | 0.0651 % |

(Bit patterns are mostly huge arguments or NaNs for `asin`, `acos`, `atan`, whose results are then
exact; near the multiples of π/2 the results are tiny and both functions agree.)

## 11. The proof of this spec

### 11.1 Its numbers

`python -P validation/investigations/glibc-dbl/tables.py` regenerates the digits `D_j` and the
constants of §3 from their definitions and compares them with glibc's: all agree; table `R`:
422 of 440 (§5.3).

### 11.2 A model from this text alone

The spec writer wrote a model of `sin`, `cos` and `sincos` step by step from §3 to §8 of this
text (`crates-gpl/tools/glibc-dbl-model`, Rust; in the GPL workspace because its author had read
glibc's source; the implementer must not open it), with table `R` and the digits `D_j` from
`tables.py --emit` (the definitions alone, no glibc file read), and ran it on the record's
arguments **(observed**, 2026-11-04, 28 threads, 6.6 s**)**:

| Function | Block hashes agreeing | Special arguments agreeing |
|---|---|---|
| `sin` | 256 of 256 (8 families × 2^27 arguments) | 184 of 184 |
| `cos` | 256 of 256 | 184 of 184 |
| `sincos` (both outputs) | 256 of 256 | 184 of 184 |

`MODEL-PASS`: glibc's bits on all 3.2 × 10^9 arguments and every special one, NaN bits included.
With the second reading of `FMA` in `sin` and `cos` (`--sse2`), against a record of glibc's
build without FMA (`glibc-dbl-2.35-x86_64-sse2-blocks.json`, the same families, made with
`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`; function hashes `sin` `1e528d6d7a721e72`, `cos`
`ab86b037da025ab2`; that build differs from the correctly rounded value on 0.1072 % and
0.0990 % of the arguments): 512 of 512 blocks and 184 of 184 special arguments agree too.

### 11.3 The 18 numbers of §5.3

The same model with glibc's table `R` in place of the definition's (read at run time from glibc's
source, in the GPL tool) **(observed**, 2026-11-04**)**:

- on the record's arguments: `MODEL-PASS` as well, and the two tables give the same result on
  every one of the 3 × 2^30 arguments of `sin`, `cos` and `sincos`;
- next to the 17 rows where the tables differ: for each row `N` and each function, 2^30 arguments
  `±(N/128 + u)` with `u` uniform on [−1/256, 1/256) (the arguments whose table-driven step reads
  that row directly): 0 of 1.8 × 10^10 results differ, for each of `sin`, `cos` and `sincos`.

A difference needs the exact correction within about 2^-107 of a rounding boundary (§5.3), about
one argument in 2^53 of those reading the 18 numbers: none was found, and none is expected among
the doubles the direct paths give these rows (about 2^48 per row); among the far more numerous
arguments that reach these rows through a reduction, some may exist.

## 12. Validation plan (the implementation)

1. **The record.** The implementation's test tool includes `families.rs`, computes the block
   hashes of §10.2 from its own `sin`, `cos` and `sincos` and compares them with the record's 768
   blocks of these three functions: every one must agree (§5.3's gap is not met on them: with
   §5.1's table the method gives glibc's bits on all of them, §11.2); a block that differs is
   listed with `blackbox list` and compared value by value. The special arguments' results must
   agree one by one, NaN bits included. Table `R` is generated from §5.1's definition (§5.2), at
   build time or as constants written by the generator with its command recorded, never taken
   from glibc.
2. **The two readings of `FMA`.** `sin` and `cos` with the fused reading against the record;
   `sincos` with two roundings. In Rust: `f64::mul_add` for the fused reading (correctly rounded
   on every platform, with or without FMA hardware; for speed, `larmorx_core::math`'s
   `glibc_dispatch!` runs the same code compiled with FMA where the CPU has it, as the copies of
   glibc's permissive functions do), and plain `*` then `+` for the other; never `mul_add` in
   `sincos`, never a plain product-and-sum where §6 to §8 write `FMA` for `sin` and `cos`. Both
   readings can be checked: the record of glibc's build without FMA
   (`glibc-dbl-2.35-x86_64-sse2-blocks.json`) is `sin` and `cos` with two roundings (§11.2).
3. **NaNs on every platform.** §4.1's NaN bits are x86-64's; on other CPUs `x / x` gives other
   NaN bits, so the implementation makes x86-64's explicitly (`larmorx_core::math`'s `x86_nan`,
   as the copies do).
4. **Every platform.** The method uses IEEE basic operations and fused multiply-adds only, so it
   gives the same bits on the six platforms; golden cases (rule 10) on §4.1's values and a few
   hundred seeded arguments of each function, recorded on Linux x64.
5. **Independently of the implementer** (the orchestrator or a validation agent): the
   implementation against the spec writer's model (§11.2) on the record's arguments and on 10^10
   more, and against glibc itself on fresh seeds.
6. **The documentation** of the three functions says what `reproduce` gives: glibc 2.35's bits on
   every argument tested, with §5.3's theoretical exception; and that `tan`, `asin`, `acos` and
   `atan` stay correctly rounded in `reproduce` (§9), with §10.3's shares.
7. **In use** (the second half): which records move when `reproduce` takes these functions (the
   handoff lists them).
