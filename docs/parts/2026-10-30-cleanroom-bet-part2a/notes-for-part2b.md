# NOTES: BET clean-room part 2a (started 2026-10-30 21:14)

## Done (13 commits b721ade..956c3a0; final code 8abb5b4, record from it)
- (a) tanh_glibc (fdlibm netlib + Estrin expm1 poly) = system tanh on 12,918,456,348 inputs.
- (b) §8.1 in full, §4.5 -e sweep, §4.7 skull, §8.2 quaternion (x87), §3.1 name fallback,
  §5.2 -F float64; -Z quaternion chain; script steps print FSL's FSLOUTPUTTYPE messages.
- (c) CORE-MATH tanh ported (larmorx_core::math::tanh), = C on 62,880 worst + 805M random;
  NOT wired as BET default (design choice for 2b: exact GLIBC-06 notice needs a 2nd search).
- Record re-made from 5bb2c5a: 142 cases, 111 pass, 0 fail; skulls 13/13 bit-identical.
- compare_records vs main: 3 STATUS fail->pass (older-75y, 7t, tumour), 20 detail diffs (improvements).
- retrial (strict harness): 237/239 agree; 73/75 open agree; open: 622, 677 (-c / COG conversion).
- Gates: Rust 3 workspaces OK (gates-rust.sh all), ruff, ruff format, spdx OK; pytest: only BET golden (4) +
  test_tables (fixed by tools.toml + tables, 786e708).
- Docs: findings, PROVENANCE (fsl, core), API page, investigations README.

## Running
- nothing (part 2a finished 22:45; golden re-recorded from 786e708, record re-made from 8abb5b4)

## Next (2b)
- betsurf original; site page; the user's decision on BET's default tanh (GLIBC-06 machinery ready
  in 8abb5b4, one line in cli/bet2.rs; exact notice = 1.9x default time; recommendation: keep
  glibc's, rule C); remove libm dep (lockfiles); re-run benchmark; spec-writer question: 4x4
  inverse conversion of centres on neurological images (cases 622, 677).

## Opened (clean-room statement)
- BRIEF.md, rules.md, CLAUDE.md (+ ORCHESTRATOR-STATE via system), docs/parts/2026-10-30-cleanroom-bet-part1/{report,notes-for-part2}.md
- specs/bet.md (§3.1-3.3, §4.2-4.7, §5.2, §8-§11); validation/investigations/bet-spec-answers/README.md
- validation/investigations/cleanroom-cbrtf/README.md, verifier Cargo.toml + main.rs (lines 1-75)
- larmorx-core: math/mod.rs, cbrtf_glibc.rs (head), tanhf.rs (head), cbrt.rs (head), array.rs (DynArray), defects.rs (GLIBC_05 lines), PROVENANCE.md (head)
- larmorx-afni: src/x87.rs, src/unifize/mod.rs (lines 440-476, GLIBC-05's notice)
- larmorx-mri: src/hmc/files.rs (150-330)
- crates/larmorx-fsl: all files edited (mesh, surface, params, raster, skull, quatern, image, mod, cli/bet.rs, cli/bet2.rs, Cargo.toml, PROVENANCE)
- validation: investigations/cleanroom-bet/{README, compare.py head, retrial.py, night_case.py head, fslrun.py head, bench.py head};
  parity/fsl_bet.py (mechanics: REAL lists, run_larmorx, _header_compare, per-file checks, thresholds, highlights);
  difftest/tools/fsl_bet.py (lines 300-335); tools.toml (fsl-bet lines); scripts/orchestration/{env.sh, gates-rust.sh, compare_records.py head}; scripts/record_golden.py (doc)
- docs: findings/fsl-bet-original.md, api/fsl-bet.md (215-259), validation/fsl-bet.md (head)
- External: netlib fdlibm s_tanh.c, s_expm1.c, fdlibm.h, readme (scratch fdlibm-netlib/); CORE-MATH tags/core-math/src/binary64/tanh/tanh.c (MIT) and tanh.wc
- Ran (black boxes): system libm tanh; FSL 6.0.7.23 bet2, bet, fslstats (-C, -R -m on FSL's own output); CORE-MATH's C compiled; crates-nc and crates-gpl gates builds/tests (not opened)
- Never opened: FSL source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica, restricted findings, register, docs/parts other than my part's folder, replica investigations, triage.toml, permanent cases, fsl_versions.py, glibc source
