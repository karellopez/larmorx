# Spec: a single-precision cube root with glibc 2.35's bits (`cbrtf`, x86-64)

(spec writer, 2026-10-30) The input for a clean-room implementation, in the main package
(Apache-2.0), of the cube root that AFNI's programs get from the C library on Linux x86-64
(glibc 2.35, Ubuntu 22.04, the system AFNI's oracles run on), bit for bit on every float32 input.

**Why.** 3dUnifize `-EPI` sets its neighbourhood's radius from the cube root of the voxel volume
(`specs/3dUnifize.md` §7). glibc's `cbrtf` is not correctly rounded (on about one input in ten
its result is one unit in the last place away), and on some realistic voxel sizes that last bit
changes the radius, its printed value or the ball of neighbours (`docs/findings/afni-relicensing-26.md`,
"The C library's maths": 8,794 voxel volumes from 0.001 mm³, 80 in [1, 100) mm³). glibc's code is
LGPL, so the AFNI replica in the main package uses CORE-MATH's correctly rounded `cbrtf` and
warns where the last bit could matter. A clean-room function with glibc's bits removes the
warning and makes the replica exact there. **The user decides** whether this part runs
(`docs/next-steps.md`, "3dUnifize `-EPI`'s `cbrtf`"; recommended); this spec lets it start at once.

**Whoever implements it must not open:** glibc's source (`<workspace>/tags/glibc-2.35/`,
`tags/glibc-2.35-download/`, any other copy), the translations and tools that hold glibc's code or
were written from it (`crates-gpl/`, its `tools/glibc-reference`, `cbrtf-measure`,
`floatmath-verify`, `sincosf-verify`), the AFNI replica (`crates/larmorx-afni-replica`), or
`docs/parts/`. It may read this spec, `docs/findings/afni-relicensing-26.md`, the numerical
literature (cube-root iterations are textbook material) and run the system's `cbrtf` as a black
box (§6). This spec holds the method as mathematics and the numbers it needs, no code; what black-box
runs confirmed is marked **(observed)**.

---

## 1. Notation and arithmetic

- **binary32** (`float`) and **binary64** (`double`): IEEE 754 formats; every operation below is
  one IEEE operation rounded **to nearest, ties to even**, in the format named. `fl32(v)` and
  `fl64(v)` are that rounding of a real `v`.
- **No fused operations:** every product is rounded before it is added (no fused multiply-add,
  anywhere), and no wider intermediate format (no x87 extended precision). On x86-64 glibc's
  function has a single build, which uses none (its results are the same with and without FMA
  hardware).
- A binary32 value converted to binary64 is exact; a binary64 value converted to binary32 is
  rounded once (`fl32`). Where the steps below round to double and then to float, the value is
  rounded **twice**, in that order (not once from the exact value).
- `q = e quo 3` and `ρ = e rem 3` are the quotient and remainder of the integer division
  **truncating toward zero**: `e = 3q + ρ`, `ρ` has the sign of `e`, `ρ ∈ {−2, −1, 0, 1, 2}`
  (`−4 quo 3 = −1`, `−4 rem 3 = −1`; `−148 quo 3 = −49`, `−148 rem 3 = −1`).

## 2. Special values

| Input `x` | Result |
|---|---|
| `+0`, `−0` | `x` itself (the sign kept) **(observed)** |
| `+∞`, `−∞` | `x` itself **(observed)** |
| NaN | a quiet NaN: the result of the IEEE addition `x + x`, which on x86-64 is the input NaN with its quiet bit (bit 22) set, sign and payload kept (`0x7fc00000` gives `0x7fc00000` **(observed)**; a signalling NaN's result is its quieted form) |

Every other input is finite and non-zero, and gives a **normal** binary32 result (its magnitude
lies between about `2⁻⁵⁰` and `2⁴³`).

## 3. Range reduction

For a finite non-zero `x`, write `|x| = m · 2^e` with `m ∈ [0.5, 1)` and `e` an integer (the
float's significand and exponent, normalised to `[0.5, 1)`): `m` is exactly representable as a
binary32. Subnormal inputs are normalised the same way (the smallest, `2⁻¹⁴⁹`, has `m = 0.5`,
`e = −148`); over all inputs `e` runs from −148 to 128.

Then `∛|x| = ∛m · 2^(ρ/3) · 2^q` with `q = e quo 3`, `ρ = e rem 3` (§1): the cube root of `m`
(in `[0.7937, 1)`), times one of five constant factors, times an exact power of two.

## 4. The approximation of `∛m`

Let `M` be `m` as a binary64 (exact).

**Step 1, a quadratic start (binary64, then binary32).** With the three binary64 constants of
§5 (`c₀`, `c₁`, `c₂`):

1. `p = fl64(c₂ · M)`
2. `d = fl64(c₁ − p)`
3. `r = fl64(d · M)`
4. `s = fl64(c₀ + r)`
5. `u = fl32(s)` — the start `u ≈ ∛m`, a binary32 (`u = c₀ + (c₁ − c₂·M)·M` in Horner form,
   evaluated in exactly that order).

**Step 2, its cube (binary32).** `t = fl32(fl32(u · u) · u)` (the first product rounded to binary32,
then the second).

**Step 3, one rational correction (binary64), Halley's iteration for the cube root.** With `U`
and `T` the binary64 values of `u` and `t` (exact):

1. `a = fl64(T + 2M)` (the product `2M` is exact)
2. `b = fl64(2T + M)` (the product `2T` is exact)
3. `y₁ = fl64(U · a)`
4. `y₂ = fl64(y₁ / b)` — this is `u · (u³ + 2m) / (2u³ + m)`, the cube root of `m` to well
   beyond binary32's precision, with the cube `u³` taken as the rounded `t`
5. `y₃ = fl64(y₂ · F[ρ + 2])`, the factor of §5 for `ρ = e rem 3`
6. `y = fl32(y₃)`

The operations are left to right as listed: the product `U · a` first, then the quotient, then
the product with the factor (not `U · (a / b)`, not the factor folded into another constant).

## 5. The constants

The three coefficients are the binary64 values nearest to the decimals given (glibc's source
writes the decimals; the doubles are what count):

| | Decimal | binary64 bits | Exact value of the binary64 |
|---|---|---|---|
| `c₀` | 0.492659620528969547 | `0x3fdf87bc378ed415` (`0x1.f87bc378ed415p-2`) | 0.492659620528969555142140279713203199207782745361328125 |
| `c₁` | 0.697570460207922770 | `0x3fe6527f4927f555` (`0x1.6527f4927f555p-1`) | 0.69757046020792279339417518713162280619144439697265625 |
| `c₂` | 0.191502161678719066 | `0x3fc8832490c2fedd` (`0x1.8832490c2feddp-3`) | 0.1915021616787190572583909897730336524546146392822265625 |

The five factors `F[0] … F[4]` stand for `2^(ρ/3)`, `ρ = −2 … 2`. With `K₁` the binary64 nearest to
1.2599210498948731648 (`2^(1/3)`) and `K₂` the one nearest to 1.5874010519681994748 (`2^(2/3)`),
they are `fl64(1/K₂)`, `fl64(1/K₁)`, 1, `K₁`, `K₂` (the two quotients each rounded once to binary64):

| | For `ρ` | binary64 bits | Exact value |
|---|---|---|---|
| `F[0]` | −2 | `0x3fe428a2f98d728a` (`0x1.428a2f98d728ap-1`) | 0.6299605249474364843109697176259942352771759033203125 |
| `F[1]` | −1 | `0x3fe965fea53d6e3c` (`0x1.965fea53d6e3cp-1`) | 0.793700525984099680698591328109614551067352294921875 |
| `F[2]` | 0 | `0x3ff0000000000000` | 1 |
| `F[3]` | 1 | `0x3ff428a2f98d728b` (`0x1.428a2f98d728bp+0`) | 1.2599210498948731906665443602832965552806854248046875 |
| `F[4]` | 2 | `0x3ff965fea53d6e3d` (`0x1.965fea53d6e3dp+0`) | 1.5874010519681995834417875812505371868610382080078125 |

(`validation/investigations/specs-memory/cbrtf_constants.py` prints these from the decimals.)
Note that `F[0]` and `F[1]` are **not** the binary64 values nearest to `2^(−2/3)` and `2^(−1/3)`
in general: they are the rounded quotients, and the bits above are the ones to use.

## 6. The result

`result = σ · y · 2^q`, with `σ = +1` when `x > 0` and `−1` when `x < 0` (the negation is exact),
`y` from step 3 and `q = e quo 3`. The scaling by `2^q` is exact (the result is normal, §2): it
changes the exponent only.

So the only roundings are those of steps 1 to 3: the binary64 operations of step 1 and their
rounding to binary32, the two binary32 products of step 2, and the binary64 operations of step 3
with their final rounding to binary32.

**Examples (observed:** the system's `cbrtf`, glibc 2.35 on x86-64, run as a black box by
`validation/investigations/specs-memory/cbrtf_blackbox.py values`**)**, input bits → result bits:

| Input | | Result | Correctly rounded |
|---|---|---|---|
| `0x3f800000` | 1 | `0x3f800000` (1) | the same |
| `0x41000000` | 8 | `0x40000000` (2) | the same |
| `0x41d80000` | 27 | `0x40400000` (3) | the same |
| `0xc1000000` | −8 | `0xc0000000` (−2) | the same |
| `0x3f000000` | 0.5 | `0x3f4b2ff5` | the same |
| `0x3f400000` | 0.75 | `0x3f689768` | the same |
| `0x40000000` | 2 | `0x3fa14518` | the same |
| `0x40800000` | 4 | `0x3fcb2ff5` | the same |
| `0x3f7fffff` | 1 − 2⁻²⁴ | `0x3f800000` (1) | the same |
| `0x413f83e6` | 11.9697 (3dUnifize's example: radius exactly 8) | `0x40126666` | `0x40126667` |
| `0x3a83127b` | 0.0010000014 | `0x3dccccd4` | `0x3dccccd3` |
| `0x3f143f57` | 0.5790915 | `0x3f556188` | `0x3f556187` |
| `0x3dd561ac` | 0.1041902 | `0x3ef0ec61` | `0x3ef0ec62` |
| `0x00000001` | 2⁻¹⁴⁹ (the smallest subnormal) | `0x26a14518` | the same |
| `0x80000001` | −2⁻¹⁴⁹ | `0xa6a14518` | the same |
| `0x007fffff` | the largest subnormal | `0x2a7fffff` | the same |
| `0x00800000` | 2⁻¹²⁶ | `0x2a800000` | the same |
| `0x7f7fffff` | the largest float | `0x54cb2ff5` | the same |

## 7. Validation plan

The function is proved equal to glibc's on **all 2^32 inputs**; nothing less counts.

1. **Exhaustive, by blocks, against the black box.** `cbrtf_blackbox.py blocks` calls the system's
   `cbrtf` (from `libm.so.6`, through `ctypes`; on the main machine glibc 2.35) on every input
   and hashes the results of each block of 2^20 consecutive inputs (FNV-1a-like over the 32-bit
   results in input order: start `h = 14695981039346656037`, for each result `h = ((h XOR bits) ×
   1099511628211) mod 2^64`; a NaN result counted as `0x7fc00000`). The 4096 block hashes of
   glibc 2.35 x86-64 are committed in
   `validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json` **(observed**,
   2026-10-30, about 2.5 minutes on 22 workers; the hash of the block hashes, as the tool prints it, `0e26908cfda8506a`**)**. The implementation's test tool computes the same
   4096 hashes from its own function: every one must agree; a block that differs is listed input by
   input with `cbrtf_blackbox.py list <first input>` and compared value by value.
2. **The whole sequence:** the same hash run through all 2^32 results in input order is
   `c679cb92d4402e33` for glibc 2.35 on x86-64 (measured 2026-10-23 with a C program calling the
   C library, `docs/findings/afni-relicensing-26.md`); the implementation's must be that number.
3. **Input by input against the earlier translation** (by someone allowed to read it, not the
   implementer: the orchestrator or a validation agent): a mode of `crates-gpl/tools/cbrtf-measure`
   (or a new tool beside it) comparing the new function with `glibc_reference::floatmath::cbrtf`
   on all 2^32 inputs, NaN bits included: 0 differ. The implementer may run that program as a
   black box.
4. **Special values and NaN bits** (§2): `±0`, `±∞` exactly; NaN inputs give NaN (the block hashes
   count every NaN alike; the quiet-bit rule of §2 is checked on a few payloads of both signs, quiet
   and signalling, in the implementation's unit tests).
5. **Every platform:** the method uses only IEEE basic operations with the roundings of §4 (no
   fused operations: in Rust, plain `*`, `+`, `/` on `f64` and `f32`, never `mul_add`), so it gives
   the same bits on the six platforms; golden cases (rule 10) on §6's examples and a few hundred
   seeded inputs (bit patterns from a seeded generator, no transcendental functions), their
   checksums recorded on Linux x64.
6. **In use:** 3dUnifize `-EPI`'s radius in the AFNI replica takes this function instead of
   CORE-MATH's; its last-bit warning then goes (it never applies), `cbrtf-measure` re-run with it
   in glibc's place finds no radius, message or ball that differs, and the replica's standard
   record is re-made: no status changes, and the cases where the warning fired now match AFNI
   without it.

**Whether this spec is complete for bit-identity.** It is meant to be: the method above is the
whole computation (no table, no branch beyond §2's special values and §3's normalisation), every
rounding is stated, and the constants are given as bits. The proof is §7.1 and §7.2 on the
implementation.
