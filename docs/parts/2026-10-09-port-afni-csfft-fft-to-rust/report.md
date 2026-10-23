# Final report: Port AFNI csfft FFT to Rust

Agent `afd9ce2a8a5d9f694`, 2026-10-09. The agent's report to the orchestrator, verbatim.

## AFNI `csfft_cox` ported to Rust: bit-exact against the compiled AFNI library

The port matches `libmri.a` bit for bit on every length AFNI's own FFT code handles: 0 mismatches. Two things need your attention before anything else:

1. **The workspace won't build until you add `crates/larmorx-afni/Cargo.toml`.** `members = ["crates/*"]` picks up the new directory, and `cargo metadata` now fails with "failed to load manifest for workspace member". Every cargo command in the repo fails until the manifest exists, including those of other agents running concurrently. The only dependency is `libm.workspace = true`. Edition 2024 is fine. Your `lib.rs` should declare `pub mod csfft;` and keep `#![forbid(unsafe_code)]`.
2. **`csfft.c` is not public domain.** Its header says "copyrighted by the Medical College of Wisconsin, 1994-2000 … released under the Gnu General Public License, Version 2". AFNI's `LICENSE.txt` lists MCW-copyrighted code (Cox et al.) as a GPL-2 exception to its public-domain notice. The ported routines are all from the MCW period (1998–1999). Only the `fftn` routing is later NIH work. This is exactly what PLAN.md's per-file check for AFNI is meant to catch. A GPL-2 derivative inside an Apache-2.0 crate needs your decision. I recorded the facts in the file header and wrote that the licence status is unresolved.

**File:** `/home/karelo/VScode_projects/super_fmriprep/larmorx/crates/larmorx-afni/src/csfft.rs`, 996 lines, about 210 of which are tests. It is already formatted with `rustfmt`.

## API
- `Complex32 { pub r: f32, pub i: f32 }` (same layout as AFNI's `complex`), with `Complex32::new`.
- `csfft_cox(mode: i32, x: &mut [Complex32])`: in place, unscaled. `mode > 0` uses exp(+i…); any other value, including 0, uses exp(−i…), as in AFNI. Lengths 0 and 1 are a no-op. It panics with a message mentioning `fftn` for lengths AFNI would hand to `fftn`. It builds the tables on every call.
- `CsfftPlan::new(n) -> Result<CsfftPlan, UnsupportedLength>`, plus `.execute(mode, x)`, `.execute_with_scratch(mode, x, scratch)`, `.scratch_len()` (always under 2n) and `.size()`. The plan is immutable and `Send + Sync`. **Use it in 3dTshift:** one plan per length, one scratch buffer per thread.
- `csfft_cox_handles(n) -> bool`: true when n ≤ 1, or when `csfft_nextup_even(n) == n` and n ≤ 32768.
- `csfft_nextup`, `csfft_nextup_one35`, `csfft_nextup_even` (the `internal_check = 1` version, as requested), all on `usize`.
- `UnsupportedLength(pub usize)` implements `Error`. There is also the constant `CSFFT_COX_MAX_LEN = 32768`.
- There is no global state. The tables are computed with exactly the C expressions.
- `cos`/`sin` use `libm` with a `// TODO(crmath)` comment, because `larmorx_core::math` has no sin/cos yet.

## Verification against the compiled `libmri.a`
- **`csfft_cox`:** all 139 lengths AFNI routes to its own code (2..=32768). The length set matches exactly between Rust and C.
  - Each length got 10 input vectors: 5 uniform in [−1000, 1000]; one of special magnitudes (±0, subnormal, tiny, huge up to 1e33, small integers); a zero-padded pair series like 3dTshift's; a zero-padded real integer series; all signed zeros; and one impulse.
  - Every vector ran with modes −1 and +1; two of them also ran with modes 0 and 2.
  - That is 3,336 records and **36,950,784 f32 values, with 0 bit mismatches** (and 0 NaN outputs).
- **3dTshift lengths:** `csfft_nextup_one35(ntt+4)` for ntt = 3..=4000 gives 36 distinct lengths (8..=4096), all of them handled and tested.
- **AFNI's static caches don't affect results:** rerunning the C transforms in a shuffled length order changed 0 values.
- **General radix-2 loop:** no AFNI-routed length reaches it, because the switch covers every power of two up to 32768. I compared it with a verbatim copy of the C, compiled with AFNI's flags, for n = 2..=65536: 2,621,400 values, 0 mismatches.
- **The nextup functions:** `csfft_nextup`, `csfft_nextup_one35` and the internal `csfft_nextup_even` agree with C for every n in 1..=40000 (0 mismatches each).
- **`cos`/`sin`:** over all 539,282 arguments the tables use, `libm` differs from glibc 2.35 `sincos` by 1 ulp in double about 3.3% of the time (17,745 cos, 17,583 sin), and **0 times after rounding to f32**.
- **Mutation checks** confirm the comparison would catch errors: each deliberately wrong variant below produced mismatches.
- **Lint and tests:** `cargo clippy --all-targets -- -D warnings` is clean. The 8 unit tests pass in debug and release (nextup values, the length set, forward-then-inverse equals n·x for all 139 lengths, a naive DFT check, panic and error cases, scratch reuse, Send + Sync). `cargo doc` with `-D warnings` is clean.

## C details the port has to reproduce
1. **`fft_3dec`:** `(bbr-ccr)*SS3` multiplies a float by a *double* constant (`0.8660254038`) and rounds the product to f32. Doing it in f32 gives 5.7M mismatches. `t4*CC3` is also a double product but exact, so doing it in f32 changes nothing (0 mismatches).
2. **The generated kernels differ from each other.** All three skip the multiply when the twiddle is 1 (i0 = 0). Only `fft32` also has sine-only butterflies at i0 = m/2 (`f1 = -x.i*s; f3 = x.r*s`). Omitting those gives 3.8M mismatches. I wrote fft8/16/32 as one loop with these variants instead of 1,300 generated lines. A parser checked every C statement against that model (scratch `check_unrolled.py`).
3. **Skipped multiplies versus the general loop** differ only in the sign of zero results. Signed-zero inputs were the only ones that caught it (21 mismatches). This matters for 3dTshift's zero-padded rows.
4. **`csfft_trigconsts`:** the angle `al = f1*PI/f2` is computed in double, cos/sin are rounded to f32, and then an f32 recurrence follows. Computing the angle in f32 gives 21.7M mismatches.
5. **`fft64`** treats k = 0 without a twiddle; `fft128` and up multiply by `cs[0] = (1, 0)`. The twiddle tables use `(double)k*th`, with `th = 2.0*PI/N` (or `PI/32.0` for `fft64`).
6. **`fft_5dec`:** `s2 = 2.0*c72*s72` is a double product, and `cs.i * ss` converts an int to float. Both turned out equivalent to the obvious f32 forms (0 mismatches).
7. **gcc merges each cos/sin pair into a single `sincos` call.** The objects contain no FMA and no x87 code.
8. **`fftn` routing:** inside `csfft_cox`, `csfft_nextup_even` runs with `internal_check = 1`. AFNI's exported `csfft_nextup_even` only rounds up to even, which I verified over 1..=40000. The recursive `csfft_cox` calls from the 3- and 5-decimations repeat the routing test, but it never sends a supported length to `fftn`.
9. **`csfft_nextup` overflows `int` in C** above about 636,000 (undefined behaviour); the Rust version uses `usize` and agrees wherever C is defined. For 3dTshift, lengths above 32768 (ntt above about 32,760) would go to `fftn`, which isn't ported.
10. **Not ported:** `csfft_scale_inverse` (3dTshift never sets it, so the unscaled default is what's reproduced) and `csfft_many`.

## Scratch files
Everything is in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/csfft-port/`:
- `harness_cox.c`: the oracle linked against `libmri.a`.
- `harness_inc.c`: compiles `csfft.c` itself to reach its static functions.
- `check_unrolled.py` and `mutate.py`.
- `verify/`: the bit-exact comparison binary.
- `lint/`: the deliverable as a library module, for clippy and tests.
- `data/`: the oracle outputs, about 330 MB.
