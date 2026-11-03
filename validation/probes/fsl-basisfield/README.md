# fsl-basisfield's spline fields, bit for bit

What it shows: that `larmorx-nc-core::basisfield` (the translation of FSL 6.0.7.23's
fsl-basisfield 2203.4: `Spline1D`, `Spline3D`, `splinefield`) gives FSL's own bits for every
quantity topup takes from a spline field, and that FSL's results do not depend on the field's
thread count. Made for the topup replica (2026-11-03, sub-task 1a).

`basisfield_probe.cc` builds a `BASISFIELD::splinefield` in FSL's own `libfsl-basisfield` for one
case directory (sizes, voxel sizes, knot spacings, order; coefficients, two images and a mask as
raw little-endian arrays; points and voxels to peek at) and prints, one value per line as its
hexadecimal bits: the coefficient grid's size; the field and its three derivatives at every
voxel (double, `Get`) and as float volumes (`AsVolume`); `Peek` at points inside and outside the
field; `PeekWide` at voxels outside it; `Jte` (image with and without a mask, a derivative, the
product of two images); `JtJ` (symmetric: two images with a mask, ones, a derivative; asymmetric:
two different derivatives) as their compressed columns (explicit zeros kept); the membrane and
bending energies, their gradients and Hessians; the field after `ScaleField`. The replica's side
is the ignored test `basisfield_probe` of `crates-nc/larmorx-nc-core/tests/basisfield_probe.rs`,
which writes the same lines from the translation.

`check.py` makes the cases (seeded: cubic and quadratic splines, knot spacings 1 to 5, a thin
field of 3 slices with a knot spacing of 4, a topup-like 24 × 24 × 16 field; images with a block
of zeros wider than a spline's support, a negative zero, scattered zeros; masks with a zero
block), runs the probe at one and at four threads, runs the replica's side, and counts the
differing lines per quantity. A last case of more than 10000 coefficients compares only the
energies, with OpenBLAS at one and at four threads (`ddot` splits its vectors above 10000
elements).

```bash
bash build.sh <scratch>/basisfield_probe
scripts/orchestration/boxed.sh 4 python -P validation/probes/fsl-basisfield/check.py \
    --probe <scratch>/basisfield_probe --work <scratch>/bfprobe
```

Result on 2026-11-03: 8 cases (1.18 million lines), 0 lines differ; FSL at 1 and 4 threads
identical in every case; the energies of 10560 coefficients identical at 1 and 4 OpenBLAS
threads (FSL's own values differ between the two).

`Set()` (sub-task 1b, 2026-11-03): the probe's `set` mode fits the coefficients to `ima1` and to
`ima2` (`splinefield::Set`) and prints them and the field they give; the replica's side runs
with `BF_PROBE_SET=1`. Every case above and four more (`set-topup-like`, 64 × 64 × 40 at knot
spacing 1: 67 coefficients along x, OpenBLAS's blocked LU; `set-odd`, 45 × 37 × 21: odd numbers
of right-hand sides; `set-quad`, quadratic splines; `set-k3`, knot spacings 3, 2, 1): 0 of
711,327 lines differ; `cubic-k232` (a voxel size that is not a float) is refused by both
("Voxel size mismatch beween basisfield class and supplied field").

Facts it found (for the topup replica's findings, restricted):

- `AsVolume` compares the field's double voxel size with the volume's float one, so a field made
  with a voxel size that is not a float (3.3 mm) refuses every `AsVolume`
  ("Voxel size mismatch beween field and volume"); topup's fields take their voxel sizes from
  float images, so it never meets this (case `cubic-k232`).
- `MemEnergy` and `BendEnergy` are `DotProduct`s: Armadillo's two-accumulator loop up to 32
  coefficients, OpenBLAS's `ddot` beyond, whose result depends on OpenBLAS's thread count above
  10000 elements. FSL's `libfsl-utils` sets OpenBLAS to one thread when it is loaded
  (`fsl_init_blas_threading`, skipped when `FSL_SKIP_GLOBAL` is set to a non-zero number), so
  FSL's programs compute single-threaded dot products unless `FSL_SKIP_GLOBAL` is set; then the
  machine's OpenBLAS thread count changes the last bits. `ddot_threads.c` shows it on its own:

```bash
gcc -O1 ddot_threads.c -L ~/fsl-6.0.7.23/lib -lblas -Wl,-rpath,$HOME/fsl-6.0.7.23/lib -o <scratch>/ddot_threads
for t in 1 2 4 32; do OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=$t <scratch>/ddot_threads 50000; done
```

  Its values at n = 33, 100, 10001, 10560 and 50000 and 1, 2, 4 and 32 threads are the unit test
  `ddot_unit_is_fsls` of `crates-nc/larmorx-nc-core/src/lapack/blas.rs`.
- FSL's threaded `JtJ` computes the elements whose row index is at least the column's along each
  direction and copies the rest; the single-threaded one copies the same elements from earlier
  columns: both store the same values (the 1-thread and 4-thread outputs are identical).
