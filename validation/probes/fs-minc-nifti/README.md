# fs-minc-nifti: MINC's `.xfm` code, nifticlib's file names and znzlib, and C's `sscanf`

The check behind FreeSurfer's foundations' items 3.1-14 and 3.1-15 (`larmorx_io::xfm`,
`larmorx_io::znz`, `larmorx_io::nifti::niftilib`, `larmorx_core::cscan`). Findings:
`docs/findings/minc-xfm.md`, `docs/findings/nifti-znz.md`.

```
bash build.sh <out>        # needs the unit oracle (oracles/freesurfer-8.2.0-unit) and tags/minc-1.5.1,
                           # tags/nifticlib-2.0.0; GCC 8.4.0 from the unit oracle, the machine's zlib
<out>/fs-minc-nifti <out> [scale] [--only scan|xfm|ops|write|nifti|znz]
```

- `libup.so`: MINC 1.5.1's volume_io files the `.xfm` code needs (`up_stubs.c` stands in for MINC
  volumes and `micreate_tempfile`; `up-include/config.h`) and nifticlib 2.0.0's `nifti1_io.c`,
  `znzlib.c`, with `shim_up.c` and `shim_nifti.c`.
- `libfs.so`: FreeSurfer 8.2.0's `libminc.a`, `libnetcdf.a`, `libnifti.a` from the unit oracle's
  build, with `shim_fs.c` and `shim_nifti.c` compiled against FreeSurfer's `packages/` headers (read
  by the compiler only; the shims call volume_io's and nifticlib's public names).
- The driver compares the ports with both: status, every value's bits, standard output and error.
  FreeSurfer's changes are held by `src/dialect.rs` (`ProbeFs`) and `checks_nifti.rs`
  (`NiftiOptions`, `Flavour::FreeSurfer`), from FreeSurfer's behaviour alone; where an original
  crashes or never returns (FS-72 to FS-76), it is not run.
- `--replay <file> <mode> <comp> <reproduce> <ops>...` prints the port's results of one znz script
  (`LXPROBE_DUMP=<file>` writes them); `LXPROBE_SHOW=<n>` prints up to n differences per family;
  `LXPROBE_KEEP=1` keeps the files of differing znz cases.

Prints `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). Run inside a memory box
(`scripts/orchestration/boxed.sh 14 ...`): it is single-threaded and small (170 MB at scale 1).
