# The tcsh measurements behind `src/csh.rs`

**Closed to FreeSurfer clean-room implementers** with the rest of this probe (the clean room
measures tcsh itself from `specs/getfullpath.md` §3).

Small tcsh scripts and the loops that ran them (relay 52 of track A, 2026-11-22), each run by
`/bin/tcsh -f` (6.21.00, Ubuntu 22.04) with `env -i PATH=/usr/bin:/bin HOME=/nonexistent-home`
(some with `LC_ALL=C`), in scratch folders holding the files the loops name (`w`: `d1/x.mgz`,
`d1/y.mgz`, `a b/f`, `afile`, a link `lnk` to `real`, ...; `w3`: `B a _c .h "q r" "*s"`, folders
`+ +1 =1 nm[x]`; `w4`: `A B C Z a b c z 0 9 + _ ~`, folders `dd1 dd2`; `w5`: one file per printable
character). What each showed is in `docs/findings/freesurfer-getfullpath.md` ("The tcsh model")
and the module's documentation; the probe's cases (`tests/fs_getfullpath/cases.rs`) repeat each
fact on the official script.

- `p1.csh`: `set fname = $argv[1]` then the word counts of `fname`, `dirname`'s and `basename`'s
  outputs (splitting, patterns, braces, an unclosed `{`).
- `p2.csh`: `set v = $argv[1]` and its words (assignment names, `=N`, patterns, ranges).
- `p3.csh`, `p6.csh`, `p9.csh`, `p10.csh`: `` set b = `...` `` with tcsh's `echo`, `/bin/echo` and
  `basename`: a command substitution's words get braces, `~`, `=N`, and patterns when the command's
  line held one. `t8.sh` writes `p11.csh` per command (`pwd`, `/bin/pwd`, `/bin/echo $cwd:q`): `pwd`'s
  output is never matched.
- `p4.csh`, `p7.csh`: `echo $argv[1]` (escapes, `-n`, braces inside a word).
- `p5.csh`: `echo a* yy* zz*` (patterns that match nothing dropped when another matched).
- `p8.csh`: `` set b = `pwd` `` in a folder named `nm[x]`.
- `p12.csh`, `p13.csh`: `if(! -e ...)` with several words (arithmetic: `Division by 0.`, `Mod by
  0.`, `Expression Syntax.`) and with a pattern (`<word>: No match.`).
- `t.sh`, `t2.sh`, `t4.sh`: the official script on odd names; `t3.sh`, `t5.sh` to `t10.sh`: the
  loops over the small scripts; `t11.py`: the range order over every printable character.
