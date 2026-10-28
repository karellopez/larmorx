## Tests that depend on the machine: done

All five items are done; the smoke tier (`pytest tests/parity`) passed in full here twice, without `-k "not allocation-refused"`. Validation code and its own documents only: no Rust, nothing larmorx computes, no site changes, no new defects (no IDs used). Branch `worktree-agent-a8060dfaf5aa8669b`, six commits: 831c754, e682a7e, efb8536, 224bbac, 051d7ef, c2ce1f9.

### 1. Allocations refused alike on every machine
- Every original program runs under a fixed address-space limit (`RLIMIT_AS`) of **64 GiB** (in every cache mode, in `verify` and in random testing; programs it starts inherit it; larmorx's in-process runs of the ANTs image programs lower the harness's soft limit to the same value). `validation/src/larmorx_validation/address_space.py`; `LARMORX_ADDRESS_SPACE_LIMIT` overrides (`96G`, `off`).
- The allocations the cases ask for (traced with `strace`): ImageMath `MD -1` 96.0 GiB, ResampleImage `allocation-refused` 109.2 GiB, ResampleImageBySpacing 110.9 GiB, N4 3.2·10¹⁶ bytes.
- The largest real calls (every one gives its stored result under the limit): mcflirt on the 600-volume multiband-8 BOLD series **9.4 GiB** of address space (the largest), antsApplyTransforms on it 6.7, ImageMath 3.6, 3dTshift's OpenMP at 32 threads 2.6, sMRIPrep's panel registrations 2.6, fMRIPrep's `bet` through Apptainer 1.8; the harness with larmorx in-process at most 19.3 GiB. 64 GiB is 6.8 times the largest original call and 32 GiB below the smallest refusal. The four cases now end *both-error* as recorded.
- Your heavy-case constraint: the limit is **derived from each case's memory budget** with the measured margin: twice the budget, never below 64 GiB. A night's 3 GiB cases run under 64 GiB; `difftest --heavy` under twice its box (memory plus swap: 144 GiB for 24 + 48 GiB, 2,280 GiB for the largest pending prediction of 1,140 GiB on the server); an explicit `LARMORX_ADDRESS_SPACE_LIMIT` is kept. So a heavy case is never refused what its box allows. Trade-off, written in `difftest.md`: a night's case that asks for more than 64 GiB of address space is now refused by both ("agree") instead of being killed and queued as heavy. Tools and numbers: `validation/investigations/address-space/`.

### 2. `bet2/one-argument`
BET's temporary folders are now `lx-fsl-gen_` / `lx-fsl_` and random testing's inputs folder `lx-difftest-inputs_` (the random part can no longer follow a dash; a temporary root holding `-h` is refused with a clear error). The cache key keeps the text a program reads in a path where a replaced path holds it: whitespace for every program, `-h`/`--help` for bet2 with one argument. **Entries changing key for this: 0** (the stored one-argument entries hold bet2's error, without `-h`).

### 3. The replay gaps
Fixed: Atropos priors named by a `%02d` pattern (the files it names are inputs: **68 entries re-keyed**), a path with a blank (one path: **14 re-keyed**), mcflirt's `out.mat+` (every file of its working directory is an input: **1 re-keyed**), Apptainer's HOME warning (`verify` starts the container with every variable the suite set; key unchanged), AFNI's history note (NIfTI-2 extensions left out of the comparison), BET's crash message (its process ID masked); and ANTs' clock and memory rules (ANTS-82, ANTS-07, ANTS-28) now applied by the cache to every ANTs run: the random-testing entries that vary on their own machine are never cached (**23 never served again**). Of 9,415 entries, 9,309 keep their keys; nothing is deleted (old entries are simply never reached). `oracle-cache verify --keys` (includes disputed entries) skips entries keyed under the old rules with the reason.
**The 89 entries** (verified on a copy, the workspace cache not written): **45 identical, 44 skipped with their reason.** The 24 entries the new rules stored also verify identical.

### 4. Smoke tier and standard tiers
- Smoke tier twice from the clean commit 224bbac: **2547 passed, 2 xfailed** both times (26 and 24 minutes); no run differed from its cache entry; the smoke marker written.
- Standard tiers of the 11 touched suites at 051d7ef (1,515 cases: fsl-bet, fsl-mcflirt-replica, ants-atropos, afni-tshift, afni-automask, afni-unifize, ants-image-math, ants-multiply-images, ants-resample-image, ants-resample-image-by-spacing, ants-n4): `compare_records.py` finds **no status change** in any. The new cases since the records include the 5 known 3dAutomask clean-room failures (the obliquity number, already queued for the clean-room part).
- ants-registration's standard tier not re-run here (its only entry under the new rule is the permanent case `mattes-nan-bin-svd-warning`, in the smoke tier, which passed both times).

### 5. Tests and documents
`validation/tests/test_address_space.py` (8 tests) and 11 new in `test_oracle_cache.py`; `validation/tests` 136 pass. `docs/validation/oracle-cache.md` (new sections "Keyed since 2026-10-28", "The address-space limit") and `docs/validation/difftest.md` updated; `machine-move-proof.md` has its section "Fixed". Helpers committed: `validation/investigations/oracle-cache/copy_entries.py`, `rekey_count.py`; the address-space tools.

### Gates
Python gates 622 passed, ruff, SPDX, `tables --check`: pass. No Rust changed (Rust gates not run). No AI attribution in any commit.

### Handoff
`<scratch>/HANDOFF.md`: exact text for `next-steps.md` (item done), CLAUDE.md, `decision-log.md`, the validation index, and an `oracle-cache resolve --check` command for after the merge (the move's verify left the 89 entries disputed; 45 now verify identical; until resolved those runs, the 38 fMRIPrep `bet` runs among them, run afresh). Nothing the rules could not settle.
