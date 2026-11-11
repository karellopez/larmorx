# Provenance: larmorx-confounds

**Ported** (an exact port of permissive upstreams, in the main package: `docs/licensing.md`,
`docs/porting-guide.md` §1) for fMRIPrep's confound estimation: nipype 1.12.0, niworkflows
1.15.4 and fMRIPrep 25.2.7 (Apache-2.0), with the arithmetic of the libraries they call, followed
operation by operation because it decides bit-identity. The logic is re-expressed in Rust; no
code was copied. The notices are in the modules that follow each upstream. The crate is filled by
the confounds' part relay by relay; this file lists what is in it.

## Relay 2e-i: SciPy's single-precision FFTs (`scipy_fft`)

nitime 0.12.1's `fftconvolve` (under nipype's DVARS AR(1) coefficients) calls
`scipy.fftpack.fft` and `ifft` of float32 and complex64 arrays of power-of-two length. SciPy
1.18.1 (the newest; PyPI sdist `scipy-1.18.1.tar.gz`, SHA-256
`52c4b7422442aba924d03ad4019852b08a92e64ea187b933135687bfe2747307`, `<workspace>/tags/scipy-1.18.1`)
computes them with ducc0 (`scipy.fft._duccfft`); SciPy 1.15.3 (the shared environment's; sdist
SHA-256 `eae3cf522bc7df64b42cad3925c876e1b0b6c35c1337c93e12c0f366f55b0eaf`,
`<workspace>/tags/scipy-1.15.3`) and 1.15.2 (fMRIPrep 25.2.6's image) with pocketfft
(`scipy.fft._pocketfft`). pocketfft and ducc0: Copyright (C) 2010-2023 Max-Planck-Society,
Copyright (C) 2019 Peter Bell (authors Martin Reinecke, Peter Bell), BSD-3-Clause (ducc0's files
are "BSD-3-Clause OR GPL-2.0-or-later"; the BSD-3-Clause terms are the ones used). SciPy:
Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers, BSD-3-Clause.

| larmorx | Upstream | Licence |
|---|---|---|
| `scipy_fft::roots::UnityRoots`, `calc`, `twiddle_survey` | SciPy 1.15.3 `scipy/_lib/pocketfft/pocketfft_hdronly.h` (`sincos_2pibyn`); SciPy 1.18.1 `subprojects/duccfft/ducc0/math/unity_roots.h` (`UnityRoots`, the same construction) | BSD-3-Clause |
| `scipy_fft::cpass::{CPass, factorize, pocketfft_plan}`, `pass2`, `pass4`, `pass8` | SciPy 1.15.3 `pocketfft_hdronly.h` (`cfftp`: `pass2`, `pass4`, `pass8`, `ROTX45`, `ROTX135`, `pass_all`, `factorize`, `comp_twiddle`; `pocketfft_c`'s choice of `cfftp` for powers of two; `cmplx`, `special_mul`, `PM`, `PMINPLACE`, `ROTX90`) | BSD-3-Clause |
| `scipy_fft::cpass::{ducc0_pass, ducc0_plan}`, `CPass::Multi`, `CPass::Vec4` | SciPy 1.18.1 `ducc0/fft/fft1d_impl.h` (`cfftp1`, `cfftp2`, `cfftp4`, `cfftp8`, `cfft_multipass` (its packets above 10000, the column twiddles, the transposition), `cfftp_vecpass` (four lanes), `cfftpass::make_pass`), `ducc0/fft/fft.h` (`cfftpass::factorize`, `util1d::prime_factors`, `pocketfft_c`, `PM`, `PMINPLACE`, `special_mul`), `ducc0/math/cmplx.h` (`Cmplx`) | BSD-3-Clause |
| `scipy_fft::rpass::{RPass, pocketfft_plan, ducc0_plan}`, `radf2`, `radf4`, `RPass::Complexify` | SciPy 1.15.3 `pocketfft_hdronly.h` (`rfftp`: `radf2`, `radf4`, `MULPM`, `exec` with `r2hc`, `copy_and_norm`, `factorize`, `comp_twiddle`; `pocketfft_r`); SciPy 1.18.1 `ducc0/fft/fft1d_impl.h` (`rfftp1`, `rfftp2`, `rfftp4`, `rfft_multipass`, `rfftp_complexify` forward, `rfftpass::make_pass`), `ducc0/fft/fft.h` (`rfftpass::factorize`, `pocketfft_r`) | BSD-3-Clause |
| `scipy_fft::{ComplexFft, RealFft, fftpack_real, fftpack_complex, FftLibrary, FftError}` | SciPy `scipy/fftpack/_basic.py` (`fft`, `ifft`), `scipy/fft/_pocketfft/` and `scipy/fft/_duccfft/` `basic.py` (`c2c`), `helper.py` (`_asfarray`, `_fix_shape`, `_fix_shape_1d`, `_normalization`), `pypocketfft.cxx` / `pyduccfft.cxx` (`c2c`, `c2c_internal`, `c2c_sym_internal`, `norm_fct`); pocketfft's `general_r2c`, `ExecC2C`, `rev_iter`; ducc0's `fftnd_impl.h` (`general_r2c`, `general_nd`'s one-dimensional path, `c2c`'s Bailey branch: the refused lengths), `fft.h` (`hermiteHelper`) | BSD-3-Clause |

The twiddles' sine and cosine: both libraries call glibc 2.35's `sincos` (GCC merges each
`cos`/`sin` pair; the extensions import `sincos` only). The tables use `larmorx_core::math`'s
correctly rounded `sin` and `cos` instead, which round to the same `float` entries on every
power of two up to 2^31 (`examples/twiddle_survey.rs`, which builds them with
`larmorx_core::math::sincos_glibc` too). The arithmetic is SSE's single precision
(`larmorx_core::numpy::NpFloat`); the extensions are built for the x86-64 baseline (no fused
multiply-add, checked with `objdump`).
