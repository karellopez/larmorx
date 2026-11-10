<!-- Draft of an issue for github.com/scipy/scipy. Status: draft, not yet filed (no issue found on 2026-11-10 that describes it; SciPy's main still has the code). When filed, add the issue's URL here and in README.md. Register entries: SCI-21 (this report), with SCI-22 and SCI-23 in "In the same function", in docs/findings/upstream-defects.md; larmorx's lx.ndimage casts the values into the output by default (rule A) and, with upstream_defects="reproduce", gives SciPy's bytes where they are the output's own, never writing past its end. Reproducers: validation/investigations/sdcflows-fieldmap-b/relay32/sci21_rank_filter_output_type.py, sci21_related.py. -->
# BUG: ndimage: the 1-D rank filters write the input's type into an `output` of another type (wrong values, writes past the end)

## Summary

For a 1-D float32, float64 or int64 input, `rank_filter`, `median_filter` and `percentile_filter`
take the fast path of `_rank_filter_1d` and hand it the output array itself
(`x = input; x_out = output`, `scipy/ndimage/_filters.py` line 2005 in 1.18.1; the same in 1.15.3
and on main). The C++ (`src/_rank_filter_1d.cpp`, `rank_filter`) switches on the **input's**
type only and writes that type through `PyArray_DATA(out_arr)`. So an `output` of any other type
(a dtype such as `np.float32`, or an array) gets the input type's bytes:

- an output of the same item size (int64 or uint64 for a float64 input, `'>f8'`) gets the bytes
  reinterpreted;
- an output of smaller items (float32, int32, int16, uint8 for a float64 input) gets
  `input.size * 8` bytes written into a buffer of `input.size * itemsize`: past its end, so the
  heap is corrupted and the process may abort later (glibc: `corrupted size vs. prev_size`,
  `double free or corruption (out)`), crash or hang;
- a float64 output of a float32 input gets the float32 bytes in its first half.

The n-dimensional path (any other shape) and the 1-D path's other input types (cast to int64 or
float32 and copied back with `np.copyto`) give the right values, so the same call on `x[None]` with
`size=(1, 3)` is correct. `output=` is documented as "The array in which to place
the output, or the dtype of the returned array"; nothing refuses another dtype.

## Reproducer

```python
import sys

import numpy as np
from scipy import ndimage

x = np.array([0.0, 1.0, 2.0, 3.0, 4.0])
dtype = np.dtype(sys.argv[1])        # run once per output type
one = ndimage.median_filter(x, size=3, output=dtype)
two = ndimage.median_filter(x[None], size=(1, 3), output=dtype)[0]
print(f"{dtype}: 1-D {one.tolist()}, 2-D {two.tolist()}", flush=True)
```

SciPy 1.18.1 with numpy 2.5.4 and SciPy 1.15.3 with numpy 2.3.5 (CPython 3.12, Linux x86-64,
glibc 2.35), each type in a new process:

```
float64: 1-D [0.0, 1.0, 2.0, 3.0, 4.0], 2-D [0.0, 1.0, 2.0, 3.0, 4.0]
int64: 1-D [0, 4607182418800017408, 4611686018427387904, 4613937818241073152, 4616189618054758400], 2-D [0, 1, 2, 3, 4]
float32: 1-D [0.0, 0.0, 0.0, 1.875, 0.0], 2-D [0.0, 1.0, 2.0, 3.0, 4.0]
int16: 1-D [0, 0, 0, 0, 0], 2-D [0, 1, 2, 3, 4]
```

(`4607182418800017408` is `0x3FF0000000000000`, the bits of 1.0; `1.875` is the upper half of
1.0's bits read as float32.) The float32 and int16 calls write 40 bytes into a 20-byte and a
10-byte buffer. We ran each of the two in 10 new processes per release, twice: whenever a process
printed, it printed the values above, but between 1 and 4 processes of 10 per type and release
did not end normally: they aborted after printing (glibc: `corrupted size vs. prev_size`; once
SIGBUS), aborted before printing (`double free or corruption (out)`), or raised ``ValueError: array
is too big; `arr.size * arr.dtype.itemsize` is larger than the maximum possible size.`` in the
next call's allocation (the 2-D line). With glibc's checking allocator preloaded
(`LD_PRELOAD=libc_malloc_debug.so.0 MALLOC_CHECK_=3`) up to 7 of 10 aborted. Over 12 input and
output type pairs of the three filters (10 processes per call and release), processes also
crashed with a segmentation fault at exit, or hung.

## Expected

The filtered values cast into the output, as the n-dimensional path and the 1-D path's other input
types give: `[0, 1, 2, 3, 4]` for every output type here.

## Possible fix

Use the fast path's direct write only when the output is the input's native type and C-contiguous,
and otherwise filter into a temporary of the input's type and `np.copyto(output, x_out,
casting='unsafe')`, as the branch already does for the other input types:

```python
if input.dtype in (np.int64, np.float64, np.float32):
    x = input
    if output.dtype == input.dtype and output.flags.c_contiguous:
        x_out = output
    else:
        x_out = np.empty(x.shape, dtype=input.dtype)
...
if x_out is not output:
    np.copyto(output, x_out, casting='unsafe')
```

A check in the C++ that `PyArray_TYPE(out_arr) == type` (and the sizes match) would turn any
remaining mismatch into an error instead of a write past the end.

## In the same function

Two smaller defects of the same branch (`sci21_related.py`), which the change above touches too:

- **A non-contiguous `output` of the input's type leaves numpy's write-back copy unresolved.** The
  C++ takes the output with `NPY_ARRAY_INOUT_ARRAY2` (`WRITEBACKIFCOPY`) and never calls
  `PyArray_ResolveWritebackIfCopy`, so `median_filter(x, 3, output=np.zeros(10)[::2])` gives the
  right values (numpy writes the copy back when it is freed) with `RuntimeWarning: WRITEBACKIFCOPY
  detected in clearing of array. Required call to PyArray_ResolveWritebackIfCopy or
  PyArray_DiscardWritebackIfCopy is missing.` (numpy 2.3.5: "in array_dealloc."). gh-25641 and
  gh-25646 fixed this for three functions of `nd_image.c`; `_rank_filter_1d.cpp` was not among them.
- **The 1-D path refuses inputs the n-dimensional path takes.** It compares dtypes by equality with
  native ones, so a big-endian float input falls to `RuntimeError: Unsupported array type`, and
  numpy's `longlong` (equal to int64, but another type number on Linux and macOS) passes the Python
  check and fails in the C++'s switch: `TypeError: Unsupported array type` in 1.18.1,
  `SystemError: <built-in function rank_filter> returned a result with an exception set` in
  1.15.3. The same data as `x[None]` with `size=(1, 3)` filter fine.
