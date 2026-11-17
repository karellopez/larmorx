# itk111: a real eigensystem whose `hqr2` fails, and the memory VNL then reads

`rg_probe.cxx` (2026-11-17, track C relay 37) runs EISPACK's `rg_` (ITK 4.13.2's `v3p_netlib`, as
`vnl_real_eigensystem` calls it, eigenvectors wanted) on one matrix given as the bits of `float`s
(row-major, as `OpenNonSymmetricEigenSystem` passes FreeSurfer's matrices), with `wr`, `wi` and the
eigenvectors preset to a signalling-NaN sentinel (`7ff4dead0000beef`), and prints `ierr` and which
entries `rg_` wrote; then `vnl_real_eigensystem` of the same matrix (`Vreal`, `D`).

Built against the unit oracle's ITK (the same libraries as `../build.sh`):

```bash
U=<workspace>/oracles/freesurfer-8.2.0-unit
$U/toolchain/bin/g++ -std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -w -isystem $U/itk-4.13.2/include/ITK-4.13 \
  validation/probes/fs-vnl/itk111/rg_probe.cxx -o <scratch>/rg \
  $U/itk-4.13.2/lib/libitkvnl_algo-4.13.a $U/itk-4.13.2/lib/libitkvnl-4.13.a \
  $U/itk-4.13.2/lib/libitkv3p_netlib-4.13.a $U/itk-4.13.2/lib/libitkvcl-4.13.a -lm
scripts/orchestration/boxed.sh 2 <scratch>/rg 4 41700000 3f800000 4106db6e 42752492 40a00000 40e00000 \
  c0dc0000 42b8db6e 41200000 41200000 7fc00000 40400000 41980000 c2740000 41a00000 3f800000
```

Measured 2026-11-17 on the fs.maths clean room's reproducer (the matrix above: a NaN on the diagonal):
`ierr 4` (`hqr2` fails on the first eigenvalue it looks for, the fourth), none of `wr`, `wi` written
(the sentinels stay), the eigenvectors all NaN; VNL then prints ` *** vnl_real_eigensystem: Failed on
4th eigenvalue` with the matrix and builds `D`, `V` and `Vreal` from `wr` and `wi` as it found them in
memory (ITK-111, docs/findings/vnl-netlib.md F-VNLN-6). In this fresh process that memory is zero
bytes: `D` all 0, every column taken as real, `Vreal` all NaN, which is what
`larmorx_core::vnl_eigen::real_eigensystem` gives (its unit test
`a_failed_hqr2_gives_vnls_result_on_zeroed_memory`).

The official `OpenNonSymmetricEigenSystem` on the same line in the open maths probe
(`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe batch <file>`, inside `boxed.sh 2`):
plainly `0`, then 20 floats `00000000` x 6, `7fc00000 7fc00000 00000000 00000000` x 3, `7fc00000
7fc00000` (its child's heap held earlier values there: `wi[0]` not 0, so columns 0 and 1 were taken as
a complex pair and never written); with `MALLOC_PERTURB_=90`, and under `guard_malloc.so` with
`MALLOC_GUARD_=over MALLOC_PERTURB_=90`, all 20 `80000000`; with `MALLOC_GUARD_=under
MALLOC_PERTURB_=165`, all 20 `7f800000`. The result is memory contents: such a line is not comparable.
