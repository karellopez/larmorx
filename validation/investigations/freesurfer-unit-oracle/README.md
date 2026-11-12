# FreeSurfer 8.2.0's unit oracle

**Closed to FreeSurfer clean-room implementers**: these scripts build FreeSurfer's source, mirror
its build files and name its internal functions. (Running the built harness is a black-box run of
FreeSurfer's code, like running one of its programs; reading these files is not.)

Queue item 3.1-02 (`docs/freesurfer/queue.md`). FreeSurfer's programs are large; a ported
function is easier to check alone. The unit oracle is FreeSurfer 8.2.0's `utils` library and the
packages it links (`jpeg`, `tiff`, `xml2`, `minc`, `netcdf`, `nrrdio`, `dicom`, `cephes`, `nifti`,
`gifti`, `expat`, `dcm2niixfs`), built from the tag exactly as the official 8.2.0 build built them,
against ITK 4.13.2, with a small program, `fsunit`, that calls one library function on given
inputs and writes its outputs. Tracks A (the replicas) and C (the permissive pieces) call it from
Python, through the oracle-output cache, to check a port function by function.

## What it is, and the proof that it is the official build

The official programs, rebuilt from the same objects, are **the official files**:

| Program | Instructions (ours, official) | Identical, addresses included | The whole file, ours stripped as the official is |
|---|---|---|---|
| `mri_add_xform_to_header` | 552,067 / 552,067 | all | identical apart from the 20-byte build ID |
| `mri_normalize` | 632,027 / 632,027 | all | identical apart from the 20-byte build ID |
| `mris_smooth` | 615,401 / 615,401 | all | the build ID, and one unwind-table entry of ITK's `vnl_real_eigensystem` (its CIE not merged with the others; ITK's objects were assembled on Ubuntu 20.04 in the official build, ours on 22.04: no instruction differs) |

Every read-only and data section is byte-identical, the static constructors run in the same order,
and the three programs give the recorded outputs on the reference runs' steps (below). `code_identity.py`
measures it (four views, strictest first: the files, the sections, the instructions with and
without their addresses, the data words and the constructors).

What made it identical (each read from the official package, never guessed):

- **The compiler is the official one**: Ubuntu 20.04's `gcc-8` 8.4.0-3ubuntu2 (every official
  binary's `.comment` names it), from Ubuntu's archive: Ubuntu's signing key verifies
  `dists/focal/InRelease`, which lists the SHA-256 of the package indexes, which list each `.deb`'s
  (`build.py`, `UBUNTU_DEBS`; pinned too). Unpacked without administrator rights into
  `<workspace>/oracles/toolchains/gcc-8.4-ubuntu`; the machine's own binutils 2.38, glibc 2.35
  headers and libraries (Ubuntu 22.04, as the official build had: its programs import
  `exp@GLIBC_2.29`, `__libc_start_main@GLIBC_2.34`, and load the machine's `libstdc++`, `libgomp`,
  `libgcc_s` at run time).
  The preparation's conda-forge GCC 8.4.0 (`--compiler conda`, behind wrappers that give it the
  machine's headers and libraries and Ubuntu's default hardening flags) gives the same instructions
  too, but its own start files (`crtbeginS.o`, `crtendS.o`: 62 instructions of GCC's
  `frame_dummy`, `register_tm_clones`, ... in each program) differ and it is configured without
  `.init_array` support: constructors go to `.ctors`, and the linker runs `romp_support.cpp`'s two
  (its `before_main` and its static initialisation) in the other order. Ubuntu's package has
  neither difference, so it is the default.
- **ITK 4.13.2 as FreeSurfer's `packages/source/build_itk.sh` builds it**: the release tarball
  (SHA-256 `d8760b27…`), static, Release, `-msse2 -mfpmath=sse`, C++11, the six module groups and
  AnisotropicDiffusionLBR (fetched by ITK's configuration from GitHub at its pinned commit).
- **FreeSurfer's own CMake files**: `top.cmake` sets, in the order and with the values of the tag's
  top-level `CMakeLists.txt` (its line numbers in the comments), what that file sets for these
  targets on the official machine (`HOST_OS` Ubuntu22, GCC 8, `-Werror` and the warning flags,
  `-fno-builtin`, OpenMP, the definitions, the strip flags, `FS_INCLUDE_DIRS`, the run path), then
  adds the tag's `packages/`, `utils/` and programs' folders unchanged, through a tree of links
  (`src/fs-shadow`). The official run path `$ORIGIN/../lib/vtk:$ORIGIN/../lib/qt/lib:` is kept:
  it decides the size of `.dynamic`, and so the address of every global the code loads.
- **The texts compiled in**: `__FILE__` (FreeSurfer's and ITK's messages print their source
  paths: `-fmacro-prefix-map` maps ours onto the official build's folders, read from an official
  program's strings at build time and never written into the repository), `__DATE__` and
  `__TIME__` (`SOURCE_DATE_EPOCH` = 2026-03-14 12:21:06 UTC, the official `version.cpp`'s build
  time: `MRIwrite` writes `FreeSurfer Mar 14 2026` into every NIfTI header's `descrip`), the
  version `8.2.0` and the build stamp of the official package.

## Building it

```
scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-unit-oracle/build.py all --jobs 10
```

About 15 minutes at 10 jobs (ITK 4.13.2 is most of it), under `<workspace>/oracles/freesurfer-8.2.0-unit/`
(never the repository): `toolchain/`, `src/`, `itk-4.13.2/`, `build/` (FreeSurfer's build tree,
`compile_commands.json`), `bin/` (`fsunit`, the three programs, `BUILD_INFO.txt` with every
library's and program's SHA-256). Logs: `<workspace>/oracles/logs/freesurfer-unit-*.log`. Steps:
`toolchain`, `itk`, `freesurfer` (each re-runnable); `--compiler conda` for the preparation's GCC.

## The harness

`fsunit <command> <arguments>` prints one JSON object on standard output; every floating-point
value both as C99's hexadecimal form (`%a`, exact) and as `%.17g`. Matrices go in and out as text:
`rows cols`, then the rows, each value in any form `strtof` reads (hexadecimal keeps a float exact).

| Command | Calls |
|---|---|
| `version` | `getVersion`, `getBuildStamp`, the compiler |
| `mri-read-write <in> <out>` | `MRIread`, then `MRIwrite` (the reader and writer chosen by the names, as FreeSurfer chooses them); the header as read |
| `mri-header <in>` | `MRIread`: type, sizes, voxel sizes, direction cosines, centre, TR, TE, TI, flip angle, `vox2ras` |
| `matrix-inverse <in> <out>` | `MatrixInverse` (float `MATRIX`; 4 x 4 and smaller through VNL's closed forms, larger through `vnl_qr`); `null` where it returns NULL |
| `matrix-multiply <a> <b> <out>`, `matrix-determinant <in>` | `MatrixMultiply`, `MatrixDeterminant` |
| `vnl-random <seed> <count> <method>` | `vnl_random(seed)` then `count` draws: `lrand32`, `drand32`, `drand64`, `normal`, `normal64` |
| `open-ran1 <seed> <count>` | `OpenRan1(&seed)` `count` times (FreeSurfer's generator: `drand64(0, 1)` as float) |
| `fs-random <seed> <count> <low> <high>` | `setRandomSeed(seed)`, then `randomNumber(low, high)` `count` times |

`MRIread` (and FreeSurfer's other readers) runs FreeSurfer's licence check: the harness gets
`FREESURFER_HOME` (the official 8.2.0 tree) and `FS_LICENSE` (the licence file read in place;
`LARMORX_FS_LICENSE` names another); LARMORX's ports need none.

From Python (`larmorx_validation.freesurfer_unit`: one thread, FreeSurfer's family of variables,
each call through the oracle-output cache, so a call seen before is served):

```python
from larmorx_validation import freesurfer_unit as fu
fu.open_ran1(1234, 5)                         # FreeSurfer's first five random numbers for seed 1234
fu.vnl_random(1234, 5, "lrand32")
fu.matrix_inverse([[0.9375, 0, 0, -127.5], [0, 1, 0, 128.25], [0, 0, 1.125, -96], [0, 0, 0, 1]])
fu.mri_read_write("orig.mgz", "out.nii.gz")   # the header as MRIread read it
```

**Adding a function** (tracks A and C): a `cmd_<name>` in `harness/fsunit.cpp` (read the inputs,
call the function, print the JSON with `num()`), a row in `COMMANDS`, a wrapper in
`freesurfer_unit.py`, a line in the table above; `build.py freesurfer` rebuilds only the harness.
A change of the harness changes its SHA-256, so the cache keys its new runs apart.

## The proofs against the official binaries

`prove.py code|replay|units|all` (in a memory box, `boxed.sh 14`; the official programs through the
oracle-output cache at one thread; `--json` writes everything):

RESULTS

## Files

| File | What |
|---|---|
| `build.py` | the build: toolchain (Ubuntu's packages checked and unpacked, or conda's wrappers), ITK 4.13.2, FreeSurfer's libraries, the programs, the harness, `BUILD_INFO.txt` |
| `top.cmake` | the shadow tree's `CMakeLists.txt` (the official top-level settings, then FreeSurfer's own CMake files) |
| `harness/fsunit.cpp`, `harness/CMakeLists.txt` | the harness, built as one of FreeSurfer's programs |
| `code_identity.py` | the three programs against the official ones: files, sections, instructions, data, constructors |
| `prove.py` | the proofs: code, the replays of the recorded steps, the units against the official programs |
| `validation/src/larmorx_validation/freesurfer_unit.py` | the Python side; its tests `validation/tests/test_freesurfer_unit.py` |
