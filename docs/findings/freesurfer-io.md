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
