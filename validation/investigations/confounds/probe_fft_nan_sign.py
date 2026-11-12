# SPDX-License-Identifier: Apache-2.0
"""Where a NaN's sign after SciPy's FFT differs from the source's operations.

pocketfft's and ducc0's butterflies negate values (``ROTX90``, ``conj``, ``-x``) and add them;
SciPy's binaries are compiled by GCC, which folds ``a + (-b)`` into ``a - b`` (and ``a - (-b)``
into ``a + b``). For numbers that is exact; for a NaN ``b`` it is not: SSE passes the NaN operand
on with its own sign, so the folded form keeps ``b``'s sign where the source's form gives the
negated one (the cases measured all fit this fold). larmorx follows the source's operations (with SSE's NaN rule), so a transform of an
input holding a NaN or an infinity can give NaNs of the other sign than SciPy's, at the same
positions. This probe prints, for small complex transforms with one NaN, both bit patterns where
they differ (``check_fft.py`` counts such cases as ``nan-bits``)::

    PYTHONPATH=<worktree>/python python -P probe_fft_nan_sign.py
"""

import numpy as np
import scipy
import scipy.fftpack as fftpack

from larmorx import _core

LIB = scipy.__version__
shown = 0
for n in (2, 4, 8):
    for j in range(n):
        x = (np.arange(1, n + 1) + 1j * np.arange(n)).astype(np.complex64)
        x[j] = complex(np.nan, 2.0)
        for forward in (True, False):
            want = (fftpack.fft if forward else fftpack.ifft)(x)
            got = _core.scipy_fftpack(x, None, forward, LIB)
            wb, gb = want.view(np.uint32), got.view(np.uint32)
            same_nans = np.array_equal(
                np.isnan(want.view(np.float32)), np.isnan(got.view(np.float32))
            )
            if not np.array_equal(wb, gb) and shown < 4:
                shown += 1
                print(
                    f"n={n} NaN at {j} {'fft' if forward else 'ifft'}: NaN positions equal {same_nans}"
                )
                print("  scipy  ", " ".join(f"{v:08x}" for v in wb))
                print("  larmorx", " ".join(f"{v:08x}" for v in gb))
print(f"scipy {LIB}: {shown} examples shown")
