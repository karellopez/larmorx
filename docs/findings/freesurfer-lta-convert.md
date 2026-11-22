# FreeSurfer's `lta_convert`: findings of the replica (queue item 3.2-09)

**Closed to FreeSurfer clean-room implementers** (it names `lta_convert.cpp`'s code; the behaviour
is `specs/lta_convert.md`). Track A, relay 51 of FreeSurfer's foundations, 2026-11-22. Every
statement measured on the official program (`validation/probes/fs-ltaconv/`) or read in
`lta_convert/lta_convert.cpp` and `utils/transform.cpp` of FreeSurfer 8.2.0.

## The program

`lta_convert` reads one transform (seven formats), converts it to `LINEAR_RAS_TO_RAS`, optionally
changes the source's and target's geometries (conform, up/downsample), inverts it, and writes it in
any of seven formats. Its `.lta`, FSL, `.xfm` and `register.dat` paths are the library's
(`LTAreadExType`, `LTAchangeType`, `LTAprint`, `LTAwrite`); its NiftyReg, ITK and voxel-space text
is read and written in the program with C++ streams, whose number formats decide the output bytes
(`std::ostream` `precision(7)` and `precision(17)`: `%.7g` and `%.17g` of the `float` matrix
element widened to `double`) and whose extraction rules decide what a short or damaged file gives
(`std::ifstream >> float`: libstdc++'s `num_get`, a failure stores 0 and sets `failbit`, every later
extraction then takes nothing and leaves its variable).

recon-all's call (step 4, `--inlta identity.nofile --src rawavg.mgz --trg orig.mgz --outlta
rawavg2orig.lta --subject <s>`) and talairach_avi's (`--inxfm ... --ltavox2vox`), fMRIPrep's and
sMRIPrep's (`--inlta <lta> --outfsl`, `--inlta identity.nofile --src --trg --outlta`, `--inlta
<lta> --outlta`, bbregister's `--outreg`) reach none of the defects below.

## The defects (FS-1159 to FS-1170)

- **FS-1159** (a: crash). `parseNextCommand` reads `argv[1]` without checking `argc`: the options
  that take a name build `std::string(argv[1])` from the `NULL` after the last argument (libstdc++
  throws `std::logic_error`, `basic_string::_M_construct null not valid`, uncaught: abort, status
  134, measured), the sampling factors call `atoi(NULL)` and the conform sizes `atof(NULL)`
  (SIGSEGV, 139, measured). What `cout` had flushed stays on standard output.
- **FS-1160** (e). `SRCCONFORM-SIZE` and `TRGCONFORM-SIZE` read `atof(argv[1])` but leave `nargs`
  at 0: the loop then meets the value, which does not start with `-`, and stops (FS-1170). Measured:
  `--srcconform-size 2 --outlta x.lta` writes nothing and says `successful`. Rule A: the value is
  stepped over by default.
- **FS-1161** (c). `readITK`'s loop is `while (!transfile.eof())`: after the last line's numbers the
  new line is not read, so `eof()` is false; `transfile >> str` then fails at the end (the sentry
  sets `eofbit|failbit`) and **leaves `str` as it was** (libstdc++ erases the string only after a
  good sentry), so the last section's branch runs again with every extraction failing: a 3D
  `FixedParameters:` adds `f - R f` to the translation a second time. Measured on
  `Parameters: 0.9 0.1 0 -0.1 0.9 0 0 0 1 1 2 3` / `FixedParameters: 10 20 30`: with the final new
  line the `.lta`'s translation is `(-2.073171, 8.658538, -3.0)`, without it `(-0.609756,
  5.487805, -3.0)` (the second is the correct one). A rotation of identity (`R = I`) or a centre of
  0 hides it, which is why ITK files written by `lta_convert` itself (`FixedParameters: 0 0 0`)
  round-trip. A last `Parameters:` rewrites the matrix with the last three numbers read (the
  translation); a last `Transform:` reads the type word again (failing: `Transform:` stays) and
  ends as `Transform type unknown`; a last comment leaves the comment in `str` (from `getline`) and
  ends as unknown. Rule A.
- **FS-1162** (a: hang). A number that does not convert sets `failbit` without `eofbit`: `peek()`
  returns the end, `>> str` takes nothing, the same branch runs again, for ever (measured: the
  official program never ends, killed by the probe's 15 s limit). A `Transform:` or unknown branch
  ends instead (its message), and a 2D `FixedParameters:` with a value not 0 ends with its error.
- **FS-1163** (a: memory contents; c). The NiftyReg and voxel-space loops assign the row from
  `v1..v4` whatever the extraction did: after a failure the row repeats the values read before;
  `float v1,v2,v3,v4;` are not initialised, so a file whose first number fails (empty: measured,
  the matrix singular, then FS-1164's crash) uses registers or stack. `readITK`'s `v1, v2` likewise
  (`v3 = 0.0`). The port marks the memory-contents variant (`MEMORY` in the probe: `SKIP`) and
  reproduces the kept values. Rule B by default.
- **FS-1164** (a). `lt->m_L = MatrixInverse(m_L, lt->m_L)` (NiftyReg, ITK), `MatrixInverse` in
  `--invert`, `writeNIFTYREG`/`writeITK`/`writeVOX`: a singular matrix gives `NULL`; `MatrixPrint`
  then says `mat = NULL!`, `LTAprint` (and the writers' element access) dereference it: SIGSEGV
  after `fopen` created the file (measured: `nrsing.txt`, empty file left, status 139). `readVOX`
  ignores `MatrixInverse(mat, lt->m_L)`'s result: the array keeps the identity or the partial
  inverse. Rule B.
- **FS-1165** (a). `readFSL` and `readREG` do not check `LTAreadExType`'s `NULL`; `getVolGeom(src,
  &lta->xforms[0].src)` after both headers dereferences it (SIGSEGV). An `.lta` with `nxforms 0`:
  `lta->xforms[0]` past a zero-length array.
- **FS-1166** (a, c). `downsampleGeom` computes `width % N` (SIGFPE for 0, status 136, measured)
  and on a size `N` does not divide prints `ERROR: MRIdownsampleN: ...` and returns: the program
  goes on with the geometry unchanged and ends with status 0 (measured: `--srcdownsample 3` of a
  4-voxel width). `upsampleGeom` with 0: `1.0 / (2 * 0)` is infinite, sizes 0. Rule B.
- **FS-1167** (a). `conformGeom` with `-dc` looks for `L`/`R`, `I`/`S`, `A`/`P` in the orientation
  string; one not found leaves its index at 3 and reads `delta[3]` and `FoV[3]` past their arrays
  (`MRIdircosToOrientationString` gives `???` for a geometry whose `ras_good_flag` is 0, as
  `identity.nofile` without `--src` has, and two axes can share a letter pair at 45 degrees).
- **FS-1168** (a). `strcpy(lta->subject, P.subject.c_str())` into `char subject[STRLEN]`, the last
  member of the heap-allocated array (not fortified): 1,030 bytes write past the allocation without
  a visible effect; 4,000 bytes crash (139), 100,000 abort (`malloc(): invalid size (unsorted)`,
  134), measured. The port keeps the whole subject.
- **FS-1169** (c). `MatrixSVD(m_L, v_z, m_V)` runs `OpenSvdcmp` on `m_L` itself, which becomes `U`,
  and returns `m_V` (so `m_U` is `m_V`: printed twice). The `.lta` is written before `--sqrt`, every
  other output after it, with `U`. Rule A.
- **FS-1170** (e). `parseCommandLine`'s loop stops at the first argument not starting with `-`
  and returns `true` without looking at what is left (measured: `foo --inlta ...` reads no input,
  `can't read input file `, status 251).

## Messages worth knowing

- `readLTA`'s target message is `ERROR readFSL: cannot read trg MRI<name>` (copied from `readFSL`);
  `writeMNI`'s failure says `ERROR writeFSL: ...`; `--trgconform` prints one colon, the other
  conform options two. All kept.
- `getVersion()` (`8.2.0`) and an empty line open every run's standard output, before the version
  options' own lines.
- The source and target headers: `readLTA` assigns the whole header (`lta->xforms[0].src = *src`:
  `VOL_GEOM`'s assignment, the cached matrices included, FS-59's rule) and sets `valid`; the other
  readers use `getVolGeom` (the fields, `valid = 1`). The file name in the geometry is the absolute
  one `MRIreadHeader` makes (a relative name gets the working folder).

## The oracle-output cache and output files (relay 51)

The cache replaces this run's places (the working folder, the temporary folders) in the streams it
stores and serves, but stores the output files as they are: an `.lta` written from relative volume
names holds the working folder's absolute name (`filename = /.../case-92/s.mgz`), and a case served
from an entry another folder stored would hold that folder's name (seen in the probe's first run:
6 cases). The probe runs every case in one folder (its layout keyed as an input, `layout.txt`, so
the earlier entries are not served); other probes whose programs write absolute names into their
outputs may want the same, or the cache could map its places in output files as in the streams.

## The cache key re-check (relay 51, item 1)

`docs/findings/freesurfer-add-xform.md` (its last sections) and `docs/validation/oracle-cache.md`:
every FreeSurfer program of larmorx-freesurfer is in `EXTENSION_FROM_WHOLE_PATH`; 4,516 official
runs of the FreeSurfer probes re-checked, 71 confusable, no result changed.
