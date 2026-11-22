# FreeSurfer 8.2.0's `getfullpath`: findings of the replica (queue item 3.2-12)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/getfullpath.md`. Track A, relay 52 (2026-11-22). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/getfullpath/` with the tcsh model
`src/csh.rs`; the probe: `validation/probes/fs-getfullpath/`; the record:
`docs/validation/freesurfer-getfullpath.md`.

## The script

`scripts/getfullpath` (42 lines of tcsh, installed unchanged as `bin/getfullpath`; the older
`fsfast/bin/getfullpath`, a csh script with other messages, is not installed): `if($#argv < 1)`
prints `getfullpath filename` and exits 1; `set fname = $argv[1];`; `` set dirname = `dirname
$fname`; ``; `if(! -e $dirname)` echoes `ERROR: cannot find $dirname` and exits 1; `pushd $dirname >
/dev/null`; `` set dirname = `pwd`; ``; `` set base = `basename $fname`; ``; `echo $dirname/$base`;
`exit 0`. Not one variable is quoted.

## The defects (rules A to D; `larmorx_core::defects`)

The names FreeSurfer's scripts pass (seg2cc's, fs-synthmorph-reg's, rca-talairach's) are plain
paths, and for those the script prints the full name. Every other character tcsh gives a meaning
to makes it re-read the name, its folder or the path: a file name with a blank, a `*`, braces or a
backslash is a valid name, and the script then prints another file's name, a mangled path or an
error (rule A: correct it; by default the name as given; `reproduce` gives tcsh's result,
measured case by case):

1. **FS-1173** (class e: a command-line quirk; rule A). Blanks split the name into words: the
   first is used, the next are read by `set` as assignments (errors for most of them; `x
   fname=d1/y.mgz` makes the script print `d1/y.mgz`'s full name); patterns and braces are
   expanded against the files (several matches: `if: Expression Syntax.` or arithmetic errors such
   as `Division by 0.` when `dirname` prints `/`); `~`, `~user`, `=N` are expanded; a folder `+N`
   is `pushd`'s stack; an empty name gives `dirname`'s and `if`'s errors. Measured on 134 fixed
   cases and 1,500 seeded random names.
2. **FS-1174** (class e; rule A). The result goes through tcsh's `echo` (`echo_style` both):
   `\t`, `\n`, `\c`, `\0nnn` and the like in a name or a folder print other bytes or end the
   output.
3. **FS-1175** (class e; rule A). A name starting with `-` reaches `dirname` as options.
4. **FS-1176** (class e; rule A). The folder's physical path is read again: its blanks collapse,
   its patterns are matched (`echo: No match.`).

No FreeSurfer step and no fMRIPrep call passes such a name (the recorded calls: 54 of 54
identical in both modes): no impact.

## The forms not reproduced (D13 gap)

- A new line in the name: tcsh's lexer gives errors this model does not follow (`x<NL>y`: `set:
  Variable name must begin with a letter.`; `x<NL>` and `<NL>x` behave differently).
- Bracket ranges: tcsh 6.21's range match does not follow the characters' codes (measured over
  every printable character: `[a-c]` matches `0 9 A B C Z _ a b c`, `[A-C]` `0 9 A B C`, `[c-a]`
  `0 9 A B C Z _ a c`: a character equal to the start, or not above the end in an order that puts
  digits, then capitals, `_`, then small letters, and leaves most punctuation out); not modelled.
- Subscripted assignments to tcsh's own variables, arithmetic with numbers, ambiguous `-e`
  operands.
Both modes then give the default with FS-1173's notice saying so (`UNMODELLED` in the probe).

## The tcsh model (`src/csh.rs`)

Black-box: every rule in it comes from runs of the official script and of small test scripts
(`validation/probes/fs-getfullpath/`'s battery, the README); no tcsh source was read. Notable
measured facts: a command substitution's output is matched as a pattern only when the command's
own line held a pattern character (`` `pwd` ``'s output never is); `if`'s words are not matched,
only `-e`'s operand is (`<word>: No match.`); the subshell's no-match message names the command
(`basename: No match.`); `pushd +0` enters a folder `+0`, `+1` is the stack; after `pushd` the
directory stack holds the folder left, so `=1` names it.
