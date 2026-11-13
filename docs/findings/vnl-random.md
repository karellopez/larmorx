# vnl_random (ITK 4.13.2) and the C library's drand48

Source-derived (VXL's `vnl_random.cxx` as ITK 4.13.2 bundles it, BSD-3-Clause; *read* entries
below): closed to the implementer of a clean room of `vnl_random` (the permissive ports' clean
rooms, later). The `drand48` entries come from POSIX and black-box runs of glibc 2.35 (glibc's code
never read). For `larmorx_core::vnl_random` and `larmorx_core::drand48` (item 3.1-04 of larmorx's
FreeSurfer foundations). Written 2026-11-12; which FreeSurfer programs use them, with which seeds:
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers).

**F-VNLR-1. `drand64` draws left to right in GCC 8.4.0's code** (*verified*). `drand64(a, b)` is
`(double(lrand32())/0xffffffff + double(lrand32())/(double(0xffffffff)*double(0xffffffff)))*(b-a)
+ a` (`vnl_random.cxx` 220); C++ leaves the order of the two calls open. GCC 8.4.0 at `-O3` keeps
the first call's result across the second and divides it by `0xffffffff`; `fs-random` confirms it
on 4,000,005 calls (0 differences). **larmorx**: draws left to right.

**F-VNLR-2. A copy of a `vnl_random` loses its cached normal number** (*read*). The copy
constructor and `operator=` copy `mz_previous_normal_flag` but not `mz_previous_normal`, so a copy
made between the two halves of a polar pair returns uninitialised memory from its next `normal()`
(ITK 4.13.2 and 5.4.5 alike). **larmorx**: `Clone` copies the cached number; no switch (memory
contents are never reproduced: D13).

**F-VNLR-3. `lrand32(lower, upper)` overflows `int`** (*read*). The range is `upper - lower + 1`
computed in `int` and converted to `unsigned long`: beyond `2^31 - 1` it overflows (undefined
behaviour; in practice a range of `2^32` divides by zero, and one from `2^31` to `2^32 - 1`
becomes a huge `unsigned long`, a divisor of 0 and a rejection loop that never ends), and
`lower > upper` (the assertion is off under `NDEBUG`) divides by zero or never returns. **larmorx**: the range in 64 bits, the
full range of `int` accepting every draw, `lower > upper` refused (a panic, as the assertion).

**F-VNLR-4. Only a seed's low 32 bits matter** (*read*). The seeding generator computes
`(previous · 1664525 + 1) & 0xffffffff` in `unsigned long`, so a negative `long` seed or one above
`2^32` acts as its low 32 bits. **larmorx**:
`VnlRandom::new(seed as u64)` gives the same sequences.

**F-VNLR-5. glibc's logarithm in `normal()`** (*verified*). `normal` and `normal64` call the C
library's `log` (glibc 2.35 on the reference machine; `-fno-builtin` keeps it a call). With the
correctly rounded `math::log` instead, 125 of about 667,000 normal numbers change in `fs-random`.
**larmorx**: the caller passes the logarithm (`math::log_glibc` for the original's bits).

**F-D48-1. glibc's state before any seeding is `X = 0`** (*verified*): POSIX leaves the first
values of `drand48` without `srand48`, `seed48` or `lcong48` open; glibc 2.35 starts from `X = 0`
with the default multiplier and addend. **larmorx**: `Drand48::default()`.

**F-D48-2. The rest is POSIX's definition, bit for bit** (*verified*): `drand48` and `erand48`
return `X / 2^48` exactly, `lrand48`/`nrand48` the high 31 bits, `mrand48`/`jrand48` the high 32 bits
as a signed number; the caller's array holds `X` least significant word first and is updated in
place; the array functions use the multiplier and addend `lcong48` set; `srand48` and `seed48`
restore the defaults; `seed48` returns the previous `X`. 12,002,016 calls against glibc 2.35, 0
differences.
