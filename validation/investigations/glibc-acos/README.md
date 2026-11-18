# glibc 2.35's double `acos` against the correctly rounded one, as a black box

Queue item 3.1-08's last piece (FreeSurfer's foundations, track C, relay 38): can
`larmorx_core::math::acos` (a port of CORE-MATH's correctly rounded `cr_acos`, bit-identical to the C
on all of `acos.wc`) also serve `reproduce`, where a replica needs glibc 2.35's bits? It can only if
glibc's `acos` is correctly rounded on every argument. This folder measures that, calling glibc's
`acos` from the system's `libm.so.6` through `dlopen`: glibc's code is never read, copied or
translated (Apache-2.0; no number or line of glibc's here).

| File | What |
|---|---|
| `src/main.rs`, `Cargo.toml` | the black-box tool (its own cargo workspace, outside larmorx's build): six families of arguments (below), each compared bit for bit; per family the count that differs, by how many units in the last place, NaN results' bits apart, the hash of glibc's results (FNV-1a in blocks of 2^22, the `glibc-dbl` record's rule), the differences per binade (families 1, 2), and every differing argument of families 4 to 6 (the first 16 of the others) into `--list` |
| `analyse.py` | summarises two `--list` files (glibc with FMA, without): family 4 per centre, below and above it; where glibc's two builds differ from each other; the direction; family 5 per binade |
| `glibc-acos-2.35-x86_64.json` | the record: glibc 2.35, x86-64 with FMA and AVX2 (the oracle machine's build of `acos`) |
| `glibc-acos-2.35-x86_64-sse2.json` | the same with glibc's FMA build switched off (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`: its build for CPUs without FMA) |

The families (SplitMix64 numbers of the state `(0xac << 56) | (family << 44) | i`; `src/main.rs`'s
header has the maps):

1. uniform on [-1, 1), 2^32 arguments;
2. every binade of `|x| < 1` equally often (subnormals included), 2^32;
3. random subnormals, 2^28;
4. the 2^25 consecutive doubles around each of 0, 0.5, 1 and the limits of `specs/glibc-dbl.md`
   §10.1 (2^-55 · 1.5, 2^-26, 0.125, 0.25, 0.75, 0.921875, 0.953125, 0.96875), both signs: 7.4 × 10^8;
5. CORE-MATH's worst cases (`src/binary64/acos/acos.wc` at commit 040ee482a8ca: 264,099 arguments,
   the hard cases of correct rounding, glibc's own test-suite failures among them), each with its
   16 neighbours on each side, both signs: 8.4 × 10^6;
6. 36 special arguments (zeros, the subnormal and normal limits, ±0.5, ±1 and their neighbours, ±2,
   the largest double, infinities, quiet and signalling NaNs with payloads).

## Result (2026-11-18, main machine: glibc 2.35, x86-64 with FMA and AVX2, 12 threads, 31 s)

**glibc 2.35's `acos` is not correctly rounded**: of 9.6 × 10^9 arguments, **4,364,142 results
differ** from the correctly rounded value (0.0454 %), every one by **one unit in the last place**
(glibc above it on 77 % of the listed ones, families 4 to 6). So `reproduce` cannot share the correctly rounded `acos` and still
give glibc's bits: an implementation with glibc's bits would be needed (a behaviour spec and a clean
room, the route of `sin` and `cos`), and `specs/glibc-dbl.md` §9 has already found that glibc's
`acos` rests on fitted tables (216 subintervals of polynomials) that no definition regenerates.

| Family | Arguments | Differ (FMA build) | Share | Without FMA |
|---|---|---|---|---|
| 1 uniform on [-1, 1) | 4,294,967,296 | 3,027,356 | 0.0705 % | 3,223,705 |
| 2 every binade of \|x\| < 1 | 4,294,967,296 | 9,611 | 0.0002 % | 10,106 |
| 3 subnormals | 268,435,456 | 0 | 0 | 0 |
| 4 neighbourhoods | 738,197,504 | 1,309,999 | 0.1775 % | 1,385,260 |
| 5 worst cases and neighbours | 8,402,784 | 17,176 | 0.2044 % | 17,189 |
| 6 special arguments | 36 | 0 | 0 | 0 |
| all | 9,604,970,372 | 4,364,142 | 0.0454 % | 4,636,260 |

- **Where.** Among the random arguments only from `|x|` about 2^-9 up (families 1 and 2: none in
  the 1,015 binades below 2^-8, among 4.2 million arguments each), most in [0.5, 1) (0.107 % of
  uniform arguments there). Around 0.125, 0.25, 0.5, 0.75, 0.921875, 0.953125 and 0.96875 (family 4)
  0.08 % to 0.7 % of the neighbours differ; around 0, 2^-55 · 1.5, 2^-26 and 1 (within 2^24 units
  of the last place) none. The hard cases (family 5) find differences at every scale, down to
  `|x|` = 2^-55 (2 arguments) and up to 1 − 3.4 × 10^-9; 16,805 of the 17,176 are the listed
  arguments themselves.
- **Special arguments, NaNs included, agree** (the same bits for ±0, ±1, ±2, ±infinity, and quiet
  and signalling NaNs with payloads): what differs is only the rounding of ordinary results.
- **glibc's own result depends on the CPU.** Its build without FMA (`GLIBC_TUNABLES`) differs from
  its FMA build: in family 4, 206,113 arguments differ from the correctly rounded value only in the
  FMA build and 281,374 only in the other (where both differ, they agree with each other); the
  hashes of families 1, 2, 4 and 5 differ. So "glibc's `acos`" is two functions; the oracle
  machine (and FreeSurfer's reference runs) use the FMA build.

The lists of differing arguments (every one of families 4 to 6, both builds):
`<workspace>/oracles/logs/fs-found-c-r38-acos-{fma,sse2}-list.txt.gz` (23 and 24 MB; `# family index
argument-bits glibc-bits correctly-rounded-bits argument`); the logs
`fs-found-c-r38-acos-{fma,sse2}.log` and `fs-found-c-r38-acos-analyse.log` beside them.

## How to run it

```bash
cd validation/investigations/glibc-acos
CARGO_TARGET_DIR=<scratch>/target cargo build --release --offline
<workspace>/larmorx/scripts/orchestration/boxed.sh 4 <scratch>/target/release/glibc-acos-blackbox \
    --threads 12 --log2n 32 --wc <workspace>/tags/core-math/src/binary64/acos/acos.wc \
    --out glibc-acos-2.35-x86_64.json --list <scratch>/fma-list.txt
GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA <workspace>/larmorx/scripts/orchestration/boxed.sh 4 \
    <scratch>/target/release/glibc-acos-blackbox --threads 12 --log2n 32 \
    --wc <workspace>/tags/core-math/src/binary64/acos/acos.wc \
    --out glibc-acos-2.35-x86_64-sse2.json --list <scratch>/sse2-list.txt
<workspace>/larmorx/scripts/orchestration/boxed.sh 6 python3 -P analyse.py \
    <scratch>/fma-list.txt <scratch>/sse2-list.txt <workspace>/tags/core-math/src/binary64/acos/acos.wc
```

Memory: under 300 MB (family 5's arguments, the lists); about 30 s on 12 threads per build. The
record is meaningful on x86-64 Linux with glibc 2.35 and a CPU with FMA and AVX2.
