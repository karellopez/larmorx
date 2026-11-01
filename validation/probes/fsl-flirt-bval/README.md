# FSL-75: where FLIRT corrupts its heap (FSL 6.0.7.23)

How the heap corruption random testing found on small images (2026-11-01) was traced to newimage's
background value (`calc_bval`), and the tools to repeat it (`docs/findings/fsl-flirt.md`, FSL-75).

- `mk_crash.py <dir> <in shape> <in voxel> <ref shape> <ref voxel> [value]` writes two constant
  float32 images (`in.nii`, `ref.nii`), e.g. `1x1x1 2.5x2.5x2.5 8x1x1 1x1.5x0.5` (FSL segfaults).
- `run_fsl.py <dir> [--gdb] [--replica[=fix|reproduce]] <program> <args...>` runs FSL's program
  (`LARMORX_FSL_623`, default `~/fsl-6.0.7.23`; not `$FSLDIR`), the replica (`LARMORX_NC_BIN`), or FSL under `gdb` (its
  backtrace: the crash is in a later `free`).
- `gdb_watch.py <dir> <address> <flirt args...>` runs FSL's flirt under `gdb` with a hardware
  watchpoint (`watch -l`) on one heap word and prints a backtrace at every write: give it the
  chunk header (`ptr - 8`) of the `free` that crashes in a first run (gdb disables address
  randomisation, so the addresses repeat). The last writes before the crash are in `calc_bval`,
  called by `subsample_by_2`.

```bash
python -P mk_crash.py /tmp/c 1x1x1 2.5x2.5x2.5 8x1x1 1x1.5x0.5
python -P run_fsl.py /tmp/c --gdb flirt -in in.nii -ref ref.nii -omat out.mat   # free() crashes
python -P gdb_watch.py /tmp/c 0x5555555d7b08 -in in.nii -ref ref.nii -omat out.mat
```
