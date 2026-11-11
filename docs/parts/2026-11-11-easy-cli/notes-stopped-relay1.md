# release 0.1.0: the easy command line — NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a274715ea981aa7f6
(branch of the harness; base 7cd35c8c)

## Relay 1 (running): sub-task 1, the options; author metadata; check_install.py's temp files; streaming measurement
- started 2026-11-11

## FROM THE ORCHESTRATOR
- 2026-11-11 (relayed during relay 1): change of plan from the user: NO RELEASE in this part ("I do not
  really want to release yet until we have the full pipeline. Just fix all this and do the things before
  Freesurfer."). Sub-task 1 unchanged (the three options, the author metadata, check_install.py's temporary
  files and cache, measuring live output, all gates incl. the parity smoke tier). Do NOT change any version
  number (stays 0.1.0rc1), no 0.1.0 CHANGELOG section or release text, no tag or dry run planned. BRIEF.md
  is being updated (the part is now "the easy command line", without the release); the next release comes
  once the full pipeline exists.
- 2026-11-11 (relayed during relay 1): FINAL OPTION NAMES from the user, replacing the brief's first
  proposal: `larmorx [--threads N] [--replica | --clean-room] [--reproduce] <family> <tool> <args>`.
  `--threads N` unchanged (0 = every CPU, default every CPU). `--reproduce`: a switch = Python's
  upstream_defects="reproduce"; without it the default (fix). `--replica` / `--clean-room`: switches =
  implementation="replica" / "original"; neither = auto; both = a usage error (`--clean-room`, not
  `--original`: on the command line "original" reads as the original FSL/AFNI program). Everything else
  holds: options between `larmorx` and the family, the variables keep working, a switch given wins over
  its variable, same output bytes, Python's API unchanged. BRIEF.md carries these names.
