## Version update, part 2: FSL 6.0.7.17 → 6.0.7.23 (2026-10-29): report

Two rounds: the agent's first report, then four items the orchestrator asked for (the site, the
mcflirt replica's full record, the five BET differences of the trial, mri-hmc's threads), and the
second report. Defect IDs used: FSL-59 to FSL-63. Branch `worktree-agent-afdb6eadb1ef500f6` from
fa4dd75, 17 commits to 09086ab; site branch `version-fsl-6.0.7.23` (60a16ec, cbbac85).

### Round 1
**What changed** (full table: `docs/findings/fsl-mcflirt.md` and `fsl-bet.md`, "FSL 6.0.7.23";
tools in `validation/investigations/version-fsl/`):
- **mcflirt's own source is unchanged** (2111.0); it is rebuilt with newimage 2601.0, miscmaths
  2412.6, NewNifti 5.0.0, znzlib 2511.3, utils 2412.6 and armawrap 0.7.0, which is Armadillo
  14.2.1 rather than 10.5.1 (the upstream watch did not list this one); GCC 13.2 → 15.2, neither
  build uses FMA. mcflirt's 137 recorded runs made again with 6.0.7.23: 127 byte-identical, the
  other 10 each with a known reason.
- **FSL-59 (new, c):** in 6.0.7.23, `-sinc_final` and `-nn_final` resample trilinearly (a
  ShadowVolume cache in newimage keeps each volume's first interpolation method). Default: the
  method asked for; `reproduce`: trilinear; named where the two differ.
- **FSL-60 (new, c):** NewNifti's `replaceSuffix` renames any path holding `.img`. Default: the
  name given; `reproduce`: FSL's renamed name.
- **bet2:** FSL-41 fixed upstream (`tm = t98`), followed in both modes; a constant image now
  crashes FSL later, at FSL-43, which the default names. New `--binaryMask` ported; **FSL-61**
  (a, a mask on another grid): refused in both modes; **FSL-62** (c, `--ct --binaryMask`):
  corrected by default, `reproduce` gives FSL's.
- **Image I/O:** zstd and bzip2 reading and writing (pure-Rust `ruzstd`, `bzip2` with
  `libbz2-rs-sys`), 16 `FSLOUTPUTTYPE` values and the new message, 8 image extensions, the build
  string and usage ID, fslpy 3.27.0's extension lists, Splinterpolator's Zeros rule.
- **No effect:** `extrapparams`, threaded `convolve`, the `NoOfThreads` default, comments.
- **Not followed yet:** Armadillo 14.2.1's 4 × 4 inverses and solves through LAPACK (OpenBLAS
  0.3.33): last bits of `.rms` values and NaN signs in `.par` in 5 cases; results identical under
  every OpenBLAS core of this CPU. Recommendation: a replica part with a ctypes probe.

**`fsl-source/` and the licence:** 68 files of 6.0.7.23 copied by `scripts/fsl_source.py`
(`MANIFEST.tsv` with versions and SHA-256, `--check` passes); `znzlib.c` replaced by the C++
znzlib and its plugins; the 6.0.7.17 copies archived in
`<workspace>/archive/2026-10-29-fsl-source-6.0.7.17/` and in git history; nothing translated from
6.0.7.17 alone.

**Harness:** `--fsl-version` / `LARMORX_FSL_VERSION` (`parity/fsl_versions.py`) in parity,
evidence and difftest; `LARMORX_FSLDIR` names any installation; mcflirt recordings per version;
every replica case reports "FSL's own step".

**Records (standard tier against 6.0.7.23, from the clean 09dd455):** mcflirt replica 203 cases,
176 of 176 compared bit-identical, 22 rejected by both, 5 expected divergences (Armadillo/LAPACK);
FSL's own step 179 of 197 identical, 18 changed, each explained. BET replica 170 cases, 125 of 125
(310 files); 0 status changes against the 6.0.7.17 record.

**Clean-room head-motion correction against 6.0.7.23** (code not touched): 120 pass, 2 fail (the
known `real/ds003345/default`; `syn/motion/small/nn_final`, FSL-59); record and default stay on
6.0.7.17. List for the clean-room agent: FSL-59 in `reproduce` (§9.3); the zstd/bzip2 output types
and message (§10.1), reading by content (§4.1); FSL-60 (§4.1); its record against 6.0.7.23 and
`DEFAULT["original"]`; `normmi` and `mutualinfo` from the new appendix B (described exactly, at
the orchestrator's request). `specs/bet.md` updated for 6.0.7.23 (formats, FSL-60, usage,
`tm = t98`, `--binaryMask` with FSL-61 and FSL-62).

**Golden** (larmorx-nc, from c58a6f6): 33 → 36 cases, 242 → 261 outputs; 28 BET outputs changed,
all headers. **Memory:** BET's 5 calls within the rule. **Trial** (0.5 h, 5,817 cases): mcflirt
replica 2,222 of 2,222 agree; BET 6 differ, 3 time out; clean-room 634 agree, 1,582 differ (known
classes plus about 260 from FSL-59). **Site:** not done (the sandbox refused git in the clone).
**Gates:** all passed; `pytest tests/parity` 2,572 passed.

### Round 2
1. **mri-hmc threads** (`cd9d9ca`, harness only): larmorx at `LARMORX_PARITY_LX_THREADS` (4 by
   default), `OMP_NUM_THREADS` set around each in-process run; within 24 threads at `--jobs 6`.
2. **The mcflirt replica's full record** (`1885fee`): the 6.0.7.17 full record restored as
   `fsl-mcflirt-replica-full` (194 cases, 173 pass, 21 rejected by both), wired as BET's; the
   site keeps its four full-tier-only cases.
3. **The BET differences:** 661 and 1297 replica mistakes, fixed (`reproduce` does FSL's fill by
   the linear index, `fill_by_index`); 141 a new defect, **FSL-63** (`bet -R` loses a leading
   `-e` through bash's `echo`; rule A); 51 and 1308 harness mistakes, fixed; 80 (shrunk) FSL-43's
   reads of memory beyond the volume, judged undefined; 80 as drawn and 141 shrunk wait for the
   Armadillo part (bet2 `-c`, `fslstats -C`). Six permanent cases; `specs/bet.md` (FSL-43's fill
   by linear index, FSL-63); BET standard record re-made from `caefd64`: 176 cases, 128 of 128
   compared bit-identical (315 files), 31 rejected by both, 17 expected divergences. Golden
   unchanged.
4. **The site:** `evidence/fsl_step.py` (cache entries `<case>-fsl-6.0.7.23`, 6.0.7.17's kept);
   every example gains FSL 6.0.7.23's panel, the replica against it, "replica − FSL 6.0.7.23"
   (bit-identical, headers included) and "FSL 6.0.7.23 − FSL 6.0.7.17" as a reference row; both
   records side by side in the case explorers; the Validation page's table gains the records
   (`summary.py`). Inventory: no loss; number check BET 96 of 96, head motion 238 of 238,
   Validation 362 of 362; build 0 problems; 33 screenshots before and after.

**Gates:** Rust (three workspaces), Python 628 passed, ruff, format, SPDX, `tables --check`;
`pytest tests/parity` 2,578 passed at 09086ab.

### At the merge (orchestrator)
- The handoff integrated: the register (FSL-41 and FSL-43 rows and sections, FSL-59 to FSL-63
  rows and sections, the history paragraph), the public defect page, licensing, `UPSTREAM.md`
  (with a table of FSL 6.0.7.23's packages from `~/fsl-6.0.7.23/conda-meta/`), `upstream.tsv`,
  CLAUDE.md (rule 2, the FSL row, the records), next-steps (item 3's FSL half done; part 3 gains
  the Armadillo port; 3b names the head-motion follow-up; the final phase gains the FSL full
  tiers), the decision log, `triage.toml` (links to committed files), golden.md.
- fsl-flirt also changed in 6.0.7.23 (2111.2 → 2111.4), which the handoff did not list:
  recorded in `UPSTREAM.md` and `upstream.tsv` (FLIRT is not ported yet; its port starts from
  6.0.7.23).
