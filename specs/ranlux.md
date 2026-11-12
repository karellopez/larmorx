# Spec: RANLUX389 and its distributions, as FreeSurfer 8.2.0 draws them (`ranlux`)

(spec writer, track D of FreeSurfer's foundations, relay 1, 2026-11-12) The input for a clean-room
implementation, in the main package (`larmorx-core`, Apache-2.0), of the random generator and the
random distributions FreeSurfer 8.2.0 draws from: Lüscher's subtract-with-borrow generator at
luxury 389 ("RANLUX389"), seeded by James's method, its uniform deviates and eight distributions
(flat, Gaussian, exponential, gamma, chi-squared, F, Student's t), **bit for bit** on every input.

**Why.** FreeSurfer's copy of these functions is a transcription of the GNU Scientific Library
(GSL 1.8, GPL-2.0-or-later), which cannot be linked into `larmorx-freesurfer`'s programs with
FreeSurfer's licence (the placement table's P-08, decision log 2026-11-12). So `larmorx-freesurfer`'s
replicas (first `mri_coreg`) call this clean room instead, and a GSL replica in `larmorx-gpl` is
its bit-exact reference. The algorithms are published (Lüscher 1994, James 1994, Knuth, Marsaglia);
this spec adds the exact expressions, operation order, constants and conventions that give
FreeSurfer's numbers. **Where FreeSurfer draws from it:** `mri_coreg` on every run (its coordinate
and intensity dithering are on by default, seed 53; fMRIPrep's `bbregister --init-coreg` runs it),
`mris_expand` with a random thickness map, `mri_volsynth` (`--pdf z`, `t`, `tr`, `F`, `Fr`,
`chi2`), `mri_glmfit`'s simulations, and FreeSurfer's random permutations and sign flips.

**Whoever implements it must not open:** GSL's source (any release, any copy: `rng/ranlux.c`,
`randist/*.c`), FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`, any other copy; among
others `utils/numerics.cpp` and `utils/randomfields.cpp`), the GSL replica (`crates-gpl/`, when it
exists), the documents written from FreeSurfer's source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`),
the spec writer's check (`validation/investigations/freesurfer-ranlux-spec-check/`), `docs/parts/`
and the scratch folders of FreeSurfer's parts. It may read this spec, the papers of §11, the
documentation of `larmorx_core::math`, and build and run the probe in
`validation/investigations/freesurfer-ranlux-probe/` (calls of FreeSurfer's functions only) and the
official FreeSurfer programs as black boxes (§10). This spec holds the method as mathematics and
the numbers it needs, no code; what black-box runs confirmed is marked **(observed)**.

---

## 1. Notation and arithmetic

- **Integers** are exact mathematical integers unless a width is named. `q = t quo d` and
  `r = t rem d` are the quotient and remainder of the division **truncating toward zero**
  (`t = d·q + r`, `r` has the sign of `t` or is 0: `−100000 quo 53668 = −1`,
  `−100000 rem 53668 = −46332`). `t mod 2²⁴` is the **non-negative** residue in [0, 2²⁴) (for a
  negative `t` it equals the low 24 bits of `t`'s 64-bit two's-complement form).
- **binary64** (`double`): every arithmetic operation is one IEEE 754 operation rounded **to
  nearest, ties to even**, in the order and grouping written (left to right where no parentheses
  are shown). No fused multiply-add anywhere, no wider intermediate format, no reassociation: the
  official build is SSE2 scalar code without FMA (`FLT_EVAL_METHOD` 0). `sqrt` is IEEE's correctly
  rounded square root; `floor` is exact.
- `log`, `exp`, `tan`: the C library's functions **as glibc 2.35 computes them on x86-64** (§6),
  not the correctly rounded ones.
- A **draw** consumes values of the generator's stream in the order this spec lists them; two
  draws written one after the other are made in that order. The number of integers each draw
  consumes is part of the behaviour (later draws of the same stream depend on it).

## 2. The generator

### 2.1 What it delivers

A stream of integers in [0, 2²⁴), one per call (FreeSurfer's `sc_rng_get`). The state is 24
integers `x` in [0, 2²⁴), a carry `c` in {0, 1}, and the count of values delivered in the current
group (§2.4). Seeding (§2.2) sets the whole state; seeding again discards everything before it.

### 2.2 Seeding (James's initialisation)

The seed is an unsigned 64-bit integer `S`. FreeSurfer's callers convert their own seeds to it as
C does, modulo 2⁶⁴: `mri_coreg`'s `--seed` is a 32-bit `int` (−1 becomes 2⁶⁴ − 1), `mri_volsynth`'s
a 64-bit `long`.

1. If `S = 0`, `S` becomes **314159265** (so seed 0 and seed 314159265 give the same stream
   **(observed)**).
2. `t₀` = `S` read as a signed 64-bit two's-complement integer (`S ≥ 2⁶³` gives `S − 2⁶⁴`).
3. For `k = 1, …, 24`:
   - `q = t_{k−1} quo 53668`, `r = t_{k−1} rem 53668`;
   - `t = 40014·r − 12211·q`; if `t < 0`, `t` becomes `t + 2147483563` (**once**: the result may
     still be negative when `|t_{k−1}|` is large);
   - `t_k = t`, and `v_k = t_k mod 2²⁴`.

   Every intermediate value fits in a signed 64-bit integer (`|12211·q| < 2.1·10¹⁸`), so this is
   exact integer arithmetic. This is Schrage's evaluation of the multiplicative congruential
   generator `t ← 40014·t mod 2147483563` (L'Ecuyer's constants, `53668 = 2147483563 quo 40014`,
   `12211 = 2147483563 rem 40014`); for `0 ≤ t_{k−1} < 2147483563` it gives exactly
   `40014·t_{k−1} mod 2147483563`, and the steps above define it for every other value.
4. The 24 values become the generator's first history, in **reverse** order: `x₂₃ = v₁`,
   `x₂₂ = v₂`, …, `x₀ = v₂₄` (`x_{24−k} = v_k`).
5. The carry `c₂₃` is **0 for every seed**. (James's 1994 Fortran starts with a carry of one unit
   when its 24th value `v₂₄` is 0; FreeSurfer never does. An implementation following James's
   listing differs from FreeSurfer exactly on the seeds with `v₂₄ = 0`; FS-101, §8.2, uses that
   rule only for the all-zero state.)
6. The group count is 0.

Consequences: seeds whose signed value `t₀` lies in (−9.43·10⁹, 9.43·10⁹) and that agree modulo
2147483563 give the same stream, because there step 3's first result is already `40014·t₀ mod
2147483563` (the bound is `53668·⌈2147483563 / 12211⌉`) **(observed**: seed 1 and seed 2147483564;
seed 2147483562 and seed 2⁶⁴ − 1, that is −1**)**; seed 2147483563 gives the all-zero state (§2.6).

### 2.3 The recurrence (Lüscher's subtract-with-borrow, lags 24 and 10)

For `n ≥ 24`:

- `d_n = x_{n−10} − x_{n−24} − c_{n−1}`;
- if `d_n < 0`: `x_n = d_n + 2²⁴` and `c_n = 1`; otherwise `x_n = d_n` and `c_n = 0`.

(`d_n` lies in [−2²⁴, 2²⁴): the case `d_n = −2²⁴` gives `x_n = 0`, `c_n = 1`.)

### 2.4 The decimation (luxury level 389)

The values `x_24, x_25, …` are computed in groups of 389 consecutive values; of each group the
**first 24 are delivered** and the other **365 are computed and discarded**. So the k-th delivered
value since the last seeding (k = 0, 1, 2, …) is `x_{24 + 389·(k quo 24) + (k rem 24)}`: the first
delivered value is `x₂₄`, the 24th `x₄₇`, the 25th `x₄₁₃`. The discarding happens after each 24th
delivered value, before the next is made (it does not wait for the next call to need it; the
stream of delivered values is the same either way).

### 2.5 Reference numbers (observed)

`fs_rng_probe raw <seed> 100000` (§10.1); the first 12 delivered values, the 25th to 28th (the
first of the second group), and the SHA-256 of the first 100,000 values written in decimal, one
per line, each line ending in a line feed:

| Seed | Values 1-12 | Values 25-28 | SHA-256 of 100,000 |
|---|---|---|---|
| 53 (`mri_coreg`'s default) | 2221969 1586980 7231467 13008083 14055568 7275409 14826403 7390714 3054659 16338312 6216067 6833642 | 4618358 3215875 9571569 7955534 | `b40812f1162500bca3938e3148b47358a9300d4c7be994cd7e03163e9115541d` |
| 0 (= 314159265) | 9056646 12776696 1011656 13354708 5139066 1388751 11163902 7730127 15531355 10387694 5359178 2422458 | 14182553 12156545 13313883 7141125 | `12d4f2cdf218304041625d02416dc679eba7d14bf3ca30ff282fadce55a1e3b7` |
| 1 (= 2147483564) | 15869483 7943651 15963989 7209501 1531321 5202087 15157652 11535256 13036256 7905523 3599347 5193754 | 13934952 7006785 8399129 15847573 | `391c821d14bb056db5e2302e22202721e758206eb7ebe90f8a74ca38c0c20ead` |
| 2 | 14961750 15887388 15150761 14419088 3062727 10404174 13538087 6293382 9295298 15811048 7198695 10387509 | 5741855 14824996 1184122 14233928 | `87c0b8ce736a3e5633677ec80d807728a0fa62f9fed7d39bc7f2378be831979d` |
| 1234 | 3935795 4662948 3129627 4675023 10704432 10483180 14725473 7470299 14142625 7824199 12409813 199111 | 5682984 9444026 15876047 5571961 | `edb3efc9bd60f606ead133110dc82c15ead6deb1360cb9c113b9c5fa6fd08338` |
| 2147483562 (= 2⁶⁴ − 1) | 907733 8833564 813226 9567714 15245894 11575128 1619563 5241959 3740959 8871692 13177953 11583545 | 64362 15992654 11533756 15540238 | `fdde7d836524bd89f00fde457d8039ac5012b5ebbba15e295ababb6db9e2a2d5` |
| 4294967295 | 14365963 312317 13562131 10456400 7149033 6738065 11506525 3307156 5308652 10629412 4310035 5329702 | 7558662 9467569 246765 9972969 | `cd8cf49f25873643013cc8df819c2571be182e31330a7e99c6c41c5ec6f3ece5` |
| 4294967296 | 13458230 8256054 12748903 888772 8680440 11940152 9886962 14842498 1567607 1757720 7909384 10523457 | 11279554 1176059 6758885 39167 | `22483b9dba48de1a1fa4d18e2325de2ec38abc68e0f1551a53d915c6ffa99f46` |
| 2⁶³ − 1 | 3870255 6549656 8109563 12752488 14566788 7591189 14597542 13769507 11190627 7623861 14244068 696095 | 355042 3265491 7406510 14546069 | `6a65e3a99aed14c45fd3ff53bf5c59c3f0b8609eb81f0408447c659723ce3621` |
| 2⁶³ | 13814694 2283907 9480795 13592356 679021 3983938 3799322 8249581 9327546 1247914 15711101 10887533 | 14990511 5995780 170277 3254111 | `17809d02bbe72bcdf755d5315c47475de9cfb6c14080ab911cbbd9c7b64e8749` |
| −53 (2⁶⁴ − 53) | 14555247 15190235 9545748 3769132 2721647 9501806 1950812 9386501 13722556 438903 10561233 9943657 | 9380956 3006348 10361317 6655061 | `bae173cae0f216f7e3af02218d6c93aa33f7078757e2646ba1fb99101af16f67` |

### 2.6 Seeds that leave the state all zero (FS-101)

When the 24 seeded values are all 0 (the carry is 0, §2.2 step 5), the recurrence gives 0 for ever:
every delivered value is 0. Seed 2147483563 does this **(observed)**: its first step gives
`t₁ = 40014·12211 − 12211·40014 = 0`, and 0 stays 0. Among the 2³² seeds `mri_coreg` can be given
(its 32-bit `--seed`, sign-extended), exactly two do it, **2147483563 and −2147483563**
**(observed, all 2³² enumerated by `fs_rng_probe zero-seeds`, 8 minutes)**; among 64-bit seeds,
the small multiples of 2147483563 do it too **(observed** for 2, 3, 4, 5 and 6 times it and −2, −4,
−5 times it**)**, and the rule of §8.2 is stated on the state, so it covers any seed that gives it.
Then a uniform draw is always 0, and every draw that needs a positive uniform or a Gaussian (the
exponential, gamma with a shape above 0, chi-squared, F and t draws, and the Gaussian itself, whose
point (−1, −1) is always rejected) never returns: the official
`mri_volsynth --pdf z --seed 2147483563` and `--pdf chi2 --dof 4 --seed 2147483563` do not end
**(observed: stopped after 20 s; seed 2147483564 ends at once)**; `mri_coreg --seed 2147483563`
dithers by 0. What larmorx does: §8.2.

## 3. Uniform deviates

- **Uniform** `U` (`sc_uni`): one integer `x` of the stream; `U = x·2⁻²⁴`, exact, in [0, 1)
  (`x / 16777216` in binary64: the conversion and the scaling are exact).
- **Positive uniform** `P` (`sc_uni_pos`): uniforms are drawn until one is not 0; `P` is that one,
  in (0, 1). It consumes one integer, or more where the stream holds zeros.

Both are a multiple of 2⁻²⁴ in binary64, so `1 − U`, `2·U` and `−1 + 2·U` are exact.

## 4. The distributions

Each is listed with its parameters, the draws it makes in order, and the exact expression. "Repeat"
means: make all the listed draws again, from the next values of the stream.

### 4.1 Flat on [a, b] (`sc_ran_flat(a, b)`)

One `U`; the result is `(a·(1 − U)) + (b·U)`. With `a = 0, b = 1` the result is exactly `U`
(`mri_coreg`'s dithering and every other "uniform" FreeSurfer draws; the products and the sum are
then exact).

### 4.2 Gaussian with standard deviation σ (`sc_ran_gaussian(σ)`; Marsaglia's polar method)

1. `U₁`, then `U₂`; `X = −1 + 2·U₁`, `Y = −1 + 2·U₂` (exact); `R = (X·X) + (Y·Y)`.
2. If `R > 1` or `R = 0`: repeat from 1.
3. The result is `(σ·Y) · sqrt((−2·log(R)) / R)`.

Each attempt consumes two integers. Only `Y` reaches the result; `X` is used only in `R` (the
method's second deviate is discarded: each call starts a new pair). `−2·log(R)` is exact (a
multiplication by −2); the division is rounded, then the square root; `σ·Y` is rounded before the
last multiplication. With `R = 1` (`X = ±1, Y = 0` or `X = 0, Y = ±1`), `log(1) = +0` and the factor is
`+0`.

### 4.3 The unit Gaussian (`sc_ugauss`)

§4.2 with σ = 1 (so `σ·Y = Y`).

### 4.4 Exponential with mean μ (`sc_ran_exponential(μ)`)

One `P`; the result is `(−μ)·log(P)` (the negation is exact; `−(μ·log(P))` has the same bits for
every μ, NaN included, §7).

### 4.5 Gamma with shape a and scale b (`sc_ran_gamma(a, b)`)

For `0 ≤ a < 2³²` (other shapes: FS-100, §8.1). Let `n = floor(a)` (an integer, `0 ≤ n < 2³²`).

- If `a = n`: the result is `b·G_int(n)`.
- Else, if `n = 0`: the result is `b·G_frac(a)`.
- Else: the result is `b·(G_int(n) + G_frac(a − n))`, with **`G_int(n)`'s draws first**, then
  `G_frac`'s **(observed** in the official build's code and through `mri_volsynth --pdf F --dof-den 25`
  and `--pdf chi2 --dof 3`, §10.4). `a − n` is exact.

**`G_int(n)`**, an integer shape:
- if `n < 12`: `Π = 1`, then for `i = 1, …, n`: `Π = Π·P_i` (one `P` each, multiplied in the order
  drawn); the result is `−log(Π)`. For `n = 0` no draw is made and the result is `−log(1) = −0`
  (so `gamma(0, b) = b·(−0)`: `−0` for `b > 0`, **(observed)** `0x8000000000000000` for `b = 1`).
- if `n ≥ 12`: `G_large(n)`, with `n` as a binary64 value (exact).

**`G_large(A)`**, `A ≥ 12` (the rejection method of Knuth's §3.4.1, with a Cauchy-distributed
proposal):
1. Once: `s = sqrt((2·A) − 1)` (`2·A` exact; the subtraction rounded) and `A₁ = A − 1`.
2. `U`; `y = tan(π_d·U)` (π_d of §5; the product rounded, then glibc's `tan`, §6);
   `T = ((s·y) + A) − 1`. If `T ≤ 0`: repeat step 2 (a new `U`).
3. `V` = one `U`. If `V > (1 + (y·y)) · exp((A₁·log(T / A₁)) − (s·y))`: repeat from step 2.
4. The result is `T`.

`s·y` is the same rounded product in steps 2 and 3. `1 + (y·y)`: the square rounded, then the sum;
the inner expression: `T / A₁` rounded, `log`, the product with `A₁` rounded, then `s·y` subtracted
(rounded), then `exp`, then the product with `1 + y²`.

**`G_frac(f)`**, `0 < f < 1` (Ahrens and Dieter's method for a shape below 1, Knuth's exercise
3.4.1-16):
1. Once: `p = e_d / (f + e_d)` (e_d of §5).
2. `U₁` = one `U`, then `V` = one `P`.
3. If `U₁ < p`: `X = exp((1 / f) · log(V))` (`1 / f` rounded, `log(V)`, the product rounded, `exp`)
   and `q = exp(−X)`. Otherwise: `X = 1 − log(V)` and `q = exp((f − 1) · log(X))` (`f − 1` rounded).
4. `W` = one `U`. If `W ≥ q`: repeat from step 2. Otherwise the result is `X`.

### 4.6 Chi-squared with ν degrees of freedom (`sc_ran_chisq(ν)`)

The result is `2·gamma(ν/2, 1)` (`ν/2` exact; scale 1 gives the gamma part itself; the doubling is
exact).

### 4.7 F with ν₁ and ν₂ degrees of freedom (`sc_ran_fdist(ν₁, ν₂)`)

`Y₁ = gamma(ν₁/2, 2)`, then `Y₂ = gamma(ν₂/2, 2)`; the result is `(Y₁·ν₂) / (ν₁·Y₂)` (two rounded
products, then the rounded quotient).

### 4.8 Student's t with ν degrees of freedom (`sc_ran_tdist(ν)`)

- **If `ν ≤ 2`:** `Y₁` = a unit Gaussian (§4.3), then `Y₂ = chisq(ν)` (§4.6); the result is
  `Y₁ / sqrt(Y₂ / ν)`.
- **Otherwise** (and for a NaN ν, which fails the test `ν ≤ 2`; Marsaglia's 1980 rejection method):
  1. `Y₁` = a unit Gaussian; `Y₂ = exponential(1 / ((ν/2) − 1))` (§4.4; `ν/2` exact, the
     subtraction and the division rounded); `Z = (Y₁·Y₁) / (ν − 2)`.
  2. If `1 − Z < 0`: repeat from 1. Otherwise, if `exp((−Y₂) − Z) > (1 − Z)`: repeat from 1.
  3. The result is `Y₁ / sqrt((1 − (2/ν)) · (1 − Z))` (`2/ν`, the subtractions, the product, the root
     and the quotient rounded in that order).

## 5. Constants

| Name | Value | binary64 bits |
|---|---|---|
| π_d | 3.141592653589793 (the binary64 value nearest π) | `0x400921fb54442d18` **(observed** in the official build's constants) |
| e_d | 2.718281828459045 (the binary64 value nearest e) | `0x4005bf0a8b145769` **(observed)** |
| default seed | 314159265 | |
| Schrage's constants | 53668, 40014, 12211, modulus 2147483563 | |
| lags | 24 and 10 | |
| group | 389 values, 24 delivered, 365 discarded | |
| integer range | 2²⁴ = 16777216 | |
| the integer shape's switch | 12 (product below, `G_large` from 12) | |

## 6. The elementary functions

| Function | Arguments it gets here | Which bits |
|---|---|---|
| `log` | `R` in (0, 1] (§4.2), products of `P`'s, `P`, `T / A₁`, `X ≥ 1` | glibc 2.35's (x86-64): `larmorx_core::math::log_glibc` |
| `exp` | the rejection tests of §4.5 and §4.8 | glibc 2.35's: `larmorx_core::math::exp_glibc` |
| `tan` | exactly the 2²⁴ values `fl(π_d · k·2⁻²⁴)`, `k = 0, …, 2²⁴ − 1` (`G_large` only) | glibc 2.35's, **which is not correctly rounded there**: on **41,708 of the 16,777,216** arguments glibc's result is one unit in the last place from the correctly rounded one (both directions; 20,360 of them with `k < 2²³`) **(observed**, `tan_check.c`, §10.3**)**. `larmorx_core::math` has a correctly rounded `tan` (CORE-MATH's) but no glibc `tan` yet: track D's piece on the C library's maths (3.1-08) specifies one; until it exists, `G_large` is exact wherever its `U` avoids those 41,708 values of `k` |
| `sqrt` | non-negative values (NaN or a negative value only in §8.1's cases) | correctly rounded (glibc's is the hardware's) |
| `floor` | the gamma shape | exact |

The official programs link the C library dynamically, so their results are those of the machine
that runs them; the reference machine has glibc 2.35 (Ubuntu 22.04).

## 7. Special values, NaNs and zeros

Where a parameter is NaN or infinite, or an operation is invalid, the results follow x86-64's SSE
arithmetic, as the official build does (`larmorx_core::math`'s `x86_nan` holds these rules):

- an operation on numbers that has no value (`∞ − ∞`, `0·∞`, `0/0`, `sqrt` of a negative number)
  gives x86-64's **default NaN**, bits `0xfff8000000000000` (the sign set), on every platform;
- one NaN operand is passed on **quieted** (its quiet bit set, sign and payload kept); of **two**
  NaN operands, the **first** of the pair as written below is passed on;
- negation and doubling (`2·g` written as `g + g` is the same) keep a NaN's payload; negation
  flips its sign.

The order of the operands where two NaNs can meet:

| Draw | Operations, first operand first |
|---|---|
| flat | `t₁ = U·b`, `t₂ = (1 − U)·a`, result `t₁ + t₂` (so b's NaN wins over a's) |
| Gaussian | `t = Y·σ`, result `sqrt(…)·t` (the root is never NaN for a finite σ) |
| exponential | `log(P)·(−μ)` (a NaN μ comes out with its sign flipped) |
| gamma | the scale last: `g·b` |
| F | `t₁ = Y₁·ν₂`, `t₂ = ν₁·Y₂`, result `t₁ / t₂` |
| t | as written in §4.8 |

Comparisons with a NaN are false, so: a NaN σ (Gaussian), μ (exponential), either bound of the
flat draw or the gamma scale gives NaN results without changing the draws; a NaN ν of the t draw takes the second branch and leaves its loop at once (both
tests false). Zeros: the arithmetic decides their signs (`gamma(0, 1) = −0` and `chisq(0) = −0`,
§4.5; `F` with a zero degree of freedom and `t(0)` give the default NaN through `0/0` or `−0/0`).
Gamma shapes that are NaN, infinite, negative or at least 2³² are FS-100 (§8.1).

## 8. Defects and the `reproduce` switch

Both are deterministic and decided by the rules of `CLAUDE.md` (D13); each keeps a `reproduce`
switch (`--upstream-defects reproduce`, `larmorx_core::defects`). By default the clean room
corrects them and says so once per run in a notice; under `reproduce` it gives FreeSurfer's
results bit for bit, except where FreeSurfer's never come (stated below). Both are behaviours seen
from outside (black-box runs and the official build's arithmetic), so their descriptions here and
in the defect register are open to clean-room implementers.

### 8.1 FS-100: gamma shapes outside [0, 2³²)

**What the original does.** It converts `floor(a)` to a 32-bit unsigned integer, which C leaves
undefined outside [0, 2³²); the official build keeps the **low 32 bits of the 64-bit truncation**,
and gives 0 for NaN, ±∞ and every `|floor(a)| ≥ 2⁶³` (x86-64's "integer indefinite" value
`0x8000000000000000`, whose low 32 bits are 0) **(observed** in its code**)**. Call that value `n′`.
It then runs §4.5 with `n′` in place of `n`: `a = n′` never holds for these shapes, so the result is
`b·G_frac(a)` when `n′ = 0` and `b·(G_int(n′) + G_frac(a − n′))` otherwise, with `G_frac` given a
shape outside (0, 1). Measured **(observed**, `fs_rng_probe dist gamma 53 3 <a> 1`):

- `a ≥ 2³²`, finite: a value of the wrong size: `gamma(2³² + 0.5, 1)` gives 3.358…
  (`0x400add92293d2764`) instead of about 4.29·10⁹; `gamma(2³² + 12.25, 1)` gives 14.37…
  (`0x402cbf45c94d9555`); `gamma(2³³ + 8, 1)` gives 8.46… (`0x4020eaa524bac70f`).
- NaN, +∞, and finite `a ≥ 2⁶³` (then `n′ = 0`): in `G_frac(a)`, `p` is NaN, 0 or below 2⁻⁶¹,
  so the first attempt with `U₁ ≠ 0` takes the second branch, where `q` is NaN or +∞ and the
  attempt is accepted: the result is `b·(1 − log(V))` (the same 3.358… for seed 53 with `a` = NaN,
  +∞ and 10³⁰⁰).
- `a < 0` and −∞: `G_frac` gets a shape below −2³¹ (or −∞); `p < 0`, so only the second branch is
  taken, and its test accepts only when `V` is among the two largest uniforms and `W = 0`
  (probability at most about 2⁻⁴⁷ per attempt; for −∞, or shapes far below −2³², never): the official
  code does not return in practice **(observed**: the probe stopped after 10 s for −1.5 and −∞; the
  official `mri_volsynth --pdf chi2 --dof -3 --seed 53` stopped after 20 s**)**.

They reach the gamma draw through `chisq(ν)` with `ν < 0` or `ν ≥ 2³³`, `fdist` with such a degree of
freedom, and `tdist(ν)` with `ν < 0` (its first branch). No program on `recon-all`'s or fMRIPrep's
paths gives such a shape (`mri_volsynth`'s and `mri_glmfit`'s degrees of freedom come from the user
or the design).

**What larmorx does.**
- `a ≥ 2³²`, finite: **rule A** (the shape makes sense; the result is wrong). The default computes
  §4.5 with the exact `n = floor(a)` (a binary64 integer, so `G_int(n) = G_large(n)`): `b·G_large(a)`
  when `a` is an integer, else `b·(G_large(floor(a)) + G_frac(a − floor(a)))`, with the notice
  "FS-100: FreeSurfer's gamma draw truncates the shape <a> to 32 bits and gives a value of the wrong
  size; larmorx uses the whole shape (--upstream-defects reproduce: FreeSurfer's value)". `reproduce`:
  the original's arithmetic with `n′`, bit for bit.
- NaN and ±∞: **rule B** (no gamma distribution has such a shape). The default is an error naming
  the shape ("FS-100: the gamma shape <a> is not a finite number"); `reproduce`: for NaN and +∞ the
  original's value, bit for bit; for −∞ the same error (the original never returns).
- `a < 0`: **rule B**. The default is an error ("FS-100: the gamma shape <a> is negative"). Under
  `reproduce` the same error: the original's loop does not end in practice (about 2⁴⁷ attempts
  expected, months of computing), and a run that does not end is not reproduced.

### 8.2 FS-101: seeds that make every value 0

**What the original does.** §2.6: the all-zero seeded state with carry 0 is a fixed point of the
recurrence, so the stream is constant 0 for ever; FreeSurfer's draws then give the same value every
time (flat: `a·1 + b·0`) or never return (every other draw). James's 1994 initialisation starts such
a state with a carry of one unit (§2.2 step 5); FreeSurfer's initial carry is always 0.

**What larmorx does: rule A** (any seed is a legitimate input; a constant "random" stream, or a draw
that never returns, is wrong). When the 24 seeded values are all 0, the default starts the
recurrence with `c₂₃ = 1` (James's rule, applied only to this state: every other seed's stream is
FreeSurfer's), with the notice "FS-101: seed <S> makes FreeSurfer's random generator give 0 for
ever; larmorx starts it with a carry of 1 (James's 1994 initialisation) (--upstream-defects
reproduce: FreeSurfer's constant stream)". The corrected stream's first values are 16777215 (ten
times: `x₂₄` to `x₃₃`, each `0 − 0 − 1 + 2²⁴` with the carry kept), then 16777214, … as §2.3 gives.
`reproduce`: the constant 0 stream; the uniform and flat draws (and gamma and chi-squared of shape
0, which draw nothing) give FreeSurfer's values; every draw that would never return reports an
error instead ("FS-101: with seed <S> FreeSurfer's <draw> never returns").

### 8.3 Not defects (kept, documented)

- Seeds congruent modulo 2147483563 alias (§2.2): James's seeding has about 2³¹ distinct streams.
- The Gaussian draw discards the polar method's second deviate (§4.2): a design (each call is
  independent of the previous call's pair), and FreeSurfer's streams depend on it.
- Shape 0 gives `−0` (§4.5): the gamma distribution of shape 0 is the point mass at 0.

## 9. What the clean room provides

A generator type (seeded as §2.2, delivering §2's integers) and the draws of §3 and §4 as functions
of it, each consuming the stream exactly as specified; an explicit `reproduce` setting for FS-100 and
FS-101 (§8), the errors as values the caller turns into its own messages, and the notices through
larmorx's usual notice channel. No global state, no thread-local state: FreeSurfer's callers own
their generators (`mri_coreg` makes three, each seeded with the same seed: the coordinate dither,
the reference's and the moving image's intensity dither). The names and the Rust API are the
implementer's choice; `larmorx-freesurfer`'s replicas will call them.

**Speed.** Every delivered value costs about 16 recurrence steps (389 per 24). The original makes
20 million integers in about 2.3 s on the reference machine, printing included (`fs_rng_probe raw 53
20000000`). The carry makes each step depend on the one before, so a group cannot be computed in
parallel as written; the clean room is built for speed in the usual sense (porting guide,
"Clean-room originals: the same behaviour, built for speed"): a tight loop over a fixed array, no
per-value branching beyond the carry, measured against the probe.

## 10. Validation plan (black box)

Everything here runs the original as a black box: the probe (calls of FreeSurfer's own compiled
functions, built from the unit oracle's libraries, track B's `<workspace>/oracles/freesurfer-8.2.0-unit`,
which reproduce the official programs' code) or the official programs. Suites run them through the
oracle-output cache (`oracle_cache.run`, a `Policy` with one thread and the family "freesurfer"),
never `subprocess.run`. When the unit oracle's harness (`larmorx_validation.freesurfer_unit`, the
`fsunit` program) gains these commands, the suites may call it instead of the probe; the probe's
commands are the model for them.

### 10.1 The generator

`fs_rng_probe raw <seed> <count>`: the first 100,000 values, compared with the clean room's
(SHA-256 of the decimal lines, then the first differing position on a mismatch), for: the eleven
seeds of §2.5; 0; 2147483563 and the others of §2.6 under `reproduce` (all 0); 2147483564;
1,000 seeds drawn by a seeded generator from [0, 2⁶⁴) and 1,000 from the 32-bit range sign-extended
(as `mri_coreg` passes them, 0 excluded); and the boundaries 2³¹ − 1, −2³¹, 2⁵³ ± 1, 2⁶³ ± 1. A
seeding after draws must give the fresh seeding's stream (the clean room's own test; the probe seeds
once per run). Expected: all identical.

### 10.2 The distributions

`fs_rng_probe dist <name> <seed> <count> [p1 [p2]]` (the 16 hex digits of each draw's bits),
20,000 draws per case, seeds 53, 1 and 4294967296, compared bit for bit:

- `uni`, `uni_pos`; `flat` with (0, 1), (−3.5, 7.25), (5, 5), (2, −1), (−0, −0), (NaN, 1), (1, NaN),
  (NaN, NaN with another payload), (+∞, 1), (0, +∞) (the default NaN where `U = 0`);
- `gaussian` with σ 1, 2.5, −1, 0, 10⁻³⁰⁰, 10³⁰⁰, NaN, +∞; `ugauss`;
- `exponential` with μ 1, 0.75, 0, −2, NaN;
- `gamma` with shapes 0, 10⁻³⁰⁰, 0.001, 0.4, 0.5, 0.999, 1, 2, 2.5, 7.3, 11, 11.5, 12, 12.5, 25, 31.6,
  1000.25, 10⁶, 2³² − 0.5 and scales 1, 2, 0.5, −1; the helpers `gamma_int`, `gamma_large`,
  `gamma_frac` on their own (their draws as §4.5 says);
- `chisq` with ν 0, 1, 3, 5, 24, 25, 30, 1000; `fdist` with (4, 9), (3, 25), (1, 1), (30, 50);
  `tdist` with ν 0.5, 1, 1.5, 2, 2.0000000000000004, 3, 7, 30, 1000, NaN;
- `mixed <seed> 14000` (one stream through a fixed rotation of every draw, so each draw's
  consumption is checked: a draw that consumed one integer too many or too few shifts every later
  value).

Reference hashes **(observed)**, seed 53, 20,000 draws: `uni` `0588698480ad8ec24542f5fe26022bf9111f8b62aacac35c9dba4aa63427f583`;
`flat(−3.5, 7.25)` `269e29edca7cc8250c431f4881377e474b907b096a7f4c42a8a2de89fe9babad`; `gaussian(2.5)`
`dddaf9501cc31f055ae579170575a39dee41feaf306e30c3c7d3529db56dec72`; `ugauss`
`d361b5e25ba5462d3a2c778159eae840620ecbfc7dbab5c8d4c45c95034afd1b`; `exponential(0.75)`
`169ea159bfca04c0d46d9778918769faa122a691af010ab112173b1e85d2a7c5`; `gamma(0.4, 1.5)`
`25e6ffbbfd151efb44bb0cb517567551bf6992802bb7c8ecb7f588de6a22fecb`; `gamma(3, 2)`
`cb0281404d16edaf9bbda1ca583db153b65c86cbfdf1e71a11a809ff57be5aba`; `gamma(7.3, 0.5)`
`cf37d457abf62138bf99e2238ce112228df0563709a56fce5481d9a4d781d58f`; `gamma(12, 1)`
`7222f769fbae0a44550f08f20e0ac54097a898edd2331f714322d5c6f1633038`; `gamma(31.6, 1)`
`3c2a34e19681197ba731382cd64f8063377a9ad264472a43c6b667bf5a7a2a79`; `chisq(5)`
`0d3b457e1a84203627d9def14eb2df8fd053ddc524e1dd860f15d0c139c4eb1e`; `fdist(4, 9)`
`8f2606831e0f4ee964814a5638e17f4f005b360c7b90951daeae26b77d038309`; `tdist(1.5)`
`5a3f4a1edc6c8ff7fb422cbc0e1c68fddc7ac74945f4f18846b4c619a26db2ae`; `tdist(7)`
`3b2a14c67236ed124516a79cf768df21d7d89b19b366a5d5a9505bed817db095`; `mixed` (14,000)
`4fd4887500e4fec3c0b6ba7a2b1427965e7622be2bdb8cdcd5bdb5c2f95d8aed`. The first draws, seed 53:
`uni` `3fc0f3c880000000 3fb8372400000000 3fdb95fac0000000` (2221969·2⁻²⁴, …); `ugauss`
`3ff75b40639cd543 bfce260050c592c5 bfc3b77b9f954954`; `gamma(3, 2)` `4024e2b5d995c6dd 400445885bf0942c
40152c924ed15944`. (`fs_rng_probe`'s `refs.sh` prints them all.)

The gamma cases with a shape of 12 or more pass only with glibc 2.35's `tan` (§6): run them on the
reference machine, and count separately the draws whose `U` fell on one of §6's 41,708 values.

### 10.3 `tan` on its 2²⁴ arguments

The clean room's `tan` (when it exists, §6) against the reference machine's glibc 2.35 `tan` on all
2²⁴ arguments `fl(π_d · k·2⁻²⁴)`: identical bits. `tan_check.c` is the method (it compares glibc with
CORE-MATH's `cr_tan`: 41,708 differ).

### 10.4 The official programs

- `mri_volsynth --pdf <z | t | F | chi2> [--dof N | --dof-num N --dof-den M] --dim W H D F --seed S
  --vol out.mgz` (FreeSurfer 8.2.0's official program, `FREESURFER_HOME` set, one thread): its
  float32 voxels equal the clean room's draws rounded to float32, taken in the order column
  (outermost), row, slice, frame (innermost) (FreeSurfer's `RFsynth`), with `z` the Gaussian of
  mean 0 and σ 1 (`+ 0` after the draw), `t` the t draw with ν = `--dof`, `F` with (`--dof-num`,
  `--dof-den`), `chi2` with ν = `--dof`. **(observed**, 2026-11-12: `z` seed 53, `chi2` 30 and 3 seed
  7, `t` 9 seed 7, `t` 2 seed 13, `F` 3/25 seed 11, 5 × 4 × 3 × 2 voxels each: 120 of 120 equal in all
  six; `compare.py` beside the probe**)**. Seeds below 0 are replaced by the time of day there: not
  for tests.
- `mri_coreg`'s default dithering (seed 53): through the replay of the recorded heads
  (`<workspace>/oracles/freesurfer-8.2.0/runs/`, the replay tool) once `larmorx-freesurfer`'s
  `mri_coreg` replica uses the clean room: its outputs identical to the recorded ones.

### 10.5 The defects

- FS-100: the default's notice and corrected values for `a ≥ 2³²` (compared with §8.1's formula
  evaluated by the clean room's own `G_large` and `G_frac`), its errors for NaN, ±∞ and negative
  shapes; under `reproduce`, the probe's values for 2³² + 0.5, 2³² + 12.25, 2³³ + 8, 10³⁰⁰, NaN and
  +∞ (bit for bit), and the errors for −1.5 and −∞ (the probe, given a time limit, does not return).
- FS-101: the default's notice and §8.2's corrected stream for each seed of §2.6; under
  `reproduce` the zero stream, `uni` = 0, `flat(a, b)` = `a·1 + b·0`, and the errors of the other
  draws (the official `mri_volsynth --seed 2147483563` does not end, §2.6).

### 10.6 Golden cases

One case per path in the cross-platform golden suite (`CLAUDE.md` rule 10): the generator's first
1,000 values for three seeds, each distribution's first 100 draws for one seed (the gamma branches
`G_int` below 12, `G_large`, `G_frac` and their sum separately; both t branches), the FS-100 and
FS-101 defaults; the outputs hashed. They need no input image and run in milliseconds. `G_large`'s
cases depend on §6's `tan`: until the glibc `tan` exists, choose seeds whose draws avoid §6's 41,708
values (the golden suite checks platforms against Linux, not against FreeSurfer).

## 11. References

- M. Lüscher, "A portable high-quality random number generator for lattice field theory
  simulations", Computer Physics Communications 79 (1994) 100–110 (the subtract-with-borrow
  generator with lags 24 and 10, and its decimation).
- F. James, "RANLUX: A Fortran implementation of the high-quality pseudorandom number generator of
  Lüscher", Computer Physics Communications 79 (1994) 111–114 (the initialisation of §2.2; mind §2.2
  step 5: its initial carry is not FreeSurfer's).
- P. L'Ecuyer, "Efficient and portable combined random number generators", Communications of the
  ACM 31 (1988) 742–751 (the constants 40014 and 2147483563); L. Schrage, "A more portable Fortran
  random number generator", ACM TOMS 5 (1979) 132–138 (the evaluation of §2.2 step 3).
- D. E. Knuth, The Art of Computer Programming, vol. 2, 3rd ed. (1997), §3.4.1 (the gamma methods:
  the product of uniforms, the large-shape rejection with a tangent, exercise 16 for shapes below 1;
  the polar method).
- G. Marsaglia and T. A. Bray, "A convenient method for generating normal variables", SIAM Review 6
  (1964) 260–264 (the polar method); G. Marsaglia, "Generating random variables with a t-distribution",
  Mathematics of Computation 34 (1980) 235–236 (§4.8's second branch).
