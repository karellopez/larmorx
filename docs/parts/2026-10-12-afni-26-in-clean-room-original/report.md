# Final report: AFNI-26 in clean-room original

Agent `a0313b1bd242cb7eb`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## AFNI-26's switch in the 3dTshift clean-room original: done

All gates pass. Branch `worktree-agent-a0313b1bd242cb7eb`, worktree `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a0313b1bd242cb7eb`. Nothing pushed or merged.

**Two things need your decision:**
- **The two implementations' defaults now differ on one case.** Take a read-only output file with `-overwrite`. The original's default refuses it on every platform: exit 1, AFNI-26 named. I tested the replica as a black box: its default replaces the file on Linux, through the rename, and names nothing. On Windows the replica's rename would retry for 2 s and then fail. The spec's default ("an error, exit 1") matches the original. You may want the replica to follow.
- **One known difference from AFNI, the same in both implementations.** Take an existing writable output in a directory that cannot be written, with `-overwrite`. AFNI replaces the file in place, exit 0. larmorx cannot create its temporary file there, so both modes fail with exit 1. I left this alone, as the brief said, and recorded it in the spec.

### Behaviour
- **New function `larmorx_afni::output::cannot_open(path)`.** It tells whether AFNI's writer could open the output, and changes nothing on disk.
  - It reports these cases: a directory with the output's name; an existing file that cannot be opened for writing (it opens without truncating, then closes); a new file whose probe cannot be created in its directory (the probe is removed when it can be).
  - Only refusals count: not found, not a directory, is a directory, permission denied, read-only file system.
  - On Windows, a sharing or lock violation (raw error 32 or 33) is not a refusal, so the existing wait-for-a-held-output path still applies.
- **`reproduce` mode** (`cli::tshift::write_output`, at the point of writing):
  - It names `defects::AFNI_26` as "reproduced".
  - It prints `** ERROR (nifti_image_write_engine): cannot open output file '<AFNI's name>'` raw: no prefix, never coloured.
  - It keeps `-verbose`'s `Wrote output` line (plain after a shift, labelled after a copy), writes nothing and exits 0.
  - For a one-row dataset without `AFNI_WRITE_1D_AS_PREFIX` it prints no message, because AFNI's 1D writer says nothing.
- **Default mode:** it names AFNI-26 as "corrected" and fails with `** FATAL ERROR: cannot open output file '<name>' (<OS reason>): nothing was written`, exit 1, at the same point.
- **Unchanged:** outputs that can be opened. A write that fails after opening is still an error in both modes.
- **Also changed:** an existing output now means an existing regular file (`output::exists`), as the spec says. This applies to the AFNI-11 check, the `N` stop and renaming. The black-box runs showed that AFNI does not treat a directory with the output's name as existing, and that renaming picks a directory's name if it comes first.

### Spec additions (§7.1, marked observed), from 32 black-box scenarios
- **Where AFNI's writer fails:**
  - a missing directory, or a path through a file;
  - a directory that cannot be written;
  - a read-only file with `-overwrite` or `AFNI_DECONFLICT=O…`;
  - a directory with the output's name;
  - a renamed output (`_AA1`) in a directory that cannot be written, or on a directory's name.
- **How the message looks:**
  - It goes to standard error. `AFNI_MESSAGE_PREFIX` never applies and it is never coloured.
  - It comes after every other message and just before `-verbose`'s line. An echoed command line still comes first.
  - Absolute paths, `../` paths and `.nii.gz` names are named as given. NIfTI-2 output prints the same message.
- **What does not produce it:**
  - A read-only file without overwrite is AFNI-11.
  - A name refused at AFNI-12's second check gives AFNI-12's message instead, even when its directory is missing.
  - A one-row dataset prints nothing (AFNI's 1D writer is silent).
- **Also added:** the in-place known difference, how larmorx detects the condition, its default, and an AFNI-26 row in the table of defaults.

### Parity
- I removed the three `errors/missing-directory*` entries from `DIVERGENCES["original"]` and left a comment.
- Record (standard tier, from clean commit 64ecce8): **314 cases: 279 pass, 32 rejected by both, 3 expected divergences, 0 failures.**
  - The 3 failed-write cases are rejected by both with the same exit status (0) and identical messages.
  - All 63 separately run cases match AFNI in exit status and messages (60 before).
  - The expected divergences left are `environment/one-row-dataset` and the 2 deliberate ones (`afni-format-output`, `voxshift`).
- The replica's three cases are also rejected by both. I did not re-run the replica's record, because the harness change does not affect it.

### Tests
- **New cross-platform test** `crates/larmorx-cli/tests/afni_globals.rs::a_failed_write_on_every_platform`. It covers shifts and copies, with an output in a missing directory, in a temporary path with blanks and `é`; it does not assume a temporary path free of `~`.
  - Default: exit 1, AFNI-26 named, no `Wrote output`.
  - `reproduce`: exit 0, ending exactly with AFNI's message and `-verbose`'s line (with `AFNI_MESSAGE_PREFIX`), nothing created.
- **New unit tests in `output.rs`** for `cannot_open`, `exists` and renaming onto a directory. A read-only directory test runs on Unix only. Both skip their permission assertions when the runner can write anyway.

### Gates (all pass)
- **Rust, root and `crates-gpl`:** `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked`.
- **Python** (wheel from `maturin build`, its `_core.abi3.so` copied into the worktree): `pytest tests/python tests/golden` 355 passed; `ruff check`, `ruff format --check` and `check_spdx` clean.
- **Parity smoke tier:** `pytest tests/parity` 1892 passed.
- **Golden checksums:** unchanged.

### Docs
- `docs/api/afni-tshift.md` and `docs/api/upstream-defects.md`: AFNI-26 rows (both implementations), and the record numbers (314 cases, 21 error cases, 63 of 63 identical).
- Register, public part only:
  - the AFNI-26 Summary row's gap is now "–";
  - the gap-count paragraph now says no **original** gap is left;
  - the D13 gaps status text and the AFNI-26 row of the slice-timing table are updated.
- `crates/larmorx-afni/PROVENANCE.md` and README's 3dTshift numbers are updated. CLAUDE.md is untouched.

### Commits
- 8e0d3e9 – WIP checkpoint: code, test, harness
- 8dd091a – docs
- d044d4c – record from 8dd091a
- 64ecce8 – Windows sharing-violation guard (no effect on Linux)
- 5c28d25 – record from 64ecce8, README and API numbers

No AI attribution in any commit.

### Clean-room statement
- **Opened:** CLAUDE.md, porting guide, `specs/3dTshift.md`, the API docs, the register before its "Restricted" heading, larmorx's own Apache code in `crates/larmorx-afni`, `-core` and `-cli`, the validation harness, tests and records. I ran AFNI's binary and the replica only as black boxes, with no GUI or help-viewer options.
- **Not opened:** AFNI source, any `crates-gpl/` file or diff, `docs/findings/afni-tshift.md` or `fsl-mcflirt.md`, `reference_src/`, `crates-nc/`, or commits 2644219 and eb38033.
- **Accidental exposures:**
  - My first `grep -rn AFNI-26` over the repo printed matching lines from `crates-gpl/` files (one PROVENANCE row, a test, two doc-comment lines) and two lines of the register's restricted section (the AFNI-26 heading and one line saying "both implementations exit 1 by default"). None of them went beyond what the spec and the brief already state. I excluded those paths afterwards.
  - A later grep over `docs/` also scanned the restricted files but printed nothing from them.
