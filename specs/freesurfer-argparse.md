# FreeSurfer 8.2.0's command-line parser of its newer programs: behaviour spec (`fs.core`, argparse)

Written by the replica author of FreeSurfer's `ArgumentParser` (track A, relay 18; queue item 3.1-21,
FreeSurfer's own code derived from Hilton Bristow's argparse) as its spec writer
(`docs/porting-guide.md` §1): behaviour, conventions, messages and the validation plan, never code or
routines. A part of `fs.core`, kept in a file of its own (as `freesurfer-fnv.md`). The programs that
use it: `AntsDenoiseImageFs`, `AntsN4BiasFieldCorrectionFs`, `fs_temp_dir`, `fs_temp_file`,
`mris_defects_pointset`, `mris_remesh`, `mri_brainvol_stats`.

Messages are FreeSurfer's fatal errors (`specs/freesurfer-core.md`: `error: <message>` and a newline on
standard error, the message also in the error log, then the program ends with the status given; with
exceptions on, FS-263's ending). Status 1 unless said.

## 1. Arguments

An argument has an optional short name (`-x`), a name (`--name` for a flag, a word without a dash for
a positional argument), a count of inputs `nargs` (a signed byte: a number, or `'+'` (one or more) or
`'*'` (any)), a type (unknown, bool, string, int, float) and whether it is required. Its *canonical
name* is its name, or its short name when it has none.

- The parser starts with two flags: `--all-info` and `--version` (no inputs, bool).
- `addArgument(name, nargs = 0, type = unknown, required = false)`: a name not starting with `-`
  (the empty name included) makes a positional argument; a name of more than 2 bytes a flag with that
  name; one of 2 bytes or fewer a short name only. `addArgument(short, name, ...)`: both names.
  `addHelp(text)`: keeps the help text and adds `-h`/`--help` (no inputs, bool).
- Names are checked (status 1): empty → `invalid argument configuration. Argument names must not be
  empty`; 2 bytes not starting with `-`, or exactly 3 bytes → `invalid argument configuration for
  '<n>'. Short names must begin with '-'`; more than 3 not starting with `--` → `... for '<n>'.
  Multi-character names must begin with '--'`. (A positional name given alone is not checked.)
- An argument whose name (not its short name) does not start with `-` is positional (so an argument
  with only a short name is positional too): required always; `nargs` 0 becomes 1 and `'*'` becomes
  `'+'`.
- `'+'` and `'*'`: a variable count, at least 1 and at least 0 inputs. Any other `nargs`: fixed, that
  many (a negative byte is taken as an unsigned count, huge).
- An unknown type becomes bool for a minimum of 0 inputs, string otherwise. Then: a minimum of 0 and
  not bool → `invalid argument configuration for '<c>'. Option flags that accept no input must be of
  type ArgType::Bool`; a minimum of 0 and required → `... for '<c>'. Required flags must accept at
  least one input`.
- Each argument has a value: for a fixed count of 0 or 1, one int (0), float (0), bool (false) or
  string (empty) by its type; otherwise a list of them (empty). It must not share a name with the
  arguments before it, compared without their leading dashes (one dash removed, and a second when the
  name is longer than 3 bytes; an empty short name compares too): `invalid argument configuration.
  Short name '<s>' of '<c>' is used twice`, where `<s>` is the earlier name (without dashes) the
  parser finds first in its index (an unordered map: which of several matching names comes first
  depends on the C++ library's hashing; larmorx takes the earliest added). A second positional
  argument with a variable count → `invalid argument configuration for '<c>'. Two positional arguments
  cannot both have a variable amount of inputs, as this could lead to an undefined boundary between
  the two`; a positional argument and a flag both with variable counts → `... for '<c>'. A positional
  argument and flagged argument cannot both have a variable amount of inputs, as this could lead to an
  undefined boundary between the two`. Its names (those not empty) then lead to it.

## 2. Parsing

`parse(argv)`: `argv[0]` is the program's name; its *base name* is what follows its last `/` (empty
when it ends with `/`).

1. For each later word that is a known name: when the words after it that are not known names are
   fewer than the argument's minimum count → `not enough inputs supplied to '<word>'` (status 2).
2. Then the words in order, with a current argument (at first none) and a count of the positional
   arguments taken:
   - A known name: the current argument's inputs are checked (§2.3), and the named argument becomes
     current with no inputs counted (each time it is named again; a list keeps growing); one that takes
     no inputs becomes true and given at once.
   - Another word is an input. With no current argument and positional arguments configured, the next
     positional argument becomes current. When the current argument has a fixed count and has all its
     inputs (also with none current), the next positional argument becomes current; when there is none
     → `unexpected argument '<word>'` (status 2). When it has a variable count and its minimum, and
     the positional arguments after it need inputs (the sum of their minimums), the words after this
     one are counted (a word starting with `-` takes away its argument's minimum; an unknown such word
     is added to the index as leading to the first argument, `--all-info`, so it is a known name from
     then on: a negative number given later sets `--all-info`), and when fewer remain than needed, the
     next positional argument becomes current. The input is then stored into the current argument's
     value: for one value, an int read as `std::stoi`, a float as `std::stof`, a bool as `TRUE`, `YES`,
     `1` / `FALSE`, `NO`, `0` (any case), a string as it is; for a list, appended. A word that cannot be
     read so, or a value that is not of the argument's type (an argument with an empty canonical name
     takes the index's entry for the empty name, the first argument's) → `input '<word>' cannot be
     converted to expected type (<type>)` (status 2; type names `int`, `float`, `bool`, `std::string`).
     The argument is now given; its count of inputs grows by one.
3. Checking an argument's inputs: a positional one with fewer than its minimum → `not enough
   positional arguments supplied`; a fixed count not met exactly → `not enough inputs passed to option
   '<c>' (expected <n>)`; a `'+'` with none → `option '<c>' requires at least one input` (all status 2).
   The last current argument is checked after the words.
4. `--version` given: `<base name> freesurfer <version>` and a newline on standard output; `--all-info`
   given: the all-info line (`specs/freesurfer-core.md`, of all of `argv`) and a newline. When either
   was given and the words other than the program's name number exactly the flags given (one or two),
   the program ends (status 0).
5. A help text of 1 byte or more and `--help` given: the help rendered (§5), the program ends (status
   0).
6. Each required argument in the order configured: when not given, the help is rendered (§5; the
   text, or none) and `missing required input '<c>'` (status 2). FS-411: a program that never set a
   help text has none only by chance (memory never written); FreeSurfer's official
   `mri_brainvol_stats` prints `No xml string passed.` there; both modes print it (noted).

## 3. Reading numbers

- `std::stoi(word)`: leading white space (space, tab, newline, vertical tab, form feed, carriage
  return), an optional sign, decimal digits (at least one), the rest ignored; a value outside the
  64-bit range or the 32-bit `int` range fails.
- `std::stof(word)`: leading white space, an optional sign, then the longest of: `infinity`, `inf`,
  `nan` with an optional `(` letters, digits and `_` `)`, a hexadecimal number (`0x`, hexadecimal
  digits with an optional point, at least one digit, an optional `p` exponent with digits), a decimal
  number (digits with an optional point, at least one digit, an optional `e` exponent with digits);
  `0x` without a hexadecimal digit reads the `0`. No number fails; the rest is ignored. The value is
  the correctly rounded float; an overflow, or an inexact result below the smallest normal float
  (after rounding with an unbounded exponent), fails (`ERANGE`). `nan(seq)`: when `seq` read as an
  unsigned 64-bit number in base 0 (`0x` hexadecimal, a leading `0` octal, else decimal; the whole
  sequence, else no payload; overflow gives the largest) has low 22 bits not all 0, those are the NaN's
  payload (the quiet bit set, the sign as given).

## 4. Queries

- `exists(name)`: `name` with or without its dashes: the first index name that, without its dashes,
  equals `name` (else `name` itself); not in the index → `'<name>' is not a known argument`. Whether the
  argument was given.
- `retrieve<T>(name)`: as `exists` for the name; then its value when `T` is the value's type (one
  `int`, `float`, `bool`, `std::string`, or a `std::vector` of one), else `invalid cast of argument
  '<name>'. This input is of type '<type>' and should be retrieved via retrieve<<type>>("<name>"). To
  change the expected type, modify the call to addArgument()` (for a list: `These inputs are of type
  '<type>' and should be retrieved via retrieve<std::vector<<type>>>("<name>")`), `<name>` as given.

## 5. The help

The help text is FreeSurfer's XML help, rendered by `outputHelpXml` (§7): no text → `No xml string
passed.` and a newline on standard error; an empty text → `Document not parsed successfully. ` and a
newline; otherwise the XML rendered.

## 6. Validation plan

The fs.io probe's piece 9 (`validation/probes/fs-io/`, command `apa`): a configuration (one of the
seven programs' own, or 1 to 5 random arguments: names from a pool with and without dashes, empty, 1 to
3 bytes and long; counts 0 to 3, `'+'`, `'*'`, -1; every type; required or not; a help text empty or
XML), a command line (made for the configuration half of the time: each argument with inputs of its
type, words of every kind for `stoi`, `stof` and the booleans, stray words, `--version`; otherwise
random names, unknown flags and words), then `exists` and `retrieve` queries of the right and wrong
types and unknown names. Lines whose answer depends on the index's order (two matching names) or on the
time (`--all-info`) are compared apart. Measured (2026-11-13): seed 13, 2,352 cases: `reproduce`
identical on 2,265, 0 different (61 counted, 26 apart); since the renderer (2026-11-14), seed 13, 297
cases: identical on 294, 0 different (3 apart).

## 7. The XML help renderer (`outputHelpXml`, `outputHelp`, `outputHelpDoc`; `utils/fsPrintHelp.cpp`)

The text is read as an XML document as libxml2 2.7.7 reads it with its defaults (entities not
substituted, blank text kept, no DTD loaded): the XML declaration with encoding UTF-8 (default),
ISO-8859-1 (each byte of 128 or more becomes its two-byte UTF-8 form) or ASCII; a byte-order mark; the
document type and its internal subset (entity declarations; element, attribute-list and notation
declarations change nothing); `\r\n` and `\r` read as `\n`; the predefined entities `lt`, `gt`,
`amp`, `apos`, `quot` always give their characters, whatever the internal subset declares (the help
files declare `lt` as `#38;#60;`); character references give their character in UTF-8. A text that is
not well formed: libxml2 prints its messages (`Entity: line <n>: parser error : <message>`, the line,
a caret) and then `Document not parsed successfully. ` (larmorx: one line `larmorx-freesurfer: the help
text is not well-formed XML (<what>); libxml2's messages are not reproduced`, then the same last line);
status -1. The programs' own help texts are well formed.

The document's root element must be `help` (any case), else `Document id of the wrong type!  The root
node is not help.` (standard error, -1). Standard output: `\t\t\t\tHelp`; then, for each child of the
root: text (blank or not) and elements named `text` (any case) are skipped; an `arguments` element
prints, for each of its children but text: two newlines and its name in capitals with its first `_`
and its first `-` made spaces, then ` ARGUMENTS`, then each of that child's children but text: before
each `intro` or `argument` but the first a newline, then the child's contents; an `outputs` element
prints its name the same way and then its children as the kinds of arguments do (`intro` and `output`
take the newline); `example` elements print their name and ` <n>` (numbered from 1 in the document);
every other element its name and its contents. At the end three newlines; status 0.

An element's contents: the concatenated text of its direct text and CDATA children (references
replaced; nested elements, comments and processing instructions contribute nothing). Printed in
lines: each starts with a newline and a tab (two tabs in an `explanation`, any case); one space at the
line's start is skipped; a line holds at most 70 characters (62 in an explanation): characters are
copied one by one while the word starting at the current character (its length the characters up to
the next space, `_` or `/`, or the end; a newline does not end a word) fits in what is left of the
line, a newline in the text ends the line (and is not printed); a word longer than the whole line is
cut at the line's end. FS-435: FreeSurfer then drops the character after the cut (by default larmorx
starts the next line with it; `reproduce` drops it; FreeSurfer 8.2.0's `mri_ca_normalize`,
`mri_ca_register` and `mri_cal_renormalize_gca` help texts have such words). FS-434: an element with no
text or CDATA child, and a comment, processing instruction or CDATA section among the root's children
or among an argument kind's or `outputs`' children, crash FreeSurfer; larmorx prints nothing for them
(both modes). FS-436: a comment or processing instruction among the `arguments` element's children is
printed by FreeSurfer as a kind `<TARGET> ARGUMENTS` (a comment: `COMMENT ARGUMENTS`) with nothing
under it; by default larmorx skips it, `reproduce` prints it.

`outputHelp(name)`: no name → `No filename passed.`, -1; a name that does not open as a file is looked
up as `$FREESURFER_HOME/docs/xml/<name>.help.xml` (no `FREESURFER_HOME`: -1, nothing printed); the file
read as above.

Validation: the fs.io probe's piece 12 (`hlp`): FreeSurfer 8.2.0's 86 `.help.xml` files and generated
help texts (sections in any case, kinds of arguments, intros, explanations, outputs, examples, both
encodings, the help files' document type, references, CDATA, comments and elements inside text, tabs,
line ends of every form, words around the line widths, the separators `_` and `/`, non-ASCII letters,
stray comments and processing instructions, empty elements, cut texts). Measured (2026-11-14): the 86
files identical in `reproduce` (83 in `fix`: FS-435); seed 13, 594 generated texts: identical on 465, 0
different (129 counted: FreeSurfer crashes, FS-434, or not well formed).
