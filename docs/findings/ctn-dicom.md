# CTN 3.0.6's DICOM file reading (`DCM_OpenFile`)

**Source-derived from CTN 3.0.6** (the DICOM Central Test Node's object library; CTN's notice,
`LICENSES/LicenseRef-CTN.txt`): closed to a clean room of CTN's reading. The port is
`crates/larmorx-io/src/dicom/` (`PROVENANCE.md` there); the evidence is the differential probe
`validation/probes/fs-dicom`. Found and measured 2026-11-13 (FreeSurfer's foundations, item
3.1-16).

## The upstream
- CTN 3.0.6 as Debian packaged it (`ctn_3.0.6.orig.tar.gz`, SHA-256 `2e21b5db...30861`), pinned in
  `<workspace>/tags/ctn-3.0.6`. Of the two releases found (3.0.6, 3.2.0), 3.0.6 is the closer to
  the copy the probe also measures: 99.98 % of `dcmdict.c`'s tokens, 95.8 % of `dcm.c`'s (the
  rest is layout, casts and two behaviours, below).
- Built as on Linux x86-64: `LITTLE_ENDIAN_ARCHITECTURE`, `LONGSIZE` 64 (`U32` an `unsigned
  int`), no thread layer (CTN's tree carries its Windows NT platform header, which turns threads
  on; the probe supplies a Linux one).

## What `DCM_OpenFile` does with a file (the behaviour the port keeps)
- **Options.** A mask with neither a byte order nor `DCM_PART10FILE` is refused before the file is
  opened ("DCM illegal stream option: Byte order"); `DCM_FORMATCONVERSION` alone is such a mask,
  so an open with only it always fails. `DCM_PART10FILE` reads a 128-byte preamble and a 4-byte
  label, then explicit little endian; a label other than `DICM` fails the open with no condition
  of its own (only "failed to open file"). An empty file returns `DCM_FILEACCESSERROR` with no
  condition pushed; the file's size is `fstat`'s as an `int`, then 32-bit.
- **Elements.** Tag, then (explicit VR) a two-byte code compared as a C string with CTN's codes
  (a zero first byte names none; the delimiter's code is the single byte 0x1E, as CTN's source
  writes it), a 2-byte length for most VRs or 2 reserved bytes and a 4-byte length for OB, OW, UN,
  UT and SQ; implicit VR: a 4-byte length. Delimiter tags (group FFFE) always take a 4-byte
  length. A code whose representation differs from the dictionary's is believed when it is OB or
  OW, when the dictionary says UN, context or retired, or with `DCM_ACCEPTVRMISMATCH`; otherwise
  the open fails ("Incorrect VR"), except for pixel data. An odd length, or one past the remaining
  bytes, closes the object and fails.
- **Dictionary.** CTN's 1,478 entries in 36 groups; an unknown group's element 0 is "Unknown group
  length", UL; other unknown elements are UN (the lookup's condition is pushed and popped again).
  UN of undefined length is read as a sequence.
- **Order.** Groups must not decrease and elements within a group must increase ("group/element
  out of order"): the object is then kept (not closed) and the open fails.
- **Sequences.** Items are read as nested objects (one recursion per level), of defined length or
  ending at their delimiter; a sequence of undefined length ends at any tag other than an item. An
  item delimiter or a sequence delimiter met where an element is expected ends that data set with
  success, at any level, the top one included.
- **Pixel data.** Never read: the file is left open and the value's offset kept; its
  representation becomes OB when Bits Allocated is 8, OW otherwise. Encapsulated pixel data
  (undefined length) is scanned item by item, the fragments skipped by seeking; the scan's own
  failures are ignored, its count of bytes kept.
- **Values.** Read whole; in the reverse byte order (big endian) US, SS, OW and AT are swapped in
  2-byte units (pixel data only when Bits Allocated is 16), UL and SL reversed in 4-byte units,
  FD and FL not at all (FS-96); AT values have their halves exchanged on a little-endian machine.
  The value multiplicity is counted from backslashes for string VRs, from the length otherwise.
- **The object.** Its size counts every byte read (padded values); the meta group's end (preamble
  + 4 + 12 + the group length's value) switches to the transfer syntax's byte order and VR
  (implicit little, explicit little, explicit big; any other UID, an encapsulated syntax,
  explicit little); every group's length element is rewritten with the length computed from its
  elements. Bits Allocated, Pixel Representation, the meta group's length, the transfer syntax and
  the Waveform Data Representation set the object's fields as they are read.
- **The arithmetic.** Lengths and remaining sizes are 32-bit and wrap: a sequence of undefined
  length inside an item of defined length can consume more than the item holds, and the reading
  then runs on past the item.
- **Conditions.** `COND_PushCondition` formats into a 1,024-byte buffer and keeps 256 bytes; the
  stack holds 100 entries and, when a push reaches 98, prints itself to standard error with
  "CONDITION Stack overflow" and restarts at entry 0 (which then shows the oldest condition, not
  the newest). The messages' `%ld` conversions receive 32-bit lengths (printed as their unsigned
  values on x86-64).
- **Process state.** `perror("")` prints the message alone. errno is what earlier calls left: a
  read whose count is negative as an `int` fails with `EINVAL` (an element of undefined length),
  closing an object whose descriptor `DCM_OpenFile` already closed gives `EBADF`, and glibc's first
  `perror` on a standard error that nothing has written to yet reopens it with
  `fdopen(dup(2), "w+")`, which fails with `EINVAL` when standard error is write-only (a file or a
  pipe): later messages print "Invalid argument". The port models these; a program's own history
  of errno is outside it (the probe's children start from 0).

## Defects (FS-90 to FS-98; rules A to D, `larmorx_core::defects`)

| ID | What | Decided | larmorx |
|---|---|---|---|
| FS-90 | a message of 1,024 bytes or more overflows the formatting buffer; a file name of 1,024 bytes or more overflows the object's copy | a | the message kept to 256 bytes, the name to 1,023, named |
| FS-91 | an element of undefined length that is neither a sequence nor pixel data: the read of its value with a count of -1 fails and is taken for complete; the value is never written, the remaining length grows by one and the value's bytes are read as the next elements (an item delimiter there ends the data set) | c (rule A) | read as a sequence (delimiter tags excepted); `reproduce` CTN's reading with the value marked unread, named |
| FS-92 | Bits Allocated, Pixel Representation, the meta group's length, the transfer syntax and the Waveform Data Representation read past a shorter value (or a sequence, or a UID without its zero byte) | a | the bytes past the value read as zero, named |
| FS-93 | a group length shorter than 4 bytes is given the 4-byte length; a Waveform Data Representation longer than 16 bytes is copied into a 17-byte field | a | the value's own bytes only; 16 bytes kept; named |
| FS-94 | (a program's copy) UL and SL of big-endian data swapped in 2-byte pairs | c (rule A) | reversed; `reproduce` the pairs (the `Dialect`'s switch), named |
| FS-95 | a UL or SL value of big-endian data whose length is not a multiple of 4 is swapped past its end until the count wraps: a crash | a | whole 4-byte units reversed, the last 2 bytes swapped as a pair, named |
| FS-96 | FD and FL of big-endian data left in the file's byte order | c (rule A) | reversed in 8- and 4-byte units; `reproduce` left, named |
| FS-97 | one recursion per nesting level: deep enough nesting exhausts the stack | a | an error past 128 levels ("DCM Failed to read stream in readSequence"), named; CTN reads 2,000 levels within an 8 MiB stack |
| FS-98 | a fragment of encapsulated pixel data with an odd length (or one past the count) closes the object, and the reader then writes through it: a crash | a | the open fails with the fragment's condition, named |

Classes as in the register: a, a crash or memory never written (never reproduced); c, a
deterministic error with a `reproduce` switch.

## The numbers (the probe, 2026-11-13)
- Scale 5 (log `<workspace>/oracles/logs/fs-found-c-r7-dicom-probe-5.log`): 10,030 files (part 10
  in seven transfer syntaxes, bare data sets in implicit and explicit VR and both byte orders,
  sequences nested up to 3 levels of defined and undefined length, native and encapsulated pixel
  data, group lengths right and wrong, the special elements, mutations, non-DICOM files, the fixed
  edge cases of every defect, a missing file, a directory, a 1,156-byte name), each opened 11
  times by a script (the four attempts of a format probe with a close and a stack snapshot after
  each, then the other byte orders and options, the stack's top, pop and dump; 3 % of the files
  with 45 failing opens that overflow the stack): against CTN 3.0.6, 9,429 identical, 601
  explained by FS-90 to FS-98 (360 of them a crash of CTN), 0 different; the same against the
  second copy (361 crashes).
