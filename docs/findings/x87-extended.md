# The x87's extended arithmetic, as FreeSurfer's binaries compute it

What the machine's x87 does with `long double` on x86-64 Linux, measured for
`larmorx_core::x87::F80` (the FreeSurfer foundations' item 3.1-03): FreeSurfer 8.2.0 computes in
`long double` in 12 reached functions (`docs/freesurfer/variation.md` §1), which GCC 8.4.0 compiles
into x87 instructions. Not source-derived: Intel's manual and black-box runs of the hardware and
of GCC 8.4.0's code (`validation/probes/x87-extended`). Written 2026-11-12.

**F-X87-1. The mode is Linux's default and every operation rounds once** (*verified*). The control
word of a process is `0x037f` (precision control 64 bits, round to nearest even, every exception
masked). Addition, subtraction, multiplication, division and `fsqrt` give the correctly rounded
result of the exact operation with the format's 15-bit exponent range: tiny results are rounded
once to the denormal grid `2^-16445` (7.8 million denormal products, 8.0 million denormal
quotients in the probe, every one equal), large ones overflow to infinity. **larmorx**: `F80`
implements exactly this; 4,748,983,348 comparisons with the hardware, 0 differences.

**F-X87-2. The NaN rules are Intel's, on an AMD Zen 4** (*verified*). Of two NaNs the quiet one
wins over a signalling one; of two of the same kind the larger significand, and with equal
significands the positive one; a signalling NaN is made quiet (bit 62); a NaN and a number give the
NaN. Every invalid operation (`∞ - ∞`, `0 × ∞`, `0/0`, `∞/∞`, the square root of a number below
zero, `-∞` included) gives the *real indefinite* (sign set, significand `0xC000_0000_0000_0000`),
and so does any operand in an encoding the x87 no longer supports (pseudo-NaN, pseudo-infinity,
unnormal), even beside a NaN. Pseudo-denormals (exponent 0, integer bit set) are read as numbers of
exponent 1 and never written: results are canonical. **Where**: Intel SDM vol. 1, §4.8.3.5 and
§8.2.2; measured on the reference machine (AMD Ryzen 9 7945HX). Not checked on an Intel CPU (the
helper laptop was unreachable on 2026-11-12); the official FreeSurfer runs of this project ran on
the AMD machine. **larmorx**: `F80` follows the rules; NaN payloads and signs are compared in the
probe.

**F-X87-3. The conversions GCC 8.4.0 emits** (*verified*, `build.sh` keeps the disassembly):
`float`, `double` and the integer types load exactly (`fld`, `fild`; a signalling NaN becomes
quiet; `unsigned long` with bit 63 set is loaded as signed and `2^64` added, exact); stores to
`float` and `double` round once with the target's subnormals and overflow (`fst`), NaNs keep the
high bits of their payload, an unsupported encoding stores the target's indefinite (`-NaN`). C's
casts to integers set the rounding control to *chop* around one `fistp` of the target's width
(`short` and `char` through a 16-bit store, `int` 32, `long` 64; `unsigned int` through a 64-bit
store, keeping the low 32 bits; `unsigned long` compares with `2^63` first and subtracts it, then
flips bit 63), and every NaN, infinity or value out of the store's range gives the *integer
indefinite*, the most negative value of that width: so `(unsigned long)` of `+∞` or of anything
from `2^64` is `0`, of a NaN `2^63`, and `(unsigned int)` of anything outside `long`'s range `0`.
**larmorx**: `F80::to_*_trunc` give these values; a replica that converts follows D13's rule for
undefined float-to-integer conversions (clamp and warn by default, these values under
`reproduce`), decided where the replica's code reaches one.

**F-X87-4. `sqrt` of a `long double`: three routes, one result** (*verified*). The `fsqrt`
instruction, glibc 2.35's `sqrtl` (a library call under `-fno-builtin`) and GCC's
`__builtin_sqrtl` (what C++'s `std::sqrt(long double)` compiles to: `fsqrt`, then a call to
`sqrtl` for `errno` when the argument is below zero) give the same bits on every case, the real
indefinite for negative arguments. glibc's `llrintl` rounds to nearest even with the integer
indefinite out of range.

**F-X87-5. The patterns depend on the extended precision** (*verified*). Six arithmetic patterns
of FreeSurfer's `long double` functions, written as plain loops and compiled by GCC 8.4.0 with
FreeSurfer's C++ flags, give the bits of `F80` composed in the source's order (2^20 cases each,
0 differences), and computed in `double` instead 1.8 % to 71 % of their outputs change: a replica
that replaced `long double` by `double` would not be exact.
