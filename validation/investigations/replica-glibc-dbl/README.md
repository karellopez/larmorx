# The replicas' `reproduce` on glibc's double `sin`, `cos` and `sincos` (2026-11-04)

BET's `-S`/`-B`/`-A`/`-A2` replica part, relay 1 (GLIBC-07; the glibc maths part's step (b)). What
it shows: where the FSL and AFNI replicas' originals call glibc's double `sin`, `cos` or `sincos`,
and that the replicas' `reproduce` mode now gives their bits there. The proofs themselves are
unit tests and ignored probes in the crates; this folder keeps the tools that found the facts.

- `trigcalls.py`: per routine of a binary, its calls to the C library's trigonometric functions
  (`objdump`). On the AFNI 26.3.05 oracle's `3dTshift`, `3dAutomask` and `3dUnifize`: every table
  routine of `csfft.c` (`fft64`, `fft128`, `fft256`, `fft512`, `fft_4dec`, `fft_3dec`, `fft_5dec`,
  `csfft_trigconsts`) and `fft_shift2` call `sincos@plt` only (GCC merges each `cos(a)`/`sin(a)`
  pair), the weighted sinc `sinf`/`cosf`. So the AFNI replica's `reproduce` takes
  `larmorx_core::math::sincos_glibc` there.
- `ztop_nan.py`: fslmaths' `-ztop` of an image with NaN, FSL against the replica, with the NaNs'
  bits. It found the replica regression the re-made record showed (`stat/ztop`,
  `stat/ztop-double`, `stat/ztop-ptoz`, `d13/ztop-inf`; main's build at 4e1f7fc too): a positive
  NaN voxel gave `0xffc00000` where FSL gives `0x7fc00000`, because cprob's `expx2`, written as
  plain Rust arithmetic, let the optimiser fold `-(m * m)` into `(-m) * m`. Fixed (f840a80):
  `expx2`'s operations are `larmorx_nc_core::sse`'s, unit test
  `cprob::tests::a_nan_keeps_fsls_sign_through_expx2_and_ndtr`.

```bash
python -P validation/investigations/replica-glibc-dbl/trigcalls.py \
    <workspace>/oracles/afni-26.3.05/bin/3dTshift <workspace>/oracles/afni-26.3.05/bin/3dUnifize
FSLDIR=<FSL 6.0.7.23> python -P validation/investigations/replica-glibc-dbl/ztop_nan.py \
    crates-nc/target/release/larmorx-nc <workspace>/oracles/inputs/fsl-maths-replica/zmap.nii.gz
```

The other evidence of the step, where it lives:

| Site | Proof | Result (2026-11-04) |
|---|---|---|
| Kiss FFT twiddle factors | `validation/probes/fsl-avwutils-lib/check_fft.py --mode reproduce` | 0 of 65,792 factors, 0 of 197,376 transform values differ from Armadillo's |
| the same, the two modes | `kissfft::tests::reproduce_takes_glibcs_sincos_where_the_probe_found_fsls_factors_differ` | the factors differ on exactly 220 in 80 lengths (the probe's) |
| sinc windows | `kernel::tests::the_windows_agree_in_both_modes` | the same float kernels at every width 1 to 1001 |
| stage-4 sinc table | `costfns::tests::the_stage4_table_is_the_same_with_glibcs_cos` | 201 of 201 the same |
| avscale | `avscale::tests::glibc07_is_named_only_where_the_reports_differ` | the half transforms and reports the same on the rotations whose `cos` glibc rounds otherwise |
| fslmaths `-sin`, `-cos` | `fslmaths::tests::double_sin_and_cos_are_glibcs_with_reproduce` | glibc's bits, GLIBC-07 named in both modes |
| AFNI csfft tables | `csfft::tests::both_modes_give_the_same_tables` | the same tables for every length |
| 3dTshift `CEXPIT` | `shift::tests::cexpit_on_every_float` (ignored; release) | 0 of the finite floats give another float pair (11 s on 14 threads) |
