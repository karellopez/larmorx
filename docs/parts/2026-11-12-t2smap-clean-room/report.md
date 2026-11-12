# t2smap clean-room part: final report (relay RS4, replica-side, agent a0c4ca46bae947d83)

The part's final report as its last relay gave it (2026-11-12), verbatim but for the placeholders.

The part is ready to merge. Branch `worktree-agent-ad7e01a7198215645` is clean at **9944fb2c**: 69 commits since main b13195fb, 137 files. The site branch `t2smap-original-pages` is at **3a66585b** (RS3 eeee3a14 plus the re-export). `evidence-blobs` is **be173fa4**, unchanged by RS4, pushed with `--force` before the site's main.

## What RS4 did
- Re-export from clean commits: the original's example cache entry re-run from 55dace9d (`validation/investigations/tedana-t2smap-site/rerun_original_example.py`, its own reason recorded; the outputs byte for byte the same); exported from 55dace9d, then from 62726fb5 with `--no-compute`.
- Site checks on the final build: build 0 problems, 33 pages; numbers 854 of 854 (one first run returned `{}` while the server was rebuilding; two re-runs gave 854/854); viewers 86, none failing, 4 of 4 drawn, all 51 views drawn with no error; the inventory against RS3's state: every count kept, the other 31 pages byte-identical, after >= before; 42 JSON values no longer present, each a rewritten text (C5's `\(c)` escapes, the §17.3 headings, RS4's 4 fixes) or provenance. Screenshots of the 7 changed panels at 1440 and 390, light and dark, before and after: `<scratch>/rs4/shots-before/` and `shots-after/` (28 each).
- C5's typographer checker: RS3's export had 32 problems (29 in the clean-room original's texts, fixed by C5; 3 in the replica's: the spread table's `--n-threads 4` shown as an en dash, TED-04's `...` as an ellipsis, the TED-16 triage entry's `--exclude` as an en dash), fixed in 62726fb5 with the corrections table's title; the final page has 0. Eight other pages (FSL, AFNI, sdcflows) have 92 such texts: a follow-up (the listing holds replica and clean-room texts and stays with the orchestrator).
- The merge's evidence summary (`evidence summary --site`): only t2smap's block changes (the original's 125 cases added: 65 bit-identical, 23 within variability, 1 divergence, 36 both refuse; the replica's counts from 69/44/25 to 102/62/40), in HANDOFF-site.toml.
- Other commits: 9a6cf822 (`tables.py`'s METRICS entry for `tedana-t2smap-original`), 9944fb2c (`crates-gpl/NOTICE` names larmorx-tedana and larmorx-hdf5 among the crates the replica includes; the notices test passes, `licences.py` / `third_party.py --check` up to date).
- The handoff: HANDOFF.toml 35 operations on 17 files, HANDOFF-site.toml 6 operations on 4 files, reasons in HANDOFF.md, generator `<scratch>/rs4/make_handoff.py`; `apply_handoff.py --check` passes for both against a trial merge with main 9fefe6a3; applied for real in the trials: tables regenerated, register 543 entries (class c 272), the doc-reading tests 667 passed (one `test_difftest.py` memory-limit case failed inside the memory box with 8 workers, passes alone), the site trial built with 0 problems after `tables --site`. One operation (an append to the register's restricted TED section) holds source-derived text: the handoff files belong with `docs/parts/`.
- Agents: all 15 listed with ids and roles in notes.md's last section ("Agents of this part"): 9 clean relays and 6 replica-side (2 spec writers and RS1 to RS4).

## The original
- `crates/larmorx-tedana`, wired as `implementation="clean-room"` / `--implementation clean-room`, chosen by `auto` when larmorx-gpl is not installed; every option, message, exit status and failure case in the spec, ANALYZE 7.5 and MGH/MGZ inputs included.
- Record: 125 cases, 124 pass, 1 expected divergence (a chaotic curve-fit regime on noise voxels), 0 failures.
- fMRIPrep's 6 calls pass (A) and (B); the (b)/(c) shares: ds003592 curvefit 0.305 % / 0.259 %, ds003592 --verbose the same, ds000210 cuedSGT 0 % / 0 %, ds000210 rest 0 % / 0.002 %, ds000258 0.124 % / 0.076 %; the ds003592 log-linear call (A) 11,299,199 of 11,299,200 values identical, (B) all identical.
- The default against the replica's default: within §17.3's thresholds in every case; in the worked example 11,380,746 of 11,406,278 values identical.
- Speed (1 / 8 threads; tedana at 1 thread): ds000210 cuedSGT original 2.72 / 1.43 s, tedana 27.5 s, replica 30.2 / 10.5 s; ds000210 rest 3.67 / 1.95, 49.0, 44.7 / 14.8; ds000258 2.92 / 1.33, 34.0, 44.3 / 13.0; ds003592 curvefit 3.05 / 1.56, 47.1, 43.4 / 12.6: 10-15x tedana at one thread, 19-30x at eight; the files identical at every thread count. Peak memory 307 MiB at 1 thread and 546 MiB at 8, against tedana's 1,244 MiB; the replica's 547 MiB at 1 thread (44 % of tedana's).
- topup settings: 54 of 54 comparisons bit for bit against FSL's files; all 234 one-value-changed controls change the outputs.

## Defects
- TED-31 (rule B): no voxel left for the fit is a clear error.
- TED-46 (rule A): voxels whose log-linear observations are all equal get the slope 0; in the curve fit measured first (rule D), then corrected to the exact solution.
- TED-01: leaves out only NaN means.
- All three in both implementations, `reproduce` giving tedana's result. Unused IDs: TED-32 to 45, TED-47 to 55, FSL-280 to 289.

## Golden, random testing, the replica record
- Golden: the main package from 303 to 318 cases (3,053 outputs); larmorx-gpl from 10 to 16 cases (324 outputs).
- Random testing: trial 1 1,761 cases with 362 differences; trial 2 1,742 cases, 1,692 agreeing, all 50 differences agreeing on replay; the descriptor's final check 129 of 129.
- The replica record (RS2): 102 cases, 62 bit-identical in both environments, 40 refused by both, from the Haswell oracle runs. ds000258's 581 values: with each oracle run's machine's AVX-512 setting, tedana 25.0.1 reproduces fMRIPrep's files bit for bit in all four curve-fit calls; with numpy's AVX-512 kernels off 629 values of ds000258's T2* map differ.

## Gates and the trial merge
- gates-python (9a6cf822): 1,472 passed, 14 skipped; CI's environment 956 passed; ruff, `tables --check` 0, SPDX 0. Rust gates not re-run (no Rust or Cargo file changed since C4's gates at 84d42e18). Parity smoke tier last passed at d6126acb (2,800 passed).
- Trial merge with main 9fefe6a3: no conflicts (six files changed on both sides merged by git); the wheel builds; all 318 golden checksums and the t2smap/topup/sdcflows Python tests pass (372 passed, 1 skipped).

## Clean-room statements (slips, from the notes)
- Relay 1: read sdcflows' Apache `b02b0_quick.cnf` head; grep lines of `difftest/tools/fsl_topup.py`.
- Relay 2b: tedana's warning-location source line appeared in its stderr.
- Relay 3a: a grep printed the replica's memory-test lines 699-724.
- Relay 3a-ii: a replay printed tedana's whole traceback; it also edited `larmorx_core::numpy::format`.
- Relay 3c: printed the shared defect-table descriptions.
- Wiring relay: a repo-wide grep whose `--exclude-dir=docs/parts` had no effect (file names only).
- C4: `git show --stat` printed replica-side probe file names and commit subjects; an unquoted heredoc ran `cargo metadata --offline` in the main checkout (nothing changed).
- C5, RS1 to RS4: none; RS4 edited no clean-room crate or module.

## Open points (follow-ups, in next-steps through the handoff)
verify-merge's `LARMORX_GPL_BIN` at the merged tree's crates-gpl build; `binaries.rs`' PATH case independent of the target directory; FSL-121's wording between the two topup implementations; the help's width at a terminal; FLIRT `extracttxt`'s memory margin; `formats` into larmorx-io with the FreeSurfer part; the speed items; the 92 typographer texts on other pages; the spec's open point 6 (a 5D echo, masks with NaN in nilearn's path, unobserved). TED-46's class: (c) (the orchestrator kept it: the register's (d) is imprecision without a D13 switch).
