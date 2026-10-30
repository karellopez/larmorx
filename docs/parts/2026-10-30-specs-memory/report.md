## Report: the specs of FSL-20/21, AFNI-75 and cbrtf; the AFNI replicas' memory (2026-10-30)

Branch `worktree-agent-a67c9c36c5d6371cb`, six commits from fb3f334 (965ec59, c0b2968, 655ba07,
a9c584f, 1c4d4f6, 5d0a982); the handoff is `handoff.md` here.

### The three specs (behaviour only; each new line marked "(spec writer, 2026-10-30)")
- **`specs/mcflirt.md`** (§10.6, §12.1, §14.1): mcflirt 6.0.7.23's black-box behaviour and what
  larmorx's default and `reproduce` must do, with exact messages, exit statuses and files: FSL-20
  (the `.mat` directory cannot be made: "Error!: Unknown Log directory return value", exit 0,
  nothing written; the default an error, `reproduce` mcflirt's), FSL-21 (fewer than 8 bins with a
  histogram cost; `corratio` aborts with "double free or corruption (out)"). Two new defects found
  by the black-box runs: **FSL-65** (an unopenable `.par`: "error: unable to open output file!",
  exit 0 after `MAT_0000` and the `.rms` files; an unopenable `.rms` skipped silently) and
  **FSL-66** (a negative `-bins` crashes mcflirt for every cost: signal 8 at −1 bins in a stage,
  `std::bad_array_new_length` at −2 or fewer); both decided by rule B. §14.1 holds 30 cases for
  the clean-room implementer; the probe `validation/investigations/specs-memory/mcflirt_fsl20_21.py`.
- **`specs/3dUnifize.md`** (§7, §9, §9.1, §10): AFNI-75's guard: when it runs, the memory
  prediction `(4/3·π·r³ + 1)·(24 + 4·w)` bytes (w workers), the limit (the given one, then
  `LARMORX_MEMORY_LIMIT`, then physical memory above 4 GiB, then 1 TiB), the exact messages, exit 1;
  `reproduce` without the guard; the cases (3 µm, 1 µm and 0.05 mm voxels refused; 0.3 mm and 3 mm
  run, bit-identical to AFNI); AFNI itself dies with signal 11 under a 2 GiB address-space limit.
  The probe `unifize_epi_ball.py`.
- **`specs/cbrtf.md`** (new): glibc 2.35's `cbrtf` (x86-64) as mathematics: special values, the
  reduction, the quadratic start in binary64 rounded to binary32, the cube in binary32, one Halley
  correction in binary64 with its order of operations, every rounding (the deliberate double
  rounding included), the constants as binary64 bits, 18 black-box examples. The validation plan:
  block hashes of all 2^32 inputs from the system's `cbrtf` as a black box
  (`cbrtf-glibc-2.35-x86_64-blocks.json`, hash of hashes `0e26908cfda8506a`, whole-sequence hash
  `c679cb92d4402e33`). Complete enough for bit-identity on every non-NaN result (NaN payloads
  specified, not exhaustively verified; NaN never matters to 3dUnifize).

### The memory fixes
- **The 3dUnifize replica:** copies AFNI does not hold released where AFNI releases them or never
  made; the arithmetic unchanged. Peak at one thread (MiB): `-T2` AFNI 99.1, before 164.8 / 151.3
  (python / binary), after 115.2 / 103.8 (allowance 117.0); `-EPI` AFNI 109.2, before 320.9 / 307.7,
  after 120.8 / 110.4 (allowance 128.1). Its standard record re-made from the clean 1c4d4f6:
  compare_records 116 of 116, 0 status and 0 detail changes. Two strict expected failures removed.
- **The Python entry point:** numpy was never imported on the command line; the fixed costs are
  the interpreter (9.0 MiB), the Rust core (4.0) and `larmorx._replica`'s imports (2.3), the last
  now deferred: 3dAutomask on the small image 21.3 → 19.6 MiB against AFNI's 7.7 and the allowance
  16.5. No import is left to defer; the strict expected failure stays, with its cause.

### Gates
Rust (three workspaces; the replica's golden checksums unchanged), Python 625 passed, ruff, SPDX,
`tables --check`; thread pools 77 passed; memory: 3 pass and 1 strict expected failure (3dAutomask
through `python -m larmorx`); parity smoke 2,586 passed and 1 expected failure.

### For the user
1. The memory test's allowance for the Python entry point (recommended: count its fixed cost apart,
   1.10 × the original's peak plus the peak of `python -m larmorx --version`, keeping the 8 MiB rule
   for the computation).
2. `cbrtf`: the spec is ready if the user agrees to the clean-room part.

### At the merge (orchestrator)
The handoff integrated: the register (FSL-65 and FSL-66 rows and † sections, FSL-20, FSL-21 and
AFNI-75's spec references, the history), the public defect page (two rows in its six-column form),
next-steps, the decision log.
