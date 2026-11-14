# windows-cross-check: a Rust test built on Linux and run on the Windows laptop

Main's CI failed on Windows only (fs.io's recorded test, `fs_io_official`, 2026-11-13 and 14), and
the Windows laptop has no Rust toolchain. This tool cross-builds a test binary (or an example) for
Windows x64 on the main machine and runs it on the Windows laptop over SSH, which gives the same
outputs as CI's Windows runners (checked: main bc258ea9's test gave CI's hash, `c78120c8...`, with
the same one differing case, 1770). Track A relay 22 used it to find the two Windows differences
(errno after a name Windows refuses: 587a5bf4; the user name from an empty environment: 59b264a4).

Nothing is installed system-wide. `setup.sh <dir>` puts into `<dir>`:
- Rust's own MinGW libraries (`rust-mingw-1.99.0-x86_64-pc-windows-gnu`, checked against its
  `.sha256` from static.rust-lang.org), which the Linux host's toolchain does not ship;
- conda-forge's `llvm-tools` (for `llvm-dlltool`: `windows-sys` and `getrandom` link Windows'
  libraries by `raw-dylib`, which needs a dlltool for the GNU target), through the micromamba on
  the machine (FSL's), with a root prefix inside `<dir>`;
- and it adds the `x86_64-pc-windows-gnu` standard library to the pinned toolchain (`rustup target
  add`, under `~/.rustup`). The linker is the toolchain's `rust-lld`.

The binary uses the GNU ABI (MinGW), not MSVC as CI does: the file system, paths and error codes
are Windows' own either way; the C runtime's maths functions would differ (larmorx's results do
not use them).

    validation/investigations/windows-cross-check/setup.sh <dir>
    # build: cargo's arguments after the workspace folder (run it in a memory box)
    scripts/orchestration/boxed.sh 14 validation/investigations/windows-cross-check/cross_build.sh <dir> \
        crates-freesurfer test -j 6 --locked --release -p larmorx-freesurfer-io --test fs_io_official --no-run
    # run: copies the .exe (and any further files) into %TEMP%\larmorx-cross on the SSH host, runs it there
    validation/investigations/windows-cross-check/run_remote.sh win-laptop <exe> [args...]

The case-by-case comparison relay 22 made: the port's `fs_io_probe` example cross-built the same
way, `fs_io_probe recorded > rec.txt`, `fs_io_probe run rec.txt reproduce` on both machines (in a
folder whose name has a blank and a `~`, as CI's temporary folders), the two outputs compared line
by line (lines both mark `SKIP` or `VARY` aside). A relative fixture folder makes the probe's
DTIwriteBVectors cases fail on both machines with the folder in the message (`/` on Linux, `\` on
Windows): an artefact of the probe's relative folder, not of the test, which uses an absolute one.
