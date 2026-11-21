# FreeSurfer's `mri_binarize` (8.2.0): findings of the replica

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; the clean room
works from `specs/mri_binarize.md`). Track D, relay 48 (2026-11-21), queue item 3.2-19.

## 1. What the port is

`mri_binarize/mri_binarize.cpp` translated whole into `larmorx-freesurfer-volumes`
(`src/mri_binarize/`), its library calls on the ported modules (fs.io, fs.mri, fs.volume, fs.filter,
fs.surf), and two pieces it needs from later modules taken early into `larmorx-freesurfer-deform`
(`src/tess.rs`): `MRIStessellate` with its `TESS*` helpers (`utils/mrisurf_timeStep.cpp`, fs.deform,
queue item 3.4-06) and `MRIvolTopoFix` (`utils/mri2.cpp` 5752, fs.volume's code, placed above the
tessellation it calls). `RFglobalStats` (`utils/randomfields.cpp` 313, reached only by `--rmin` and
`--rmax`) is in the program's file: fs.core's `randomfields.rs` cannot read volumes. Nothing it
reaches waits for a later module. Every option of the parser (the card's list) is handled; the
second `--erode` branch of FreeSurfer's parser is unreachable (the first takes `--erode` as
`--erode-corner`), and `print_version` only through `--version`, which the version options take
first.

## 2. The defects (rules A to D)

- **FS-1018** (class b): the binarisation loop is `#pragma omp parallel for ... reduction(+ :
  nhits)` over columns, but the match loop's index `n` is `main`'s variable, shared by the threads
  (`int nargs, c, nhits, n, ...`; only `r`, `s`, `mergeval`, `maskval`, `val` and `Matched` are
  declared inside). With more than one thread a thread's `for (n = 0; n < nMatch; n++)` sees the
  others' `n`: a voxel can skip match values or test entries past `nMatch` (the global array's
  zeros: a voxel of 0 can match). Races are never reproduced: larmorx computes as one thread in both
  modes and notes FS-1018 when FreeSurfer would use more (`--threads` above 1, or
  `OMP_NUM_THREADS` above 1 or unset: OpenMP's default is the machine's cores, and
  `if_ROMP(assume_reproducible)` lets the loop run in parallel). The count loop's reduction is
  correct.
- **FS-1019** (class f, rule A): `--rmax` prints `Setting max thresh to %g` with `MinThresh`.
- **FS-1020** (class a, rule B): `MRIfdr2vwth`'s failures (a non-`float` input, no voxel) return
  before `*vwth` is set; `main` ignores the return and prints and uses the uninitialised local
  `FDRThresh`. `reproduce` takes 0 (the defined parts: the messages).
- **FS-1021** (class a, rule A): `MRIvolTopoFix` returns `binseg0` itself when its tessellation's
  Euler number is 2 already (`if(eno == 2) return(binseg0);`); `main` then `MRIfree(&OutVol)` and
  `OutVol = tmpvol`: the freed volume is written (and tessellated). Observed: a segmentation fault
  for the 16^3 cube and the ds000005 v8 hippocampus (after writing a volume), status 252 for two
  other v8 hippocampi (a write that failed on freed memory). larmorx keeps the volume; `reproduce`
  ends with status 139.
- **FS-1022** (class c, rule A): `MRIvolTopoFix`'s erosion loop protects the original voxels with
  `for(int k=1; k < crs.size(); k++)`: the first set voxel (columns outermost) is not protected and
  can be eroded away. Measured: the 20^3 block with a tunnel (`validation/investigations/
  freesurfer-mri-binarize-probe/`'s `torus_20.mgz`) loses voxel (4, 5, 3), its first; on real
  data, ds000005's classic hippocampus (`real.py`). larmorx protects every voxel (`reproduce` from
  the second) and notes FS-1022 when an erosion removed the first.
- **FS-1023** (class a, rule A): `MatchValues`, `SrcReplace`, `TrgReplace`, `SrcReplaceNN` and
  `ReplaceWindowNN` are `int[1000]` globals filled without a bound. Measured with `--match 1 2 ... n`
  on the official binary: up to 1,006 values run normally, 1,008 overwrite `frame` (`ERROR:
  requested frame=1007 >= nframes=1`: entry 1007 lies on `frame`), 1,012 and more the threshold
  flags (`ERROR: cannot specify threshold and match values`); `--match-ctab` with
  `FreeSurferColorLUT.txt` (more than 1,000 valid entries) crashes without output. The overwritten
  globals are the binary's layout: not reproduced; larmorx keeps every entry in both modes.
- **FS-1024** (class f, rule A): `--dilate-vertex` with fewer than four arguments prints the usage
  line of `--dilate-vertex-sum`.
- **FS-1025** (class a, rule B): `MRIremoveVolumeIslands` and `MRIremoveVolumeHoles` return `NULL`
  for `nKeep == 0`, and `main` passes `BinVal` as `nKeep`: with `--inv`, `--gm` or `--binval 0` the
  output becomes `NULL` and a later step dereferences it (the next of these, `MRIwrite`, the count's
  `OutVol->width`). `MRIvolTopoFix` gives `NULL` after more than 1,000 dilations (a binarisation
  without voxels never reaches Euler number 2: an empty surface has 0); in practice FS-1027 ends it
  first. larmorx ends with a message (status 1); `reproduce` with status 139.
- **FS-1026** (class a, rule B): the stand-alone options read numbers into uninitialised locals
  (`rasCenter[3]`, `voxFoVRAS[3]`, `int id` in the keep loop, `int vno`, `double radius`, `double
  targetSum`; `--match-ctab`'s `int xid`) with an unchecked `sscanf`. `reproduce` takes 0.
- **FS-1027** (class a, rule A): `MRIStessellate` `lcalloc`s its working lists at their limits
  (`MAXFACES` = 20,000,000 `tface_type`s of 36 bytes, two face tables of `600 * height * width`
  ints, `MAXVERTICES` = 10,000,000 `tvertex_type`s of 52 bytes, a vertex table of `800 * height *
  width` ints: about 1.25 GB of address space for a 32^3 volume) and frees none. Only the touched
  pages cost memory, so one tessellation is cheap, but `MRIvolTopoFix` tessellates at every step:
  the address space or a memory limit runs out, and `lcalloc`'s `MGHprint_error` prints `Cannot
  calloc()` and calls `exit(0)` (status 0, nothing written). Observed for the empty binarisation:
  after 55 dilations under the oracle-output cache's runner, a crash after 865 in a 4 GiB box.
  larmorx's lists grow with what they hold and are released; FS-1027 is noted from the 32nd
  tessellation of a fix (so the probe counts such runs, not compares them).
- `--copy <volume>` (no ID: rule C, documentation only): the volume is read and its dimensions
  checked (with the merge volume's message), then never used; the help says it copies values.
  Kept in both modes.

## 3. Notes on the translation

- The binarisation's `double mergeval = BinValNot` is reset only once per column (it is declared
  in the column loop), so with a merge volume each voxel overwrites it and without one it stays
  `BinValNot`: the same in the port.
- `MRIframeSum(InVol, InVol)` writes each voxel's sum into frame 0 of the input itself (the input's
  type: a `uchar` sum saturates at 255) and keeps its frames; `--frame-and`'s minimum is
  `InVol->nframes - 0.5` after it, the frame count unchanged. `MRImultiplyConst(InVol, -1.0, InVol)`
  writes in the input's type. Both are loops on the input in the port (no copy).
- `--replace` without `--replaceonly` discards the binarisation (`OutVol = MRIreplaceList(InVol,
  ...)`), while the first count printed is the binarisation's; the second count is the replaced
  volume's voxels equal to `binval`. FreeSurfer's behaviour, kept.
- The nearest-neighbour distance is `sqrt((float) dc * dc + dr * dr + ds * ds)`: C's left-to-right
  sum, a `float` plus each `int` square (exact for any window FreeSurfer can allocate).
- `getcwd(cwd, 2000)` into a zeroed global: a longer path leaves `cwd` empty.
- The tessellation's face tables persist across slices (`face_index_table1` is never cleared, and
  becomes `face_index_table0` after each slice), and `TESScheckFace` assigns corners `n` of 1 to 3
  through whatever face number the table holds at that place, a stale one included; the port
  keeps both tables as they are (six planes of `height * width`, all FreeSurfer indexes). The
  vertex table `TESSaddVertex` fills is never read and the `tvertex_type`'s face list is never
  used: neither is kept.
- `mri_normalize`'s writes (relay 47) passed an intent of 0 to fs.io's `MRIwrite` where FreeSurfer
  calls `MRIwrite(mri, name)` with its default `MGZ_INTENT_UNKNOWN` (-1); with 0 the MGH writer
  resets the volume's intent and version to the MRI ones (no change for every recorded and probe
  input, which have them). Corrected to -1 (`main.rs`, `helpers.rs`, fs.filter's `normalize.rs`):
  its hash tests unchanged.

## 4. Validation

`docs/validation/freesurfer-mri-binarize.md`: the open probe's 130 command lines (seeds 1 and 2),
the real calls on the recorded heads (`validation/probes/fs-binarize/real.py`: 81 calls on 6 runs),
the golden cases and the memory entry.
