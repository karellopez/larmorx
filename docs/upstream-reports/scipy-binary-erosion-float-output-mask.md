<!-- Draft of an issue for github.com/scipy/scipy. Status: draft, not yet filed (no issue found on 2026-11-10 that describes it; SciPy's main still has the code). When filed, add the issue's URL here and in README.md. Register entry: SCI-25 in docs/findings/upstream-defects.md; larmorx's lx.ndimage keeps the elements outside the mask by default (rule A, a warning when SciPy's result differs) and gives SciPy's result with upstream_defects="reproduce". Reproducer: validation/investigations/sdcflows-fieldmap-b/relay32/sci25_erosion_float_output.py; the wider measurement: relay22/morph_cases.py. -->
# BUG: ndimage: an iterated `binary_erosion` with a `mask` into a float `output` erodes elements outside the mask

## Summary

With `iterations` other than 1, a structure whose centre is true and `brute_force=False`,
`binary_erosion` (and `binary_closing`, which calls it) runs the first pass with
`binary_erosion` and the rest with `binary_erosion2` (`_morphology.py` line 282 in 1.18.1).
`NI_BinaryErosion2` (`src/ni_morphology.c`) protects the elements outside the mask by reading the
**first byte** of each output element as an `npy_bool`, storing it in the mask's copy and writing
`_false` there:

```c
if (*(npy_int8*)pm) {
    *(npy_int8*)pm = -1;
} else {
    *(npy_int8*)pm = (npy_int8)*(npy_bool*)pi;
    *(npy_bool*)pi = _false;
}
```

and at the end writes the stored byte back (`if (value >= 0) *(npy_bool*)pi = value;`). For a
bool or (little-endian) integer output that byte is the value, so the element is set to 0 during
the passes (no pass erodes it) and restored. For a float32 or float64 output the first byte of
both 0.0 and 1.0 is 0: an outside element holding 1.0 keeps 1.0 during the passes, the passes
erode it like any inside element, and the restore writes back only a zero byte. The documentation
of `output` ("Array of the same shape as input, into which the output is placed") puts no limit on
its type, and the result differs only by the output's type.

## Reproducer

```python
import numpy as np
from scipy import ndimage

x = np.ones(5, bool)
mask = np.array([1, 1, 1, 0, 0], bool)
for dtype in (bool, np.uint8, np.float32, np.float64):
    out = np.zeros(5, dtype)
    ndimage.binary_erosion(x, iterations=4, mask=mask, output=out)
    brute = np.zeros(5, dtype)
    ndimage.binary_erosion(x, iterations=4, mask=mask, output=brute, brute_force=True)
    print(f"{np.dtype(dtype).name}: {out.astype(int).tolist()} (brute_force: {brute.astype(int).tolist()})")
```

SciPy 1.18.1 with numpy 2.5.4 and SciPy 1.15.3 with numpy 2.3.5 (CPython 3.12, Linux x86-64):

```
bool: [0, 0, 0, 1, 1] (brute_force: [0, 0, 0, 1, 1])
uint8: [0, 0, 0, 1, 1] (brute_force: [0, 0, 0, 1, 1])
float32: [0, 0, 0, 0, 1] (brute_force: [0, 0, 0, 1, 1])
float64: [0, 0, 0, 0, 1] (brute_force: [0, 0, 0, 1, 1])
```

Element 3 lies outside the mask and must keep its input value, as it does for bool and uint8
outputs and with `brute_force=True`. In a wider measurement (60 seeded masks and inputs,
iterations 2, 3, 0 and -1, erosions, dilations and closings into eight output types, both
strategies), 600 of 2,520 results into float32 and float64 outputs differed from the bool output's,
all of them erosions or closings with the propagation strategy; dilations are right (their marking
writes a 1 into that byte, which no comparison then takes for the value).

## Expected

The same result for every output type: the elements outside the mask keep their values.

## Possible fix

Run `binary_erosion2` on a bool array and copy into the caller's output: in `_binary_erosion`,
when `output.dtype != bool`, let the first pass write into a temporary bool array, iterate on it,
then `output[...] = tmp`. Or, in C, save and restore whole elements of the output's type instead
of their first byte.
