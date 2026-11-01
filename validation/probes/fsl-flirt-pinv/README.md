# FLIRT's parameter-subset pseudo-inverse

What it shows: which parameter-subset masks the FLIRT replica (`larmorx-nc fsl flirt`) can invert
in closed form with FSL 6.0.7.23's own bits, and which it must refuse until LAPACK's SVD is
translated (`docs/findings/fsl-flirt.md`, restricted, "The pseudo-inverse of the parameter-subset
masks").

FLIRT maps between its 12 parameters and a subset with `pinv(parammask)` (Armadillo 14.2.1's
`pinv`, LAPACK's divide-and-conquer SVD on FSL's OpenBLAS). `pinv_probe.cc` runs exactly that,
linked against FSL's own LAPACK and BLAS, on the masks FLIRT builds (the search's 12 x 4 and
12 x 3, the 2D schedule's, the identity) or on masks read from standard input, and prints every
element as a C99 hexadecimal float. `check_masks.py` feeds it:

- 105 masks the replica accepts (columns of single ones on distinct rows, every column count and
  several orders; the search's 12 x 4 mask; FSL's `sch2D_4dof`): every element must equal the
  replica's closed form (unit columns give 1; the search's scale column
  `(1/sqrt 3)^2 = 0x1.5555555555557p-2`; `sch2D_4dof`'s `(1/sqrt 2)^2 = 0x1.ffffffffffffep-2`);
- 21 masks the replica refuses (a column of several ones alone or beside another column): only
  reported, with how many rows the closed form would miss (17 on 2026-11-01).

```bash
g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I ~/fsl-6.0.7.23/include pinv_probe.cc \
    -L ~/fsl-6.0.7.23/lib -llapack -lblas -Wl,-rpath,$HOME/fsl-6.0.7.23/lib -o <scratch>/pinv_probe
python -P check_masks.py --probe <scratch>/pinv_probe
```

`OPENBLAS_CORETYPE=Haswell` is set for the probe (FSL's programs run on OpenBLAS's Haswell
kernels in every record, FSL-64). Result on 2026-11-01: `accepted: 105 masks, 0 differ`.
