# `getfullpath` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-12: the default, tcsh's reading of the name with every message
measured, the defects with their switches, the forms not reproduced, and the validation plan, §7).
Written by the replica's author (track A, relay 52) as the spec writer: behaviour and messages, for
the clean room (`larmorx-fs-volumes`), which works from this file and black-box runs of the official
script only. Every statement about tcsh is measured on the official script (`/bin/tcsh` 6.21.00 as
Ubuntu 22.04 builds it, GNU coreutils 8.32's `dirname`, `basename`, `pwd`, the C locale) and on
small test scripts; **(observed)** marks them.

## 1. What it is for

`getfullpath <name>` prints the full name of a file: the physical path of the name's folder (every
link resolved, as `/bin/pwd` prints it), `/`, the name's last component. seg2cc runs it on
`$SUBJECTS_DIR/<subject>/mri/aseg.auto_noCCseg.mgz` and `norm.mgz`, fs-synthmorph-reg on its
input, target and output folder, rca-talairach on an expert-options file; FreeSurfer's scripts take
its standard output as the name.

## 2. The default (the name as given)

1. No argument, or an empty first argument: `getfullpath filename` + `\n` on standard output,
   status 1. Arguments after the first are ignored.
2. The folder: GNU `dirname` of the name, without option parsing (the name's trailing `/`s
   removed; no `/` left: `.`; only `/`s: `/`; otherwise what is before the last `/`, its trailing
   `/`s removed, `/` when nothing is left).
3. The folder does not exist (`stat`, links followed): `ERROR: cannot find <folder>` + `\n` on
   standard output, status 1.
4. It cannot be entered: `<folder>: Not a directory.` (a file), `<folder>: Permission denied.` (no
   search permission), + `\n` on standard error, status 1.
5. Else `<physical path of the folder>/<last component>` + `\n` on standard output, status 0; the
   last component is GNU `basename` of the name (trailing `/`s removed, what follows the last `/`;
   only `/`s: `/`). So `/` gives `///`, `/.` gives `//.`, `d1/` gives `<path of the current
   folder>/d1`, `a/.` with `a` a file gives `a: Not a directory.`.

Nothing is written. The name and the paths are printed as they are (no escapes, blanks kept).

## 3. What tcsh does (`reproduce`) (observed)

The script runs, in order: `set fname = $argv[1]`; `` set dirname = `dirname $fname` ``;
`if(! -e $dirname)` (then `echo "ERROR: cannot find $dirname"`, status 1); `pushd $dirname >
/dev/null`; `` set dirname = `pwd` ``; `` set base = `basename $fname` ``; `echo $dirname/$base`;
status 0. No variable is quoted, so each use reads the words again:

- **Splitting.** An unquoted variable gives each of its words split at blanks (space, tab), empty
  pieces dropped. In `set fname = $argv[1]` the first piece is the value and every later piece is
  read as an assignment: `name`, `name=value` or `name = value` (a later `fname` assignment
  replaces the value); a name must start with an ASCII letter or `_` (`set: Variable name must
  begin with a letter.`) and hold only ASCII letters, digits and `_` (`set: Variable name must
  contain alphanumeric characters.`); `name[sub]=...` with a subscript that is not a number, a
  `N-M` range or `*` (or text after the `]`): `set: Subscript error.`; with a valid subscript:
  `<name>: Undefined variable.`. These messages end the script with status 1 (standard error).
- **Expansion of a word**, in order: braces (`a{b,c}d` gives `abd acd`; the words `{`, `}` and
  `{}` stay; `{}` inside a word is an empty choice; an unclosed `{`: `Missing '}'.`); a leading
  `~` (the `HOME` variable's value, `No $home variable set.` without one; `~user`: that user's
  home folder from the password file, `Unknown user: <user>.`); a leading `=N` or `=-` that is the
  whole word or followed by `/` (the directory stack: before `pushd` it holds the current folder
  only, after it the new folder then the one left; `=0` the current folder, `=-` the last entry,
  `=N` beyond the stack: `Directory stack not that deep.`); then, when the word holds `*`, `?` or
  a `[` with a `]` after it, its matches among the files (each `/`-separated piece matched against
  the folder's names; `*` any characters, `?` one, `[...]` one of a set, `^` first negating it, `!`
  an ordinary character; a name starting with `.` only matched by a piece starting with `.`, `.*`
  matching `.` and `..`; the matches in byte order). In one command, patterns that match nothing
  are dropped when another pattern matched, else the command fails: `set: No match.`, `echo: No
  match.`, `dirname: No match.` / `basename: No match.` in a command substitution (which then
  prints nothing and the script goes on).
- **Command substitution** (`` `cmd` ``): the command runs on the words of its line (expanded as
  above, in the folder current then); its output is split at blanks and new lines, and `set` gives
  each word its braces, `~` and `=N`, and matches it as a pattern only when the command's line
  held `*`, `?` or `[` (from `fname`'s words): `` `pwd` ``'s path is never matched.
- **`dirname` and `basename`** (GNU): a word starting with `-` (but `-` alone) before a `--` is an
  option: `-z`/`--zero` and any prefix of `--zero` accepted; `--help`, `--version` and their
  prefixes print a text of many words; other short letters `<prog>: invalid option -- '<c>'`, other
  long options `<prog>: unrecognized option '<word>'`, each followed by `Try '<prog> --help' for
  more information.`; no operand left: `<prog>: missing operand` and the same line; these on
  standard error, the output empty. `dirname` prints each operand's folder on its own line.
- **The test `if(! -e $dirname)`**: the words of `dirname` split but not matched: none: `if:
  Missing file name.`; one: matched alone (no match: `<word>: No match.`), tested for existence;
  more: the next words are read as arithmetic on the result: `/` or `%` with no number after it
  (the end, or another operator): `Division by 0.` / `Mod by 0.`; `+`, `-` (no number after them)
  leave the test's value, `*` makes it 0 (false) and the script goes on; any other word: `if:
  Expression Syntax.`. All status 1.
- **`pushd`**: its words expanded; none: `pushd: No other directory.`; two or more: `pushd: Too
  many arguments.`; `+N` with N above 0: `pushd: Directory stack not that deep.` (`+0`, `+` and
  other words are folders); a folder that cannot be entered: as §2.4.
- **`echo`**: words joined by one space; a first word `-n` drops the final new line; `\a \b \e \f
  \n \r \t \v \\` are their characters, `\0` followed by up to three octal digits is that byte,
  `\c` ends the output (no new line); any other backslash stays.
- **The folder printed** is `pwd`'s physical path read again as words: several blanks or a tab
  become one space; joined with `/` and the first word of `base`.

## 4. The defects (`larmorx_core::defects`)

The default (§2) and tcsh (§3) give the same for every name and folder without blanks, pattern
characters, braces, a leading `~`, `=N` or `-`, backslashes, a folder `+N`, and an empty name. When
they differ, a notice names the cause, in both modes:

- **FS-1173**: the name holds blanks, patterns, braces, a leading `~` or `=N`, is empty, or its
  folder is `+N` (§3's splitting, matching, expansions, `pushd`'s stack).
- **FS-1174**: a backslash in the name or the folder's path (§3's `echo`).
- **FS-1175**: a name starting with `-` (but `-` alone): `dirname`'s options (§3).
- **FS-1176**: the folder's physical path holds blanks or patterns (§3's last point, `echo: No
  match.`).

## 5. Forms not reproduced

Where tcsh's handling of a name is not reproduced, both modes give the default (§2) with FS-1173's
notice saying so: a new line or carriage return in the name; a pattern's bracket range (`[a-c]`:
**(observed)** tcsh 6.21 matches a character equal to the range's start, or not above its end in an
order that is not the characters' codes: `[a-c]` matches `0`, `A`, `Z`, `_`, `a`; `[A-C]` matches
`0`, `A`, `B`, `C` but not `a`); a subscripted assignment to a variable tcsh sets itself (`path`,
`home`, ...); an arithmetic test with a number after an operator; a `-e` operand matching several
names.

## 6. Exit statuses

0 a full name printed; 1 every message of §2 and §3 (the usage included).

## 7. Validation plan (for the clean room)

Black-box runs of the official script (`$FREESURFER_HOME/bin/getfullpath` by name, `PATH` with
`/usr/bin:/bin`, `LC_ALL=C`), each case in a folder of its own holding the same files (folders
`d1`, `a b`, `a  b`, `real`, `sp ace`, `+1`, `+0`, `+`, `=1`, `sub/mri`, `q*dir`, `nomatch[x]`, a
folder whose name holds a tab, one whose name holds `\t` as two characters, a folder without
permissions; files `d1/x.mgz`, `d1/y.mgz`, `d1/q r`, `d1/*star`, `a b/f`, `+1/f`,
`sub/mri/aseg.mgz`, `afile`, `a`, `B`, `_c`, `.h`, `q r`, `*s`; a link `lnk` to `real`), compared
line by line (exit status, both streams; the case's folder, as given and as its physical path,
written `<DIR>`): the scripts' calls (absolute names in existing folders, relative names, `..`);
every message of §2; every rule of §3 (each splitting, assignment and subscript message, each
expansion and its error, matches of none, one and several names, the stack before and after
`pushd`, `dirname`'s options, every arithmetic form of the test, `pushd`'s forms, every `echo`
escape, folders whose paths hold blanks, tabs, patterns and backslashes); seeded random names made
of those pieces, run in the case's folder or one of its folders. The default must give §2 on every
case; `reproduce` every official line but the forms of §5; seg2cc's and fs-synthmorph-reg's calls
in FreeSurfer's reference runs identical in both modes.
