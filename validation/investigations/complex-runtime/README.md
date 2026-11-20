# The C runtime's complex functions behind FreeSurfer's matrix square root, as black boxes

The official FreeSurfer 8.2.0 `mri_robust_register` imports `cabs` and `csqrt` from glibc 2.35 and
`__divdc3`, `__muldc3` from the system's `libgcc_s.so.1` (its dynamic symbols, `nm -D`): the complex
absolute value, square root, division and multiplication its matrix square root needs. This folder measures those four functions **as black boxes**:
their code (glibc's LGPL, libgcc's GPL) is never read; each is compared bit for bit with a candidate
written from a published algorithm (Kahan, "Branch Cuts for Complex Elementary Functions", 1987, for
the square root; Smith, "Algorithm 116: Complex division", CACM 1962, and C11's Annex G for division
and multiplication).

`bb.c` (Apache-2.0): `gcc -O2 -fno-builtin -o bb bb.c -lm && ./bb <log2 n> <family>`; families of
SplitMix64 arguments: 0 uniform on [-1, 1), 1 log-uniform magnitudes in [2^-30, 2^30] with random signs,
2 random bit patterns (finite only). It prints how many results differ from: the square root `t =
sqrt(0.5 (hypot(x, y) + |x|))`, `(t, 0.5 (y / t))` for `x >= 0`, else `(0.5 (|y| / t), copysign(t, y))`
(A), and the same with `(|z| + |x|) 0.5` and `y / (2 t)` (B); Smith's division; the plain product.

## Result (2026-11-18, the main machine: glibc 2.35, GCC 12.3's libgcc_s, 2^22 cases per family)

| Family | Cases | csqrt vs A | csqrt vs B | `__divdc3` vs Smith | `__muldc3` vs plain |
|---|---|---|---|---|---|
| 0 uniform | 4,194,304 | 0 | 0 | 0 | 0 |
| 1 log-uniform | 4,194,304 | 0 | 0 | 0 | 0 |
| 2 random bits | 4,186,096 | 2,032 | 15,003 | 0 | 0 |

- **glibc's `csqrt` is formula A with glibc's own `hypot`** wherever nothing overflows or underflows; the
  2,032 differences of family 2 are arguments within a few binades of overflow or underflow, where glibc
  scales (A overflows to infinity or flushes to 0).
- **`__divdc3` is Smith's algorithm** on every finite argument measured; **`__muldc3`** the plain
  product (it is reached only when a product's parts are both NaN).
- glibc's `hypot` (and so `cabs` and `csqrt`) is not correctly rounded on about 0.5 % of arguments
  (`validation/probes/coremath-hypot`); the project decided the correctly rounded `hypot` in both modes
  (D4). How FreeSurfer's replica takes these functions: the replica's findings (closed to its clean-room
  implementers).

## csqrt's form (2026-11-20, track C relay 41)

`csqrt_form.c` (Apache-2.0): `gcc -O2 -fno-builtin -o csqrt_form csqrt_form.c -lm && ./csqrt_form
<log2 n>` compares glibc 2.35's `csqrt` with the form `larmorx_core::cplx::csqrt` takes (glibc's own
`hypot` inside the form, so only the form is compared), on five families of SplitMix64 arguments in
three modes (both parts drawn, the real part zero, the imaginary part zero). The form, read from the
measurements:

- the special values of C11's Annex G, with glibc's NaN `0x7ff8000000000000` wherever a part is NaN
  without an infinity to keep (an infinite imaginary part gives `(+inf, y)`; an infinite real part
  `(+inf, ±0)` for `+inf`, `(+0, ±inf)` for `-inf`, the other part the default NaN for a NaN `y`);
  `(±0, ±0)` gives `(+0, y)` (the measured table is in the unit test `csqrt_special_values_are_glibcs`
  of `crates/larmorx-core/src/cplx.rs`);
- a part above `DBL_MAX / 4`: both parts times 1/4, the result doubled; both parts below `2 DBL_MIN`:
  both times 2^108, the result times 2^-54;
- then a zero real part: `r = sqrt(|y| / 2)` in both parts (`(r, copysign(r, y))`; Kahan's formula
  would round `y / (2 t)` differently on about half of the arguments); a zero imaginary part:
  `(sqrt(x), y)` for `x > 0`, `(+0, copysign(sqrt(-x), y))` for `x < 0`; otherwise Kahan's `t =
  sqrt((hypot(x, y) + |x|) / 2)`, `(t, (y / t) / 2)` for `x >= 0`, `((|y| / t) / 2, copysign(t, y))`
  for `x < 0`.

| Mode | Family | Cases (2^22 drawn) | Differ |
|---|---|---|---|
| both parts | 0 uniform | 4,194,304 | 0 |
| both parts | 1 log-uniform | 4,194,304 | 0 |
| both parts | 2 random bits | 4,190,301 | 47 |
| both parts | 3 log-uniform over the whole range | 4,194,285 | 0 |
| both parts | 4 near overflow or subnormal | 4,023,978 | 10,616 |
| real part zero | 0 to 4 | 20.4 million | 0 |
| imaginary part zero | 0 to 4 | 20.4 million | 0 |

The differences are all arguments with a part above `DBL_MAX / 4` whose result has a part much
smaller than the other (glibc keeps more bits of it there); FreeSurfer's matrix square roots never
reach them (the eigenvalues of registration matrices). With the correctly rounded `hypot` in place
of glibc's (the project's decision), the replica's `csqrt` differs from glibc's also where glibc's
`hypot` misses the last bit (about 0.5 % of the general arguments: GLIBC-07's `hypot`).

## libgcc's division and product on special values (2026-11-20, track C relay 41)

`annexg.c` (Apache-2.0): `gcc -O2 -fno-builtin -o annexg annexg.c -lm && ./annexg` compares the
system's `__divdc3` and `__muldc3` (Ubuntu 22.04's `libgcc_s.so.1`, GCC 12) with Smith's division
and the plain product with C11 Annex G.5.1's recovery (the algorithms of
`larmorx_core::cplx::divdc3` and `muldc3`) on every combination of 15 values for the four parts
(zeros of both signs, ±1, -2, ±inf, quiet and signalling NaNs with payloads, the smallest
subnormal, the largest number, 0.1, 1e20, -2^63, the smallest normal): 50,625 cases each.

| | bits differ | beyond the NaNs' bits |
|---|---|---|
| `__divdc3` | 5,545 | 1,825 |
| `__muldc3` | 2,346 | 0 |

So the families above (random numbers, where every pair of NaNs was counted as equal) hid two
things: the system's `__divdc3` is not plain Smith where a part of the divisor is zero or
subnormal or near overflow (GCC 12's division scales its operands and changes its formula when the
ratio of the divisor's parts underflows; for example `(0 + 1i) / (0 + 2^-1074 i)` gives `(inf,
-nan)`, Smith `(inf, +0)`), and both pick other NaN bits than the Annex G algorithms written in
source order. The matrix square root divides sums of square roots of eigenvalues (parts of
magnitude about 1, or exactly zero imaginary parts, where the two agree: the fs-robust probe's
9,400 square roots are identical); the extreme operands are an open item of the replica (GCC 12's
scaled division and the NaN bits, measured as a black box).
