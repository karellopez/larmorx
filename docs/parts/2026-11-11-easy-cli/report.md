# The easy command line (2026-11-11): the final report

Relay 3's final report, condensed, with local paths replaced by placeholders; the orchestrator's
merge notes follow it. The part's agents, their roles and their notes are in `notes.md`; the
stopped first attempt's brief and notes in `brief-first-version.md` and `notes-stopped-relay1.md`.

## What the part built

- **larmorx's own options before the family**, mirroring the Python API (the user: "I prefer
  mirrored names"): `larmorx [--n-threads N] [--implementation auto|replica|clean-room]
  [--upstream-defects fix|reproduce] <family> <tool> ...`, in `larmorx`, `lx`, `larmorx-nc` and
  `larmorx-gpl` (the companions: `--implementation replica` a no-op, the others refused). Parsed once
  (`larmorx_core::cli`), passed to every family as an explicit overlay (the process environment never
  changed); an option wins over its variable, with the variable's bytes; wrong values exit 2 with the
  usage. A tool's own thread option (tedana's `--n-threads`, topup's `--nthr`) wins when given;
  otherwise the run's thread count applies (before: one thread for those two).
- **`clean-room`** as the implementation's name (the user: "I prefer clean-room instead of
  original") in Python, the command line, `LARMORX_IMPLEMENTATION` and what a result reports;
  `original` accepted (Python warns with a `DeprecationWarning`); the clean-room AFNI programs' own
  verbose line; 49 golden registry values renamed (checksums unchanged).
- **A tool's messages while it runs** through the console script (the first line of a 12.6 s
  `N4BiasFieldCorrection -v`: 12.54 s before, 0.01 s now).
- **FSL-11 for every FSL tool, both implementations:** `FSLOUTPUTTYPE` unset writes `.nii.gz` (the
  bytes of `FSLOUTPUTTYPE=NIFTI_GZ`) with one notice in one shared text
  (`larmorx_core::defects::fsl11_notice`, per-tool words: `standard_space_roi` exits 0 without an
  image in FSL); `reproduce` stops where FSL stops, with FSL's message and exit status (checked
  against FSL 6.0.7.23's own runs). FSL-09's notices name the program that ran.
- **Docs:** the README's First steps (where pip puts the commands, the options with the variables and
  the Python arguments, one example for bash/zsh and one for PowerShell, the notices explained), run as
  typed in bash, an interactive zsh 5.9.2 and Windows PowerShell 5.1 (0 failed); no comments in
  copyable shell blocks (zsh's prompt does not take `#` as a comment), with a test and a site build
  check; 17 API pages renamed; CHANGELOG's Unreleased section; the site's installation page describes
  0.1.0rc1 and names the options as coming with the next release.
- **Also:** PyPI's author (Karel Mauricio López Vilaret, with the e-mail CITATION.cff gives) for the
  next release; `check_install.py` keeps pip's temporary files and cache in its work folder.

## Numbers

- Smoke tier (6 workers, the part's last code commit `d300ef43`): 3,020 passed, 0 failed. Python
  1,440 passed; CI's environment 933; `test_release` 65. Golden checksums unchanged in the whole part.
- Records re-made from `d300ef43`: `mri-hmc` 170 cases, 145 pass, 21 rejected by both, 4 expected,
  0 fail (its `no-fsloutputtype` case now 13 of 13 checks); `fsl-prelude` 166 cases, 138 pass, 28
  rejected by both, 0 fail (`orig-types/unset` 14 of 14).
- CI cost: the CLI's output-type test trimmed to 3.7 s per platform; the full set in the parity tier.

## Open points (in next-steps' follow-ups through the handoff)

1. The parity harness: a failed check inside a case classed "rejected by both" or "expected
   divergence" does not fail its test (how `mri-hmc`'s case failed silently after relay 2b).
2. The register's FSL-11 section body (restricted) still describes only mcflirt and flirt: a
   replica-side edit to widen it.
3. The README's advice for a refused `Activate.ps1` is untested (the Windows helper's policy is fixed).

## The orchestrator's merge (2026-11-11)

- Merged as `c2583cc0` after the part's 21 commits on main `7cd35c8c`.
- The site's inventory after the merge: one page has one number fewer (`tools/mri-hmc`: 263 -> 262);
  it is the synthetic/motion group's expected-divergence count, which became 0 because the case's
  result changed (it is now rejected by both, with 13 checks instead of 1) and the builder draws no
  zero counts. The case is still shown, with more evidence; judged no loss of evidence (the rule of
  2026-10-24 concerns removing or downsampling what the site shows).
