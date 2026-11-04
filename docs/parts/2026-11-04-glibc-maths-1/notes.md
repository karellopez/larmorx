WORKTREE: <workspace>/larmorx/.claude/worktrees/agent-ab2e16f01a215a0a8
(branch worktree-agent-ab2e16f01a215a0a8, fast-forwarded to main 981d07f, then the commits below)

# glibc maths part, first half: NOTES

## Environment (the sandbox refuses `source`)
- `bash <scratch>/run.sh <command...>` runs a command from the worktree with env.sh's environment
  (PATH with the venv and cargo, FSL off, PYO3_PYTHON, OPENBLAS_CORETYPE=Haswell).
- cargo directly: `~/.cargo/bin/cargo ...`; in crates-nc/ or crates-gpl/: `cd <worktree>/crates-nc && ~/.cargo/bin/cargo ...`
  (the sandbox refuses `cd` to a computed path and `$VAR` operands: spell paths out).
- target/ seeds were copied into the worktree (root, crates-nc, crates-gpl): builds are incremental.
  The wheel's `_core.abi3.so` (built at 431b145) is in `<worktree>/python/larmorx/` (git-ignored).
- Helper scripts: `<scratch>/tools/` (splice.py, switch_nc.py, switch_afni.py: one-off edits, done;
  proofs.sh, stage2.sh, stage3.sh: the proof and gate runs). Results: `<scratch>/results/`.
  Verifier binary: `<scratch>/verifier-target/release/glibc-maths-verify` (verify | sse2 | hashes).

## Relay 1: sub-task 1 (the copy) — DONE
Commits:
- `4d223de` (1/3) the copy: `crates/larmorx-core/src/math/{sincosf,logf,expf,powf,exp,log,pow,atanf}_glibc.rs`
  + `x86_nan.rs` (SSE's NaNs, larmorx's own) + mod.rs (pub API with docs; FMA dispatch
  `glibc_dispatch!`/`fma_glibc!`; tests: FMA copies identical, glibc's seeded hashes); per-file tests with
  glibc's vectors and stride hashes; the verifier `validation/investigations/glibc-maths/verifier/` (own
  workspace; dlopen libm.so.6; `samples.rs` included verbatim by `crates-nc/larmorx-nc-core/tools/glibc-float-verify copy`);
  check_spdx MIXED entries (`Apache-2.0 AND MIT`; `Apache-2.0 AND LicenseRef-fdlibm` as tanh_glibc.rs).
- `7958e0e` (2/3) one translation: nc-core `glibc_float.rs`, `glibc_double.rs`, `fdlibm.rs` (atanf, atan2f,
  atanf_with_at0, AT0) and AFNI `sincosf.rs` (+ its `sse2` module) re-export the core's; `logf_compat`,
  `log_compat` and fdlibm's double `tanh`/`expm1` stay in nc-core; NOTICE, crates-nc/NOTICE,
  LICENSES/LicenseRef-fdlibm.txt (SunPro variant added), PROVENANCE (core, nc-core, AFNI), AFNI Cargo
  licence field (MIT dropped), docs/architecture.md (Maths bullet), docs/findings/platform-math.md
  (section "glibc's own bits for reproduce: what lives where"), the investigation README and results.
- `431b145` (3/3) the SSE2 proof's result, crates-nc/LICENSES/NOTICE-larmorx.txt re-copied (the nc
  notices test), pointers in docs/findings/{afni-tshift,fsl-mcflirt,fsl-maths}.md.
Proofs (glibc 2.35, main machine, 28 threads), bit for bit with NaN bits, 0 differ everywhere:
- `verify` at 4d223de (result-2026-11-04.json): 49 checks, 63,082,423,336 comparisons, 29 s.
  sinf/cosf/sincosf(both)/logf/expf/atanf all 2^32 vs glibc; sinf/cosf FMA+SSE2 vs the AFNI translation
  all 2^32; powf 6 x 2^28 + 30,276 special pairs; atan2f 5 x 2^28 + 30,276; exp, log 6 x 2^28 + 174;
  pow 8 x 2^27 + 30,276. Re-run at 7958e0e: pass.
- `glibc-float-verify copy` at 4d223de (copy-2026-11-04.txt): 44 checks core vs nc-core, 0 differ.
- `sse2` at 7958e0e (sse2-2026-11-04.json, GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA): the SSE2 builds vs
  glibc's all 2^32; the builds differ on 34 inputs (12 sinf, 22 cosf).
- After the switch: nc-core's `glibc-float-verify` (sinf ... atan2f vs the C library) 0 differ.
Replicas did not move: nc `cargo test --workspace` (unit tests + golden.rs), AFNI unit tests,
`larmorx-cli` `afni_replica_golden` (incl. 3dTshift wsinc5/wsinc9) and `afni_replica_cli`, Python golden.
Gates: gates-rust root + gpl OK (7958e0e), nc OK (431b145); gates-python --no-build OK (431b145: 782
passed, 620 without FSL); check_spdx OK; tables --check OK.
Findings worth a line in the handoff: the replica core's `pow` and the AFNI `sinf`/`cosf` computed NaNs
made from numbers with Rust's own arithmetic, so on AArch64 their sign bit was clear; the copies give
x86-64's NaNs everywhere (no golden case reaches them; nothing moved on x86-64).

## (Relay 1's pointers for sub-task 2, now done below)
- Sources: `<workspace>/tags/glibc-2.35/sysdeps/ieee754/dbl-64/` (s_sin.c with its reduce/do_sin/do_cos
  routines, branred.c/h, usncs.h, sincostab.c, s_sincos.c; s_tan.c + utan.h/utan.tbl; e_asin.c +
  uasncs.h/asincos.tbl; s_atan.c + atnat.h/uatan.tbl; mydefs.h, dla.h: the double-length macros, which
  use fma under `__FP_FAST_FMA`), and `sysdeps/x86_64/fpu/multiarch/s_sin-fma.c`, `s_tan-fma.c`,
  `e_asin-fma.c`, `s_atan-fma.c` (built with -mfma -mavx2: GCC contracts a*b+c, so which operations are
  fused must come from `objdump -d` of the system's libm.so.6, `__sin_fma` etc., as the Arm translations'
  were).
- The register: GLIBC-07 (docs/findings/upstream-defects.md ~line 251 and ~3626: frequencies already
  measured on image intensities); probe `validation/probes/glibc-double-maths/` may have reusable code.
- The verifier is reusable for the black-box record: add `sin`, `cos`, `tan`, `asin`, `acos`, `atan`,
  `sincos` to `Libm` (dlopen), block hashes per family (as
  `validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json`), and the share where
  glibc differs from `larmorx_core::math::{sin,cos,tan,asin,acos}` and `atan2(x, 1)` (correctly rounded).
  Put the spec's own probe and generators under `validation/investigations/glibc-dbl/`.
- Tables (sincostab: sin/cos of i/128 hi/lo; utan.tbl; asincos.tbl; uatan.tbl): a mathematical
  definition + a generator each, proved by regenerating bit for bit; a table no definition reproduces
  goes to the orchestrator (licence question). Never paste glibc's tables or code into the spec.
- Then a small model from the spec's text alone (mpmath or scratch Rust) reproducing the hash blocks.

## Relay 2: sub-task 2 (`specs/glibc-dbl.md`) — DONE for sin/cos/sincos; tan/asin/acos/atan STOPPED (tables)
Commits: `467d7b8` (record tool + record + tables prover), `21c6020` (spec, model, README, SSE2 record).
- `specs/glibc-dbl.md`: sin, cos, sincos fully (constants with definitions, table R defined, the three
  kernels, the two reductions, the paths, the two readings of FMA: fused in sin/cos's FMA build, two
  roundings in sincos's single build), record (§10), frequencies (§10.3), model results (§11), plan (§12).
- `validation/investigations/glibc-dbl/`: families.rs, blackbox/ (record/list), the record
  `glibc-dbl-2.35-x86_64-blocks.json` (7 functions, 1,600 blocks, 6.7e9 args, 14.8 s) and
  `...-sse2-blocks.json` (sin, cos with GLIBC_TUNABLES), tables.py (definitions vs glibc's tables;
  --emit, --search, --print), README.
- `crates-gpl/tools/glibc-dbl-model` (GPL-3.0-or-later AND LGPL-2.1-or-later; workspace member, Cargo.lock
  refreshed; gates-rust gpl OK): the model from the spec's text: `check` MODEL-PASS (sin, cos, sincos:
  768/768 blocks, 184/184 special each; `--sse2` 512/512 against the SSE2 record); `tables`: definition's
  table vs glibc's: 0 differ on the record's 3 x 2^30 and on 17 x 2^30 per function next to the 17 rows.
- FINDINGS for the orchestrator (licence question for the user):
  (1) table R (sin/cos of N/128): 422/440 from the definition (correctly rounded double-double); 18 low
      parts (17 rows) differ by 1-2 units of their last place (one by 39); 111 rules tried. The spec uses
      the definition; with it the model gives glibc's bits everywhere tested. Options in spec §5.3:
      (a) keep the definition (recommended), (b) give glibc's 18 numbers (LGPL data), (c) stay CR.
  (2) tan: F/FF/G are CR functions of 186 points, the points are searched (no rule); asin/acos: 216
      fitted polynomials (2,568 numbers); atan: 241 searched points + 964 fitted coefficients. Not
      specified (spec §9): the user decides whether these data may be used; the record is ready.
- GLIBC-07 frequencies (all families, of 2^27 x families): sin 0.100 %, cos 0.092 %, sincos 0.108/0.099 %,
  tan 0.188 %, asin 0.077 %, acos 0.053 %, atan 0.065 %; on uniform angles 0.138 % (sin/cos), 0.250 % tan.
- Not run (not needed: no replica or core code changed): gates root/nc, gates-python, smoke.
  ruff on tables.py OK, check_spdx OK, gates-rust gpl OK (relay2-gates-gpl.txt in results/).
- Disassembly kept in <scratch>/dis (objdump of the system's libm.so.6 with libc6-dbg symbols, annotated
  by tools/annot.py): sincos and the large reduction have 0 fused ops; sin/cos's 70 each read from it.

## Relay 3: sub-task 3 (decisions, handoff, final report) — DONE. THE PART IS COMPLETE.
Commits: `ed81380` (specs/glibc-dbl.md: §5.3 decided = the definition, the gap documented; §9 tan/asin/acos/atan
correctly rounded in reproduce too, record kept; §12 points 1 and 6; the investigation README), `efd0db2`
(docs/findings/platform-math.md: the decisions). Head efd0db2, clean.
Handoff: <scratch>/HANDOFF.toml (27 ops on 10 files, op 26 the site's roadmap row: `apply_handoff.py ... --site
<site> --check` passes in the worktree with the site's main checkout; anchors unchanged on main 9486336) and
<scratch>/HANDOFF.md (reasons, merge steps, the second half's brief points: (a) clean-room sin/cos/sincos_glibc +
originals' reproduce wiring + records; (b) a replica-side step for the FSL replicas' sin/cos/sincos; open points:
who does (b), AFNI tanhf/expm1f not in the core, ANTs out of scope).
Gates at efd0db2 (log oracles/logs/glibc-maths-relay3-gates.log, results/relay3-*.txt): gates-rust all OK (root,
gpl, nc), gates-python --no-build OK (782 passed / 620 without FSL), tables --check OK, check_spdx OK.
No parity, smoke or golden run in this half (no output can move; the merge's verify-merge runs the smoke tier).
Next: none in this part; the orchestrator merges, then the second half.

## FROM THE ORCHESTRATOR (keep this section in every rewrite)
- (2026-11-04, after relay 2) **The user decided both licence questions** (decision log, "GLIBC-07's tables"):
  (1) **keep the definition** for the sin/cos table (option (a) of §5.3: write the spec's §5.3 as decided, the 18
  differing numbers and the measured absence of any result change documented); (2) **`tan`, `asin`, `acos`, `atan` stay
  correctly rounded in `reproduce` too** (no glibc data in larmorx): the spec says so and keeps their black-box record
  for the frequencies; their behaviour sections need not be written. **Relay 3 = sub-task 3** (the handoff and the
  final report): GLIBC-07's register row and texts (sin, cos, sincos exact in `reproduce` after the second half; tan,
  asin, acos, atan correctly rounded in both modes by the user's decision, with the frequencies), next-steps (the second
  half's scope: the clean-room `sin`/`cos`/`sincos` from `specs/glibc-dbl.md`, the originals' and replicas' `reproduce`
  wired to the new functions, the records they move), `licensing.md` (the permissive functions now in the main package,
  the GPL model tool in `crates-gpl/`), the CLAUDE.md repo-map line through the handoff.
- (2026-11-04, after relay 1) Relay 1's copy is accepted as reported, the audited FMA dispatch in
  `larmorx_core::math` included (one `unsafe` call site behind `is_x86_feature_detected!`, its AUDIT and SAFETY
  comments; rule 5 allows audited code). In the handoff, add the dispatch to the architecture's list of audited
  `unsafe` sites if such a list exists.
- **Relay 2 = sub-task 2** (`specs/glibc-dbl.md`), as the brief says; split it per function family (2a sin/cos/sincos,
  2b tan, 2c asin/acos/atan) if it would pass about 250 steps. The spec is the second half's only source: behaviour,
  maths, data defined mathematically with a generator, the black-box hashes and the model's check. A table that no
  definition reproduces: stop that function and report it (a licence question for the user), go on with the others.
