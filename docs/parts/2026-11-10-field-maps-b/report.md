## The field maps' part B: final report (relays 1 to 32, 2026-11-08 to 10)

The last relay's report as returned to the orchestrator; the full texts are in `notes.md` and `handoff.md` beside
this file (machine paths written as `<workspace>` and `<scratch>`). The role: the port author (permissive
upstreams; no clean-room crate touched). Branch `worktree-agent-a33e6cc1a7edea6ca`, head 5385a9f9, 139 commits
from main f5e89ff4 (main's d1491604 cherry-picked as 8e4ae409).

### The port
An exact port of sdcflows 2.17.1 (its estimator search defaults to 2.15.2's: SDC-17 rule A), niworkflows 1.15.4,
nipype 1.12.0's command lines, pybids 0.22.0's index (`lx.bids`), scikit-image 0.26.0, SciPy 1.18.1's ndimage
(`crates/larmorx-ndimage`) and fMRIPrep 25.2.7's field-map steps. Covered: the PEPOLAR, direct and SyN estimators,
the masks, the workflows, the derivatives, larmorprepx's field-map stages and the BIDS app `larmorx sdcflows
sdcflows`. `qwarp_workflow` waits for AFNI's 3dQwarp and FreeSurfer's `mri_robust_template`.

### Records and real calls
- Records, all pass: `ndimage-skimage` 10/10, `ndimage-skimage-full` 1/1 (numpy's float32 `exp` on all 2^32
  inputs), `niworkflows-helpers` 5/5, `sdcflows-estimation` 19/19, `-full` 14/14, `-app-shared` 8/8; none refused
  by both, none expected to differ, no failures.
- fMRIPrep's real calls against fMRIPrep's own files and graphs on the field-map oracle runs, all re-checked
  after the runs made again with Haswell's OpenBLAS kernels: the BOLD fit's field-map stages on 4 runs, the
  estimation stage on 85 cases, `unwarp_workflow` 81/81, fMRIPrep's topup = FSL 6.0.7.23's.
- The step (numpy 2.3.5 / SciPy 1.15.3 to 2.5.4 / 1.18.1): ndimage-skimage 287 comparisons, 126 changed, each with
  a known reason, none varying; niworkflows-helpers 2 and sdcflows-estimation 6, none changed.

### Defects
89 entries (SDC-12..46, PIPE-18..40, SCI-06..32, SKI-01..04), 61 with `reproduce`. By rule: A 42, B 15, C 8;
undefined conversions clamped with a warning 3; never reproduced (D13) 8; fixed upstream and followed 2;
messages kept 4. Open: PIPE-35 (rule D, measured with the anatomical brain extraction's port) and the switches of
SDC-23, SDC-42 and PIPE-36, which wait for other ports. Seven upstream drafts in `docs/upstream-reports/` (the
projects' issues searched on 2026-11-10): new issues for SDC-45, PIPE-36, SCI-21 (+ SCI-22, SCI-23) and SCI-25;
comments on open issues for PIPE-32 (sdcflows#455) and SCI-29 (scipy#24502); PIPE-37 is fixed on pybids' master
(#1273), not yet released: nothing to file (at most a request for a release).

### Golden, memory, random testing, speed
- Golden: 41 cases (303 in all, 2,660 outputs), recorded from clean commits after the whole smoke tier passed
  (2,927, 0 failed); `py.skimage.random_walker.nan` is the first case hashing NaN outputs on the other platforms.
- Memory (larmorx against the original): brain_masker 431 / 1,438 MiB, IntensityClip 84 / 143, RobustAverage
  108 / 149, ReconstructFieldmap 230 / 1,438.
- Random testing: 312,081 cases, 0 differences, 8 timeouts (a non-converging random walker; fixed since: 73 to
  199 s, sdcflows' own time), 17 triage entries; 13 permanent cases.
- Speed, one thread, original to larmorx: brain_masker 8.04 to 6.93 s, IntensityClip 1.40 to 1.63 s (slower),
  RobustAverage 0.20 to 0.23 s (slower), ReconstructFieldmap 0.89 to 0.30 s; the BIDS app on five of sdcflows'
  runs 1.2 to 5.3 times faster (ds005752 21 to 4 s, ds006736 79 to 56 s).

### The site
The page `/tools/sdcflows-estimation/`: five examples (PEPOLAR ds006736, the direct field map ds005752,
brain_masker on ds002603's fMRIPrep call, epi_mask on ds003592 with SCI-13/14's correction: Dice with fMRIPrep's
brain mask 0.464 to 0.904, and the SyN viewer: larmorx's warp is ANTs' one-thread warp byte for byte; ANTs at 4
threads does not repeat itself on this call, 0.70 Hz run to run; fMRIPrep's own field lies within that band), nine
tables, the speed chart and 57 cases. Build 0 problems; every other page's HTML identical; the inventory 0
losses; numbers 527/527; 76 viewer values; 5/5 views drawn; screenshots at 1440 and 390, light and dark.
`evidence-blobs` d680cd2a (1,424 blobs: 81 added, none replaced or removed).

### Gates
At 5385a9f9 (clean, boxed): `gates-python --no-build` exit 0 (pytest 1,292 passed, 13 skipped; CI's
environment 901), `tables --check` and `check_spdx` OK; the three Rust workspaces' gates at relay 28 (no Rust
change since).

### Merge notes (from the part)
`numpy_linalg.rs`: main's `dbdsqr_upper_ncc` kept, its wrapper given `clear_negative_zero`; `catalogue.py`
auto-merges (the roadmap entry turned into stages S04 to S07); `evidence/summary.py` both pages (t2smap's first);
`defects.rs` both families; `python/larmorx/__init__.py` by hand; `Cargo.lock` main's then `cargo metadata`;
`golden.json` either header, then re-recorded; the ndimage duplicate (`larmorx-interp`'s and `larmorx-ndimage`'s
SciPy filters) a next-steps follow-up; `ants-syn.md`'s thread paragraph scoped to its own cases (2a87704a).

### The agents (33)
Relay 1 a4cf64edc320dd324 (+ the stopped attempt aaea7f61392fc0d27), 2 a33e6cc1a7edea6ca, 3 ac1175bc60550e332,
4 a1fd06ee6416ffe30, 5 a58deb7daf1547dd5, 6 ac5079c785f056c44, 7 a2d2e605fadc15ca8, 8 a15a4cf4128e2669f,
9 ae78b46850d0dea1a, 10 a5033e389fa9c9e19, 11 a1a6fcf4ee03fcfb5, 12 ab43113819ad79ad9, 13 a9b207f19eb5645ce,
14 a7ededd34215e0b82, 15 a1daa4575f3643c99, 16 a19e3c20cbb8f7312, 17 ae878eb5ad8d31a90, 18 a7ba42bb360567da2,
19 a902bb371637afc9b, 20 aa4f59554aad847a1, 21 af0974b3dbdc43289, 22 a694a281b3b4a0f88, 23 a8db1ba28a0b88624,
24 a3f4ba1c1e9c59f40, 25 aabe6c62889486dea, 26 acc44b330546a5099, 27 aff910a486c582678, 28 aa113697e2f6b45b7,
29 ac9c94451e2b8426e, 30 a0e0bb7a68ebd3989, 31 a56b27a2845fe52d5, 32 a5c1cdfa1e9ad900e.

### Merge notes (the orchestrator, 2026-11-10)
- The merge into main (on d6d3e8e7) stopped at the 15 files relay 32 named; every resolved file, and every other
  file of the merge (337), is identical to relay 32's trial merge (`<scratch>/r32/merge`), whose resolutions were
  taken. The site's `fmap-b-pages` (c1904f8e) merged without a conflict; `evidence-blobs` ae1a3b62 to d680cd2a
  (1,343 + 81; every earlier blob the same).
- HANDOFF.toml's 89 operations applied (19 files; `NOTICE` and its two copies identical); `tables --site`,
  `tables --check`; `register_counts.py` 541 entries, no decide, no yes; the summary re-exported; the site build
  0 problems. Added by the orchestrator: CLAUDE.md's agents lines (one agent at a time since 2026-11-09).
- The merge's own fixes (9f65b4a7): `Cargo.lock` gains `larmorx-ndimage` (`cargo metadata`), and `cargo fmt` on the
  two merged hunks (numpy's module order, `dbdsqr_upper`'s call).
- `verify-merge.sh` on 9f65b4a7 (clean; boxed 96 GiB, 24 threads; oracle cache `use`): the Rust gates of the three
  workspaces, the GPL and NC builds, the Python gates, the ten suites and the evidence re-export passed. The
  records against the committed ones (`compare_records.py`): no status differs in any suite; `sdcflows-fieldmap`
  37, `numpy-linalg` 28, `nifti-io` 257, `resample-series` 139, `ants-apply-transforms` 258, `fsl-topup-replica`
  177 identical; the detail differences, each explained, the committed records kept: 13 permanent random-testing
  cases newer than the records, all passing (`ndimage-skimage` 3, `niworkflows-helpers` 2, `sdcflows-estimation`
  8); `match-histogram`'s SKI-02 message now spelled as sdcflows spells it (`dtype('<f4')`, a later relay's
  permanent case); `ndimage`'s count of SCI-31 (5 calls, counted as the same before SCI-31 was registered); the
  contract audit's count of the part's source files (its diff against main: 0 after the merge); `tedana-t2smap-
  replica`'s three real calls follow the fMRIPrep runs made again with Haswell's kernels (known; re-made by
  t2smap's clean-room part). The evidence re-export changed only the exporter's stamp.
- The golden step failed there, because of how it was started: the checkout given as `.` made `PYTHONPATH`
  relative, and the four memory tests of the field-map steps, which start larmorx in a temporary folder, could
  not import it (each failed in 0.1 s). The memory and thread files run again with absolute paths: 237 passed.
  `verify-merge.sh` now makes the checkout's path absolute (5ecc08db).
- The golden record from 5ecc08db (`record_golden.py`, boxed): the parity smoke tier 2,968 passed, 0 failed (24
  workers 279 s, then the memory and thread files serially 1,319 s); 303 cases, 2,660 outputs, every checksum the
  same (only the header changed).
