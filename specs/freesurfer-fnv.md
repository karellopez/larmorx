# FreeSurfer 8.2.0's FNV hashes: behaviour spec (`fs.core`, the FNV hashes)

Written by the replica author of FreeSurfer's FNV side (track A, relay 14; the orchestrator's
assignment of 2026-11-13) as its spec writer (`docs/porting-guide.md` §1): behaviour, maths,
conventions and the validation plan, never code or routines. A part of `fs.core`, kept in a file of
its own while `specs/freesurfer-core.md` is finished for the clean room by another relay; the two are
to be joined. The FNV-1a algorithm itself (Fowler, Noll, Vo; public domain) is in the main package
(`larmorx_core::fnv`, its public description).

## 1. The accumulator

FreeSurfer's FNV hashes use FNV-1a 32's offset basis 2,166,136,261 (0x811c9dc5) and prime 16,777,619,
with the hash kept in a 64-bit unsigned integer: for each byte, the hash is XORed with the byte and
multiplied by the prime modulo 2^64 (never reduced to 32 bits, so the low 32 bits are FNV-1a 32's
and the high bits carry on).

- `fnv_init()` = 0x811c9dc5. `fnv_add(hash, bytes)`: the accumulator over the bytes, from `hash`.
- `fnv_hash(i, k, counter, lo, hi)`: the accumulator from `fnv_init()` over 12 bytes: `i`, `k` and the
  counter's value, each as a 32-bit unsigned integer, little-endian; the counter is then increased by
  1 (the caller sees the new value; it is a signed 32-bit integer: past 2^31 - 1 the increase is
  undefined in FreeSurfer's language, and the replica wraps to -2^31). The result: the hash's
  low 32 bits as a `double` divided by 4,294,967,295, `f`; `(hi - lo) × f + lo` computed in `double`
  (a product then a sum, each rounded); rounded to `float` (to nearest). NaN or infinite `lo` or `hi`
  give what IEEE arithmetic gives.

## 2. `FnvHash`

- A hash starts at 2,166,136,261. Adding bytes runs the accumulator over them; adding a number adds
  its bytes as they sit in memory on a little-endian machine (`int` 4, `float` 4, `double` 8, ...).
- Printed (`<<`): the value in decimal.
- `write(filename)`: the file (created or emptied) holds the value in decimal and a newline; a file
  that cannot be opened is left alone and nothing is reported. `write(filename, suffix)`: the value,
  ` | `, the suffix and a newline. The newline is one byte (0x0a) on every platform.

## 3. Validation plan

- The probe `validation/probes/fs-maths` command `fnvh` (the official build's `fnv_hash`, `FnvHash`
  and `fnv_add` called by name): `i` among 0, 1, -1, 7, 123,456, -2·10^9, 2^31 - 1; `k` from -5 to
  100,000; counters 0, 1, 99, -7, 2^31 - 5, -2^31 with 0-4 draws; `lo` and `hi` among ±0.001, ±0, ±1,
  ±10^300, NaN, infinity, 0.1; 0-40 random bytes for `FnvHash`; the floats' bits, the counter, the
  value in decimal, `fnv_add`'s in hexadecimal, the printed form and the two files' bytes.
- Measured (2026-11-13): see `docs/findings/freesurfer-maths.md`, "FNV".
