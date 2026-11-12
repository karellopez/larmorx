# fMRIPrep's confounds: SciPy's single-precision FFTs under nitime

nipype's DVARS estimates each voxel's lag-1 autocorrelation with nitime's `AR_est_YW`, whose
`crosscov` is an FFT convolution of float32 series through `scipy.fftpack`. What porting it showed
about SciPy 1.15's pocketfft and SciPy 1.18's ducc0 (both BSD-3-Clause as SciPy vendors them; ducc0 is
"BSD-3-Clause OR GPL-2.0-or-later", ported under BSD-3-Clause), so this file is not restricted. The
port: `crates/larmorx-confounds/src/scipy_fft/` (`FftLibrary {Ducc0, Pocketfft}`, `RealFft`,
`ComplexFft`, `fftpack_real`/`fftpack_complex`), the binding `_core.scipy_fftpack`. The scripts:
`check_fft.py`, `probe_fft_nan_sign.py` and the survey `crates/larmorx-confounds/examples/twiddle_survey.rs`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## Which library each SciPy runs (read, verified)

- **SciPy 1.18.1**: `scipy.fftpack.fft` → `scipy.fft._duccfft` (`scipy/fft/_duccfft/pyduccfft.cxx`
  over ducc0's `fft1d_impl.h`, `fft.h`, `unity_roots.h`, `cmplx.h`, vendored as `subprojects/duccfft`).
  **SciPy 1.15.3 and 1.15.2** (fMRIPrep 25.2.6's): → `scipy.fft._pocketfft` (`pypocketfft.cxx` over
  `scipy/_lib/pocketfft/pocketfft_hdronly.h`; `c2c` of a real array → `c2c_sym_internal`: the real
  transform, then the conjugate half).
- Both extensions are built for the x86-64 baseline (`objdump`: no `vfmadd`, no `ymm`): every float
  operation is one SSE rounding. Both import glibc's `sincos` (and `sincosl`), never `sin`/`cos`: GCC
  merges each `std::cos(a)`, `std::sin(a)` pair of the twiddle construction into one `sincos(a)`.
- nitime's `fftconvolve` pads to `fsize = 2 ** int(np.ceil(np.log2(2T - 1)))`, a power of two, so only
  the radix-2/4/8 passes are reached.

## The twiddles: glibc's `sincos` never shows after the float rounding

`sincos_2pibyn` (pocketfft) and `UnityRoots` (ducc0) build them the same way: `ang = double(0.25L π /
n)` (x87 long double; for a power of two exactly `fl(π)/(4n)`: x87's π drops `0x235` below double's),
two short double tables of `cos`/`sin` of multiples of `ang`, their product rounded to float.
**glibc's `sincos` against the correctly rounded `sin`/`cos`: 234 of the 262,172 double roots of every
power of two to 2^31 differ in their last bit, none of the 2^32 - 1 float entries** (the survey, about
20 s): larmorx uses CORE-MATH's `sin`/`cos` in both modes; no GLIBC-07 switch is needed and none
exists (verified).

## The two libraries' algorithms (read; verified bit for bit)

- The same radix-2/4/8 complex butterflies and `radf2`/`radf4` real passes, so the bits agree up to a
  complex length of 256 and a real one of 512. pocketfft runs the factor passes as 8s and 4s with a
  2 moved first (real: 4s, the 2 first, last to first).
- ducc0, asked to vectorize a 1-D transform (SciPy's `general_nd`/`general_r2c` do for 1-D arrays):
  complex lengths with `300 < n <= 100000` and `n % 4 == 0` take `cfftp_vecpass` (a radix-4 pass with
  `l1 = 1`, `ido = n/4`, then four transforms of `n/4` on the table of `n`, sub-sampled); above 10,000
  inside a multipass two packets of the prime factors (`[128, 128]` for 16,384: a column pass with the
  table's twiddles, then a transposing pass); real `n > 1000` with `n % 8 == 0` take
  `rfftp_complexify` (the `n/2` complex transform of the pairs with its own table, then the split with
  the table of `n`).
- SciPy's layers: a real input's spectrum has `-0` imaginary parts at indices 0 and `n/2`; `ifft` of a
  real array is the forward real transform scaled by `1/n` with the imaginary parts negated; `1/n` is
  `float(1/(long double)n)`, exact for powers of two.
- **The step**: about 16,500 of 36,000 synthetic cases differ between the libraries (complex lengths
  from 512, real from 1,024). **On fMRIPrep's real DVARS inputs**: ds005752 (`T` 10, `fsize` 32) and
  ds005040 identical between SciPy 1.15.2 and 1.18.1; ds000256 (`T` 164, `fsize` 512: the forward real
  transform identical, the backward complex one differs on all 2,000 voxels checked), ds005454 (280,
  `fsize` 1,024) and ds004958 (600, `fsize` 2,048) differ on every voxel: **fMRIPrep 25.2.6's AR(1)
  coefficients, hence its `std_dvars`, differ in their last bits from the newest SciPy's on runs with
  `T >= 129`** (counted per real call in [confounds-dvars.md](confounds-dvars.md)).

## NaN signs (a finding, no ID: a documented tolerance on NaN signs only)

With a NaN or an infinity in the input the NaN positions are identical but some NaNs' signs differ
from SciPy's: SciPy's binaries behave as GCC's folding of `a + (-b)` into `a - b` predicts (SSE
passes a NaN operand on with its own sign), while the source negates (`ROTX90`, `conj`, `-x`) and then
adds. larmorx follows the source's operations (rule 8) with SSE's NaN rule, the same on every
platform; the checks count these cases apart as `nan-bits` (`probe_fft_nan_sign.py`). Not reachable
with finite BOLD data; nipype's DVARS prints a NaN as `nan` whatever its sign.

## Refused (follow-ups)

- Lengths that are not powers of two (radix 3, 5, 7, 11, the generic pass, Bluestein): `FftError::
  NotPowerOfTwo`; nitime pads to a power of two.
- ducc0's Bailey 2-D algorithm for complex lengths from 131,072 (`fftnd_impl.h`'s `c2c`):
  `FftError::TooLong`; nitime reaches it only for series longer than 32,768 volumes.

Port them when a caller needs them.

## Proof (validated) and speed

`check_fft.py`'s full tier: SciPy 1.18.1 (numpy 2.5.4) 35,464 cases identical, 746 `nan-bits`, 0
differ; SciPy 1.15.3 (numpy 2.3.5) and **1.15.2 inside fMRIPrep 25.2.6's image** 35,824 identical,
505 `nan-bits`, 0 differ: every length 1 to 2^20 real and complex to 2^16 (ducc0) and 2^20
(pocketfft), both directions, padded, cut, strided and reversed inputs of 15 kinds, the refusals;
nitime's calls on synthetic series of 2 to 9,000 volumes and on 2,000 voxels of fMRIPrep's own DVARS
inputs in ds005752, ds000256, ds005040, ds005454 and ds004958 (30,000 calls). Speed of one call
through the binding (which builds the plan each time; SciPy caches its plans): length 32 larmorx 0.9
µs against SciPy 1.7-2.9 µs; 512 4.6-7.8 against 2.8-4.5; 2,048 18-31 against 5.6-9.7; 16,384 158-286
against 36-168. DVARS builds one plan per `fsize` and reuses it; the passes still allocate per call
(a speed item).
