# The clean-room `cbrtf` with glibc 2.35's bits: the proof on all 2^32 inputs

`larmorx_core::math::cbrtf_glibc` is a clean-room implementation (Apache-2.0) of the cube root
that AFNI's programs get from the C library on Linux x86-64 (glibc 2.35), written only from
`specs/cbrtf.md` and black-box runs of the system's `cbrtf`. It serves `reproduce` in the
clean-room 3dUnifize's `-EPI` radius (`specs/3dUnifize.md` §7; register entry GLIBC-05: glibc's
`cbrtf` is not correctly rounded). The default keeps the correctly rounded
`larmorx_core::math::cbrtf` (CORE-MATH), as the user decided on 2026-10-30.

## What the verifier does

`verifier/` is a small Rust program of its own (its own cargo workspace, never part of larmorx's
build). It opens the C library's `libm.so.6` with `dlopen` and calls its `cbrtf` as a black box
(its code is never read), and on every one of the 2^32 float inputs:

1. compares `cbrtf_glibc`'s result with the system's, bit for bit, NaN results included (no
   canonicalisation in this comparison);
2. computes the spec's block hashes (§7.1: 4096 blocks of 2^20 consecutive inputs, FNV-1a-like
   over the 32-bit results in input order, a NaN counted as `0x7fc00000`) for both functions,
   and compares them with the 4096 hashes the spec writer committed from glibc 2.35
   (`validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json`);
3. computes the same hash through the whole sequence (§7.2), and through every 4099th input
   (the unit test `matches_glibc_on_a_stride`);
4. counts the inputs where glibc's result is not the correctly rounded one
   (`larmorx_core::math::cbrtf`).

`list <hex input>` prints a block's differing inputs; `bench` times the three cube roots.

## Result (2026-10-30, main machine, glibc 2.35, x86-64, 22 threads): `result-2026-10-30.json`

| | |
|---|---|
| Inputs compared bit for bit | 4,294,967,296 of 4,294,967,296 identical (0 differ) |
| NaN inputs (both signs, quiet and signalling) | 16,777,214, every result's bits identical (the quiet bit set, sign and payload kept) |
| Block hashes (§7.1) | 4096 of 4096 equal to the committed hashes of glibc 2.35, for `cbrtf_glibc` and for the system's `cbrtf` alike |
| Whole-sequence hash (§7.1's hash through all inputs) | `335d1fb5fb714c95`, from the system's results and from `cbrtf_glibc`'s (the unit test `matches_glibc_everywhere`) |
| Every 4099th input | `fcc5fa1d77606cb4` (the unit test `matches_glibc_on_a_stride`) |
| glibc's results not correctly rounded | 453,492,162 of the 4,278,190,082 non-NaN inputs (10.6 %) |
| Time | about 7 s on 22 threads (both functions on every input) |

**The spec's §7.2 number, `c679cb92d4402e33`, is not reproduced** by the system's own results:
§7.1's hash run through all 2^32 results of the system's `cbrtf` gives `335d1fb5fb714c95`
(NaN canonicalised; `0593a26dd8f14c95` with NaN bits kept; byte-wise FNV-1a over the
little-endian results `1d1f9003574eee11`, `13473947173c4391` with NaN bits kept). Since every
input agrees bit for bit with the system's function, and every block hash with the committed
ones, the difference is in how that number was formed, not in the function: a correction for the
spec writer (§7.2 should give the convention of the 2026-10-23 program, or this number).

**Speed** (`bench`, one thread, 2^26 normal inputs, best of 5): `cbrtf_glibc` 3.1 ns per call;
the system's `cbrtf` (through a function pointer) 7.1 ns; CORE-MATH's correctly rounded `cbrtf`
3.7 ns.

## How to run it

```bash
cd validation/investigations/cleanroom-cbrtf/verifier
CARGO_TARGET_DIR=<scratch>/verifier-target cargo build --release --offline
<workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    <scratch>/verifier-target/release/cbrtf-glibc-verify verify \
    ../../specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json --threads 22 --out result.json
<scratch>/verifier-target/release/cbrtf-glibc-verify bench
```

It ends with `VERIFY-PASS` (every input identical and every block hash equal) or `VERIFY-FAIL`
(exit status 1). The memory it needs is under 1 GiB. On another C library or architecture the
system's results differ, and so the comparison does: the committed hashes are glibc 2.35's on
x86-64. The unit tests in `crates/larmorx-core/src/math/cbrtf_glibc.rs` carry the stride and
whole-sequence hashes, so every platform's `cargo test --release -- --ignored` checks the same
bits without a C library.
