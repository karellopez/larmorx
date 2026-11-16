# CTN 3.0.6's DICOM file reading (`DCM_OpenFile`, `DCM_GetElement`, `DCM_GetElementValue`)

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

## What `DCM_GetElement` and `DCM_GetElementValue` do (item 3.1-56, 2026-11-16)
FreeSurfer's DICOM reader asks for every element it uses with these two calls, by tag: first
`DCM_GetElement`, then `DCM_GetElementValue` with a buffer of the element's length (or one its own
allocation rule gives) and a context of `NULL`.
- **`DCM_GetElement(&object, tag, &element)`.** The first group of the tag's number, then its
  first element of the tag; the caller gets a copy of the element (tag, representation,
  description, multiplicity, length) with its value pointer `NULL`. Pixel data have the
  representation the reading gave them (OB or OW) and, encapsulated, the length 0xFFFFFFFF. A
  missing element pushes "Requested element (%x %x) not found in DCM_GetElementVM" (the routine's
  old name: FS-894); no object, "NULL object passed to routine DCM_GetElementVM". On a failure the
  caller's element is left as it was.
- **`DCM_GetElementValue(&object, &element, &rtnLength, &ctx)`.** Looks the element up again (the
  same search) and refuses, in this order: no or an illegal object; no group of the number
  ("Requested element (g e) not found in DCM_GetElementValue"); a group that holds no element (the
  same message with the group's number twice: FS-894; reachable with `DCM_NOGROUPLENGTH` when a
  group's only element was its group length); no element of the tag; a request whose own
  representation says SQ ("DCM Cannot retrieve value of element with SQ representation (%08x) in
  (%s)", the tag in 8 digits; the element's own representation is not looked at: FS-951); a
  context past the value's length ("DCM Illegal context value in DCM_GetElementValue"). Then it
  sets `*rtnLength` to l = min(the request's length, the value's length - the context), copies l
  bytes from the context on, moves the context past them, and returns `DCM_NORMAL` when the
  context reaches the value's length, `DCM_GETINCOMPLETE` (no condition pushed) otherwise. A
  request of 0 bytes at the end succeeds.
- **Where the bytes come from.** A value read with the file (every element but pixel data) is
  copied from memory, already in the machine's byte order. Pixel data left in the file are read
  through the object's descriptor: `lseek` to the value's offset plus the context, `read` of l
  bytes with the count as an `int`; fewer bytes than l (the file's end, or a file cut since the
  open), or -1 (a count of 2 GiB or more, which the kernel refuses with EFAULT; a descriptor a failed
  open already closed, EBADF, which a failure that keeps the object leaves), push "DCM failed to
  access file: <the object's name> in DCM_GetElementValue" with `*rtnLength` already set and the
  bytes read in the buffer, the context not moved. The check compares the count read, as unsigned,
  with l: the -1 of a failed read of l = 0xFFFFFFFF (an encapsulated value asked for whole) passes
  (FS-952). An encapsulated value's bytes are the file's from its first item tag on. In the reverse
  byte order, 16-bit pixel data are swapped in 2-byte units over the l bytes read, from the piece's
  own start (a piece at an odd context is swapped out of step with the pixels; an odd l: FS-953);
  8-bit pixel data are left.
- **The context** is a `void *` used as an offset: CTN 3.0.6 compares and adds it as a 32-bit number,
  the copy as a `long` (the same for every offset a value can have; only a caller that sets it to 4
  GiB or more by hand sees the two differ).

## Defects (FS-90 to FS-98, FS-894, FS-951 to FS-953; rules A to D, `larmorx_core::defects`)

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
| FS-894 | `DCM_GetElement`'s messages name the routine `DCM_GetElementVM`; `DCM_GetElementValue`'s "not found" for a group that holds no element shows the group's number for the element's | f (rule C) | kept (messages only; FreeSurfer prints the stack as it is) |
| FS-951 | a sequence's value asked for with a request that does not say SQ: CTN copies from its list of items' memory (the list's head, then the heap past it) | a (memory contents) | refused with the SQ message (rule B); `reproduce` the bytes not given (`ValueRead::unknown`), named |
| FS-952 | a request of 4,294,967,295 bytes of pixel data left in the file: `read` refuses the count (EFAULT) and its -1, as unsigned, equals the request: `DCM_NORMAL`, nothing written into the buffer | a (memory never written) | "failed to access file" (rule B); `reproduce` CTN's success with nothing written, named |
| FS-953 | an odd count of 16-bit pixel data in the reverse byte order: the swap runs on past the bytes two at a time until its 32-bit count wraps: a crash | a | the whole pairs swapped, the last byte kept, named |

Classes as in the register: a, a crash or memory never written (never reproduced); c, a
deterministic error with a `reproduce` switch; f, a message.

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
- The element requests (2026-11-16, scale 5, log `<workspace>/oracles/logs/fs-found-c-r35-dicom-probe-5.log`):
  the same 10,030 files and the element cases' 8 (big-endian 16- and 8-bit pixel data,
  encapsulated pixel data in both byte orders, a group of only its group length, an object kept by
  a failure after its pixel data, native and encapsulated, pixel data only inside an item), each
  opened five times (the format probe's four options, then one of six others) with requests on
  every object an open leaves: FreeSurfer's (the element, then its value whole), the value again at
  its end, pieces of 0 to 43 bytes, a context past the value, a request that says SQ, a sequence
  asked for as another representation, requests of 2 GiB - 1, 2 GiB, 4 GiB - 1 and the length + 2
  of pixel data left in the file, a piece at an odd context, a tag only the value request names,
  missing elements and groups, the file cut before its pixel data are asked for again, then the
  requests on the closed handle. Against CTN 3.0.6: 10,038 cases, 8,710 identical, 1,328
  explained (410 of them a crash of CTN, at the open or by FS-91's and FS-951's copies), 0
  different; of the explained cases whose original finished (918), every element request and value
  was checked one by one: all equal but the values the port names as memory it cannot give (FS-91,
  FS-951). Against the copy: 8,718 identical, 1,320 explained (408 crashes), 912 checked, 0
  different.
