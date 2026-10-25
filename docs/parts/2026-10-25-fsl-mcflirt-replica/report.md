# Final report: The FSL replica package and the mcflirt replica

Agent `a5874ac3558d79d85`, 2026-10-24. The agent's report to the orchestrator, verbatim.

## Summary

The mcflirt replica works and is bit-identical to mcflirt everywhere I compared. On the full tier of its record, 155 of 155 compared cases give mcflirt's bytes: every file, the exit status and both message streams. In the other 21 cases both programs fail, with the same messages and status. No case fails.

Gates pass: Rust fmt/clippy/test in the root workspace and in `crates-nc` (I did not re-run them in `crates-gpl`), the Python gates (473 passed), and the head-motion parity tests (189 passed). I did not finish the full `pytest tests/parity`; details under Unsettled. Nothing is pushed or merged.

- **Branches:** `worktree-agent-a5874ac3558d79d85` (larmorx, base 5317323, clean tree) and `fsl-mcflirt-evidence` (site clone at `<scratch>/site`, commit 014ee4c).
- **Handoff for the shared documents:** `handoff.md` beside this report.

## The `larmorx-nc` package (`crates-nc/`)

- **Workspace:** its own Cargo workspace (root `Cargo.toml` excludes it). Members: `larmorx-nc-core`, `larmorx-nc-mcflirt`, `larmorx-nc-cli` (program `larmorx-nc fsl mcflirt`) and the dev tool `larmorx-nc-core/tools/glibc-float-verify`. It never links `crates-gpl`.
- **Licence files:**
  - `LICENSE` is FSL's licence text.
  - `NOTICE` carries the FSL, Armadillo/armawrap (Apache-2.0), fdlibm (SunPro) and Arm (MIT) notices.
  - `LICENSES/` holds the Apache-2.0 text, the larmorx NOTICE and Arm's MIT licence.
  - Every source file has an SPDX header containing `LicenseRef-FSL`.
- **FSL's original source:** `fsl-source/` holds 28 unchanged files from 5 FSL packages, listed with SHA-256 in `MANIFEST.tsv`. `scripts/fsl_source.py` copies and checks them.
- **The wheel** (`pyproject.toml`, licence `LicenseRef-FSL`): I built it and checked it. It carries the original FSL source and the Rust translation as licence files, as the FSL Licence requires.
- **Tests that guard this:** `tests/notices.rs` checks that the notice copies are current and that every file in the source manifest is present and unchanged.
- **Dispatch:**
  - `larmorx_cli::replica` has a `LARMORX_NC` package (found through `LARMORX_NC_BIN`, then the scripts directory, then `PATH`; install hint `pip install larmorx-nc`).
  - `larmorx fsl mcflirt` is a new CLI family; its original is the clean-room `mri hmc`.
  - `lx.mri.hmc(..., implementation="auto"|"replica"|"original")` runs the replica on files and reads back the matrices, `.par` and `.rms`. A new binding, `_core.mri_hmc_world`, converts the matrices to the RAS/ITK forms and FD.
  - Two options mcflirt's command line lacks (`in_plane="never"`, `normcorr_count="weights"`) run the original under `"auto"` and raise an error with `"replica"`.
- **CI:** fmt, clippy and tests for `crates-nc` added as steps inside the existing `gpl` job, with one shared cache.
  - Measured locally (3 jobs, cold): clippy 12 s, tests 20 s.
  - Estimate: +1–2 min per platform cold, about 0.5 min warm; ×6 platforms that is +6–12 min cold, about 3 min warm.
- **Orchestration scripts:**
  - `gates-rust.sh` gained `nc` and `all` modes.
  - `verify-merge.sh` now runs `all` and builds the replica.
  - New `build-nc.sh`.

## The core crate (`larmorx-nc-core`)

It translates the FSL libraries the next replicas will share:
- NIfTI-1/2 and ANALYZE I/O as FSL does it, including pairs, gzip, byte swapping and `FSLOUTPUTTYPE`.
- `volume<float>`: extrapolation, centre of mass, background value, left-right order, reading and saving with FSL's type conversions and messages.
- Interpolation: trilinear, sinc kernels, cubic splines.
- FLIRT's cost functions: normcorr (plus its smoothed and sinc variants), least squares, correlation ratio, Woods, mutual information and normalised MI.
- The optimiser, and the transform algebra from miscmaths (`make_rot`, compose/decompose, `rms_deviation`, `.mat` reading).
- Armadillo 10.5.1's evaluation rules as armawrap uses them.
- x87 long-double emulation for the NIfTI quaternion code.
- glibc 2.35's float functions (below).

Its API is documented in `PROVENANCE.md`; BET's library parts can move in at the merge.

## mcflirt coverage

- Every option, message, exit status and failure case of mcflirt 2111.0 (FSL 6.0.7.17). That includes `-gdt` (reproduced), `-verbose 20` with the fixed-notation state of `cerr`, `-init` errors, `Log::establishDir` with its `+` names, and the aborts (mcflirt's text, then one replica line and exit 1).
- Threads run volumes in parallel only in the unchained stages and the final resampling. Outputs are identical at 1 and 3 threads.
- **The C library's maths.** The oracle runs did not use the CORE-MATH preload. With correctly rounded maths, 12 of the 79 synthetic recorded runs differed from mcflirt as built. Each of those was identical to mcflirt run with the CORE-MATH preload, so the cause was glibc's `sinf`/`sincosf`/`logf`.
  - The replica therefore carries glibc 2.35's FMA build of `sinf`, `cosf`, `sincosf` and `logf`, translated from Arm's MIT-licensed Optimized Routines (the same code glibc ships; no LGPL code in the package). It matches the C library on all 2^32 inputs of each.
  - This follows the 3dTshift replica's precedent; I recorded it as a proposed decision-log entry in the handoff.
- **The binary follows the source (rule 8):** conda-forge GCC 13.2, no FMA instructions in any FSL library, so no source-built oracle is needed.
- Findings are in `docs/findings/fsl-mcflirt.md`, section "The replica". The tools behind them are in `validation/investigations/fsl-mcflirt-replica/`.

## Parity: suite `fsl-mcflirt-replica`, record `docs/validation/fsl-mcflirt-replica.{md,json}` (full tier, from clean commit 920ee8e)

| Tier | Cases | Bit-identical | Both fail alike |
|---|---|---|---|
| smoke | 110 | 91 | 19 |
| standard | 172 | 151 | 21 |
| full | 176 | 155 | 21 |

- **Recorded runs:** 137 (125 + 12).
- **Edge cases made for the replica:** 32 (23 + 9).
- **fMRIPrep's own mcflirt calls** on the five oracle runs: 7 of 7. fMRIPrep's own `.mat` files (from its container's FSL, which has newer libraries) are also identical to the replica's: 1,273 of 1,273 matrices.
- **D13 fix pass:** every correction is named exactly when it changes the output.
- **The clean-room original as a third column:** the record's table puts the original's numbers beside the replica's; they are read from `mri-hmc.json`, not recomputed.
- **Oracle cache:** every mcflirt run goes through one function, `run_oracle` in `parity/fsl_mcflirt_replica.py`, ready for the oracle-output cache at the merge.

## D13 and new defects

- **Shared with the clean-room original** (corrected by default, reproducible with `reproduce`): FSL-01, -03, -05 to -10, -13, -14.
- **FSL-04** (`-gdt`): refused by default; `reproduce` gives mcflirt's run.
- **New FSL-20, rule B:** when the `.mat` directory cannot be created (a missing parent directory), mcflirt exits with status 0 and writes nothing more. The replica makes this an error; `reproduce` keeps mcflirt's behaviour.
- **New FSL-21, rule B:** with a histogram cost and `-bins` below 8, the first stage has no bins. The matrices are arbitrary, and `corratio` writes before its arrays and aborts ("double free or corruption"). The replica refuses this by default; `reproduce` gives mcflirt's matrices and its abort.
- **FSL-15, rule C:** kept; `-init` is "apply to all vols", as documented.
- **FSL-18, rule C:** not a defect. `n²σ/Σ` equals Woods' `nσ/μ`.
- FSL-04, -20 and -21 are added to `larmorx_core::defects`.
- No new **decide** entries.

## Golden

- `crates-nc/larmorx-nc-cli/tests/golden.rs`: 23 cases, 179 outputs, recorded from clean commit 53481ee (checksums identical to an earlier recording).
- Main package: new case `cli.fsl.mcflirt`, recorded with `--skip-parity` (see Unsettled); every other checksum unchanged.
- The `py.mri.hmc*` golden cases now pass `implementation=ORIGINAL`.
- New tests: a CLI test file (`tests/cli.rs`), a NIfTI round trip in every output type, and Python tests for the replica path (a stand-in program, plus the real replica when it is built).

## Site (head-motion page)

The replica is shown beside mcflirt and the clean-room original. Following your rule for this work, nothing was removed; before and after:

| | Before | After |
|---|---|---|
| Viewer panels | 7 | 10 |
| Difference panels | 5 | 9 (replica − mcflirt: bit-identical) |
| Metric table rows | 9 | 14 |
| Cases | 134 | 176 (the original's 134 unchanged) |
| Agreement bars | 8 | 8 + 10 for the replica |
| Default view | 4 panels + 3 differences | 5 panels + 4 differences |

- Grid and volumes are unchanged.
- Number check: 182 of 182 agree (before: 116 of 116).
- Re-export is byte-identical; the build reports 0 problems.
- The full inventory table is in HANDOFF.md. Screenshots at 1440 and 390, light and dark, are in `<scratch>/shots/before` and `<scratch>/shots/after`.
- The full-page captures do not paint the image panels on either page; the viewport shots (`viewer-*`) show them rendering.

## Unsettled

- **Full `pytest tests/parity` not run.** `record_golden.py`'s own parity run was too slow on the shared machine: it reached 30 % in 1.5 h, mostly in the AFNI 3dUnifize suites, and I stopped it.
  - I recorded the golden checksums with `--skip-parity` instead; `golden.json` notes that parity was skipped.
  - I ran the head-motion parity tests separately (189 passed).
  - The rest of the parity tier needs re-running at merge.
- **The clean-room original lacks FSL-20 and FSL-21.** They need a spec writer for `specs/mcflirt.md`; I did not edit any clean-room file.
- **Packaging decision:** whether the `[all]` extra should include `larmorx-nc`. `docs/licensing.md` says it is installed by name only.
- **Informal timing** (fMRIPrep's command on ds000005, shared machine): the replica took 20 s on 3 threads and 36 s on one; mcflirt took 70 s in the same run set.
- **Merging with the BET branch** touches `crates-nc/Cargo.toml` members and `Cargo.lock`, `FSL_TOOLS` in `larmorx-nc-cli`, `FILES` in `scripts/fsl_source.py`, the NOTICE and README, and the golden cases.

## At the merge (orchestrator)

The `[all]` question was settled by the user's decision of 2026-10-16 (`[all]` installs every replica package, `larmorx-nc` included; `[exact]` only `larmorx-gpl`); the extra was added to `pyproject.toml` and `docs/licensing.md` corrected. `run_oracle` now goes through the oracle-output cache (`Policy(threads=1, family="fsl")`). The full parity tier and the golden checksums were re-run and re-recorded from the merge.

## Follow-up (2026-10-25), after the data panel's runs and random testing

At the merge's verification the suite picked up the data panel's fMRIPrep runs, and two calls
differed (ds000256 from MAT_0122, ds002422 from MAT_0166); random testing then found 57 of 584
cases differing. The agent was sent back. Its report, verbatim:

**The cause was a bug in my translation, not in mcflirt.** In my port of glibc's `atanf` (`crates-nc/larmorx-nc-core/src/fdlibm.rs`), I took the constant `aT[0]` from fdlibm's source comment, `0x3eaaaaaa`. The decimal literal the compiler actually reads is `3.3333334327e-01`, which rounds to `0x3eaaaaab`. glibc 2.35's `.rodata` holds `0x3eaaaaab` (I checked it with objdump; every other constant of `atanf`/`atan2f` matches).
- The wrong value changes `atanf` by one ulp on 1,037,658 of the 2^32 inputs.
- On ds002422 rest it hit 1 of mcflirt's 600 rotation decompositions: volume 166, start of stage 3, `atan2f(0x3c7c2fbe, 0x3f7ff83d)` gave `…24b` instead of `…24a`.
- On ds000256 CTS203 no-feedback it hit volume 122, and the chained first stage carried it to the last volume.
- mcflirt itself is deterministic on both (two runs give identical output). With the CORE-MATH preload, 1 of 200 and 64 of 164 matrices change, so glibc's own rounding is what the replica has to reproduce.

**How I found it.** LD_PRELOAD interposers on FSL's own `decompose_aff` and `Costfn::cost`: stage 2 of volume 166 matched bit for bit (26 evaluations); FSL's cost function gives the replica's costs at the replica's matrices; replaying mcflirt's 600 `decompose_aff` calls through the replica isolated the one call and its `atan2f`. Probes: `validation/probes/fsl-mcflirt-affine-trace/`; replay tool: `crates-nc/larmorx-nc-core/tools/affine-trace-replay`.

**The fix and its tests (e687cd1, aa2c28f):** `aT[0]` is now `0x3eaaaaab`; `glibc-float-verify` checks `atanf` on all 2^32 inputs and `atan2f` on 1,444 special-value pairs plus 3 × 2^30 seeded pairs: 0 differences; unit tests `atanf_glibc_bits`, `atan2f_mcflirt_ds002422`, `decompose_aff_mcflirt_ds002422`; both fMRIPrep calls now give mcflirt's matrices: 200/200 and 164/164.

**Random testing.** All 61 findings, shrunk and original recipes (122 runs), are bit-identical to mcflirt in every file, both streams and the exit status, with the D13 fix pass included. `:616` was a separate issue: the default renames the `-meanvol` output to `<stem>_mean_reg` (FSL-08) but printed no notice; FSL-08's notice is now given (d196995). Permanent cases: six in `validation/difftest/cases/fsl-mcflirt/` (attached once random testing's harness hook is on main) and three self-contained smoke cases `edge/atanf-phantom-{2,11,27}` (205bee4).

**Memory (37d6505).** The 8 GiB box killed two runs on ds004958 (600 volumes): the suite's own process reached 7.75 GiB because the fix pass loaded whole images. Comparison and change-counting now run in 16 MB chunks, exactly equal to the whole-array results (`validation/tests/test_fsl_mcflirt_compare.py`); the full tier ran in `boxed.sh 16`.

**New record** (full tier, clean commit 205bee4): 194 cases, 173 of 173 compared bit-identical, 21 rejected by both alike; recorded runs 137 (125 + 12), edge cases 35 (26 + 9), fMRIPrep calls 22 on 12 oracle runs (22 pass); fMRIPrep's own `.mat` files identical as text: 4,517 of 4,517; 50 of 57 original-program runs from the oracle cache.

**Gates.** crates-nc fmt, clippy, tests (golden unchanged); ruff, SPDX; `pytest tests/python tests/golden validation/tests` 539 passed; `pytest tests/parity -k mcflirt` 113 passed. Site: the head-motion page re-exported with the panel's exporter (number check 182/182, cases 176 → 194, inventory after ≥ before), local `evidence-blobs` 5b89607 (295 blobs).

## Follow-up (2026-10-25, evening): NaNs on ARM

After the publication (9b7e8ee) the `gpl` CI job failed on the three ARM platforms in the
crates-nc golden case `mcflirt.nonfinite` (NaN and infinite voxels). The agent was sent back. Its
report, in short (verbatim text in the session; the commits carry it): x86-64's SSE makes a NaN
from numbers with the sign set, AArch64's with it clear, and when two NaNs meet the compiler's
operand order decides which passes on. An LD_PRELOAD probe (`libnan_trap.so`, `NAN_TRAP_ARM=1`
emulates ARM's NaN on x86) reproduced the ARM CI's checksums exactly. Fix (ef73e6f):
`larmorx-nc-core/src/sse.rs` makes SSE's NaN rules explicit (a NaN made from numbers is x86's
default NaN; operands pass on in the source's order; NaNs from the input keep their bits), used
by the interpolation, the centre of mass, the `-stats`/`-meanvol` sums, `-gdt`'s gradient, the
glibc float functions, Armadillo's small-matrix algebra and miscmaths; an all-NaN affine
decomposes as FSL's binary does (200 of 200 probe cases); no measurable cost. NaN matrix: 160
cases × release, debug and both with ARM's NaN emulated: all four agree; in `reproduce` mcflirt's
bytes except 32 cases of FSL-22 (`calc_bval` sorting NaNs: undefined in C++; the orchestrator
decided: NaNs last, a warning in `reproduce`, libstdc++'s GPL code not emulated). Golden
(bba2ff9): one checksum changed, to mcflirt's own `.par`. Gates: crates-nc fmt, clippy, tests;
Python 539; 48 full-tier cases and 5 NaN cases pass.
