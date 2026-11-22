# freesurfer-make-uchar-errno: when the C library's `exp` sets `errno`

A black-box measurement of the system's `exp` (no library source read): which arguments set
`errno` to `ERANGE`. FreeSurfer's `LTAtransformAtPoint` calls `exp` for each transform's weight;
`mri_make_uchar` calls it at every voxel, and with an input that is not coronal its per-voxel error
prints `errno`'s message, so the official program's messages depend on it (relay 51 of track D).

    gcc -O2 -o exp_errno exp_errno.c -lm && ./exp_errno

## Result (observed, glibc 2.35, x86-64, 2026-11-22)

`ERANGE` exactly when a finite argument gives 0 (below -745.13321910194122: every argument whose
result rounds to zero) or an infinity (above 709.78...); a subnormal result (-708.4 to -745.13) does
not set it; `-inf` (result 0), `+inf`, NaN and 0 do not. The replica models this in
`crates-freesurfer/larmorx-freesurfer-volume/src/ltaapply.rs` (`LTAtransformAtPoint`) on the result
its `exp` gives.
