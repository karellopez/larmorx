# fs-random

The check of FreeSurfer 8.2.0's two permissive random-number sources, ported into `larmorx-core`
(the FreeSurfer foundations' item 3.1-04, `docs/freesurfer/queue.md`):

- `larmorx_core::vnl_random` (VXL's `vnl_random`, FreeSurfer's `-seed` generator through
  `OpenRan1`) against **ITK 4.13.2's own `vnl_random.cxx`** (the version the official binaries
  link), compiled with the shim `shim.cxx` by GCC 8.4.0 (the official compiler, conda-forge's, in
  `<workspace>/oracles/toolchains/gcc-8.4`) with FreeSurfer's C++ flags (`-std=c++11 -O3 -DNDEBUG
  -fopenmp -fno-builtin` and Ubuntu's default hardening); `stub/` holds the three ITK headers
  `vnl_random` includes that a CMake configuration would generate (two macros and an empty export
  macro: they change no arithmetic). `build.sh` keeps the disassembly (`vnl_random.dis`): GCC 8.4.0
  calls `lrand32` twice in `drand64` and divides the first result by `0xffffffff`, as in the
  official binaries (`docs/freesurfer/variation.md` §3);
- `larmorx_core::drand48` (POSIX's 48-bit generators, written from POSIX's definition) against the
  machine's C library (glibc 2.35, called as a black box: its code is never read).

`fs-random <libvnlrandom.so> [n_seeds] [--record <file>]` runs, for `n_seeds` seeds (default
2,000), 2,000 randomly chosen `vnl_random` calls per seed (`lrand32`, its two ranged forms,
`drand32`, `drand64`, `normal`, `normal64` with glibc's logarithm (`math::log_glibc`), `restart`,
`reseed`, both reseeds, FreeSurfer's `(float) drand64(0.0, 1.0)`) and, per seed, `srand48`,
`seed48` and `lcong48` each followed by 1,000 randomly chosen calls of the nine `drand48`
functions (the state before any seeding included), compared bit for bit; it counts how often the
correctly rounded logarithm (`math::log`, LARMORX's default) would change a normal number. Prints
the counts and `VERIFY-PASS` or `VERIFY-FAIL`; `--record` writes the hashes of fixed sequences
(five seeds each) that the unit tests of `vnl_random.rs` and `drand48.rs` check on every platform.

```bash
bash validation/probes/fs-random/build.sh <scratch>/random <itk-4.13.2-source>
scripts/orchestration/boxed.sh 4 <scratch>/random/fs-random <scratch>/random/libvnlrandom.so 2000 \
    --record <scratch>/random/hashes.rs
```

`<itk-4.13.2-source>`: the unpacked `InsightToolkit-4.13.2.tar.gz` of ITK's v4.13.2 GitHub release
(SHA-256 `d8760b279de20497c432e7cdf97ed349277da1ae435be1f6f0f00fbe8d4938c1`,
`docs/freesurfer/third-party.md` §10). Meaningful on x86-64 Linux with glibc 2.35 (the reference
machine).

**Results**: see `crates/larmorx-core/PROVENANCE.md` (`vnl_random.rs`, `drand48.rs`).
