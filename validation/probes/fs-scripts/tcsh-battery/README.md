# tcsh-battery: the shell facts behind `src/script.rs`

Small tcsh scripts (run by `/bin/tcsh -f` 6.21.00 in a scratch folder, each in a memory box) whose
outputs fixed the replica's shell model for seg2cc, mergeseg and fscalc (relay 58 of track A):

- `t1.csh`: `pushd`/`popd`'s printed stack (each entry and a blank, `$HOME` as `~`); a pipeline's
  status (`false | true` gives 1: `anyerror`); `echo` with no word and `-n`; a `>>` into a missing
  folder (`<name>: No such file or directory.` ends the script).
- `t2.csh`: `if($v == yes)` with `$v` empty is false, no error.
- `t3.csh`: `echo "ERROR: $cmd"` with `cmd` unset: `cmd: Undefined variable.`, status 1.
- `t4.csh`: `$0` is the script's absolute name; `cat` of a missing file in a pipeline; a backquote
  of nothing gives no word.
- `t5.csh <dir>`: `pushd`'s names (links kept, `..` taken off as text; the start from `PWD`).
- `t6.csh`, `t7.csh <word>`: `if($e > 0)` for words that are not numbers (`Expression Syntax.`,
  `Badly formed number.`), `08` and `010` (decimal), a number past 64 bits.

The probe's cases (`check.py`) repeat each fact on the official scripts.
