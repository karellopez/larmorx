# nlohmann/json 3.2.0: what it reads and writes

**Source-derived from nlohmann/json 3.2.0** (MIT): closed to a clean room of nlohmann's reading
and writing. The port is `crates/larmorx-core/src/json/` (`PROVENANCE.md`); the evidence is the
differential probe `validation/probes/fs-json`. Found and measured 2026-11-13 (FreeSurfer's
foundations, items 3.1-19 and 3.1-20).

## Reading
- **Positions.** A parse error's position is the number of characters the lexer has read
  (`get` minus `unget`), counted from 1; the end of the input counts as a character each time
  it is read.
- **The last token read** (in a lexer error's message) is everything read since the last string
  or number began, white space and earlier punctuation included (`[1, tru]` gives "last read:
  '1, tru]'"); control characters as `<U+00XX>`, other bytes as read.
- **The end.** A zero byte ends the input wherever a value may start or follow. `json::parse`
  wants only white space after the value ("unexpected ...; expected end of input");
  `operator>>` leaves what follows in the stream, having taken one character past a number.
- **A byte-order mark** (EF BB BF) is skipped at the start; EF not followed by BB BF is "invalid
  BOM".
- **Numbers.** A non-negative integer is `uint64_t` when it fits, a negative one `int64_t`
  (`-0` is the integer 0); otherwise, and for fractions and exponents, `strtod`'s double (an
  infinity is `out_of_range.406`, "number overflow parsing '...'"); leading zeros, a bare `-`,
  `1.`, `1e` are errors with their own messages.
- **Strings.** Escapes `\" \\ \/ \b \f \n \r \t \uXXXX` (surrogate pairs combined; a lone high
  surrogate "must be followed by U+DC00..U+DFFF", sic, a lone low one "must follow
  U+D800..U+DBFF"); unescaped controls refused; UTF-8 checked byte by byte against RFC 3629's
  ranges (overlong forms, surrogates and code points past U+10FFFF refused).
- **Objects** keep their keys in `std::map` order (bytewise); a key given twice keeps its last
  value.

## Writing
- `dump(indent)` with a negative indent is compact (`{"a":1,"b":[1,2]}`), otherwise pretty with
  `indent` characters per level and `": "` after keys (an indent of 0 still breaks lines); empty
  objects and arrays are `{}` and `[]`. `operator<<` is pretty when the stream's width is
  positive (the width the indent, the fill its character) and resets the width.
- **Strings**: `"`, `\` and controls escaped (`\b \t \n \f \r`, others `\u00XX` in lower-case
  hex), the rest copied; with `ensure_ascii`, code points from 0x7F as `\uXXXX` (surrogate pairs
  past U+FFFF). Invalid UTF-8 is `type_error.316` ("invalid UTF-8 byte at index N: 0xXX",
  "incomplete UTF-8 string; last byte: 0xXX"); escaped bytes pass through a 512-byte buffer
  written out when fewer than 13 bytes are left after a whole character, so a stream keeps what
  the buffer had written before the error.
- **Numbers**: integers in decimal; non-finite doubles as `null`; finite ones by Grisu2 (not
  always the shortest digits) in decimal notation for decimal exponents from -4 to 15
  (`100000000000000.0`, `0.0001`), otherwise `d.ddde+XX` with at least two exponent digits
  (`1e+15`, `1e-05`, `5e-324`); `-0.0` for negative zero.

## Accessors (as programs use them)
- The non-const `operator[]` with a key makes a null value an object and adds a missing key as
  null; with an index it makes null an array and grows an array with nulls; on any other value it
  is `type_error.305` ("cannot use operator[] with <type>"; a copy may word it otherwise: the
  `Dialect`).
- `get<double>` takes numbers only; `get<float>` and `get<int>` also booleans ("type must be
  number, but is <type>" otherwise); conversions are C++ casts (x86-64's indefinite integer for
  a double out of range); `get<std::string>` takes strings only. `size()` is 0 for null, the count
  for arrays and objects, 1 otherwise; a range-for visits an array's elements, an object's values
  in key order, nothing for null, a primitive once.

## Defect (FS-99; rules A to D, `larmorx_core::defects`)

| ID | What | Decided | larmorx |
|---|---|---|---|
| FS-99 | the indent string (512 characters) is doubled, with spaces, each time an indent passes it (past 512 characters a tab indent becomes spaces; an indent step larger than the doubled string reads past its memory); the writer and the destructor recurse once per level (a value nested deeply enough exhausts the stack) | c (rule A) for the padding, a for the rest | the indent character kept (`reproduce`: spaces); no read past, no recursion; named |

## The numbers (the probe, 2026-11-13)
See the probe's README and the log `<workspace>/oracles/logs/fs-found-c-r7-json-probe-5.log`.
