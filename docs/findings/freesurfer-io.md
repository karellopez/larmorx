# FreeSurfer 8.2.0's file helpers and volume formats (`fs.io`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines; the behaviour is in `specs/freesurfer-io.md`). Written by the replica author of queue item
3.1-25 (FreeSurfer's foundations, track A, relay 10). Defect IDs FS-167 to FS-176, decided by rules
A to D (CLAUDE.md, D13); the register's entries are in the orchestrator's handoff.

## Part 1: fio.cpp, bfileio.cpp, MRIio_old.cpp (3.1-25, piece 1)

### FreeSurfer links its own, older znzlib

`packages/nifti/znzlib.c` (FreeSurfer's copy, the one `libnifti.a` and every program link) reads a
compressed file with `gzread(file->zfptr, buf, ((int)size) * ((int)nmemb)) / size` and writes with
`gzwrite(..., size * nmemb) / size`: one zlib call, the count of complete members, no
`ZNZ_MAX_BLOCK_SIZE` loop, no `** znzread: read short by N bytes` warning and no
`nmemb - remain/size` count (nifticlib 2.0.0's, which `larmorx_io::znz::ZnzFile::read`/`write`
translate, with FS-76 and FS-77). Measured: a 5-byte `.gz` read as two `znzreadInt`s gives the count
0 and no warning for the second (the probe's `zr zi ... 0000000100`). So `fio::fs_znzread` and
`fs_znzwrite` (FreeSurfer's form) call `GzFile::gzread`/`gzwrite` and `StdioFile::fread`/`fwrite`
through `ZnzFile::gz()`/`stdio()`. The `(int)` product overflows past 2^31 bytes (a negative length,
zlib's `gzread` refuses it with -1); larmorx returns -1 there too. A member size of 0 divides by zero
(SIGFPE) in FreeSurfer: FS-76's case, 0 here. **For track C / the orchestrator:** `larmorx_io::znz`
needs a FreeSurfer dialect (or these two functions moved there) for the volume readers of 3.1-27 and
later (MGZ through `znzread` of whole slices).

### FS-167 and FS-49: the one-number readers at the end of a file (memory contents)

`fread1` (`fio.cpp` 73: `unsigned char c;`), `fread2` (83: `short s;`), `freadLong` (194),
`freadFloat` (287: `char buf[4];`), `znzread1` (334), `znzreadDouble` (439), `znzreadInt` (455),
`znzreadLong` (469), `znzreadShort` (484), `znzreadFloat` (570) read into an uninitialised local;
`fread` and `gzread` copy the bytes they get, so a short read leaves the rest as the stack held it.
`freadInt` (182) and `freadDouble` (169) are FS-49 (registered with `MatrixReadFrom`); `freadInt`'s
test `(err = fread(...)) < 0` can never be true (`fread` returns a `size_t`, 0 or 1), so it never
prints. `fread3` (96) clears its `unsigned int i = 0` first: defined. The `Ex` forms read into the
caller's variable and swap it whatever the count (rule C, kept). Both modes fill the missing bytes
with 0 and note it; the probe compares those bytes as wildcards (`?`). Seen in the official build: the
stack held 0 for `fread1`'s byte, 0x00 for `fread2`'s, and earlier strings' bytes for the doubles
(`a0122532435f0000`): memory contents indeed.

### FS-176: swapLong64 returns through a double

`machine.cpp` 37-53: `double w; ... w = *((long64 *)(pVar)); return w;`: the reversed 64-bit integer
is converted to `double` (cvtsi2sd, round to nearest even) and back (cvttsd2si: 2^63 gives
0x8000000000000000). `fio.cpp` calls it in `freadLong` (on the number read: the result rounded),
`fwriteLong` (on the number to write: the stored bytes are the rounded byte-reversed number),
`znzreadLong` and `znzwriteLong`. Measured: `fwriteLong(-8417788160845815563)` writes
`80 2e 02 44 5e 4b e0 f5` where the number is `8b 2e 02 44 5e 4b e0 f5`; `freadLong` of
`21a0c5007f737ffe` returns `21a0c5007f738000`. Callers: `fstagsio.cpp` and `tags.cpp` (tag data
lengths, `znzwriteLong(dlen)`, `znzreadLong`): small non-negative lengths, whose reversed form has
only its top bytes set, are exact, so the `.mgz` tags of recon-all are not affected; any negative or
large number is. Rule A: by default exact; `reproduce` rounds (`machine::swap_long64`). Relay 1 had
noted it (`machine.rs`) for when a program reaches it.

### FS-168: fio_dirname reads the name after freeing it

`fio.cpp` 633-640: for a name of one character (after the trailing slashes), `free(pname)` comes
before `if (l == 0 || pname[0] == '/')`. The official build (`objdump` of `fio_dirname`) loads
`pname[0]` after the `free` and the `calloc(2)`: glibc's tcache has written its next pointer
(safe-linked: the chunk's address shifted right by 12, xored with the bin's head) over the first
bytes, so the byte compared is an address bit pattern. Measured: in one run `fio_dirname("a")` gave
`/` (twice, in different case files) and `.` in another; `"b"` gave `.`. The intended result is `.`
(the one character cannot be `/`: a lone `/` is stripped to the empty name). Both modes `.`, noted;
the probe compares these lines apart (`VARY`).

### FS-169: fio_DirIsWritable's 2,000-byte buffer

`fio.cpp` 765: `char tmpstr[2000]; sprintf(tmpstr, "%s/.junk.54_-_sdfklj", dirname)`. The official
build is compiled with `_FORTIFY_SOURCE` (`__sprintf_chk`): a result of 2,000 bytes or more aborts
with `*** buffer overflow detected ***: terminated` (signal 6) before any file is made. Measured on
names of 1,982 and 4,100 bytes. Both modes use the whole name (rule B would make it an error; a long
directory name is a sensible input, so the port proceeds: the system decides, `ENAMETOOLONG` past
4,095 bytes).

### FS-170, FS-171, FS-172: TSV::read

- FS-170: `fgets(line, 5000, fp)` (1045) at the end of an empty file returns NULL and leaves
  `char line[5000]` uninitialised; `strtok_r` then reads it. In the probe the stack held a NUL first
  (the result `no data found`), which is the port's answer.
- FS-171: `size_t ln = strlen(token) - 1; if (token[ln] == '\n') ...; ln = strlen(token) - 1; if
  (token[ln] == '\r') token[ln] = '\0';` (1055-1058, and 1064-1067 for the later fields): a field
  that is exactly `\n` becomes empty, `ln` wraps to `SIZE_MAX`, and `token[ln]` is the byte before the
  field. For later fields that byte is the tab before them (inside the line); for the first field
  starting at `line[0]` it is the stack byte before the buffer, written with 0 when it is `\r`.
  No visible effect; noted (both modes the empty name).
- FS-172: the loop `while(!feof(fp))` (1074) with `fscanf(fp, "%lf", &val)` (1079) treats a matching
  failure (0) as a value: `row.push_back(val)` keeps the previous number (`double val;` uninitialised
  before the first). glibc's `%lf` leaves the failing character unread, so when nothing at all was
  read the next call fails at the same place and the loop never ends; rows of stale numbers grow
  until `std::bad_alloc` (the probe's 1 GiB limit) or the probe's 5 s alarm. Where the failed
  conversion read characters (`-.-.5`: `-.` read, failing at the second `-`) the loop moves on;
  `reproduce` follows it there. Rule B by default (an entry that is not a number in a table of
  numbers makes no sense): an error.

### FS-173: TSV::print's format

`fio.cpp` 1111-1115: `char fmt2[1000]; sprintf(fmt2, "%s%s", fmt.c_str(), "\t"); fprintf(fp, fmt2,
data[r][c]);`: the caller's format with one `double` argument. `%d` printed `1` for every number in
the probe (the register `fprintf` reads for an `int`), `%s` crashed; a format of 999 bytes or more
overflows `fmt2` (`__sprintf_chk`: abort). No caller of `TSV::print` or `TSV::write` exists in
FreeSurfer 8.2.0 (only `mri_glmfit` reads tables); the inventory reaches them statically. Both modes
refuse such formats.

### FS-174: lcalloc ends the program with status 0

`MRIio_old.cpp` 29-52: `MGHprint_error` prints its message and calls `exit(0)`. Measured:
`lcalloc(2^64 - 1, 2)` prints `Cannot calloc()` and the child exits 0. Callers: `mri_tess.cpp`
(`mri_mc`'s and `mri_pretess`' tessellation arrays), `mrisurf_timeStep.cpp`, `mri_tessellate.cpp`,
`tkregister2.cpp`: an allocation that fails (an enormous volume) ends those programs "successfully",
so a script (recon-all's `mri_tessellate` step) goes on to read an output that was never written.
Rule A: status 1 by default.

### FS-175: the byte swaps past the buffer

`bfileio.cpp` 51-118: `for (n = 0; n < nmax; n += 8)` swaps whole groups of `nbufbytes` rounded up.
`fio.cpp` 297 and 312 pass `nbufbytes` 1 for a 4-byte buffer (intended: one float); every other
caller passes a multiple of the size (`mriio.cpp` 2954, 3662, 5319-5321, 6474-6475, 7943,
`fscnpy.cpp` 123-154, `bfileio.cpp` 328-454). The port's slices know the buffer's end: a group past
it is left as it is (both modes), noted.

### Kept (rule C), no defect ID

- `fwrite2` clamps to [-32767, 32767] (`fio.cpp` 133), not -32768.
- `fwrite3` writes `v << 8`'s top three bytes: the low 24 bits; FreeSurfer's quad and triangle files
  use it for counts and for the magic numbers (-1 & 0xffffff is `QUAD_FILE_MAGIC_NUMBER`).
- `fio_dirname("")` and `fio_basename("")` give `/` (`fio.cpp` 631-641, 691-701); `fio_dirname`
  keeps a slash before the last one (`a//b` gives `a/`); `fio_extension` returns everything after the
  last dot, slashes included. No caller passes such names on the default paths.
- `fio_DirIsWritable` truncates and removes a file already named `.junk.54_-_sdfklj`.
- `TSV::write`'s failure message says `TSV.read()` (1123).

### The probe (validation/probes/fs-io)

`fsio.cpp` runs each case in a forked child limited to 1 GiB of address space and 5 seconds, inside a
fixture directory, and prints FreeSurfer's results; `check.py` compares them with the port's
(`examples/fs_io_probe.rs`, `tests/fs_io/cases.rs`) through the oracle-output cache. Seed 1, 1,100
cases: `reproduce` identical on 1,000 of 1,000 comparable lines (98 with memory digits), 12 `VARY`
lines identical, 88 `SKIP` (53 FS-175, 32 tables, 3 FS-169); `fix` differs only with notices (FS-01
31, FS-167 31, FS-49 4, FS-172 2, FS-174 71, FS-176 47). Seed 7, 22,000 cases (log
`<workspace>/oracles/logs/fs-io-probe-seed7.log`): `reproduce` identical on 20,279 of 20,279
comparable lines (2,150 with memory digits), 268 `VARY` lines identical (164 `fio_dirname`, 104
empty tables), 1,453 `SKIP` (864 FS-175, 519 tables, 70 FS-169); `fix` differs only with notices
(FS-01 831, FS-49 207, FS-167 729, FS-171 1, FS-172 44, FS-174 1,162, FS-176 810). Portable seed 1, 1,000 cases: 911 identical,
89 hashed as `SKIP`, hash `ce3ddcd0...` (`tests/fs_io_official.rs`), passing with AArch64's NaN
emulated (`nan_trap.c`, `NAN_TRAP_ARM=1`, `NAN_TRAP_DIR` set: 0 NaNs to flip) in debug and release.

## Part 2: tags.cpp, fstagsio.cpp, mri.h's VOL_GEOM::write/read, colortab.cpp's binary tables (3.1-25, piece 2)

**Restricted** (closed to FreeSurfer clean-room implementers: it cites FreeSurfer's source). Track A,
relay 11. Taken early by the orchestrator's decision of 2026-11-13: tags.cpp's reached functions (queue
item 3.1-30) and colortab.cpp's binary readers and writers (3.1-33), which FStagsIO calls.

### The name buffers (the orchestrator's decision (2), 2026-11-13)

- `VOL_GEOM::fname[STRLEN]` has the member initialiser `= {'\0'}` (mri.h 202): every `VOL_GEOM`, an
  `MRI` (a subclass) included, starts with 1,024 zero bytes. The copy constructor (mri.h 250) runs the
  initialiser and then `strcpy`: a copy's bytes after the name are zeros; the assignment operator (293)
  `strcpy`s into the existing buffer: the bytes after the name stay. `VOL_GEOM::write` (512-559) writes
  `memcpy(buf, fname, 512)` (`.mgz`): those bytes reach the file. Every reached allocation of a
  structure holding a `VOL_GEOM` is zeroed (transform.cpp 646-651 `calloc` for LTAs, 2132 and 2614 for
  TRANSFORMs; gcamorph.cpp's GCAMalloc `calloc`; MRI's constructor), so no reached name byte is memory
  never written: the decision's second clause ("default zeros with a notice") has no case today; a later
  item that meets a `malloc`ed one adds the notice. `MRI_FRAME::name` (mri.h 114) is the same (frames
  `calloc`ed, mri.cpp 211) and `write_mri_frames` writes all 1,024 bytes (fstagsio.cpp 441).
- Port: `larmorx-freesurfer-core::cname::CName` (1,024 bytes, `strcpy` keeps the tail); `VolGeom::fname`
  and `MriFrame::name` use it (fs.mri: every `fname = x.fname.clone()` was a `strcpy`: vol_geom.rs'
  copy_fields, mri.rs's MRIallocFromVolGeom/copyVolGeom/MRIcopyVolGeomFromMRI/MRIcopyHeader, copy.rs's
  MRIcopy). The fs-mri probe's hash is unchanged by it.
- In `write_gcamorph_geom` (fstagsio.cpp 534-535) the geometries are written from copies, so their tails
  are zeros there: the only writer of `VOL_GEOM::write` today never writes a stale tail. A frame name's
  tail does reach the `.mgz`.

### Mechanisms of FS-177 to FS-194

- **FS-177** tags.cpp 42 and 241, fstagsio.cpp 1065: `ErrorExit(..., "failed to calloc %u bytes!\n",
  tag, len)` with `len` a `long long`: `%u` takes the low 32 bits (measured: -1 prints 4294967295).
- **FS-178** the skippers `calloc(len, 1)` the whole length before `fread`/`znzread` (tags.cpp 41, 240;
  fstagsio.cpp 1064); read_mri_frames' rest (fstagsio.cpp 950-952, 1189-1191) `calloc`s and reads into a
  NULL buffer when the allocation fails (no check: a crash where the file has bytes). In the probe (1 GiB
  address space) lengths beyond about 1 GiB fail; the probe skips cases above 256 MiB.
- **FS-179** `fscanf(fp, "%s", buf)` into `char buf[MATRIX_STRLEN]` (tags.cpp 182, 186), `sscanf(buf, "%s ...",
  ch, ...)` into `char ch[100]` (tags.cpp 409 in znzReadMatrix; fstagsio.cpp 836 in read_matrix).
- **FS-180** the 1,600-byte `buf` is not NUL-terminated (tags.cpp 407, fstagsio.cpp 831: `znzread` of
  exactly MATRIX_STRLEN into an uninitialised stack array); glibc's `sscanf` takes the string's length
  first (`_IO_str_init_static_internal` with size 0: `strlen`), so it reads past the array whenever the
  bytes hold no NUL; the numbers change only where the scan reaches the end.
- **FS-181** `M->rptr[1..4][1..4]` (tags.cpp 154-169, 202-217, 357-372; fstagsio.cpp 362-377,
  1088-1103) with `rptr` of a smaller matrix: an out-of-bounds pointer read.
- **FS-182** `strcat(cmd_line, argv[i])` (tags.cpp 129-131) into `mri_segment`'s `char
  cmdline[CMD_LINE_LEN]` (4,096, const.h 53; mri_segment.cpp 125).
- **FS-183** fstagsio.cpp 1019-1020: `calloc(dlen + 1)` with `dlen` = the tag length - 20: -1 gives
  `calloc(0)` then `znzread(p, 1, SIZE_MAX)` (a plain file reads the rest of the file into it; zlib's
  `gzread` refuses the negative `int` and returns -1); below -1 the allocation fails and `pedir` is NULL
  (a crash later wherever a caller `strlen`s it).
- **FS-184** fstagsio.cpp 222: `dlen += (x * y * z * len)` in `int`s.
- **FS-185** mri.h 611: `char buf[remaining]`, a VLA of the name's length - 512.
- **FS-186** mri.h 604: `znzread(fname, 1, to_read, fp)` without clearing or terminating; a `to_read` of
  0 or less reads nothing (the old name stays); the `.mgz` form (593-594) clears only 512 bytes.
- **FS-187** colortab.cpp 612-616, 747-751, 990-992 (`malloc(len + 1)`, `fread`/`znzread` of `len`,
  `strncpy` from it: no NUL unless the file's bytes hold one), 647, 804, 1021 (`strncpy(..., STRLEN-1)`
  into a `malloc`ed entry whose last byte is never written), 1174 (znz version 2 reads straight into the
  `malloc`ed entry name). Version 1 entries end at `name[len] = 0` (648, 1022), so their names are
  defined when the file gives all `len` bytes.
- **FS-188** colortab.cpp 648, 1022: `name[len] = 0` with `len` ≥ STRLEN; `sizeof(CTE)` is 1,064 (the
  probe's `fsl sizeof`): 1,024 ≤ `len` < 1,064 lands on `ri`..`count`, rewritten or unused.
- **FS-189** colortab.cpp 774 and 1147: `ct->entries[structure]` with `structure` ≥ `nentries`.
- **FS-190** colortab.cpp 1124-1125, 1173-1174: `toread = min(len, STRLEN-1)` and no skip.
- **FS-191** colortab.cpp 588-595 (version 1: `CTABfree(&ct)` then `ct->nentries` in the message: a NULL
  dereference), 612, 643, 747, 800, 990, 1019 (`malloc(len + 1)`: `len + 1` overflows for INT_MAX; a failed allocation
  passes NULL to `fread` and `strncpy`: crashes, measured in the probe).
- **FS-192** the readers' `freadInt`/`znzreadInt` at the end of the file return memory contents (FS-49,
  FS-167); the probe's official runs show such tables crashing (a garbage name length) or reading
  garbage entries.
- **FS-193** fstagsio.cpp 182-185 (strlen whole) against mri.h 551-552 (at most 512 written).
- **FS-194** colortab.cpp 679-701, 1053-1071: version 1 writes `nentries` but only the non-NULL entries;
  CTAB_VERSION_TO_WRITE (colortab.h 33; set at colortab.cpp 115, 452) is 2; version 1 is set only by the version-1 readers
  (599, 977).

### Other facts

- `sizeof(MRI_FRAME)` is 1,288 on x86-64 (measured: `fsl sizeof`): the `.mgz` frame tag's length is
  12,880 per frame. The frame's float `TM` written by `write_mri_frames` is the double `MRI_FRAME::TM`
  converted (fstagsio.cpp 445), read back as a float into the double (890) and, for a diffusion frame,
  again as a double (922; written as a double at 479).
- `255 - t` (colortab.cpp 654, 811, 1028, 1181) overflows for `t` = INT_MIN (undefined; the official build wraps):
  the port wraps (`wrapping_sub`). No file stores such an alpha; not registered.
- `VOL_GEOM`'s default constructor leaves `valid`, `width`, `height`, `depth` unset (mri.h's `VOL_GEOM()
  {}` with no initialisers for them): the probe sets them before printing (a probe's own fix, found when
  its first run printed stack values).
- The C-stream colour table readers print `CTABreadFromBinaryV1: could not read parameter(s)` for
  version 2's names too (colortab.cpp 749, 802: the version-1 text); `ErrorPrintf` prints `strerror(errno)`
  first when `errno` is set (the probe saw `Cannot allocate memory` from a failed `malloc` of a garbage
  length before the crash).

### Measured (2026-11-13, the probe `validation/probes/fs-io/`, unit oracle's build)

- Seed 7, 46,000 cases (23 commands x 2,000; boxed 10 GiB, the official run about 40 minutes): `reproduce`
  identical on 40,710 of 40,710 comparable lines (2,070 with memory digits); 3,003 lines whose official
  result is memory contents compared apart (1,769 equal, 1,234 not: FS-49, FS-167, FS-180, FS-187, FS-192);
  2,287 not comparable (FreeSurfer crashes, reads for ever or depends on the machine). Of piece 2's 24,000
  cases: 20,453 comparable lines identical, 2,731 compared apart, 816 not comparable. `fix` differs only
  with a notice (FS-176, FS-174, FS-01, FS-49, FS-167, FS-186, FS-193, FS-177, FS-184, FS-172, FS-190,
  FS-178, FS-192). The port's 46,000 lines are identical with and without AArch64's NaN rules emulated
  (`nan_trap.c`, NAN_TRAP_ARM=1 with NAN_TRAP_DIR set; 1,356 NaNs flipped), in both modes.
- Seed 1, 2,200 portable cases (`tests/fs_io_official.rs`): 1,927 identical (110 with memory digits), 273
  hashed as SKIP, SHA-256 4bf83403...; passing in debug and release and under the trap.
- Three things the first runs taught (fixed): (1) glibc's `calloc` failure leaves `errno` ENOMEM, which
  `ErrorExit` prints first (`error: Cannot allocate memory`); (2) glibc's `fscanf` reads nothing after a
  complete `nan` or `inf`, so a stream that ends there keeps its end-of-file flag clear (the port's
  scanner now asks the stream whether a read found the end, `cscan::Stream::hit_end`); (3) a colour
  table of millions of empty places printed one mark per place made 33 MB lines that ran check.py out
  of its 8 GiB box: the probe prints the non-empty entries with their indices, and messages beyond 1
  MiB by their length (`BIGTEXT`).

## Part 3: fscnpy.cpp, gifti.cpp 47-212, dti.cpp's b-value files (3.1-25, piece 3)

**Restricted** (closed to FreeSurfer clean-room implementers: it cites FreeSurfer's source). Track A,
relay 13. The behaviour is in `specs/freesurfer-io.md` §19-24; the defect texts (behaviour only) in
`larmorx_core::defects` (FS-199 to FS-220).

### fscnpy (FS-199 to FS-213)

- The object's layout (fscnpy.h, x86-64): `__magic_string` (a pointer, 0-7), `__header_len_bytes[2] =
  {2, 4}` (8-15), `__major_version` (16), `__minor_version` (17), two bytes of padding, `__header_len`
  (`size_t`, 24-31), ... `mriio.cpp` 799 makes it with `new` (heap memory never written but for what
  the constructor sets).
- **FS-199/FS-200.** fscnpy.cpp 234 `fread(&__header_len, 1, __header_len_bytes[__major_version-1], fp)`:
  major 3 reads `__header_len_bytes[2]`, the four bytes at offset 16: major (3), minor, the two padding
  bytes: 3 + 256 x minor (+ the padding's) bytes into the 8-byte `__header_len`. Version 3.0 (padding
  0) reads 3 of the 4 length bytes; the header text then starts at the length's fourth byte (0) and
  the `dict` buffer lacks the header's last byte, its `'\n'`, so the parse of line 249 runs past it
  (measured: NumPy's own version 3.0 file crashes the probe, `CRASH 11`). Majors 0 (the high half of
  `__magic_string`, a PIE address: about 21,000 bytes into an 8-byte field), 4 and up (padding,
  `__header_len` itself, `__endian`...) are memory or nonsense; measured: version 0 crashes.
- **FS-201/FS-202.** 237 `char dict[__header_len]` is a variable-length array on the stack (up to 4 GiB
  for version 2); 238's short read leaves its rest unwritten. 241-245: `restheader`, another VLA.
- **FS-203.** 249 `for (; *currpos != '\n'; currpos++)`: the scan ends only at a newline seen where an
  item starts; 255's key scan stops at a newline but 320's increment then steps over it; 282 `while
  (*currpos != ',')` and 292 `while (!isdigit(*currpos))` and the shape loop 296-317 have no end test
  at all; 267-270 read two bytes and step two without looking.
- **FS-204.** 298-307: `value` is reset to NULL after each element; a `,` or `)` before the next digit
  calls `atoi(NULL)` (SIGSEGV). NumPy writes a one-dimensional shape as `(3,)`: every 1-D `.npy`
  crashes FreeSurfer (measured).
- **FS-205.** 338 `while (__shape[__ndims-1] == 1)` has no `__ndims > 0` test: an all-ones shape pops
  to empty and reads `__shape[-1]` (the malloc chunk's size field, not 1), then 346 `while
  (__shape[0] == 1)` reads the stale 1 and calls `erase(begin())` on an empty vector: libstdc++'s
  move of `end - (begin + 1) = -1` elements, `memmove` of `(size_t) -4` bytes (measured: `CRASH 11`).
- **FS-206** (fs.mri, mri.cpp 108-129). `MRI::Shape::Shape(const std::vector<int>&)` prints its
  message for a count other than 3 or 4 and still reads `shape[0]`, `shape[1]`, `shape[2]`: past the
  vector's elements for fewer than 3. In npy2mri the vector is `__shape` after the squeeze: its
  storage (libstdc++: growth 1, 2, 4, 8 by `_M_realloc_insert`, new places unwritten; `pop_back`
  leaves the place; `erase(begin())` moves the rest down and leaves the last place) still holds the
  dropped extents, so `(1, 3, 4, 1)` gives depth 4 (measured: `3 x 4 x 4 x 1`, the verbose line) and
  `(2, 4, 1)` depth 1; a place beyond the storage or left by a growth is heap memory (measured 0 for
  `(2, 3)`: a volume without voxels). `larmorx_freesurfer_mri::mri::Mri::new_beyond` takes those
  places (`fscnpy::ShapeVec` models the vector); `Mri::new` (lists made for the call: their storage
  ends at their elements) gives memory, 0 with `reproduce`.
- **FS-207.** `__endian`, `__dtype_char`, `__dtype_len`, `__fortran_order` are not set by the
  constructor (8-26): a header without `'descr'` or `'fortran_order'` leaves them as heap memory; an
  empty `__shape` reads `__shape[-1]` from a NULL data pointer (crash).
- **FS-208.** 126-127: `__dtype_len == 8` calls `byteswapbuffloat` (4-byte groups), not
  `byteswapbufdouble`.
- **FS-209.** 521 prints `%p` of `data_src` and `data_dst` (heap addresses: ASLR).
- **FS-210.** 288 `__fortran_order = !strcmp(value, "False") ? false : true`.
- **FS-211.** 111 `size_t bytes_per_slice = __dtype_len * mri->width * mri->height` multiplies ints.
- **FS-212.** 118's short `fread`s are only counted (printed with `verbose`).
- **FS-213.** 241-246 skip `16 - total % 16` bytes when the total is not a multiple of 16; NumPy's
  `read_array_header` reads the data right after `header_len` bytes (its writer pads inside the
  header: `_wrap_header`, to `ARRAY_ALIGN` = 64 bytes in NumPy 2.3.5's `_format_impl.py`; the format's
  version 1.0 text asks for 16).
- Kept (not defects): `'|'` (and any order byte but `'<'`) counts as "different endian" (only 1-byte
  types carry `'|'`, and they are not swapped); keys are compared with their quotes, so a header
  written with double quotes has none of them (FS-207); unknown keys are skipped with the first byte
  of their value; a repeated `'shape'` appends; `array contiguous reordering ...` is printed for every
  C-order array, verbose or not; `__getarchendian` is `'<'` on every platform larmorx supports;
  `exec_progress_callback` (131) does nothing without freeview's callback; `fscnpy::read` (39-64: no
  caller; its short-read test compares items with bytes) and `fscnpy::write` (empty) are not ported.

### gifti.cpp 47-212 (FS-214 to FS-216)

- Both functions are `static` (no symbol in `libutils.a`): the probe compiles gifti.cpp itself into
  `fsio` (`gifti_statics.cpp`, with gifti.cpp's own compile command from the unit oracle's
  `compile_commands.json`) and calls them through two wrappers.
- **FS-214.** 75: `fprintf(stderr, "... data=%p\n", da)` passes the array, not its data.
- **FS-215.** 175-180: the COMPLEX64 case reads `*((double *)(da->data) + ...)`.
- **FS-216.** 143-201: `(dim0_index * dims_1)` and `(dim1_index * dims_0)` are int products added to
  the pointer.
- `NIFTI_TYPE_INT8` reads through `char` (signed on x86-64 Linux; unsigned on AArch64 Linux, not an
  official FreeSurfer platform): the port reads `i8`. `gifti_add_empty_darray`'s failures are out of
  memory only.

### dti.cpp (FS-217 to FS-220)

- **FS-217.** 853-858 and 936-940 open the file, then 859/942 read `bvals->rows` of a NULL matrix
  (measured: `CRASH 11`, the file left empty). `MRIwriteType` (mriio.cpp 1268-1290) calls
  `DTIwriteBVectors(mri->bvecs, ...)` whenever `mri->bvals` is set: a volume with b-values and no
  gradients crashes there.
- **FS-218.** 859 `bvals->rptr[n][1]`, 942 `rptr[n][1..3]`: `rptr[n] + c` runs into row `n + 1`; past
  `data + rows x cols` it reads MatrixAlloc_new's block (matrix.cpp 418-421: rounded up to 64 bytes,
  only `nelts` floats zeroed, 476-477) or beyond: memory.
- **FS-219.** 985 `char tmpstr[100]`, filled by 991-996 and 1003-1008 without a bound (Ubuntu's GCC
  builds with the stack protector: an overflow that reaches the canary aborts).
- **FS-220.** 1127-1131 `bvec->rptr[i][1] = vol->bvecs->rptr[f][i]` and back for `i = 1..3`.
- `DTIparsePulseSeqName` has no caller in FreeSurfer 8.2.0 (the inventory lists it in 3.1-25 as part
  of dti.cpp's reached object); `DTIbvecChangeSpace` is called by the DICOM readers (DICOMRead.cpp 578,
  6048, queue item 3.7-14) with `FSENVgetenv`'s desired space; `MatrixMultiplyD(M, bvec, bvec)` is in
  place (the port copies the input, as FreeSurfer's MatrixMultiplyD does).
- `%d` of a number beyond `int` (`sscanf`, glibc): the low 32 bits of the `long` (measured:
  `999999999999` gives -727379969), as `larmorx_core::cscan::sscanf_i32` gives.

### The probe's piece-3 commands

`npy`, `gda`, `gdv`, `bvw`, `psn`, `bcs` (validation/probes/fs-io/fsio.cpp; the port's side
`tests/fs_io/piece3.rs`). Results: below, "Measured (piece 3)".

### Measured (piece 3, 2026-11-13, the probe `validation/probes/fs-io/`, unit oracle's build)

- Seed 7, 58,000 cases (29 commands x 2,000; boxed 12 GiB, about 35 minutes): `reproduce` identical on
  51,221 of 51,221 comparable lines (1,761 with memory digits); 3,745 lines of memory contents or
  addresses compared apart (2,009 equal); 3,034 not comparable. Piece 3's 12,000 cases: 10,387
  comparable lines identical (`npy` 1,055 with 636 not comparable and 309 compared apart; `gda` 2,000;
  `gdv` 1,802 and 198 apart: FS-214's addresses; `bvw` 1,720, 69 crashes of FS-217, 211 apart; `psn`
  1,953, 47 of FS-219; `bcs` 1,857, 143 apart: FS-220 past the matrix). `fix` differs only with a notice
  (piece 3's: FS-206 202, FS-213 207, FS-218 194, FS-55 217, FS-212 70, FS-220 49, FS-210 30, FS-208 21,
  FS-215 11). The port's 58,000 lines are identical with and without AArch64's NaN rules emulated
  (NAN_TRAP_ARM=1 with NAN_TRAP_DIR set; 1,877 and 1,996 NaNs flipped in `fix` and `reproduce`).
- Seed 1, 2,800 portable cases (`tests/fs_io_official.rs`): 2,468 identical (73 with memory digits),
  332 hashed as SKIP, SHA-256 5832c485...; passing in debug and release and under the trap (150 and
  148 flips).
- What the first runs taught (fixed): (1) the shape's places past a list of fewer than 3 extents are
  set even where they all hold 1 (the first run left them 0: `(2, 4, 1)` gave depth 0, FreeSurfer 1);
  (2) a message that prints the data file's name differs between the two sides' file names: the
  `.npy` cases now read `x.npy` in the fixture on both; (3) in the first full run two piece-2 cases
  (`ctr`, `ztr`) ended by the probe's 5-second alarm (`CRASH 14`) while six agents loaded the machine;
  run alone, both gave the port's lines at once.

## Part 4: the volume format probe (3.1-27, piece 1)

**Restricted** (closed to FreeSurfer clean-room implementers: it cites FreeSurfer's source). Track A,
relay 14. The behaviour is in `specs/freesurfer-io.md` §25-33; the defect texts (behaviour only) in
`larmorx_core::defects` (FS-221 to FS-228). Ported: `utils/mri_identify.cpp` whole (lines 47-1208;
697-1195 taken early from 3.1-28 because `mri_identify` calls every test), `MRIgetVolumeName`
(mriio.cpp 472-486, from 3.1-28), `checkBrukerFiles` and `is_bruker` (Bruker.cpp 201-260, 834-849),
`ghc`, `ghi`, `ghf`, `get_signa_header_info`, `is_signa` (signa.cpp 69-198, from 3.1-30), `IsDICOM`,
`IsSiemensDICOM`, `GetObjectFromFile`, `GetElementFromFile`, `AllocElementData`, `FreeElementData`
and `DCMPrintCond` as the Manufacturer element reaches them (DICOMRead.cpp 1164-1460, 1479-1522,
5410-5549), `DCM_GetElement` and `DCM_GetElementValue` (CTN's dcm.c 949-1100, 2699-2717, as
FreeSurfer's copy has them) for that element only.

### Mechanisms of FS-221 to FS-228

- **FS-221.** Fixed buffers filled by `strcpy`/`strcat`/`sprintf` without a bound: `mri_identify`'s
  `char fname[STRLEN]` (MRIgetVolumeName's `strcpy(name_only, string)`, mri_identify.cpp 203-209,
  mriio.cpp 476), `string_to_type`'s `char ls[STRLEN]` (154), `is_analyze`'s `hfname[STRLEN]` (`strcpy`
  731 and `strcat(".hdr")` 739), `is_sdt`'s `header_fname[STR_LEN]` (980, 983-985), `is_nifti1`'s
  `fname_stem[STRLEN]` (1036), `IsDICOM`'s `static char file[1024]` (DICOMRead.cpp 5417, filled 5430: a static, so it
  overwrites the next statics), `is_bruker`'s five `char ...File[512]` (Bruker.cpp 837-841, filled by
  `checkBrukerFiles`' `strcpy`+`strcat` 214-252), `IDtypeFromStem`'s and `IDnameFromStem`'s `char
  tmpstr[2000]` (`sprintf("%s.nii")` ... `"%s_000.bfloat"`), `bhdr_precisionstring`'s and
  `bhdr_precision`'s `tmpstr[2000]`. The official build aborts on the stack protector's canary for
  `string_to_type` of 1,030 bytes (probe: `CRASH 6`). Every name the C functions then use is still the
  whole string (the copy keeps its terminator past the buffer), so both modes use it whole; the probe
  counts these lines, never compares them.
- **FS-222.** `is_mnc` (mri_identify.cpp 776-781): `for (dot--; *dot != '.' && dot > fname; dot--)`
  starts one byte before a dot at the name's start and reads it before testing `dot > fname`, then
  compares from there; the compared text's second byte is the name's dot against `m`, so the answer is
  always "no". The port starts at the name.
- **FS-223.** `is_mgh` (802-815) compares `fname`, not `ext`, with `.mgh`/`.mgz`/`.mgh.gz` (its comment
  describes the extension); `ext` is computed and unused. No caller in FreeSurfer 8.2.0.
- **FS-224.** `is_signa` (signa.cpp 189): `if ((ret = fread(h, ...)) != HLENGTH) return (0);` before
  `fclose(fp)`.
- **FS-225.** `ghi` (signa.cpp 77-91): `num = (num << 8) | *(header + 2*offset + i)` with `char` signed
  on x86-64 (the official build's `movsbl`): the second byte's sign extension ORs ones into the high
  bits. The first byte's sign extension is a 16-bit signed reading, so the correction keeps it.
- **FS-226.** `IDextensionFromName` (486-535) `strncpy`s the last 3/4/6/11 bytes into `ext` and
  compares them with `nii` ... without looking at the byte before (`IDstemFromName` compares `.nii`
  with the dot). Callers: `MRISfileNameType` (mrisurf_io.cpp 2561: a name it takes for a volume is
  read and written as a volume), mri_convert's frame splitting, mri_surf2surf, mrisutils.
- **FS-227.** `AllocElementData` (DICOMRead.cpp 1252-1305) gives US/SS 2 bytes, UL/SL/AT/FL 4, FD
  `multiplicity` x 8 (`calloc`), then `DCM_GetElementValue` `memcpy`s the element's whole length into it
  (dcm.c 1059): a longer value writes past the allocation (within glibc's minimum chunk of 24
  usable bytes the answer is the text up to the first zero; beyond, memory). The comparison is
  `strcmp(e->d.string, "SIEMENS")` on the union's pointer.
- **FS-228.** Representations `AllocElementData` has no case for (OT, UT, DD, RET, CTX, DLM) leave
  `d.ot` NULL after its message; `DCM_GetElementValue` then `memcpy`s into NULL (a value of 1 byte or
  more) or returns normally and `strcmp(NULL, ...)` faults (length 0): SIGSEGV either way (probe:
  `CRASH 11`). A string representation with an undefined length (CTN keeps the value unread, FS-91)
  allocates `length + 17` = 16 bytes (32-bit wrap) and writes `d.string[0xffffffff]`.

### Other facts

- `FreeElementData` (1408-1460) has no case for UN (which `AllocElementData` handles as text): it
  prints `FreeElementData: 21 unrecognized` without a newline and leaks the value. A Manufacturer
  stored as UN (explicit VR files) prints it on every `IsSiemensDICOM`: kept in both modes (rule C, no
  result depends on it), noted here only.
- `checkBrukerFiles` prints `INFO:    d3proc %s` for the reco file too (a copied line, 256): kept.
- `IDtypeFromStem` does not look for `.img` while `IDnameFromStem` does; `IDtypeFromStem` has no caller.
- `is_sdt` replaces the extension found anywhere in the name (`a.d/s` looks for `a.spr`): FreeSurfer's
  convention for extensions throughout the file; kept.
- `ghf`: the union's `long ii` is 8 bytes; only its low 4 are written before `thing.ii >> 24`, `&
  sign_bit` and `& dmantissa`, which use bits 0-31 only: the uninitialised high half never reaches the
  result. A zero fraction keeps the exponent (GE's `mvtsunf`): `0x40000000` is 0.0005, not 0.
- `is_signa`'s sums: `fabs(float)` is libstdc++'s `float` overload (the official build's `andps` and
  `addss`), compared with the `double` literal after `cvtss2sd`.
- `IsDICOM`'s fourth attempt (`DCM_FORMATCONVERSION | DCM_ACCEPTVRMISMATCH`) is always refused by CTN
  before the file is opened (no byte order). After a failed attempt the reader may already have closed
  the object, so the next `DCM_CloseObject` pushes "NULL object passed to routine DCM_CloseObject"
  (track C's finding, docs/findings/freesurfer-permissive.md); the stack is emptied at the end.
- `GetObjectFromFile` (DICOMRead.cpp 1207-1243) repeats the four opens with `options | DCM_ACCEPTVRMISMATCH`
  and does not empty the stack between them; `GetElementFromFile` empties it only on success, so a
  missing Manufacturer leaves the failed attempts' conditions and "Requested element (8 70) not found
  in DCM_GetElementVM" on the stack (the probe dumps it after each case).
- `errno`: most tests set it to 0 after a failed `fopen` or short `fread`; `is_cor`, `is_bruker` and
  `checkBrukerFiles` keep `stat`'s error; `IDisCurv`, `is_signa` and `IsDICOM` keep `fopen`'s, and
  `IDisCurv` and `is_signa` keep the failed read's (21 for a folder). The port's `CFile` now fails a
  folder's reads with `EISDIR` on every platform (on Windows it used to give an empty file) and keeps
  the read's error (`read_errno`).
- Two points for larmorx-io (track C's `larmorx_io::dicom`, told through the orchestrator): (1)
  `CondStack::dump` writes the stack's texts through `cfmt::Streams` (`&str`), so a message holding a
  byte that is not UTF-8 (a broken VR code, `DCM Unrecognized VR code (P\xbd)`) prints U+FFFD; the
  port prints the stack with its own byte-exact dump (`dicom_probe::dump_conditions`); (2) `open_file`
  uses one path both to open the file and in CTN's messages: the port passes the name joined to the
  program's folder, so CTN's messages show that path where FreeSurfer shows the name as given (only
  with `FS_DICOM_DEBUG` or in a dumped stack; the probe shows them relative to its fixture). A variant
  taking the name for the messages would close both.

### The probe's piece-4 commands

`sty`, `tts`, `mgvn`, `mid`, `isx`, `idf`, `dcm` (validation/probes/fs-io/fsio.cpp's `run_piece4` and
`setup`; the port's side `tests/fs_io/piece4.rs`). Each file case makes its files in the fixture's
`w` folder (removed first). Results: below, "Measured (piece 4)".

### Measured (piece 4, 2026-11-13, the probe `validation/probes/fs-io/`, unit oracle's build)

- Seed 7, 72,000 cases (36 commands x 2,000; boxed 12 GiB, about 45 minutes): `reproduce` identical on
  65,077 of 65,077 comparable lines (1,743 with memory digits); 3,770 lines of memory contents or
  addresses compared apart (2,074 equal); 3,153 not comparable. Piece 4's 14,000 cases: 13,914
  identical, 86 not compared (`dcm` 77: FS-227 and FS-228, FreeSurfer's `CRASH 11`; `sty` 8: FS-221's
  1,030-byte extension, `CRASH 6`; `mid` 1). `fix` differs only with a notice (piece 4's: FS-225 19,
  FS-226 7, FS-223 2).
- Coverage of the first 7,000 piece-4 cases (`<scratch>/A/r14/p4stats.py`): `mri_identify` returned
  every format it can give (-1, 0-12, 14-18, 20-30, 32-34; 13, 19, 31 and 35 never come out of it);
  each `isx` test said yes on cases made for it (curvature 5, Signa 7, Bruker 12, NIfTI-1 pair 6,
  ANALYZE 4, ...); `dcm`: `IsDICOM` yes and no, its kept answer, `IsSiemensDICOM` 1 and 0 with 135
  warnings, 16 sequences and 45 unhandled representations, with and without `FS_DICOM_DEBUG`.
- Seed 1, 3,600 portable cases (`tests/fs_io_official.rs`): 3,243 identical (98 with memory digits),
  357 hashed as SKIP, SHA-256 1caecb5a...; passing in debug and release and under AArch64's NaN
  emulation (NAN_TRAP_ARM=1 with NAN_TRAP_DIR; 64 flips each).
- What the first runs taught (fixed): (1) CTN's condition stack printed through larmorx-io's text
  streams turned a broken VR code's byte into U+FFFD: the port dumps it byte for byte; (2) `IDisCurv`
  and `is_signa` of a folder leave `errno` 21 from the failed read: `CFile` now keeps a failed read's
  error (and fails a folder's reads on every platform).

## Part 5: mosaics and MATLAB files (3.1-27, piece 2)

**Restricted** (closed to FreeSurfer clean-room implementers: it cites FreeSurfer's source). Track A,
relay 15. The behaviour is in `specs/freesurfer-io.md` §34-39; the defect texts (behaviour only) in
`larmorx_core::defects` (FS-229 to FS-248, FS-250, FS-251; FS-249 was not used). FS-245 marks every
result that depends on memory contents (the cause noted with its own defect as well): larmorx keeps
one notice per defect, so a "memory" detail of a defect already noted for another reason was lost
and such lines were compared (one in the seed-7 run: a compressed stream cut by the file's end, which
FreeSurfer decodes on into its uninitialised buffer). Ported: `utils/mosaic.cpp` whole, `utils/matfile.cpp` whole.

### Code generation

- `VolSS2MosSS`/`MosSS2VolSS` (official object code): the `floor()` calls on integer quotients are
  folded away; the tile column is the `idiv` remainder; the bounds `x > n - 1` are compiled as
  `n <= x` (they differ from the source only for `n` = -2^31); the order of the divisions is
  `ncmos % ncvol`, `nrmos % nrvol`, then `svol / (ncmos / ncvol)` (`MosSS2VolSS`: `cmos / ncvol`,
  `rmos / nrvol`); each `idiv` faults on 0 or on -2^31 / -1 (FS-229, `SIGFPE`: probe `CRASH 8`).
- matfile.cpp passes its `long32` fields to `%ld` zero-extended (`mov 0x4(%rbx),%edx`): -1 prints
  4294967295. `(char)real_matrix[row][col]` is `cvttsd2si` to 32 bits and the low byte (FS-244).
  `(long32)fval` is `cvttss2si` (FS-234).

### Mechanisms

- **FS-229.** `ncmos % ncvol` and the other divisions of mosaic.cpp 79, 89, 100 and 104 (`VolSS2MosSS`), 145,
  155, 166, 170 and 171 (`MosSS2VolSS`) have no zero check.
- **FS-230.** `DIFFERENT_ENDIAN(mf)` (matfile.cpp 47) is `mf->type != MATFILE_PC` for every version
  but 5, `MATFILE_PC` being 0: only type 0 (little-endian, double, full) is taken as this machine's
  order. `MatReadHeader0` (458-501) and `MatReadHeader`'s version 4 part (1023-1052) then call
  `swapBytes` (503-509: type, rows, columns, name length; not `imagf`), and `readMatFile` swaps the
  numbers. MATLAB's own rule is the thousands digit of the type (0 little-endian, 1 big-endian).
- **FS-231.** `readMatFile` 386-388: `nitems = fread(&sval, 1, sizeof(short), fp); if (nitems !=
  sizeof(char))`: a 2-byte read fails the test. With 1 byte left the read passes and `sval`'s other
  byte is the uninitialised local's.
- **FS-232.** `readMatFile` 397 and `znzreadMatFile` 723-724 read `sizeof(long)` (8) bytes per
  32-bit element; the swap is `swapLong32(lval)`, of its low 32 bits.
- **FS-233.** `char bval` and `short sval` (signed on x86-64) for every 8- and 16-bit type, `long` of
  4 file bytes never for unsigned 32-bit; the version 5 comments of 320-335 have signed and unsigned
  the wrong way round, both mapped alike.
- **FS-234.** `fval = (float)swapLong32((long32)fval)` (414, 731).
- **FS-235.** The imaginary loop (433-452) reads only `MAT_DOUBLE` and for the other types assigns
  `dval`, an uninitialised local in `readMatFile` (315; 1.0 in `znzreadMatFile`, 676, swapped again at
  every element of the other byte order).
- **FS-236.** `mf->imagf = (long)(m & ctmp[3])` (939) with `m = 1 << 4` and `ctmp` the flags word's
  bytes: the complex flag is bit 11 of the word (byte 1 in little-endian order), and `ctmp[3]` is its
  high byte; `znzMatReadHeader` (1123-1124) tests `unbuff[18]` likewise and then overwrites `imagf` (1127).
  The imaginary numbers follow the real ones directly in `readMatFile`, without the real part's
  padding and the imaginary part's tag.
- **FS-237.** After the name, `fread(&(mf->type), 1, 4)` (1005) and `fseek(fp, 4, SEEK_CUR)` (1011)
  take a small numbers element's word as the type and skip its data.
- **FS-238.** `padding = 8 - ((int)mf->namlen - (int)mf->namlen / 8 * 8)` (985).
- **FS-239.** The small form's `memmove(&namlen_temp, &mf->namlen, sizeof(short))` (993) keeps the
  word's low half, the element type (1); names are `calloc(namlen)` with no room for a zero (980,
  995, 1140, 1152) and printed with `%s` (1014, 1160).
- **FS-240.** `znzMatReadHeader` (1061-1169): `memmove(&(mf->mrows), &unbuff[32], sizeof(long))` and
  the same for `ncols`, `namlen` and `type` copy 8 bytes into 4-byte fields (`imagf`, `version`,
  `mrows` overwritten); `test_data[0] == 0 && test_data[1] == 0` (1137) is false for every name tag
  MATLAB writes (its type 1 is in byte 0), so the small branch runs, whose `DIFFERENT_ENDIAN` (1149)
  reads `mf->type`, never set by `MatlabRead2`'s uninitialised `MATFILE mf` (768) when the clobbered
  version is not 5; `padding = 48 + ((((int)mf->namlen - 1) / 8) + 1)` (1142) counts blocks, not
  bytes; `size_unbuff = 100 * size` (1109) bounds the decoding; `znzreadMatFile` reads at the fixed
  offset 56 (712-750). The line of 1158-1165 is an `ErrorPrintf(DIAG_VERBOSE, ...)`: printed always.
- **FS-241.** `MatlabRead2`'s `MATFILE mf` (768) and `MatReadHeader`'s `dt` and `tmp` (869, 871) are
  uninitialised; `fread(...) != 1 && ferror(fp)` (its version 5 reads, 911-1008) reports errors only,
  so at the end of the file they stay as they were.
- **FS-242.** `MatlabRead2` never closes `fp` (uncompressed) or `znzfp` (792; `fp` 781); `MatFileRead` never
  closes `fp` (153).
- **FS-243.** `matAlloc` (286-307), the names' `calloc` (480, 980, 995, 1040, 1140, 1152), `malloc(size)`
  (1102) and `calloc(size_unbuff)` (1110) of sizes from the file; `matFree(NULL, rows)` after a failed
  `matAlloc` (MatFileRead 200) dereferences a null pointer; under the probe's 1 GiB address space the
  official build also throws `std::bad_alloc` from `ErrorPrintf`'s formatting (probe `CRASH 6`).
- **FS-244.** `bval = (char)real_matrix[row][col]` (190).
- **FS-246.** `MatReadHeader` reads two dimensions at 160 and 164 and then the name tag at 168, which
  is the third dimension's place for a 3-D array.
- **FS-248.** `MLFC`'s `varname[1000]` and `varmtx[1000]` (matfile.h) are filled by
  `mlfc->nvars++` (570-613) without a bound.
- **FS-250.** `MatFileRead` ignores `MatReadHeader0`'s `NULL` (158).
- **FS-251.** `if ((a == (char)EOF) || ...)` (888) and `if (c == (char)EOF)` (572) compare `char`s
  with -1.

### Decisions

Rules (CLAUDE.md, D13): FS-229 B (both modes return 1 with a clear message; the crash is never
reproduced); FS-230 to FS-238 and FS-240 A (the default reads MATLAB's formats by their
definition); FS-239 A (messages: the whole name); FS-241 B (a clear error, `NULL`); FS-242
resource leak (both modes close; no answer changes, no notice); FS-243 crash or machine-dependent
(not reproduced; larmorx refuses above 256 MiB of numbers as a failed allocation, both modes);
FS-244 A (saturation, the rule of FS-02); FS-245 memory contents (both modes 0, never
compared); FS-246 B; FS-247 A; FS-248 A (all kept); FS-250 C (no
caller; kept in both modes); FS-251 C (no valid file holds 0xff there; kept in both modes).

### Who calls these

`MatlabRead` is `MatrixRead` (matrix.cpp 1064), `mri_glmfit`'s design matrices (`--X`), the
ANALYZE reader's orientation file beside an `.img` (mriio.cpp 5066); `MatlabWrite` is `MatrixWrite`
and the ANALYZE writer's `.mat` (mriio.cpp 5563, 5861); `ReadMatlabFileVariable` is `fsgdf.cpp` 340.
recon-all's volumes are `.mgz` and NIfTI, and fMRIPrep reads none of these files: no fMRIPrep impact
known. The mosaic functions serve the Siemens DICOM reader (fs.dicomread, 3.7).

### Measured

- The probe (`validation/probes/fs-io/`, piece 5): seed 9, 3,500 cases of `mvm`, `mmv`, `mck`, `mlw`,
  `mlr`, `mfr`, `mrc`: `reproduce` identical on 2,990 of 2,990 comparable lines; 308 not compared
  (FreeSurfer's crashes: `SIGFPE`, `SIGSEGV` after `matAlloc` fails, `std::bad_alloc` thrown while
  formatting an error under the probe's 1 GiB, prints passing the 5 seconds), 202 compared apart
  (memory contents); `fix` differs only with notices. Seed 1, 4,300 portable cases of the 43
  commands: 3,836 identical, 464 hashed as `SKIP` (`e8e6a116...`, `tests/fs_io_official.rs`, passing
  in debug and release and under `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`).
- Seed 7, 86,000 cases of the 43 commands: `reproduce` identical on 76,947 of 76,949 comparable lines
  (1,699 with memory digits); 4,496 not compared, 4,555 compared apart. The two others are a
  `VOL_GEOM::read` and an `FStagsIO` case of piece 2 where the official build was stopped by the
  probe's 5-second alarm (`CRASH 14`) while this relay built and tested beside the run; run again
  alone they are identical (the oracle-output cache keeps the timed-out lines). Piece 5's 14,000
  cases: 11,902 compared, all identical, 1,276 not compared, 822 compared apart; `fix` differs only
  with notices.
- SciPy's files (`matfile_scipy.py`, 400 matrices: version 4, version 5, compressed; doubles, singles,
  signed and unsigned 8- to 32-bit integers, complex): FreeSurfer reads 51 of 136 version 4 matrices
  right (the little-endian doubles, and the big-endian ones), 33 of 136 version 5 and 0 of 128
  compressed; the default reads all 400 as SciPy wrote them.
- The probe's harness: a matrix of more than 65,536 numbers is printed as an FNV-1a hash of its floats
  (`FSIO_HASH_MIN` sets the limit for the official side, to print one whole); the first build of that
  hash in `fsio.cpp` had the 64-bit offset basis with a digit dropped, found by computing the hash of
  the official numbers in Python (the port's was right).


## Part 6: reading volumes and registration files (relay 16 of track A, for track D's surfaces)

**Source-derived (FreeSurfer 8.2.0's `utils/mriio.cpp` 109, 488-1105, 10853-11245, 12195-12420;
`utils/registerio.cpp` 43-162; `utils/resample.cpp` 109-118; `packages/minc/minc_gaussian.c`,
`minc_volume_io.c`): closed to FreeSurfer's clean-room implementers.**

- `mri_read` finds the last `@` and the last `#` on the whole name with `strrchr` before it writes
  a NUL at either: so in `x.mgz@mgz#0` the format's text ends at the `#` (`mgz`) and in
  `x.mgz#0@mgz` the frames' text ends at the `@`.
- `nan_inf_check` on a volume read without voxels: `MRIFseq_vox` through `slices`, which
  `MRIallocHeader` never allocates: a crash for a float volume with a frame list (FS-255; reached
  by `MRIreadHeader`/`MRIreadInfo` of `name#n`).
- `mghRead`'s header skip in a compressed file ends with `znzread(buf, total_bytes - count, 1,
  fp)`: for a volume without voxels the member size is 0, and FreeSurfer's `znzread` divides the
  bytes read by it (SIGFPE, measured: the probe's `CRASH 8`; FS-76's mechanism, nifticlib's).
- `MRI::initSlices` advances the row pointer only for uchar, short, unsigned short, RGB, int, long,
  float and (by the byte count) float complex: a tensor (and bitmap, double complex) volume's rows
  all point at the chunk's start, so `mghRead`'s `MRIFseq_vox` writes overwrite one row.
- `mghRead`'s `buf = calloc(bytes, ...)` with a negative `int` byte count fails (`errno` ENOMEM,
  measured), and `znzread` into the null buffer of a compressed file returns -1 (zlib's
  `(int)len < 0` check, which also sets the stream's error so every later read fails: the TR, the
  tags); the replica gives `larmorx_io::znz::GzFile::gzread_too_long` for that state.
- `FS_MGZIO_USEVOXELBUFREAD`: `bufsrc = &MRIseq_vox(mri, 0, 0, 0, start_frame)` dereferences
  `slices[start_frame * depth]`: past the one-frame volume for a frame after the first, and a
  crash for a volume without slices (`slices` of 0 entries; measured `CRASH 11`) (FS-256).
- `MRITAGread` is called by `mghRead` with its defaults (`ras_xform` NULL): the
  `TAG_RAS_XFORM_EXTRA` case has no `niftiheaderext` check and calls `read_ras_xform_extra(NULL)`
  (FS-253).
- `TAG_MGH_XFORM`: `fstagsio.read_data(mri->transform_fname, len)` reads `len` bytes into the
  1,024-byte field (FS-254 past it; `transform_fname` is followed by the `General_transform`);
  `sprintf(tmpstr, "%s/transforms/talairach.xfm", fio_dirname(fname))` into 1,000 bytes (FS-254).
  The file is read with `input_transform_file`, FreeSurfer's copy (`packages/minc`), whose
  `create_linear_transform` uses `minc_volume_io.c`'s static `compute_transform_inverse` (no
  identity check; `minc_inverse.c`'s global one checks the first product against the identity and
  exits) on `minc_gaussian.c`'s `invert_4x4_matrix` (`invert_4x4_matrix_wkr_fast`; `doPrint` is
  false in the build).
- `TAG_CMDLINE` and `TAG_PEDIR`: `calloc(len + 1)` then `read_data(.., len)` and `[len] = 0`: a
  length of -1 gives an empty string after writing one byte before the allocation; below -1 or
  beyond memory a null buffer and a crash (FS-257). `TAG_FIELDSTRENGTH` reads `sizeof(float)`
  bytes whatever the tag's length (no defect ID: FreeSurfer always writes 4; a file with another
  length is then read as further tags, both modes).
- `regio_read_register`'s `char tmp[1000]` takes `fscanf("%s")` without a width (FS-258); the `.lta`
  branch reads `lta->xforms[0]` without checking `num_xforms` (FS-258) and calls `LTAread` and
  `TransformLTA2RegDat` (transform.cpp, fs.volume: the replica takes them through
  `registerio::RegisterLta`). `perror` (glibc 2.35) duplicates standard error's descriptor and
  `fdopen`s it `"w+"` while standard error is not yet oriented: on a descriptor open for writing
  only that fails with EINVAL and `errno` stays EINVAL (measured in the probe's child, whose
  standard error is a file opened `O_WRONLY`); the probe orients standard error first.
- `chklc()` in `MRIreadType`/`MRIread` exits 255 without `FREESURFER_HOME` (the licence check, a
  runtime matter larmorx's replica does not have: `larmorx-freesurfer-core::chklc` returns 1); the
  probe calls `mri_read` directly for its `mrd` command.
- Measured (2026-11-13, the probe's piece 6): seed 3, 310 cases; seed 5, 1,250; seed 11, 2,500:
  `reproduce` identical on 1,777 of 1,778 comparable lines (572 not compared: formats not ported,
  `.lta`, crashes, skips beyond 256 MiB; 150 compared apart), the one other FS-256's crash, since
  noted.

## Part 7: Bruker's readers (3.1-27, relay 17 of track A)

Restricted (FreeSurfer's source lines). `utils/Bruker.cpp` 146-849.

- **FS-259** (class a; both modes the whole strings, noted). `splitParameterValue` (261) `strcpy`s the
  name into the caller's `Parameter[256]` and the whole remainder after `=` (its newline included,
  before the loop that cuts it) into `Value[128]`; the three readers' buffers are stack arrays
  (`char line[512]; char Parameter[256]; char Value[128];`), so a name of 256 bytes or more or a
  remainder of 128 or more writes over the neighbouring locals. Measured: none (undefined); the
  probe's own buffers are 8,192 bytes, so such lines are counted, not compared.
- **FS-260** (class a, resource leak; both modes close, no notice, as FS-242). `readBrukerReco` never
  calls `fclose` on success (its last lines compute the sizes and return); `readBrukerD3proc` (338)
  and `readBrukerReco` (753) return without it on an unsupported type; `readBrukerVolume` (807) on an
  unknown one.
- **FS-401** (class a, memory never written; both modes 0, noted; the probe marks such lines
  `VARY`). `brukerRead`'s `width`, `height`, `depth`, `TR`, `TE`, `TI`, `flip_angle` and every
  `BrukerTransform bTran` field (stack, no initialiser) that `d3proc`, `reco` or `acqp` does not set:
  `MRIalloc` then gets sizes and a type from the stack, the geometry and times likewise.
- **FS-402** (class c, rule A: default the type's size; `reproduce` FreeSurfer's). `readBrukerVolume`
  (813): `fread(mri->slices[k][j], sizeof(short), mri->width, fp)` for every type. A `_32BIT_SGN_INT`
  study (`MRI_INT`) gets `2 * width` bytes at each row's start and zeros after (MRIalloc's); a byte
  volume (only through FS-401's type) overlaps the next row and its last row writes `width` bytes
  past the chunk.
- **FS-403** (class a, crash; both modes NULL, noted; counted, not compared). A short read in
  `readBrukerVolume` (814-818) closes the file and `MRIfree(&mri)` on its own copy of the pointer;
  `brukerRead` (194) then `MRIfree(&mri)` on the caller's copy: a double free (glibc aborts; the probe
  sees signal 6).
- **FS-404** (class c, rule A: default byte-swapped for `bigEndian`; `reproduce` as read).
  `readBrukerReco` does not read `$RECO_byte_order` and `readBrukerVolume`'s `swap_bytes_flag` is
  always 0 ("this was not needed"): ParaVision's big-endian studies (older SGI-era data) are read in
  little-endian order. The default reads ParaVision's parameter.

Behaviours kept (rule C, both modes):
- `readBrukerD3proc` (318-327): two `else if (!strcmp(Parameter, "$IM_SIT"))` branches; the first,
  `sscanf(Value, "%d", pz)`, makes the second (frames, `ERROR: nframes %d but one is supported.`)
  unreachable, so the depth is the later of `$IM_SIZ` and `$IM_SIT`. It looks deliberate (the first
  branch placed before the older one: a 2-D multi-slice study's slices are counted in `IM_SIT`), and
  no study at hand shows whether correcting it helps (rule D would measure it on real ParaVision
  data: none in the data panel). Kept in both modes; the spec says so.
- `$RECO_wordtype` compares 13 characters (`strncmp(..., 13)`: `_16BIT_SGN_IN`), so `_16BIT_SGN_INX`
  is taken for short; harmless.
- Messages kept as printed: `ERROR: value must follow RECO_size` for a missing `$RECO_ft_size` line,
  `ERROR: float value must follow ACQ_slice_offset` for the gradient matrix, the reco file's check
  line `INFO:    d3proc <reco>` (§30), the open failures without a newline, `OWNER` at the end of the
  file printing the stale buffer.
- `$ACQ_flip_angle`'s conversion runs even when `sscanf` read nothing (the old value converted).
- `buildVoxToRASTransform` passes the transform's doubles to `stuff_four_by_four`'s `float`
  parameters: the geometry is single precision (`MatrixMultiply` of floats).
- Object code (the unit oracle's `Bruker.cpp.o`, GCC 8.4.0 `-O3`): `readBrukerReco`'s voxel size is
  `fov * 10.0` (the third axis `10.0 * fov[2]`: the same for every input, one NaN operand at most),
  then `/ size`; the offset `(ft_size - 1) * (-vox)` then `* 0.5` (the source's `/ 2.`; equal for
  every input). larmorx computes `mul64(fov, 10)`, `div64(., size)`, `mul64(-vox, ft - 1)`,
  `div64(., 2)`: the same bits, NaNs included.

Measured (the probe's piece 7, `tests/fs_io/piece7.rs`): seed 11, 763 cases: `reproduce` identical on
648, 0 different, 85 counted (FS-259, FS-403), 30 apart (FS-401); seed 13, 3,815 cases: 3,221
identical, 0 different, 426 counted, 168 apart; `fix` differs only with FS-402 (39) or FS-404 (16)
noted.

## Part 8: FreeSurfer's 2-D images (3.1-26's line 65 and 3.1-27's image.cpp 104-3815, relay 18 of track A)

Restricted (FreeSurfer's source lines). `utils/image.cpp` (the 14 functions the programs reach, of
the file's 73: `ImageAlloc` 65, `ImageAllocBuffer` 104, `ImageFree` 136, `ImageCopy` 384,
`ImageClearArea` 526, `ImageCheckSize` 977, `ImageSetSize` 1005, `ImageCopyFrames` 1021,
`ImageRescale` 1217, `ImageScaleDown` 1241, `ImageScaleUp` 1368, `init_header` 3748, `h_copy` 3808,
`free_header` 3815), `utils/hips.cpp` 4-5 and 42-47 (the globals and the conversion stubs) and
`utils/filter.cpp` 713-1041 (`ImageConvolve1d`, `ImageReduce`, `ImageReduce1d`, which
`ImageScaleDown` calls; ported into `larmorx-freesurfer-io::image` because fs.filter's crate sits
above fs.io). Callers in the reach (the callgraph of `<scratch>/A/r8/edges.json`): `MRItoImageView`,
`MRItoImage` (mri.cpp, track C's fs.volume), `mrisp.cpp` (`MRISPalloc`, `MRISPread`, `MRISPclone`,
`MRIStoParameterization(s)`, `MRISgradientToParameterization`), `filter.cpp`'s `ImageReduce`,
`mrimorph.cpp`'s `mriWriteImageView` (the only `ImageRescale` caller: `mri_em_register`'s
diagnostic image views, `scale = target_size / rows`); `mriio.cpp`'s `mri_read` and `MRIwriteType`
free images. None of them passes `imagebufsize` (always 0), converts formats in `ImageCopy` or
`ImageCopyFrames` (`mrisp.cpp`'s `ImageCopyFrames(Ip, Itmp, 0, Ip->num_frame, 0)`: floats), or
scales a multi-frame image.

### Code generation (the official object, `oracles/freesurfer-8.2.0-unit/build/fs/utils/CMakeFiles/utils.dir/`)
- `ImageReduce1d`: `movss pixel, %xmm0; mulss k[i], %xmm0; addss %xmm0, %xmm1` (the total the
  destination: `total + pixel * k`). `ImageConvolve1d`: the same `addss` order (`mulss`'s order
  does not matter: the kernel is never NaN). No vectorised float reductions in either.
- `ImageClearArea`'s `(char)val`, `ImageScaleDown`'s and `ImageScaleUp`'s `(unsigned char)float`:
  `cvttss2si` into 32 bits, the low byte stored. `ImageScaleUp`'s `(int)((float)outCol / scale)`:
  `cvttss2si` (and `cvttps2dq` in the vectorised byte loop: the same results).
- `ImageCopyFrames`' `(float)unsigned int`: `cvtsi2ss` of the zero-extended 64-bit value (scalar)
  and `cvtdq2ps` halves (vectorised): both the correctly rounded conversion.
- `ISPOW2` (`macros.h` 118, with `exp2(f)` defined as `pow(2.0, f)` there): `log2f`, `nint`,
  `pow`; `nint(log2(1.0 / scale))` with double `log2`. The relocations of `ImageScaleDown`: `log2f`,
  `log2`, `pow`, `nint` (14 calls).

### Mechanisms
- **FS-405** (class a; both modes skip the write and read 0, noted; counted, not compared). No
  function checks a frame, an area or a pixel place against the buffer: `ImageClearArea` with a
  `frame` at or beyond `num_frame`, or extents in use beyond the stored ones;
  `ImageAllocBuffer`'s `hmemset(I->image, 0, I->sizeimage * I->num_frame)` after `calloc(npix)` of a
  given smaller `imagebufsize`; `ImageCopy`'s `h_copy` of `Isrc->numpix * Isrc->sizepix` bytes into a
  new image allocated from `Isrc->rows`/`cols` (not `orows`/`ocols`), or with a frame count of 0;
  `ImageCopyFrames`' frame places; the scales' coordinates; `ImageConvolve1d`/`ImageReduce1d` on
  images whose extents in use pass the stored ones; `h_copy`'s `int bytes` (a frame of 2 GiB or more:
  a short or negative `memmove`); `while (size--)` with a negative `size`; `ImageScaleDown`'s
  power-of-two path with 0 reductions copies `ImageCopy(inImage, outImage)` of every frame from the
  advanced pointer (frames past the end), and its `ImageCopy` frees a caller's output smaller than
  the reduction (use after free). Seed 11: of the 119 counted lines, 51 crash in FreeSurfer, 20 end
  with other bytes, 48 happen to agree.
- **FS-406** (class a, undefined conversions; `reproduce` x86-64's `cvttss2si` results, the default
  clamps, CLAUDE.md's rule for undefined float-to-integer conversions).
- **FS-407** (class c, rule A). `ImageCopyFrames` (1088-1110): the byte-to-float and byte-to-int
  branches write `IMAGEFseq_pix(outImage, 0, 0, frameno)` / `IMAGEIseq_pix(outImage, 0, 0,
  frameno)`, and the int branches `IMAGEFpix(outImage, 0, 0) + pix_per_frame * frameno` etc.,
  where the same-format branches use `dst_frame + frameno - start`. Unreached by the programs (no
  caller converts).
- **FS-408** (class c, rule A: the default reduces every frame and keeps the output's frame count;
  `reproduce` stops after the first). `ImageScaleDown` (1316-1327) reassigns `inImage` to the
  reduction (`inImage = ImageReduce(Itmp, NULL)`, one frame), and the frame loop's condition `frame <
  inImage->num_frame` then ends the loop after frame 0; `ImageCopy(inImage, outImage)` sets
  `outImage->num_frame` to the reduction's 1. The end of the function restores `inImage->image` on
  the leaked reduction (the caller's input is never advanced on that path, its `firstpix` untouched).
- **FS-409** (class a, use after free; both modes return the copy, noted; counted). `ImageRescale`
  (1226) ignores `ImageCopy`'s result; `ImageCopy` frees an `outImage` with fewer pixels or frames
  and copies into a new one; `ImageRescale` then sets the freed image's sizes and returns it.
  Unreached (its only caller passes no output).
- **FS-412** (class a, rule B: the default refuses the scale with an error; `reproduce` goes on).
  `ImageScaleUp`/`ImageScaleDown`/`ImageRescale` compute `nint((float)rows * scale)`: beyond the int
  range or NaN, `nint`'s `(int)` gives -2147483648 (FS-12), `ImageCheckSize`'s 64-bit products wrap
  (`INT_MIN * INT_MIN * frames` is 0 or -2^62 or 2^62 ...), so the check can pass and the loops run over
  -2147483648 extents (nothing done) or `ImageRescale`'s `ImageAlloc` fails. Found by the port: with
  FS-12's default clamp the same products let a scale of 1e30 loop 2^31 times per pixel (a case of seed
  13 never ended), so the default refuses such scales before any of that.
- **FS-410** (class c, rule A: the default exits with `ERROR_NO_MEMORY`). `ImageAlloc` (75):
  `ErrorExit(Gerror, ...)` after `ImageAllocBuffer` returns `ERROR_NO_MEMORY` without setting
  `Gerror`: the code is the previous error's, usually `NO_ERROR` (0), so `exit(0)`. Measured: the
  probe's `ial` of 2^53 bytes with `Gerror` 0 ends with status 0 after `error: Cannot allocate
  memory` and the message.

### Behaviours kept (rule C, both modes; no ID)
- `init_header` (3748) has no case for `PFCOMPLEX` (4) and `PFDBLCOM` (7): they take `sizepix` 1
  (`default:` with `PFBYTE`), so a complex image gets a byte per pixel. Nothing in the reach makes a
  complex image (MRISP, MRItoImage: floats and bytes), so a correction would change no result
  (rule C: measured to bring no gain); the spec says so.
- `init_header` with `alloc` (`ImageAllocHeader`, unreached) sets `firstpix` before allocating
  (NULL).
- The HIPS conversions are stubs (`error: hips is not supported`, 0): `ImageCopy` between formats
  copies nothing after the message. FreeSurfer removed HIPS deliberately; kept.
- `ImageReduce`'s `static IMAGE *Itmp` and `ImageConvolve1d`'s `static` index tables: only memory
  differs (the port makes them each call); the tables are pure functions of the extents and the
  length.
- `ImageScaleDown`'s byte-to-byte bounds check prints `in: %d, %d --> out: %d, %d!` and `exit(2)`;
  negative places are not checked (FS-405).

## Part 9: fs.labels' text colour tables and lookups (3.1-33's first piece, relay 19 of track A)

Restricted (FreeSurfer's source lines). `utils/colortab.cpp`: `CTABreadASCII` 55, `CTABreadASCII2` 70,
`CTABreadASCIIttHeader` 191, `CTABalloc` 420, `CTABunique` 495, `CTABcountRepeats` 527, `CTABrandom`
552, `CTABreadDefault` 1245, the counts and lookups 1267-1677 (`CTABhasEntry` 1314 too), `CTABprintASCII`
1690, `CTABwriteFileASCII` 1720, `TissueTypeSchema` 1779, `TissueTypeSchemaDefault` 1800,
`TissueTypeSchemaDefaultHead` 1957, `CTABmerge` 2480 (the binary tables: Part 2; `CTABfree` and
`CTABdeepCopy`: fs.mri's `ctab`). Callers in the reach: `FSENVgetenv` (every program that reads the
environment), `mri_segstats`, `mri_aparc2aseg`, `mri_ca_label`, `mris_volmask` (`CTABreadDefault`),
`mri_gtmseg`, `mri_compute_volume_fractions`, `mri_label2vol`, `cma.cpp` (`TissueTypeSchema`),
`mrisurf_defect.cpp` (`CTABalloc` + `CTABunique` + `CTABmerge` with a table sized to hold the merged
one: `MAX(nentries, vol->ct->nentries)`), `mri_gtmseg`'s `--ctab-merge` (`CTABmerge` of a user's
table: FS-418 reachable), `mris_make_face_parcellation` (`CTABcountRepeats`), track D's GIFTI reader
(`CTABgetNumberOfValidEntries`, `CTABfindDuplicateNames`, `CTABrgb2Annotation`).

### Mechanisms
- FS-413: `CTABreadASCII2`'s first pass reads lines through `fgetl` (which cuts each line at its
  first `#`), its second through `fgets` (whole lines). A name holding `#` hides its line from the
  first pass (`sscanf` assigns 2 values), so the size `max_structure + 1` does not count it; the
  second pass assigns 6 or 7 and indexes `ct->entries[structure]` without a bound check. A negative
  structure (the first pass ignores it: `structure > max_structure` is false) indexes before the
  array. Both read `ct->entries[structure]` (a heap word: a non-NULL value prints the duplicate
  message through a wild pointer, NULL stores a fresh `malloc` there). Measured: crashes (signal 11)
  or wrong alpha values (`-1215751936`) in the probe.
- FS-414: `CTABreadASCIIttHeader` (a) calls `GetNthItemFromString(line, 0)` whenever
  `CountItemsInString(line)` > 0: the latter counts items separated by `isblank` (space, tab), the
  former by the C++ stream's white space (`isspace`): a line of `\v`, `\f` or `\r` among blanks has 1
  item for the first and none for the second (`fs::error() << "asking for item 0, but 0 items in
  string"`, NULL, then `strcmp(NULL, "#ctTType")`: signal 11); (b) `GetNthItemFromString(line, 1)` on
  `#ctTType` alone (NULL into `sscanf`: signal 11); (c) `int segid` and the second pass's `structure`,
  `name[STRLEN]`, `r`, `g`, `b`, `t` are locals that `sscanf` leaves untouched when it stops (the
  return values are not checked): an earlier line's values, or the stack's contents before any; a
  structure outside `[0, segidmax]` indexes `ct->entries` out of bounds. The loops are `cp = (char
  *)1; while (cp != NULL) { cp = fgets(...); ...}`: at the end of the file `fgets` returns NULL with
  the buffer unchanged, so the last line is processed once more (no visible effect); an empty file
  (unreachable through `CTABreadASCII2`, which needs a structure line) would read `line[5000]`
  uninitialised.
- FS-415: `CTABreadASCIIttHeader` returns NULL for `nct == 0` before `fclose(fp)`.
- FS-416: `CTABreadDefault` dereferences `fsenv->FREESURFER_HOME` after `FSENVgetenv()` without a
  NULL check; `TissueTypeSchemaDefault(Head)` the same, and then `sprintf(ct->TissueTypeSchema, ...)`
  on the NULL `CTABreadASCII` gives when the table does not read.
- FS-417: `CTABrandom`, `CTABcountRepeats` read and write `ct->entries[i]->...` for every `i <
  nentries` without a NULL check; `CTABunique` writes `ct->entries[0]` (for `CTABalloc(0)`: the
  `calloc(0, 8)` block's first word, NULL in practice: signal 11).
- FS-418: `CTABmerge` loops over `merge->nentries` and indexes `ct->entries[n]` without comparing
  with `ct->nentries`.
- FS-419: `TissueTypeSchemaDefault(Head)` overwrite `ri`, `gi`, `bi` of `CTABalloc(5|6)`'s entries but
  not `rf`, `gf`, `bf`, which keep `CTABalloc`'s random draws (entry 0's are zeroed by `CTABunique`).
- FS-420: `calloc(max_structure + 1, 8)` (and `calloc(segidmax + 1, 8)`, `CTABalloc`'s `malloc` per
  entry): beyond the machine's memory `calloc` fails and the error returns (or, in `CTABalloc`, the
  error's argument `ct->nentries` dereferences the NULL `CTABfree` left: a crash); within it, a
  table. The probe's child has 1 GiB of address space.
- FS-421 (fs.io, `MRITAGread`): `MRI::MRI` sets only `transform_fname[0] = '\0'`; `TAG_MGH_XFORM`
  reads the tag's `len` bytes into the buffer with `fstagsio.read_data` without adding a NUL, so a
  text without one continues into the buffer's never-written bytes (a fresh `new MRI`: heap
  contents). Found by seed 1's light run once piece 10 changed the probe's heap history: the
  official line printed `w/none.xfm` followed by an earlier case's output text.

### Behaviours kept (rule C, both modes; no ID)
- `CTABreadDefault` reads the default table twice (once inside `FSENVgetenv`, once itself): its
  duplicate and error messages appear twice.
- `CTABunique` reports failure (`INFO: ... could not find a unique set in <nmax> tries`, -1) when the
  last try succeeded, and for `nmax` ≤ 0.
- A table of only structure 0 is `badly formed` (`max_structure <= 0`).
- `CTABalloc`'s entries' `TissueType` and `count` are never written (memory contents): no reached code
  reads them (the probe does not print them); 0 here.
- `CTABcopyName` (`strncpy`) leaves the caller's buffer without a NUL when the name has `name_len`
  bytes or more; the reached callers pass `STRLEN`-sized buffers, longer than any name.
- `CTABfindDuplicateNames` counts ordered pairs (each pair twice, three equal names six times) and
  prints a line for each, then halves the count.
- Signed overflow of `structure + idbase`, `255 - t`, `(b << 16) + ...`: GCC's code wraps (32-bit
  `add`/`sub`/`shl`); the port wraps.

### Code generation
- The float colours `(float)i / 255.0`: `cvtsi2ss`, `cvtss2sd`, `divsd` by 255.0, `cvtsd2ss` (the
  division in double, then rounded to float); `CTABcountRepeats` compares the floats with `ucomiss`
  (no NaN possible).

### Measured (piece 10, 2026-11-13, the probe `validation/probes/fs-io/`, unit oracle's build)
- Seed 11, 330 piece-10 cases: `reproduce` identical on 279, 0 different (44 SKIP, 6 VARY).
- Seed 13, 1,664 piece-10 cases: `reproduce` identical on 1,415, 0 different (230 SKIP: FS-416 76,
  FS-417 52, FS-414 crashes 32, FS-413 20, FS-420 12, FS-418 8, FS-11 3; 19 VARY: FS-414's memory
  never written). `fix` differs only where a defect is noted (FS-414's default error, FS-01).
- Seed 1's light set with piece 10 (the recorded test): 4,796 cases, 4,164 identical (55 with
  memory digits), 0 different, 632 SKIP or VARY; `d9fa9bda...`; the NaN trap (AArch64's NaN rules)
  passes in debug (3,835 flips) and release (3,546).

### Registration files, the rest (3.1-34, relay 19)
Restricted. `utils/registerio.cpp`: `regio_print_register` 161, `regio_write_register` 207,
`regio_read_mincxfm` 243, `regio_write_mincxfm` 322, `regio_read_registermat` 523 (callers in the
reach: `mri_vol2vol`, `mri_surf2surf`, `bbregister`'s helpers, `mri_coreg`, `tkregister2_cmdl` (the
`larmorx-nc` program), `talairach_avi`'s `.xfm` writers through `regio_write_mincxfm`).
- FS-422: `regio_read_mincxfm` returns 1 after the `MNI Transform File` check without `fclose(fp)`.
- Kept (rule C, no ID): `fgetl`'s return value is ignored, so the first line is compared and the
  file information copied from the buffer itself (leading white space included); at the end of the
  file the second `fgetl` leaves the first line's text, which becomes the file information;
  `regio_write_register` returns 0 whatever `regio_print_register` returned; the messages'
  `unrecoginized` and `Could open ... for writing`; `*R` set before the numbers are read, so a failed
  read hands the caller a partial matrix with status 1; the file reopened without a check (a file
  removed between the two opens crashes: not reachable).
- Measured: seed 13, 710 piece-10 registration cases (142 of each): `reproduce` and `fix` identical
  on all; seed 1's light set with them: 4,797 cases, 4,217 identical, 0 different, 580 SKIP or VARY
  (`5e19ca25...`), the NaN trap passes in debug (4,084 flips) and release (3,954).

## Part 10: fs.labels' labels, control points, annotation tables and point sets (3.1-33's second piece, relay 20 of track A)

Restricted (FreeSurfer's source lines). `utils/label.cpp`: `LabelReadFrom` 54, `LabelRead` 120,
`LabelFree` 215, `LabelWriteInto` 534, `LabelWrite` 580, `LabelAlloc` 804, `labelLoadTransform` 1121,
`LabelCopy` 1246, `LabelRemoveDuplicates` 1276, `LabelHasVertex` 2543, `LabelRealloc` 2557;
`include/label.h` 35-63 (`LABEL_VERTEX`, `LABEL`), `include/mri.h` 86-89 (`FS_COORDS_*`);
`utils/ctrpoints.cpp`: `MRIreadControlPoints` 37, `MRIwriteControlPoints` 128, `MRIfillPoints` 495;
`utils/annotation.cpp`: the statics `atable`/`num_entries` 51-52, `read_named_annotation_table` 115,
`read_annotation_table` 171, the lookups 224-316, `set_atable_from_ctable` 524, the printers 62-89;
`include/annotation.h` (`annotation_table_file`); `include/pointset.h` (the class, its inline
members), `utils/pointset.cpp` (`save` 17, `save_as_ctrlpoint` 57, `loadfsPointSet` 74,
`writeCentroidTable` 103). Not ported here: `LabelToScannerRAS` 3111, `LabelToSurfaceRAS` 3150,
`LabelToVoxel` 3198 and `LabelVoxelToSurfaceRAS` 3285 (they need `RASFromSurfaceRAS_`,
`voxelFromSurfaceRAS_`, `MRIvoxelToSurfaceRAS`: fs.volume, a crate above fs.io), the functions on
surfaces (fs.surf), `loadfsPointSet` (nlohmann/json: `larmorx_core::json`, not yet on this branch).
Callers in the reach: `mri_label2vol`, `mri_label2label`, `mri_cor2label`, `mris_ca_label`,
`mris_label2annot`, `mri_segstats` (labels); `mri_normalize`, `mri_segment` and the control-point
options (`MRIreadControlPoints`; `MRIfillPoints` from `mri_edit_wm_with_aseg`'s path option);
`mris_anatomical_stats`, `mri_aparc2aseg` (annotation tables); `mri_segcentroids` (point sets).

### Mechanisms
- FS-423: `sprintf(area->space, "%s", str + strlen("vox2ras="))` into `char space[100]`; the bytes
  past it overwrite `avg_stat`, `vertex_label_ind`, `mri_template`, `mht`, `mris` (a text of about
  120 bytes or more makes `vertex_label_ind` a wild pointer that `LabelFree` passes to `free`).
- FS-424: `calloc(0, sizeof(LV))` returns glibc's minimum chunk; the loop's `nlines == n_points`
  never holds, so every line is stored at `lv[nlines]` past the chunk (measured: `malloc(): unsorted
  double linked list corrupted`, signal 6, when the next allocation meets the damaged heap).
- FS-425: the places are `calloc`'d for the count; the loop ends at the end of the file with
  `n_points` unchanged.
- FS-426: `calloc(n_points, sizeof(LV))` (and `LabelAlloc`'s, `LabelRealloc`'s `realloc`) of a count
  the machine cannot hold: `NULL` (an exit with the messages of §77/§78) or, under Linux's
  overcommit, a label that touches every page (`LabelAlloc`'s loop of `vno = -1`).
- FS-427: `char label_name0[STRLEN]` (`sprintf`), `lname[STRLEN]`, `subjects_dir[STRLEN]`
  (`strcpy`; `LabelReadFrom`'s `strncpy(subjects_dir, cp, STRLEN - 1)` leaves no NUL for 1,023 bytes
  or more), `LabelAlloc`'s `strcpy(area->subject_name, subject_name)`.
- FS-428: `LabelWriteInto` calls `fclose(fp)` before each `return (1)`; `LabelWrite` then calls
  `fclose(fp)` on the same stream (glibc: a double free of the `FILE`).
- FS-429: `LabelCopy`'s `memmove(adst->lv, asrc->lv, asrc->n_points * sizeof(LABEL_VERTEX))` into a
  given `adst` of fewer places.
- FS-430: `sscanf(cp, "%d %s %d %d %d %*d", ..., atable[i].name, ...)` into `char name[100]` (the
  bytes past it go into the next entry's `index`, `r`, `g`, `b`, `annotation`, which its own line
  then overwrites, or past the array for the last entry); `CTABcopyName(pct, i, atable[i].name,
  100)` is a `strncpy` that leaves no NUL for 100 bytes or more (the name then runs into the next
  entry's integers); `sprintf(fname, "%s/%s", ...)` into `char fname[STRLEN]`.
- FS-431: `points.erase(points.begin() + index)` with an index outside the vector.
- `LabelVoxelToSurfaceRAS` (not ported yet) converts with `MRISsurfaceRASToVoxel` when the label
  has a surface, the opposite direction of its name: to be decided with fs.volume's part.

### Behaviours kept (rule C, both modes; no ID)
- `LabelAlloc` with a subject name exits when `SUBJECTS_DIR` is not set although it does not use
  it; `LabelCopy` into a new label passes the source's subject (always a string, perhaps empty), so
  it exits likewise.
- `LabelReadFrom` returns `NULL` without freeing the label on its error paths (memory only);
  `LabelRead` prints `errno`'s text twice when the file does not open (`perror(NULL)`, then
  `ErrorPrintf`'s own).
- `LabelRemoveDuplicates` counts a point marked by two earlier points twice in its message.
- `read_annotation_table` and `read_named_annotation_table` leave the file open; a table of no
  entries is read again (with its message) by every lookup.
- `MRIreadControlPoints`' `ErrorReturn(NULL, (ERROR_BADPARM, "cannot open control point file",
  fname))` prints no name (the format has no `%s`).
- `fsPointSet::print` prints its row number `i`, which is never incremented: 0 on every row.
- `MRIwriteControlPoints`' format `... but %d\n` gives a blank line after the message.

### Measured (piece 11, 2026-11-13, the probe `validation/probes/fs-io/`, unit oracle's build)
- Seed 13, 2,678 piece-11 cases: `reproduce` identical on 2,594, 0 different (84 SKIP: FS-429 43,
  FS-430 19, FS-431 15, FS-423 3, FS-424 3, FS-426 2). Seed 7, 8,034: identical on 7,783, 0 different
  (251 SKIP). `fix` differs only where a defect is noted (FS-01's NUL byte, FS-12's clamped `nint`
  in `MRIfillPoints`, FS-425's short labels).
- Seed 1's light set with piece 11 (the recorded test, `HEAVY_SEED1` re-measured: [2248, 3687, 4361]):
  4,797 cases, 4,291 identical (51 with memory digits), 0 different, 506 SKIP or VARY
  (`48b6b766...`; with the portable `lrd` names below: `5f2d9dfd...`), debug 1.1 s; the NaN trap (AArch64's NaN rules) passes in debug (2,857 flips)
  and release (2,683). The new case stream met a debug-build overflow in piece 8's generator (a
  wrapped buffer length plus 8): now `wrapping_add`, the value a release build always gave.
- Windows (CI, 2026-11-14): the recorded hash differed on both Windows runners, and between them,
  because `mri_read` names a file without `/` by the working directory and a `/` (on Windows
  `C:\...\fix/f`), which the test's strip (the platform's separator, pieces 4 and 6 only) left in a
  Bruker check's message: the random temporary folder reached the hash. The strip now removes the
  fixture's path in every form (`cases::strip_fix`), for every piece. The portable `lrd` cases no
  longer ask for `w/a.LABEL` beside `w/a.label` (Windows' and macOS' file systems ignore case): the
  upper-case name is `w/b.LABEL` there; seed 1's light set re-recorded (`5f2d9dfd...`, the same
  4,291 identical and 506 SKIP or VARY). The test names its first differing case from per-case
  digests (`tests/fs_io/official_digests.txt`).

## Part 11: MRIread's and MRIwrite's GIFTI branches, MRIwrite, loadfsPointSet (relay 21 of track A)

Restricted (FreeSurfer's internals). Behaviour: `specs/freesurfer-io.md` §83-86.

### Mechanisms
- `mri_read` (mriio.cpp 783-784) calls `MRISreadGiftiAsMRI(fname_copy, volume_flag)` with the copy
  that has the working directory before a name without `/`; `MRIwrite` (1309) runs `chklc()`, then
  `mri_identify(fname)` (negative: `ErrorReturn(ERROR_BADPARM, ...)`), then `MRIwriteType` (1109); the
  directory check is `stat` + `S_ISDIR` (no readability test, unlike `fio_IsDirectory`), skipped for
  `MRI_CORONAL_SLICE_DIRECTORY`; `mriWriteGifti` at 1214-1215.
- FS-432: the vector branch builds each name with `sprintf(tmp, ...)` into `char tmp[1024]`; `ext` is
  `strrchr`'s result after `.gz` was cut with a NUL and put back, so the extension keeps `.gz`; a NULL
  `ext` prints `(null)` (glibc's `%s`).
- `errno`: `stat` of a name that does not exist sets 2 before the writer; `insertCommonMetaData`'s
  `getpwuid(geteuid())` ends with `errno` 0 (glibc's `getXXbyYY_r` wrapper sets `errno` to its status,
  0 for found and not found; measured with gdb on the probe: 2 at `mriWriteGifti`'s entry, 0 at
  `gifti_valid_gifti_image`). gifticlib's `fopen` of a file that does not open leaves its error (2).
  Both are set in `gifti_vol.rs` and `gifti_common.rs` (track D's code in this crate: the surface writers'
  `insertCommonMetaData` calls in larmorx-freesurfer-surf do not set it yet: track D's note).
- loadfsPointSet (pointset.cpp 74-92): `std::ifstream is(filename); is >> j;` (nlohmann 3.2.0's stream
  adapter: a stream that did not open reads EOF; a folder opens and its first `underflow` throws
  libstdc++'s `std::__ios_failure` with `errno`'s text), `ps.vox2ras = j["vox2ras"]` (non-const
  `operator[]`, `get<std::string>`), the range-for over `j["points"]` copying each point, `point["coordinates"]`
  copied, `ps.add(coord["x"], coord["y"], coord["z"], point["legacy_stat"])`: GCC 8.4.0 evaluates the
  four arguments right to left (measured: a point with a string `x` and an array `legacy_stat` throws the
  array's message), so `legacy_stat`, `z`, `y`, `x`. FreeSurfer's copy of json.h words `type_error.305`
  "with a string argument with" (json.h 14249). The probe's `CRASH 6` is the port's status 134.

### Measured (piece 12, 2026-11-14, the probe `validation/probes/fs-io/`, unit oracle's build)
- `mgi`, `mwr`, `lps` seed 13 (900 cases) and seed 7 (1,800): `reproduce` identical on 1,589 of seed
  7's, 0 different (210 counted: the writers not ported yet, FS-521's files mixing intents, FS-255);
  `fix` differs only with notices (FS-433, FS-01, FS-522). Track D's `larmorx-freesurfer-surf` tests
  (`fs_surfio_official`, `gifti_writers`) unchanged with the errno changes.

## Part 12: the label coordinate conversions and MRIwriteFrame (relay 22 of track A)

Restricted (FreeSurfer's internals). Behaviour: `specs/freesurfer-io.md` §87-89. The port:
larmorx-freesurfer-volume's `label_coords.rs` and `io_frames.rs` (track A's modules there: they need
fs.volume's matrices and `MRIcopyFrame`; the orchestrator's layering of 2026-11-14).

### Mechanisms
- `LabelToScannerRAS` (label.cpp 3111-3145) initialises `M_surface_to_RAS = RASFromSurfaceRAS_(mri,
  NULL)` at its declaration, before `LabelCopy`, and assigns it again at 3124 after the voxel step (the
  first matrix leaks); the loop runs over `lsrc->n_points` reading and writing `ldst->lv[i]`.
  `LabelToSurfaceRAS` (3150-3193) also makes the matrix at its declaration, before the switch, so a
  label already in tkregister RAS or in voxels still pays for it (and ends the program on a singular
  tkregister vox2ras); its `default:` is `ErrorExit(ERROR_UNSUPPORTED, ...)`, so `LABEL_COORDS_NONE`
  (0) ends the program. `LabelFromScannerRAS` is a macro for `LabelToSurfaceRAS` (label.h 78).
  `LabelToVoxel` (3198-3236) calls `voxelFromSurfaceRAS_(mri)` after the scanner step and `nint` of
  each `V3_X(v2)` (float promoted to double). `LabelVoxelToSurfaceRAS` (3285-3305) reads `lsrc->lv[i]`
  and calls `MRIvoxelToSurfaceRAS` per point (each call makes `surfaceRASFromVoxel_` anew).
- FS-437: `LabelVoxelToSurfaceRAS` calls `MRISsurfaceRASToVoxel((MRIS *)lsrc->mris, mri, ...)`
  (mrisurf_mri.cpp 7199-7247) for a label with a surface: that function maps surface RAS to voxels,
  the opposite of what the caller (voxels to surface RAS) needs; `MRISsurfaceRASFromVoxel`
  (7473-7530, the same matrices and their inverse) is the intended one. Rule A: the default calls
  the latter. Both use only `mris->vg` (`MRIallocHeader(vg.width, vg.height, vg.depth, MRI_UCHAR, 1)`
  + `MRIcopyVolGeomToMRI` when `vg.valid`, else `mri`), so the port takes the surface's `VolGeom`
  (`Label::mris` is an opaque handle in larmorx-freesurfer-io); their static per-thread vectors
  change nothing visible.
- FS-438: `MRIwriteFrame` (mriio.cpp 1297-1308) ignores `MRIwrite`'s return value and returns
  `NO_ERROR`; callers that test it (mri_cal_normalize.cpp 414) cannot see a failed write. Rule A.
  A negative frame is not checked: `MRIcopyFrame` reads it through `MRIgetVoxVal` (fs.mri's notices).

### Measured (piece 13, 2026-11-14, the probe `validation/probes/fs-io/`, unit oracle's build)
- `lcv`, `mwf` seed 7 (1,000 cases): `reproduce` identical on 947, 0 different (53 counted: 30 writers
  not ported yet, 23 FS-429); seed 13 (3,000): 2,823 identical, 0 different (177 counted); `fix` differs
  only with notices (FS-437, FS-438, FS-01, FS-12, FS-522). In the fsio harness the surface is
  `MRISalloc(0, 0)` with its `vg` read from the case.

## Part 13: MGH and MGZ written (relay 23 of track A)

Restricted (FreeSurfer's internals). Behaviour: `specs/freesurfer-io.md` §90-92. The port:
larmorx-freesurfer-io's `mriio_write.rs` (`mgh_write`, `mri_tag_write`), wired into `mriio.rs`'s
`mri_write_type`.

### Mechanisms
- `mghWrite` (mriio.cpp 11247-11518) writes each number with `znzwriteInt`/`znzwriteShort`/
  `znzwriteFloat` and ignores their counts; only the `MRI_UCHAR` row (`znzwrite(&MRIseq_vox(mri, 0, y,
  z, frame), 1, width, fp) != width`) is checked. Its `default:` (`MRI_LONG`, `MRI_RGB`, `MRI_TENSOR`,
  `MRI_BITMAP`, `MRI_DOUBLE_COMPLEX`) and that row's error `ErrorReturn` without `znzclose(fp)`
  (FS-439): the `gzFile`'s input buffer (8 KiB in zlib 1.2.11's `gz_init`) still holds the 284-byte
  header, never compressed (the file `gzopen` created stays empty); a plain `FILE` keeps it in its
  buffer until `exit` flushes the streams (the probe calls `fflush(NULL)` after the write, which shows
  the same state). `znzclose(fp)`'s result is ignored (FS-440).
- `FS_MGZIO_USEVOXELBUFWRITE` (11357-11427): `voxelbuf = &MRIseq_vox(mri, 0, 0, 0, start_frame)` =
  `slices[start_frame * depth][0]`; `byteswapbufshort`/`byteswapbuffloat` (bfileio.cpp 81-117) swap
  `bytes_per_slice` bytes of the volume's own chunk in place before each `znzwrite` (FS-443: never
  swapped back). `bytes_per_voxel == 8` (`MRI_LONG`, `MRI_FLOAT_COMPLEX`) is swapped in 4-byte
  groups. `MRI::initSlices` (mri.cpp 260-305) advances the row pointer for uchar, short, ushort, RGB,
  int, long, float and (by `bytes_per_vox * vox_per_row`) float complex only: a tensor's rows all point
  at the chunk's start, so the write starts there whatever the frame. With `depth * nframes == 0`,
  `slices` is `calloc(0)` and `slices[0]` is read past it (FS-445: SIGSEGV in the probe).
- `MRI::ischunked` is an `int` without an initialiser (mri.h 826) and the constructor returns before
  setting it when `alloc` is false (mri.cpp 231): a header-only volume's flag is heap contents
  (FS-446; in the probe it was non-zero, and the voxel-buffer branch then crashed on `slices`, NULL).
- `MRITAGwrite` (12421-12534) tests `mri->frames[0].label` and `FZERO(mri->frames[0].thresh)`
  (`fabs(f) < FLT_EPSILON`, macros.h 80) without checking `nframes`: `frames` is `calloc(nframes,
  sizeof(MRI_FRAME))` (mri.cpp 211), past its end for 0 frames (FS-441). The `#if 0` block of
  `tag_data` is not compiled.
- With a chosen frame (`frame >= 0`) the header's `nframes` is still `mri->nframes` (FS-442); no
  FreeSurfer caller passes a frame (MRIwriteType, MRISurfOverlay.cpp 195: -1). A frame at or beyond
  `nframes` reads beyond the chunk through `MRISseq_vox` (FS-444).
- `exec_progress_callback` (utils.cpp 1551) does nothing without `progress_callback` (only freeview
  sets one).

### Found while comparing (the probe's other pieces)
- The tag readers' `znzreadInt` at the end of the file was noted as FS-167 (memory contents) although
  `znzTAGreadStart` (tags.cpp) and `FStagsIO::read_tagid_len` return 0 at `znzeof` and never use the
  number: 139 of 588 `mgw` cases were marked VARY for it. Without the spurious notice four reader
  differences showed (fixed): `mghRead`'s header-alone skip of a negative byte count (an unknown type's
  size -1) is `znzseek(fp, total_bytes, SEEK_CUR)`, glibc's `fseek` failing with EINVAL, which
  `mri_read` then leaves in `errno`; `FileExists("")` (utils.cpp 445: `fopen("", "r")`, ENOENT) for an
  empty `TAG_MGH_XFORM` name, where the port's `resolve(dir, "")` opened the folder.
- gifti.cpp 2950 (`readLabelTable`) `malloc`s each colour entry without setting `TissueType` and
  `count` (heap contents: in one seed-13 case the bytes of the XML text, `959723828`); the probe's
  `mgi` now leaves them out on both sides, as for `CTABalloc`'s entries (track D's reader: a notice to
  add there).
- Windows: `minc::input_transform_file` joined the program's folder to a relative name and volume_io's
  `expand_filename` (in `reproduce`, FS-79) took the folder's backslashes for escapes, twice (the
  existence check and the open both expand): a label's `talairach.xfm` was not found (seed 1's case
  1194). The folder is now given with `/`.

### Measured (piece 14 and the re-runs, 2026-11-14, the probe, the unit oracle's build)
- `mgw` seed 7 (588 cases): `reproduce` identical on 495, 0 different (69 SKIP, 24 VARY); seed 13,
  10,294 cases of `mgw`, `mwr`, `mgr`, `mrd`, `mrh`, `mri`, `mgi`: identical on 8,056, 0 different
  (1,838 SKIP, 400 VARY); seed 7, 12,000 cases of every command: 10,807 identical, 0 different; piece
  13 seed 13, 3,000: 2,930 identical, 0 different (70 SKIP: FS-429); `fix` differs only with notices.
- The recorded tests re-recorded: `fs_io_official` 9e99627e... (4,799 cases, 4,327 identical, 472
  counted), `fs_io13_official` 3dbe8afd... (1,200, 1,174 identical, 26 counted); both pass under the
  ARM NaN emulation (debug and release) and on Windows x64 (the cross-check tool on the Windows
  laptop; seed 1's set compared case by case: only the probe's relative-folder `bvw` artefact).

### NIfTI (`niiWrite`, mriio.cpp 9522-9760; same relay)
- `niiWrite` reshapes an ico7 overlay with `mri_reshape(mri0, 27307, 1, 6, nframes)` (mri2.cpp 653-735:
  `MRIallocSequence`, `MRIcopyHeader`, `MRIcopyPulseParameters`, a voxel copy whose `switch` has only
  uchar, short, ushort, float, int and long) and writes, and changes the intent of, the copy; the
  port's `reshape_ico7` (mri2.cpp is fs.volume's: written here for this one call).
- `mriToNiftiQform` (10245-10398) works in `float r11..r33, a, b, c, d, xd, yd, zd, r_det` with
  `double` literals (`1.0`, `0.5`, `0.25`) and `sqrt` (double): each statement rounds to float at the
  assignment; `qfac` starts at -100000 and stays there when `r_det == 0.0` (only a printed warning).
  `extract_i_to_r` gives the offsets; `mriToNiftiSform` (10229) copies `MRIxfmCRS2XYZ(mri, 0)`.
- `hdr.dim[2..4]` are `short`s: the checks `mri->height > shortmax` (and depth, frames) with
  `shortmax = (int)pow(2, 15)` let 32,768 through, stored as -32,768 (FS-449). `hdr.dim[1]` for 32,768
  columns and more is -1 with the count in `glmin` (FreeSurfer's own convention, its reader's).
- `MRI_LONG` maps to `DT_SIGNED_INT`, `bitpix` 32, and the data loop writes `znzwrite(buf, bitpix / 8,
  width)` from `&MRIseq_vox(mri, 0, j, k, t)`: the first `4 width` bytes of a row of `8 width` (FS-447).
- `sprintf(hdr.descrip, "FreeSurfer %s", __DATE__)`: `Mar 14 2026` in the official 8.2.0 binaries
  (`strings` of `bin/mri_convert`) and in the unit oracle's `libutils.a` alike.
- `__getMRITAGlength` (12538-12731) mirrors `MRITAGwrite`'s order with `FStagsIO::getlen_*` and the
  same `frames[0]` test (FS-441); `__writeFSniiextensionHeader` (12865-12899) assigns the `unsigned
  short` intent to `mri->intent` (an intent of 65,539 becomes 3 there, where `mghWrite` keeps 65,539).
- `znzclose(fp)`'s result is ignored (FS-448); the header, the extension flag and each row's
  `znzwrite` are checked (`ErrorReturn` after `znzclose`). The ErrorReturns for the types do not set
  `errno = 0` first.
- Measured: seed 13, 3,920 `mgw`/`mwr` cases (788 NIfTI names, 41 ico7, 43 of 32,768 rows): reproduce
  identical on 3,603, 0 different. Three differences before the FS-58 checks: header-only volumes
  written as NIfTI (and one reshaped) crash FreeSurfer at the first row (`MRIseq_vox` on `slices`,
  NULL).

