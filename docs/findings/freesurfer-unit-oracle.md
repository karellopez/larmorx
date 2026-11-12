# FreeSurfer 8.2.0's official build, as its binaries show it, and the unit oracle

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and binaries:
`docs/porting-guide.md` §1). Queue item 3.1-02, track B relay 1 (2026-11-12). The tools:
`validation/investigations/freesurfer-unit-oracle/` (its README says how to run each).

## 1. The official build is reproducible from the tag, to the byte

Three programs built by the unit oracle from the 8.2.0 tag (`mri_add_xform_to_header`,
`mri_normalize`, `mris_smooth`), stripped as the official ones are, are **the official files apart
from the 20-byte build ID** (`code_identity.py`): every instruction at every address, every read-only
and data section, the static constructors in the same order. The one exception is in
`mris_smooth`: one unwind-table entry (the cold part of ITK's `vnl_real_eigensystem` constructor)
keeps its own CIE in the official file where ours shares one (40 bytes of `.eh_frame`, 229 of
`.eh_frame_hdr`); no instruction differs. The official ITK objects were assembled on Ubuntu 20.04
(its `__FILE__` paths name a folder `4.13.2_ubuntu20`), ours on Ubuntu 22.04.

What the official build was, read from its files:

| | Evidence |
|---|---|
| Ubuntu 20.04's `gcc-8` **8.4.0-3ubuntu2** for FreeSurfer and ITK alike | every binary's `.comment` holds that one string; Ubuntu's own package reproduces the code, constructors and start files (§2) |
| Ubuntu 22.04's glibc 2.35 and binutils 2.38 (FreeSurfer, built in an Ubuntu 22.04 container) | `exp@GLIBC_2.29`, `__libc_start_main@GLIBC_2.34`; `.plt.sec` (the CET-enabled PLT of binutils 2.38 with `-fcf-protection`); the same layout from this machine's binutils 2.38 |
| ITK 4.13.2, built separately on Ubuntu 20.04 with the same compiler | the `__FILE__` strings of VNL's messages; the CIE above |
| FreeSurfer's top-level CMake on `HOST_OS` Ubuntu22 with GCC 8 (code.md §1) | the flags reproduce the code exactly |
| the run path `$ORIGIN/../lib/vtk:$ORIGIN/../lib/qt/lib:` | `readelf -d`; the unit oracle's programs differ in every global's address without it (the `.dynamic` section's size moves the GOT by 16 bytes) |
| `__DATE__ " " __TIME__` of `utils/version.cpp`: **Mar 14 2026 12:21:06** | the string in `getAllInfo`'s read-only data and an immediate of its code |

## 2. conda-forge's GCC 8.4.0 is not the official compiler, by two details

The preparation installed conda-forge's GCC 8.4.0 (`<workspace>/oracles/toolchains/gcc-8.4`; relay
5's N4 measurement, `freesurfer-inventory/gcc84/`). With wrappers giving it the machine's headers
and libraries and Ubuntu's default flags (`-fstack-protector-strong -fstack-clash-protection
-fcf-protection=full -Wformat -Wformat-security`, `-D_FORTIFY_SOURCE=2` when optimising), it
produces the same instructions for every FreeSurfer, ITK and VNL function, but:

- its own start files (`crtbeginS.o`, `crtendS.o`) differ from Ubuntu's (about 60 instructions of
  `frame_dummy`, `register_tm_clones`, `__do_global_ctors_aux`), and its link spec adds a run path
  into the conda folder (`-rpath <conda>/lib`: the programs would load conda's newer `libstdc++`
  and `libgomp`);
- it is configured without `.init_array` support: constructors go to `.ctors`, which the linker
  reverses into `.init_array`, so a file with two constructors runs them in the other order
  (`utils/romp_support.cpp`: `before_main`, which caps OpenMP's threads, and the file's static
  initialisation; harmless there, as neither reads the other).

So the unit oracle defaults to Ubuntu's own package (`build.py --compiler ubuntu`), and relay 5's
conclusion stands (its measurement concerns instructions, which are identical).

## 3. What a port must write and know

- **`MRIwrite`'s NIfTI and ANALYZE headers carry the build date**: `descrip` = `"FreeSurfer " __DATE__`
  (`utils/mriio.cpp` 8294, 9645, 10051), "FreeSurfer Mar 14 2026" in every official 8.2.0 program.
  A replica that writes another date writes other bytes (track A's `fs.io`, items 3.1-25..32).
- **`MRIread` checks FreeSurfer's licence** (`chklc()`, `utils/mriio.cpp` 994 and after): without
  `FREESURFER_HOME` (or `FS_LICENSE` naming a file) a program stops with "FreeSurfer environment
  FREESURFER_HOME is not defined", exit status 255. The ports need no licence (decision log
  2026-10-19); the message and status belong to the replica's `reproduce` only if a spec says so.
- **`OpenRan1`** (`utils/numerics.cpp` 1256-1280) is `vnl_random::drand64(0, 1)` of a static
  generator, reseeded when the seed changes, rounded to float; `setRandomSeed` draws one value on
  its own (`utils/utils.cpp` 77-101). Measured: the harness's `open-ran1` equals `vnl-random`'s
  `drand64` rounded to float for seed 1234, and the official programs' draws (README, the proofs).
- **`MatrixInverse`** of a float `MATRIX` up to 4 x 4 goes through VNL's closed-form `vnl_inverse`
  after `vnl_det` (exactly 0 gives NULL); larger ones through `vnl_qr` (`utils/numerics.cpp`
  1091-1146).

## 4. FS-61: the official programs search the working directory for shared libraries

The run path ends in an empty entry (`$ORIGIN/../lib/vtk:$ORIGIN/../lib/qt/lib:`): the Qt folder
is prepended to an empty `CMAKE_INSTALL_RPATH` (`cmake/FindQT.cmake` 91), then VTK's
(`CMakeLists.txt` 672). glibc reads an empty entry as the current directory: `LD_DEBUG=libs` shows
an official program looking for `libz.so.1` as `glibc-hwcaps/x86-64-v4/libz.so.1`, ...,
`tls/libz.so.1`, `libz.so.1` relative to its working directory before the system's folders. A
`libz.so.1` (or `libgomp.so.1`, `libstdc++.so.6`, ...) in a subject's folder would be loaded by
every FreeSurfer program run there. Not a computation (class: the build); LARMORX's ports are Rust
programs without a run path, so nothing to correct or reproduce in them. The unit oracle keeps the
run path (it decides the official layout) and runs only in folders its callers make. Proposed for
the register and an upstream report (the orchestrator's).
