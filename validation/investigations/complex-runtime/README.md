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
