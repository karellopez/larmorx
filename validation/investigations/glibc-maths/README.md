# glibc 2.35's permissive functions in `larmorx_core::math`: the proofs of equal bits

On 2026-11-04 the user decided that glibc 2.35's exact functions that come from permissive code
move into the main package, so that the clean-room originals' `reproduce` gives the original
programs' bits and every crate shares one translation (decision log, "glibc's exact maths for
`reproduce`"). They were translated in the FSL replica core (`larmorx-nc-core`'s `glibc_float`,
`glibc_double`, `fdlibm`) and, for `sinf` and `cosf`, in the AFNI replicas (`sincosf`):

| `larmorx_core::math` | glibc 2.35 (x86-64, FMA build) | From |
|---|---|---|
| `sinf_glibc`, `cosf_glibc`, `sincosf_glibc` (and the SSE2 build `*_glibc_sse2`) | `__sinf_fma`, `__cosf_fma`, `__sincosf_fma` (`__*_sse2`) | Arm's Optimized Routines v19.11, MIT |
| `logf_glibc`, `expf_glibc`, `powf_glibc` | `__logf_fma`, `__expf_fma`, `__powf_fma` | Arm |
| `exp_glibc`, `log_glibc`, `pow_glibc` | `__exp_fma`, `__log_fma`, `__pow_fma` | Arm |
| `atanf_glibc`, `atan2f_glibc` | `__atanf`, `__ieee754_atan2f` (one build) | fdlibm, Sun's notice |
| `tanhf_glibc`, `expm1f_glibc` (2026-11-13) | `__tanhf`, `__expm1f` (one build) | fdlibm, Sun's notice (`s_tanhf.c` `dacb3c557d21…`, `s_expm1f.c` `b3b864ad6abe…`) |
| `acosf_glibc` (2026-11-13) | `acosf@@GLIBC_2.2.5`: `__ieee754_acosf` behind glibc's SVID wrapper (one build) | fdlibm, Sun's notice (`e_acosf.c` `60a8c9b2d144…`); the wrapper's NaN for `\|x\| > 1` (`0x7fc00000`) measured, its code (LGPL) not translated |

The default everywhere stays correctly rounded (`larmorx_core::math::sinf`, ...); these are for
`reproduce` and the replicas. glibc's LGPL functions (the double `sin`, `cos`, `tan`, `asin`,
`acos`, `atan`: GLIBC-07) are not among them.

## What the tools do

`verifier/` is a Rust program with its own cargo workspace (never part of larmorx's build). It
opens the C library's `libm.so.6` with `dlopen` and calls its functions as a black box (the
current symbol versions: `logf@@GLIBC_2.27`, `exp@@GLIBC_2.29`, ...), and compares, bit for bit
(NaN results included, no canonicalisation):

- `verify float`: every one of the 2^32 float inputs of `sinf`, `cosf`, `sincosf` (both outputs),
  `logf`, `expf`, `atanf`, `tanhf`, `expm1f`, `acosf`, against glibc; `sinf_glibc`, `cosf_glibc` and their SSE2 builds
  against the AFNI replicas' translation (before it became a re-export); and counts where glibc's
  two builds differ.
- `verify pairs`, `verify double`: `powf`, `atan2f`, `exp`, `log`, `pow` on the seeded families
  of `verifier/src/samples.rs` (the ranges the tools meet: BET's exponents, TFCE's powers, FAST's
  energies, rotation angles, probabilities, image intensities, the overflow and underflow limits,
  subnormals, tiny and huge exponents, random bit patterns; more than 10^9 arguments per
  function) and every pair (or value) of 174 special values (zeros, infinities, quiet and
  signalling NaNs of both signs with payloads, the paths' limits and their neighbours).
- `sse2`, run with `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA` (glibc then resolves its SSE2
  builds): `*_glibc_sse2` against glibc's SSE2 builds on all 2^32 inputs, and the FMA builds
  against them (they must differ on exactly 34 inputs, which shows the SSE2 build ran).
- `hashes`: the hashes the unit tests of `larmorx_core::math` check, from glibc's own results
  (every 4099th float input; 65,536 seeded arguments of the others), so the unit tests compare
  with glibc's bits on all six platforms in CI.

`crates-nc/larmorx-nc-core/tools/glibc-float-verify copy` (in the FSL Licence's workspace, as it
links the replica core) compares the core's copies with the replica core's translations on the
same inputs (`samples.rs` is included verbatim), every 2^32 float input of the one-argument
functions too. It is the proof of the copy at the commit before the replica core became
re-exports (`4d223de`); afterwards both sides are the same function.

## Results (2026-11-04, main machine: glibc 2.35, x86-64 with FMA and AVX2, 28 threads)

| File | What | Result |
|---|---|---|
| `result-2026-11-04.json` | `verify` at larmorx `4d223de` (the copy beside the earlier translations): 49 checks, 63,082,423,336 comparisons | 0 differ; 29.1 s |
| `copy-2026-11-04.txt` | `glibc-float-verify copy` at `4d223de`: 44 checks against the FSL replica core | 0 differ |
| `sse2-2026-11-04.json` | `sse2` (glibc's SSE2 builds forced) | 0 differ; the builds differ on 34 inputs (12 `sinf`, 22 `cosf`) |
| `result-float-2026-11-13.json` | `verify float` at larmorx `d4862a00` (with `tanhf`, `expm1f`, `acosf`, and the AFNI replica's `tanhf` and `expm1f`; 6 threads): 18 checks, 77,309,411,328 comparisons | 0 differ; 71.0 s |

Per function (identical of compared; NaN results identical of NaN results):

| Function | Against glibc 2.35 | Against the earlier translation |
|---|---|---|
| `sinf_glibc`, `cosf_glibc`, `sincosf_glibc` | 4,294,967,296 each (16,777,216 NaN) | FSL core: all 2^32; AFNI: all 2^32 (and the SSE2 builds) |
| `logf_glibc` | 4,294,967,296 (2,155,872,254 NaN: negatives and NaNs) | all 2^32 (and `logf_compat` over it) |
| `expf_glibc`, `atanf_glibc` | 4,294,967,296 each | all 2^32 (`atanf` also with fdlibm's comment value of `aT[0]`) |
| `tanhf_glibc`, `expm1f_glibc` | 4,294,967,296 each (16,777,214 NaN each) | AFNI replica's `fdlibm::tanhf`, `expm1f` (3dUnifize's): all 2^32 (2026-11-13) |
| `acosf_glibc` | 4,294,967,296 (2,164,260,862 NaN: `\|x\| > 1` and NaNs) | – (new, 2026-11-13) |
| `powf_glibc` | 1,610,612,736 sampled + 30,276 special pairs | the same |
| `atan2f_glibc` | 1,342,177,280 sampled + 30,276 special pairs | the same |
| `exp_glibc`, `log_glibc` | 1,610,612,736 sampled + 174 special each | the same |
| `pow_glibc` | 1,073,741,824 sampled + 30,276 special pairs | the same |

The replicas' outputs did not move: after the switch to re-exports, the FSL replicas' unit tests
and golden test (`crates-nc`, `cargo test --workspace`) and the AFNI replicas' unit tests and
golden cases (`crates/larmorx-cli/tests/afni_replica_golden.rs`, which include 3dTshift's weighted
sinc) pass unchanged.

## How to run it

```bash
cd validation/investigations/glibc-maths/verifier
CARGO_TARGET_DIR=<scratch>/verifier-target cargo build --release --offline
LARMORX_COMMIT=$(git rev-parse --short HEAD) <workspace>/larmorx/scripts/orchestration/boxed.sh 8 \
    <scratch>/verifier-target/release/glibc-maths-verify verify --threads 28 --out result.json
GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA <workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    <scratch>/verifier-target/release/glibc-maths-verify sse2 --threads 28 --out sse2.json
<scratch>/verifier-target/release/glibc-maths-verify hashes
# the copy against the FSL replica core (meaningful at 4d223de; same functions afterwards):
cd ../../../../crates-nc && cargo build --release -p glibc-float-verify
<workspace>/larmorx/scripts/orchestration/boxed.sh 8 target/release/glibc-float-verify --threads 28 copy
```

Meaningful only on x86-64 Linux with glibc 2.35 and a CPU with FMA and AVX2 (the oracle machine).
Memory: well under 1 GiB.
