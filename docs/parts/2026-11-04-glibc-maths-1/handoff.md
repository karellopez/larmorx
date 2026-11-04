# HANDOFF: glibc's exact maths for `reproduce`, first half (relays 1 to 3)

Branch `worktree-agent-ab2e16f01a215a0a8` (worktree
`<workspace>/larmorx/.claude/worktrees/agent-ab2e16f01a215a0a8`), from main 981d07f (main has since
gained only 9486336, the decision log's "GLIBC-07's tables"); commits 4d223de, 7958e0e, 431b145
(relay 1: the copy), 467d7b8, 21c6020 (relay 2: the spec, the record, the model), ed81380 and
efd0db2 (relay 3: the user's decisions in the spec, its README and `platform-math.md`; head, clean,
the commit the final gates ran on). No site branch: this part has no program and no tool page.

The exact changes to the shared documents are `HANDOFF.toml` beside this file: 27 operations on
10 files. `python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <site>
--check` passes in the worktree with the site's main checkout (op 26 is the site's roadmap row;
without `--site` only that op is refused). The anchors are unchanged on main (9486336 touches only
the decision log). No TOML, JSON or YAML file is touched; `validation/tools.toml` and the
generated tables do not change; the register's counts do not change (no new entry).

## At the merge (the orchestrator)

1. Apply `HANDOFF.toml` (with `--site`), re-read the touched files (docs/orchestration.md step 6).
   `register_counts.py`: unchanged (386 entries); `python -m larmorx_validation tables`: no change.
2. Verification: the Rust code that changed is `larmorx-core`'s `math` (relay 1: the copies, the
   dispatch, the tests), `larmorx-nc-core` (`glibc_float.rs`, `glibc_double.rs`, `fdlibm.rs` now
   re-export the core's) and `larmorx-afni-replica`'s `sincosf.rs` (re-exports), plus the new GPL
   member `crates-gpl/tools/glibc-dbl-model` (its `Cargo.lock` refreshed). This half ran no parity
   suite or smoke tier (the brief: only if a replica's output could move, and no test showed a
   moved bit: the replicas' unit tests, the nc golden test, the AFNI golden cases and the Python
   golden suite pass unchanged on this machine); `verify-merge.sh` runs the smoke tier as usual.
   No golden checksum changes, none recorded.
3. CI: no new job, no new golden case. The copies' unit tests (glibc's own vectors, recorded
   values, seeded hashes of glibc's results, FMA copies against portable ones) run in every
   platform's `cargo test`. The two investigation tools (`validation/investigations/glibc-maths/verifier`,
   `validation/investigations/glibc-dbl/blackbox`) are their own cargo workspaces, not built by CI
   or the gates (they `dlopen` the system's `libm.so.6`).

## Why each change

- **The register (GLIBC-02, -03, -07, the dated paragraph).** GLIBC-02 and GLIBC-03 named the FSL
  replica core's translations and, in GLIBC-03, an LGPL translation in the AFNI replica that the
  move of 2026-10-29 had already replaced with Arm's MIT code: both now say where the one copy
  lives (`larmorx_core::math`) and how it was proved. GLIBC-07's row and entry carry the user's two
  decisions of 2026-11-04 (the table from its definition; `tan`, `asin`, `acos`, `atan` correctly
  rounded in `reproduce` too), the spec, the record's shares (§10.3), and a gap column that names
  what the second half closes. The summary row stays behaviour-only (the table's rule).
- **The API page's GLIBC-07 row:** the user-facing state (nothing changes for users in this half;
  the four functions' decision is final, the three others are planned).
- **next-steps:** item 4 lists the part; item 7 gets the first half's paragraph and the second
  half's scope (below); the two follow-ups that waited for this (fslmaths' original's `exp`,
  `log`, `pow`; FLIRT's original's `sinf`, `cosf`, `atan2f`) now say the copies exist and the
  wiring is the second half's; the GLIBC-07 follow-up says it is scheduled.
- **licensing.md:** Arm's row (now nine functions, in the core, one translation), a row for fdlibm
  and a row for glibc's own LGPL code (never copied; the behaviour specs); `larmorx-gpl`'s row
  (development tools); the FSL replicas' notices paragraph (the double `pow`/`exp` translated
  once, in the core); the process rules close `crates-gpl/` as a whole to clean-room implementers
  (every tool there keeps or reads glibc's LGPL code, and `glibc-dbl-model` was written by someone
  who had read it).
- **CLAUDE.md** (the orchestrator's file): rule 1's permissive list gains fdlibm; `crates-gpl/`'s
  description and the clean-room list name the whole directory with its glibc tools; rule 2's Arm
  bullet names the core's copies and adds glibc 2.35 itself (behaviour specs only); the repo map's
  `crates-gpl/` row names `tools/glibc-dbl-model`.
- **porting-guide §1:** the same closed list as CLAUDE.md.
- **architecture.md, Safety:** there is no list of audited `unsafe` sites (the orchestrator asked
  to add the dispatch to one if it existed). The Safety bullet said "`#![forbid(unsafe_code)]` in
  every crate", which has not been true since 2026-10-09: `larmorx-core` denies `unsafe` except in
  one audited module, `larmorx_core::math`'s `fma` (the FMA-compiled kernels, each call behind
  `is_x86_feature_detected!`, with AUDIT and SAFETY comments); relay 1's `glibc_dispatch!` /
  `fma_glibc!` live in that module (one `#[allow(unsafe_code)]` in the crate). The new bullet says
  so.
- **status.md:** the part in the "Done" line, with the next step.
- **golden.md:** the copies are Rust-only, like `tanh_glibc`: what checks them on every platform.
- **The site's roadmap** (optional): a completed row in plain words; the orchestrator may prefer
  one row when the second half is done.

## The second half's brief points

Two roles, one after the other (the clean-room implementer may not read `crates-nc/`, so it cannot
wire the replicas):

**(a) The clean-room implementer** (never read glibc, nor `crates-gpl/`, `crates-nc/`, `docs/parts/`):
- writes `sin_glibc`, `cos_glibc` and `sincos_glibc` in `larmorx_core::math` from
  `specs/glibc-dbl.md` alone (its opening says what it may read: the record, `families.rs`,
  `tables.py` without `--print`, the black-box tool, the literature); table `R` generated from §5.1's definition
  (§5.2; at build time or as generated constants with the generator's command recorded); the
  fused reading of `FMA` in `sin` and `cos`, two roundings in `sincos` (§1, §12.2); x86-64's NaNs
  (`x86_nan`); the core's `glibc_dispatch!` for speed (§12.2); the validation of §12 (the record's
  768 blocks and 184 special arguments per function, the SSE2 record for the two-rounding reading,
  golden cases of §4.1's values and seeded arguments, every platform), and the documentation point
  §12.6 (glibc's bits on every argument tested, §5.3's theoretical exception; the four functions
  correctly rounded);
- wires the clean-room originals' `reproduce` to the `*_glibc` functions wherever their specs name
  glibc's (the default stays correctly rounded): `specs/fslmaths.md` §7, §10.2, §18 and §19
  (`exp`, `log`, `pow`, `sin`, `cos` in double; `expf`, `logf`, `powf`; TFCE's `pow`/`powf`; no FFT
  in the original), `specs/fslstats.md` §5.5 (entropy's `log`), `specs/flirt.md` §3, §9.2, §12
  (`sinf`, `cosf`, `atan2f`, the sinc window's double `cos`, `exp`; item (10)'s `real/dof6` trace),
  `specs/fast.md` §8.2 (`exp`, `log`, `expf`, `logf` with the C library's NaN for a negative
  logarithm and `-inf` for 0, which the spec defines: the replica core's SVID wrappers
  `logf_compat`/`log_compat` were not copied), `specs/topup.md` (§4.3's single-precision rotation
  sine and cosine; `exp` and `log`, §12; its FSL-201 correction keeps correctly rounded double
  ones), `specs/bet.md` §8 and §8.1 (`pow`, `exp`; `tanh` already `tanh_glibc`), `specs/mcflirt.md`
  appendix B (`logf`) and wherever else it names the C library's functions;
  the clean-room call sites today: `crates/larmorx-fsl` (maths 24, fast 20, flirt 13, bet and
  betsurf 4, cli 4, topup 3) and `crates/larmorx-mri` (hmc 13) calls of `larmorx_core::math`'s
  correctly rounded functions, tests included;
- re-makes the records that move (from a clean commit): `fsl-maths` (its double `-sin`, `-cos`,
  `-exp`, `-log`, `-pow` cases, now FSL's values: the `{fsl-maths.identical}` count rises; `-tan`,
  `-asin`, `-acos`, `-atan` stay within one unit), `fsl-stats` (entropy), `fsl-flirt` (the trace
  comparisons of item (10)), `fsl-fast`, `fsl-topup`, `fsl-bet`, `mri-hmc` (`-cost mutualinfo`,
  `normmi`); the memory and thread tests if a call path changes; the site pages whose numbers move.

**(b) A replica-side step** (an agent that may read `crates-nc/`; it does not edit clean-room crates):
- wires the FSL replicas' `reproduce` to `sin_glibc`, `cos_glibc`, `sincos_glibc` where FSL calls
  glibc's double functions: fslmaths' `-sin` and `-cos` (`larmorx-nc-avwutils/src/fslmaths.rs`,
  beside `-exp` and `-log`, which already pair the correctly rounded and glibc's values), the Kiss
  FFT's twiddle factors (`larmorx-nc-core/src/kissfft.rs`: `cexp` of `i y`, i.e. `sincos(y)` for
  `|y| > DBL_MIN`, which the code already mirrors), the kernels' Hanning and Blackman windows
  (`kernel.rs`, `costfns.rs`'s `q_hanning`: glibc's and the correctly rounded `cos` agree on every
  argument they use, `tools/glibc-float-verify kernel`, so nothing moves) and `miscmaths::sqrtaff`'s
  `cos` (avscale; its `asin` stays correctly rounded by the user's decision);
- removes GLIBC-07's tolerance for `sin` and `cos` in the replica suite and the fslmaths
  difftest descriptor (`fsl_maths_replica.py`'s `glibc` cases and `GLIBC_FREQUENCY`; keep it for
  `tan`, `asin`, `acos`, `atan`);
- re-makes `fsl-maths-replica` (`difftest/fft-twiddles-glibc-sincos` from expected divergence to
  bit-identical; `fn/double-sin`, `fn/double-cos` stay passing; `fn/double-tan` stays an expected
  divergence) and, as a check that nothing moved, `fsl-flirt-replica` and `fsl-mcflirt-replica`
  (the windows, avscale).

## Open points for the orchestrator

1. **Who does (b):** the next replica-side agent (BET's `-S`, `-B`, `-A`, `-A2` replica, as its
   first relay) or a small relay of its own after (a). Recommended: after (a) merges, as the BET
   replica part's first relay, so (b) builds on the merged functions.
2. **AFNI, outside the decision's list:** the AFNI clean-room originals' `reproduce` could now take
   `sinf_glibc`/`cosf_glibc` (3dTshift's `-wsinc5`/`-wsinc9`) at no licence cost; 3dUnifize's
   `tanhf` (fdlibm, Sun's notice) is still only in the AFNI replica crate (`fdlibm.rs`, with
   `expm1f`), so the clean-room 3dUnifize keeps CORE-MATH's `tanhf` in both modes. Moving `tanhf`
   and `expm1f` into the core is the same kind of copy the user approved (permissive code), but it
   was not on the list: ask with the next batch, or leave.
3. **ANTs (GLIBC-01):** out of scope (ANTs' parity runs with the CORE-MATH preload; ITK's replicas
   are correctly rounded by rule 5). Nothing to do unless the user wants ANTs' `reproduce` to give
   glibc's bits too.
4. **No table without a definition is left open:** the user decided both questions; the 18 low
   parts of table `R` are the documented gap of spec §5.3.
5. Relay slips: none in relay 3 (no clean-room crate edited; only the spec, the investigation
   README, `docs/findings/platform-math.md` and the scratch handoff files changed).
