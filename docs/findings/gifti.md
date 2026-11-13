# gifticlib 1.12 and expat 2.0.1: GIFTI files and the XML under them

**Source-derived from gifticlib 1.12 (public domain) and expat 2.0.1 (MIT)**: closed to the
implementer of a future clean room of these routines (porting guide §1). Found while porting
items 3.1-09 to 3.1-12 of larmorx's FreeSurfer foundations into `larmorx-io` (`gifti`, `xml`) and
`larmorx-core` (`cscan::strtof`, `sscanf_f32`), 2026-11-13. The check:
`validation/probes/fs-gifti`. FreeSurfer's copies (how they differ, measured as a black box):
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers; this file
describes the upstreams only).

## The upstreams

- gifticlib 1.12: github.com/NIFTI-Imaging/gifti_clib at commit
  8ec2a95bce6360c3f7f655b36767e4d04c61d77c (2012-06-15, "gifti library version 1.12, 15 June,
  2012"; the only commit on the library's files between 1.12 and 1.13), pinned in
  `<workspace>/tags/gifticlib-1.12` (GitHub's archive of the commit, SHA-256
  965406c74a48a0bf1a58d6d556189da120de5e5fbb6209b145805325be9c0416).
- expat 2.0.1: libexpat's tag `R_2_0_1`, pinned in `<workspace>/tags/expat-2.0.1` (SHA-256
  6a26c3fe91d2a05d05fac187c0a092e1602c335ee12b008d9fe4da30788402b9), as gifticlib uses it:
  `XML_ParserCreate(NULL)` (no namespace processing, no protocol encoding), twelve handlers (start
  and end tags, character data, processing instructions, comments, CDATA sections, the default
  handler, the XML declaration, the document type, element declarations), no external entities,
  no parameter-entity parsing; built with `XML_DTD` and `XML_CONTEXT_BYTES` 1024.

## Why the parser is a translation

The first parser (written from expat's behaviour, 2026-11-13) disagreed with expat on 49 % of the
probe's generated documents (31 % of the UTF-8 ones without an internal subset): where a character
datum is split, which markup reaches the default handler, the positions of errors in the XML
declaration, a byte order mark cut by a piece, internal subsets, encodings. Matching expat's events
piece by piece needs its tokenizers and processors as they are, so `larmorx_io::xml` is now a
translation of expat 2.0.1's paths for this configuration (MIT; `crates/larmorx-io/PROVENANCE.md`).
Behaviour a caller sees, kept from expat:

- Character data are cut where expat's tokenizer stops: at a markup character, at each line break
  (reported alone as `"\n"`, CR LF and CR included), at `]`, at an invalid or cut multibyte
  character, and at the end of the piece fed (a CR or a `]` at the end of a piece waits for the
  next). Documents in ISO-8859-1 or UTF-16 reach the handlers in UTF-8, in pieces of at most 1024
  bytes (expat's `dataBuf`); the default handler's text too.
- With a default handler set, internal entities are not expanded in content (their references,
  `&e;`, go to the default handler); they are in attribute values. A reference to an undeclared
  entity is an error unless the document has a `DOCTYPE` with an external identifier or a
  parameter-entity reference (then it is skipped, in content to the default handler).
- An internal subset's declarations reach the default handler token by token, except element
  declarations (the element declaration handler, with expat's content model) and the parts with
  a handler; attribute defaults declared there are added to the start tags (after the specified
  attributes, in declaration order; a default after an `#IMPLIED` declaration of the same
  attribute is ignored); a parameter-entity reference stops the processing of later declarations
  unless the document is standalone.
- The XML declaration is reported before its encoding is checked; an unknown encoding name (only
  ISO-8859-1, US-ASCII, UTF-8, UTF-16, UTF-16BE and UTF-16LE are known, case-insensitively) is
  error 18 at the name; UTF-16 declared in a one-byte document is error 19.
- `XML_GetCurrentLineNumber` counts lines (CR, LF, CR LF) up to the current event in the document's
  encoding (UTF-8 before it is known).

## gifticlib's behaviour worth knowing

- The reader feeds expat whole buffers of `buf_size` bytes (32768 by default, changed by each
  `DataArray`'s expected size) and decides the last call by `blen < sizeof(buf)`, the size of a
  pointer (8): a final call with the last 0 to 7 bytes (usually 0). Harmless.
- `gxml_data`'s fields (the counts of `DataArray`s seen and expected, the base64 errors, the stack
  depth, the data offsets, whether a text target or an image is set) persist from one read or write
  to the next; the verbose display at verbosity 4 and above shows the previous call's values.
  `larmorx_io::gifti::Gifti` keeps them.
- `gifti_free_image` prints what it frees at verbosity 3 and 4; the reader calls it after a parse
  error and after a failed `DataArray` list.
- A NaN label colour passes the range check (`rgba < 0.0 || rgba > 1.0`).
- The writer's size message is written in two parts with the size's own messages between them.
- The `%.*s` of the buffer messages stops at a 0 byte.
- GZipBase64Binary: `uncompress` with the array's size as the output limit; a short or corrupt
  stream leaves the bytes it produced (the rest of the array zero). The compressed bytes the writer
  produces are zlib 1.2.11's; larmorx's are zlib-rs's (pure Rust: the decompressed values are equal,
  the stream equal in about 73 % of the probe's files), compared by content as larmorx compares
  `.h5` files (decision log 2026-10-16); the "compress buffer (N% ...)" message at verbosity 3 and
  above shows the stream's size.

## The numbers

`validation/probes/fs-gifti` at scale 5 (2026-11-13): 40,040 parser comparisons with expat 2.0.1
and FreeSurfer's copy on generated documents in pieces, 0 differ; 68,030 GIFTI comparisons with
gifticlib 1.12 and FreeSurfer's copy (values, checks, displays, messages at every verbosity, files
written back), 0 differ; 100,000 `sscanf("%f")` strings, 0 differ from glibc. Where an original
reads memory it never wrote or crashes, the defined parts are compared (the probe's README).

## Defects (D13; the register's FS-80 to FS-89)

| ID | Where | What | larmorx |
|---|---|---|---|
| FS-80 | label tables | colours given for some labels only leave holes (or a short array); a `Label` without a valid `Key` gets no key: the writer, the check and the display read memory never written | 0, with a notice (both modes) |
| FS-81 | Base64 decoding | after skipped bad characters, the bytes carried to the next piece come from the wrong place | the real remainder; `reproduce` the wrong bytes (never-written ones as 0) |
| FS-82 | type table | `NIFTI_TYPE_FLOAT128` is 6 bytes (a `long double` takes 16) | 16; `reproduce` 6 |
| FS-83 | ASCII decoding | the counters of a value's pieces (complex, RGB) persist from one array to the next | reset per `Data`; `reproduce` kept |
| FS-84 | metadata | an `MD` without a `Name` text is compared through a null pointer (a crash) | no match, with a notice (both modes) |
| FS-85 | element stack | an element outside its parent (`Data`, `MatrixData`, a coordinate system outside a `DataArray`, `Name`/`Value` outside `MD`), a dimension past `Dim7` when writing, the end of a `Data` of an array a list leaves out before any array is stored: a null or out-of-range pointer (a crash or memory never written) | skipped, with a notice (both modes) |
| FS-86 | text elements | character data (white space included) after a CDATA section or a child element inside `Name`, `Value`, `Label`, `DataSpace` or `TransformedSpace`, or inside one of them outside its parent, is written through a null pointer (a crash): `<Name> <![CDATA[x]]> </Name>` crashes gifticlib | appended to the element's text (or dropped without a place), with a notice (both modes) |
| FS-87 | messages | at verbosity 6 and 7 the `MatrixData` reader's buffer messages show the `Data` element's buffer and length, partly memory never written | the matrix buffer's; `reproduce` the `Data` buffer's (never-written bytes from zeros), with a notice |
| FS-88 | expat | a UTF-16 document of an odd length whose last token is cut by its odd byte is converted past its end (a crash or a hang) | the odd byte dropped, with a notice (both modes) |
| FS-89 | expat | an error reported inside a multibyte character (a non-ASCII character in an XML declaration's pseudo-attribute: `toAscii` stops in the middle of it) makes `XML_GetCurrentLineNumber`'s count step over it and read past the document (a crash) | the lines before it, with a notice (both modes) |

Kept (rule C, behaviour only): `gifti_intent_is_valid` accepts every code; the extra empty final
`XML_Parse`; the `%p` values in some messages (larmorx prints placeholders); expat 2.0.1's
`utf8_toUtf8` keeping the lead byte of a character cut by its output limit (it shows only in
`toAscii`, so in where an XML declaration error is reported, which larmorx reproduces).
