# csfft-verify

Checks that `larmorx_gpl_afni::csfft` gives the same bits as AFNI 25.2.09's compiled
`csfft.c`.

```bash
crates-gpl/larmorx-gpl-afni/tools/csfft-verify/run.sh \
    <workspace>/oracles/afni-25.2.09/build/src  /tmp/csfft-verify
```

`run.sh` needs the AFNI oracle build (`scripts/build_afni_oracle.sh`), gcc, Python 3 and
cargo, and runs on Linux x86-64. It:

1. builds two C harnesses with AFNI's compiler flags against the oracle's `libmri.a`:
   - `harness_cox.c` runs AFNI's `csfft_cox` on every length it computes itself (139 lengths
     from 2 to 32768: 2^a·3^b·5^c with b, c ≤ 3), both directions, ten kinds of input (random,
     special values such as subnormals and signed zeros, 3dTshift-like zero-padded pairs), and
     `csfft_nextup*` for 1 to 40000;
   - `harness_inc.c` includes `csfft.c` to reach its static parts: `csfft_nextup_even` as
     `csfft_cox` calls it, the general radix-2 loop (unreachable through `csfft_cox`), and every
     argument the twiddle tables pass to `sincos`, with glibc's results;
2. checks with `check_unrolled.py` that the one loop replacing `fft8`/`fft16`/`fft32` performs
   exactly the butterflies of AFNI's generated code;
3. runs `csfft-verify` on the data (about 330 MB): every value must match bit for bit.

Result on 2026-10-10 (glibc 2.35, gcc 11.4, AFNI 25.2.09): 36,950,784 values of `csfft_cox`
and 2,621,400 of the radix-2 loop, 0 differ. Over the 539,282 table arguments, the correctly
rounded `cos`/`sin` of `larmorx_core::math` differ from glibc's in the last bit of the double
for 789 and 760 of them, and for none once rounded to float, which is all AFNI keeps.
