# Final report: 3dTshift globals: -overwrite, -D (replica+spec)

Agent `a5cefece59c950414`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## Part 2a is done: AFNI's global options and environment variables in the 3dTshift replica

Branch `worktree-agent-a5cefece59c950414` (worktree `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a5cefece59c950414`). `main` (10d5922) is merged in. Nothing was pushed. There is no AI attribution in any commit.

**Result:** in `reproduce` mode the replica matches AFNI byte for byte on every compared case of both new categories (57 cases), exit status and messages included. All gates passed before each commit:
- both Rust workspaces: fmt, clippy and tests;
- `maturin develop --release --skip-install`, then pytest on `tests/python` and `tests/golden` (346 passed);
- ruff and the SPDX check.

### Commits
- `35afc3b` WIP checkpoint: the replica, the Python wrapper, the CLI tests and the defect registry.
- `353946b` The replica's global options and variables, the parity categories, spec §7.1, and five new golden cases.
- `8bfbe98` x87 `long double` emulation and all the docs.
- `4aaad72` Merge of `main`. Its decision-log conflict is resolved: main's decision entry is kept, and mine follows it as a "part 2a done" entry.
- `b5d51be` Validation records from `4aaad72` (a clean tree; logs in `oracles/logs/globalopts-records*.log`) and the GPL golden checksums recorded from it.

### Global options (observed with the oracle)
- **Where they can stand:** anywhere, including in place of another option's value (`-tpattern -overwrite alt+z` works). The help check runs before they are taken out, so `-overwrite -help` gives "Unknown option: -help".
- **`-overwrite`** sets `AFNI_DECONFLICT=OVERWRITE` and suppresses AFNI's "Over-writing" warning.
- **`-ok_1D_text`** sets `AFNI_1D_ZERO_TEXT=YES`. **`-skip_afnirc`** is accepted; larmorx never reads `.afnirc`.
- **`-Dname=value`:** AFNI parses the text after `-D` with `sscanf("%255s %1023s")`, so it splits at blanks (in the spec as observed behaviour only):
  - `-DX=a b` sets `X` to `a=b`;
  - `-DX= YES` and `-DX =YES` set `X` to `=YES`;
  - `-DX=` and `-D=YES` set nothing, silently;
  - `-DX`, `-D` and `-dX=` are unknown options;
  - the last `-D` wins, and `-D` wins over the environment.
- **Other global options** (`-V`, `-echo_edu`, `-np`, `-pif`, `-all_opts`, `-h_find`, `<<` inclusions, …) are unknown options in larmorx. What AFNI does with each is in the spec.

### The variables 3dTshift consults (`environ::CONSULTED`)
Found by reading the source, then confirmed with an `LD_PRELOAD` `getenv` trace over 40 oracle runs, which saw 52 names.

**Applied, with AFNI's effect and messages:**
- `AFNI_DECONFLICT`:
  - `OVERWRITE` exactly: overwrite with a warning;
  - other values starting with O/o: overwrite quietly;
  - Y/y: rename to `_AA1`…`_ZZ9` with a warning;
  - upper-case N: a shift stops with a fatal error (exit 1), but a copy falls back to AFNI-11;
  - lower-case n, or anything else: as unset (AFNI-11).
- `AFNI_ALLOW_ARBITRARY_FILENAMES`: AFNI still refuses an empty name, and at write time a name beginning with `-`.
- `AFNI_PATH_SPACES_OK`, `HOME` (`~/` and the command-line echo), `AFNI_NOREALPATH` (AFNI names the input by its real path in messages).
- `AFNI_NIFTI_PRIORITY` (S/Q, an illegal-value warning, a handedness warning).
- `AFNI_NIFTI_SCALE=NO`: integer data keep the slope and lose the intercept (AFNI-23).
- `AFNI_NIFTI_TYPE_WARN` (none, one or three raw lines).
- `AFNI_NIFTI_VIEW` and `AFNI_DEFAULT_STD_SPACE` (the output's xform codes); a warning for sform code 7.
- `AFNI_NO_OBLIQUE_WARNING` (AFNI's oblique report).
- `AFNI_1D_ZERO_TEXT`, `AFNI_USE_FGETS`.
- `AFNI_NIFTI_WRITE_TYPE`: 2 (also 2.9) writes NIfTI-2; `2k` is 2048.
- `AFNI_ALLOW_MILLISECONDS` (AFNI-19), `AFNI_WRITE_1D_AS_PREFIX` (AFNI-20).
- `AFNI_MESSAGE_PREFIX`, `AFNI_MESSAGE_COLORIZE` (AFNI-21), `AFNI_ECHO_COMMANDLINE`, `AFNI_DONT_LOGFILE`.

**Accepted, no effect, each with its reason in the code:** the header-extension variables, debugging (`AFNI_TRACE`, `AFNI_DEBUG`, …), `AFNI_QUIET_STARTUP`, `.afnirc`, `AFNI_FLOATSCAN`, `AFNI_BYTEORDER`, the atlas paths, the tcat selectors, AFNI's own format, `AFNI_RANDOM_SEEDVAL`.

**Not consulted:** a `-D` of any other variable is applied with "has no effect (larmorx)".

**Fix mode keeps D13's corrections:**
- `AFNI_ALLOW_ARBITRARY_FILENAMES`, `AFNI_PATH_SPACES_OK`, `AFNI_ALLOW_MILLISECONDS`, `AFNI_WRITE_1D_AS_PREFIX` and `AFNI_MESSAGE_COLORIZE` change nothing that is written or printed in fix mode; they only decide whether AFNI-12, -18, -19, -20 or -21 applies.
- `AFNI_DECONFLICT` acts as in AFNI, except that where AFNI-11 applies the default is an error before the work.

**Design:** the process environment is never changed (no `unsafe`). It is read once into `Settings` (names case-insensitive on Windows), the `-D` options are applied in command-line order, and the table is passed explicitly to every routine.

### Defects registered (`docs/findings/upstream-defects.md`: summary 132 entries, counts, gaps)
- **AFNI-11 corrected:** the `AFNI_DECONFLICT` details above; with `-verbose`, AFNI names a file it did not write.
- **AFNI-12 corrected:** what `AFNI_ALLOW_ARBITRARY_FILENAMES` still refuses.
- **New:**
  - AFNI-18 (e): an input name with a blank is read as a list of datasets, so an input in a directory with a blank cannot be opened.
  - AFNI-19 (c): `AFNI_ALLOW_MILLISECONDS` writes `toffset` in milliseconds.
  - AFNI-20 (c): a one-row dataset is written as a 1D text file. Not reproduced: AFNI's file has a random identifier.
  - AFNI-21 (f): colour escape codes in labels, also into files and pipes.
  - AFNI-22 (e): `-D` parsing.
  - AFNI-23 (c?, to decide): `AFNI_NIFTI_SCALE=NO` drops the intercept.
  - AFNI-24 (d): the NIfTI library's `long double` arithmetic.
- **x87 emulation (AFNI-24):** found because a NIfTI-2 output's `quatern_c` differed in its last bit. New module `larmorx_gpl_afni::x87` emulates x87 sums, products and quotients with integer arithmetic.
  - It matches GCC on 5 million random operations; 1,071 of them differ from plain double arithmetic.
  - NIfTI-1 and NIfTI-2 headers now match AFNI byte for byte on 120 random oblique inputs and 160 qform-only, near-180° and trace-near-0 ones.

### Parity (records in `docs/validation/`, from `4aaad72`)
- **Replica:** 308 cases; 276 pass, all 276 bit-identical in `reproduce`; 29 rejected by both; 3 expected divergences (AFNI's own format, `-voxshift`, AFNI-20); 0 failures.
  - The 57 global/environment cases: exit status and standard output and error identical, colour codes included. Only AFNI's banner, compile date and larmorx's own lines are set aside.
  - The fix pass names AFNI-19 and AFNI-22. AFNI-18's case is rejected by both in `reproduce`, so the fix pass does not cover it.
  - Existing golden checksums are unchanged.
- **Original:** 223 pass (163 bit-identical), 17 rejected by both, 68 expected divergences (56 new, pending part 2b), 0 failures.

### Cross-platform CI tests (tiny)
- **`gpl` job, six platforms:**
  - `tshift_global_options` in `crates-gpl/larmorx-gpl-cli/tests/cli.rs` (0.05 s) covers:
    - `-overwrite` after the dataset and as an option's value, on `été 1.nii`, with no temporary file left behind;
    - `AFNI_DECONFLICT=YES` via `-D` and via the environment, and `-D` NO winning over the environment;
    - `AFNI_NIFTI_WRITE_TYPE=2` via `-D` and via the environment.
  - A Windows-only test holds the output open for 0.3 s and checks the retry. I could not run it locally; CI will.
  - Five new golden cases: overwrite, NIfTI-2 by `-D` and by environment, milliseconds in both modes.
- **`test` job:** `tests/python/test_replica.py::test_afni_global_options_and_variables_reach_the_replica`.

### Spec (behaviour only, marked observed)
New `specs/3dTshift.md` §7.1 covers:
- the settings rules; the global options and the `-D` parsing table; the unsupported options;
- the variable table and the variables with no effect; how each acts in default mode; `AFNI_DECONFLICT`;
- input names with a blank; the input's name in messages; xform codes and template spaces; the oblique report;
- NIfTI-2 and extended precision; `-tpattern @file` messages; message rules (prefix, colour, echo, banner);
- defaults and notices for AFNI-18 to -22; the cross-platform design.

Also updated: the defects intro, §6.1, §6.3, §7, §8 (Python) and §9.

### Docs
- Restricted findings: `docs/findings/afni-tshift.md`.
- API pages: `docs/api/afni-tshift.md` (new section "AFNI's global options and environment variables") and `docs/api/upstream-defects.md`.
- `crates-gpl/README.md`, `PROVENANCE.md` (new rows; it also corrects which files are MCW and which NIH in three module headers), `docs/validation/golden.md`, `docs/overview.md`.
- `docs/decision-log.md` and `docs/porting-guide.md` (new §9.1, the settings-table pattern, and a note on `long double`).

### Notes for you
- **To review:** `lx.afni.tshift` passes the process environment's AFNI variables on to the replica. It forces `AFNI_PATH_SPACES_OK=YES` and plain messages, and suppresses the oblique report. Whether the Python API should read AFNI variables at all (rule 6, no global configuration) is flagged in the decision log; part 2b's original should do whatever is decided. AFNI-23 also needs a decision.
- **Not reproduced (documented):** AFNI's log file and its 100 MB size warning; no echo when `HOME` is unwritable; AFNI's tracing output.
- **Probing side effect:** while probing AFNI's `-h_view`, AFNI opened gedit on the desktop with its help file. I closed that process. I did not run `-h_web` or the other browser-opening options.
- **Stale numbers:** `README.md` still quotes old 3dTshift numbers (217 of 217) from before this task. I left it for you, along with CLAUDE.md.
