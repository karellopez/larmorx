## tedana's `t2smap` replica: final report (relays 1 to 10, 2026-11-08)

The last relay's report as returned to the orchestrator, condensed (the harness refused a report file); the full texts
are in `notes.md` and `handoff.md` beside this file. The role: the replica author and the part's spec writer (tedana's
source read; no clean-room crate touched). Branch `worktree-agent-ac7d861cd17e74f0d`, head 3709e6d4, from main
ec557ff5. The part's agents: ac7d861cd17e74f0d, a1e74b87182689d97, af895e326a1e6d7a0, a5a8b57937a8b1b27,
a30aa31ad03a8cbb0, ae98246030672c0a4, a16720ab7bb61dc5f, ad7066973cb2d2a09, a255b30a7008412b5, a7792b13ea745f138,
a5667128987e5c88e, ad96fb3c5454f2d1f, a2d62cc94db86c8b1.

### The replica
tedana 26.0.3's `t2smap` complete (every option, output, message, exit status and failure) in `larmorx-gpl`
(GPL-3.0-only under LGPL-2.1's section 3; the user, 2026-11-08), run by `lx.tedana.t2smap` and
`larmorx tedana t2smap`; the numpy, SciPy, nilearn and CPython code it needs in the main package's Apache crates
(`larmorx_core::numpy`, `larmorx_core::python`, `crates/larmorx-optim`, `larmorx_transform::numpy_linalg::lstsq`,
`larmorx_interp::nilearn`, `larmorx_io::nifti::nibabel_affine`).

- Record `tedana-t2smap-replica` (from clean commit 7b9cb9c3): 69 cases, 44 pass, 25 refused by both alike, 0
  expected divergences, 0 failures; bit-identical to tedana in both library environments.
- fMRIPrep's real calls: five on three oracle runs (ds003592, ds000210's two tasks, ds000258), every one identical to
  tedana 26.0.3; against fMRIPrep's own files (tedana 25.0.1) the four curve-fit calls differ in 12,295, 12,565,
  35,226 and 41,897 values: tedana's own version step (fMRIPrep's ds003592 T2* map is tedana 25.0.1's run here).
- tedana's own variability on ds003592 (values that change): tedana 25.0.1 for 26.0.3 176,938; the shared library
  releases 28,808; numpy's AVX-512 kernels 22,899; 4 threads 1.
- `specs/t2smap.md` (1,305 lines) and its model (`validation/investigations/t2smap-spec/`: tedana's bits on 30
  black-box cases and fMRIPrep's calls), for the clean-room original.

### Defects (TED-01 to TED-18; NP-03, NP-04 new; GLIBC-07 extended)
TED-05 rule A (tedana 26 no longer caps the full T2* map; tedana 25.0.1 did; pull request #1290 gives no reason);
TED-06 measured, then rule C (kept, no switch); rule A: TED-01, TED-07 to TED-09, TED-14, TED-17, TED-18; rule B:
TED-02 to TED-04, TED-10 to TED-13, TED-16; TED-15 rule A, rule B for a one-volume series. The replica names NP-03
and GLIBC-07 exactly, per family of functions (up to four passes; the orchestrator kept it: the replica's role is
exactness); the clean-room original names them in one pass where the function's own value is written (spec §15.2).

### Golden, memory, random testing, speed
Golden: 10 GPL cases (203 outputs) in CI's existing `gpl` jobs; the main package's checksums unchanged. Memory 545 /
543 MiB against tedana's 1,243 / 1,224 (44 %). Random testing: a half-hour trial of 3,886 cases, its 59 differences
and 11 notices fixed or registered (TED-16 to TED-18), a 10-minute check of the final build (1,268 cases, 1,267
agree, 1 notice fixed, a permanent case); 18 permanent cases. Speed against tedana on fMRIPrep's curve-fit calls 0.8
to 1.1 times at one thread, 3.0 to 4.2 times at 14; the log-linear call 0.6 and 0.8 times (the notices' passes).

### The site
A new page for `t2smap`: ds003592's maps (opening on the voxel where TED-05's cap changes the T2* map most) and
combined series with fMRIPrep's own file, ds000258's maps, every call and file, tedana's own spread, the corrections,
memory, the trial, speed and every case. Build 0 problems, numbers 616/616, viewers 66 (0 failing), every viewer
drawn, the 23 other evidence pages' HTML unchanged, 56 screenshots. `evidence-blobs` ae1a3b62 (1,343; 68 added, none
replaced).

### Open
The replica prints its maths notices at the cost of extra passes (above). fMRIPrep's ds000210 and ds000258 runs were
made with OpenBLAS's AVX-512 kernels (the orchestrator's finding, next-steps: made again before the clean-room part).
The gates of relays 8b to 10 covered the crates they changed; verify-merge runs the full gates.

### Merge notes (the orchestrator, 2026-11-08)
- Merged into main 1225f38d (on 6840f4d8). Four conflicts (`crates/larmorx-core/{PROVENANCE.md, src/lib.rs,
  src/numpy/mod.rs}`, `validation/investigations/t2smap/README.md`): main's copies are relay 2's commit 0ed9cee2's
  (the shared numpy module brought into main early), so the branch's versions were taken. Two permanent random-testing
  cases held a machine path in a traceback (this part's `qform-signed-zero` and main's
  `ants-registration/four-dimensional-field-parameters`): written as `<workspace>`. The site's `t2smap-pages`
  (c40ae49c) into the site's main (8e97a4ca); `evidence-blobs` 96bca689 → ae1a3b62 (the manifest's SHA-256
  f29298798b957e77 matches; 68 added, none replaced or removed).
- HANDOFF.toml's 104 operations applied (26 files; `NOTICE` and its two copies identical); `register_counts.py` 452
  entries; `tables --site`, `tables --check`, the summary re-exported; the site build 0 problems. Added by the
  orchestrator: the audit's brief rules name the replica's record, permanent cases, evidence module and site checks
  (they hold a few words of tedana's source; decision log), next-steps' two follow-ups (fMRIPrep's ds000210 and ds000258
  runs again with Haswell's kernels before the clean-room part; tests that refuse a TMPDIR holding `-h`).
- The clean-room audit's merge step (6608931e): 13 agents archived; 0 blocking, 0 to judge.
- Verified (`verify-merge.sh` on 6608931e, 22:59-23:48, 8 threads): VERDICT PASSED: the three Rust workspaces' gates,
  the GPL and nc builds, the Python gates, the smoke tier and golden (every checksum unchanged, only the header);
  re-runs against the committed records, no status or detail differs: `tedana-t2smap-replica` 69/69, `numpy-linalg`
  28/28, `sdcflows-fieldmap` 37/37, `nifti-io` 254/254 and 3 new cases (ds000258's echoes, which the test-data catalog
  gained for this part: pass), taken as its new record (257); the page re-exported with no change.
