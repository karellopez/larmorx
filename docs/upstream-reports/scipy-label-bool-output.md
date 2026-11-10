<!-- Draft of a comment for github.com/scipy/scipy/issues/24502 ("ndimage.label does not raise exception when output cannot store largest label", open since 2026-02-03; its PR gh-24579 was closed without merging). Status: draft, not yet posted. When posted, add the comment's URL here and in README.md. Register entry: SCI-29 in docs/findings/upstream-defects.md; larmorx's lx.ndimage refuses labels above 1 in a bool output by default (rule A: SciPy's "insufficient bit-depth" error, one label written as True), and writes SciPy's bytes with upstream_defects="reproduce". Reproducer: validation/investigations/sdcflows-fieldmap-b/relay32/sci29_label_bool_output.py. -->
# `label` into a bool output: what the output then holds, and a small fix that keeps the documented contract

*To be posted as a comment on scipy/scipy#24502.*

Still present in SciPy 1.18.1 and on main (2026-11-10: `_ni_label.pyx` views a bool output as
`uint8`, with the comment `# XXX - trigger special check for bit depth?`, and
`fused_write_line[np.uint8_t]` checks overflow at 255). One more detail that matters for the
choice: the bool array does not hold `True`/`False` afterwards but the labels' bytes, values
numpy's bool type does not have.

```python
import numpy as np
from scipy import ndimage

x = np.array([1, 0, 1, 0, 1])
out = np.zeros(5, bool)
n = ndimage.label(x, output=out)
print("output=array:", n, out.tolist(), "bytes", out.view(np.uint8).tolist())
labels, n = ndimage.label(x, output=bool)
print("output=bool:", n, labels.dtype, "bytes", labels.view(np.uint8).tolist())
mask = x.astype(bool)
n = ndimage.label(mask, output=mask)          # the documented in-place use
print("in place:", n, "bytes", mask.view(np.uint8).tolist())
try:
    ndimage.label(np.tile([1, 0], 200), output=np.zeros(400, np.int8))
except RuntimeError as err:
    print("int8, 200 labels:", f"RuntimeError: {err}")
```

SciPy 1.18.1 (numpy 2.5.4) and 1.15.3 (numpy 2.3.5) alike:

```
output=array: 3 [True, False, True, False, True] bytes [1, 0, 2, 0, 3]
output=bool: 3 bool bytes [1, 0, 2, 0, 3]
in place: 3 bytes [1, 0, 2, 0, 3]
int8, 200 labels: RuntimeError: insufficient bit-depth in requested output type
```

numpy's own operations mostly read any non-zero byte as `True`, so the result looks like the
input mask; its raw bytes (`tobytes()`, a view, a file written from the buffer, C extensions)
carry the labels. Up to 255 working labels pass the `uint8` check; beyond, `label` retries in
`int32` and raises as for any type the labels do not fit.

**A fix within the documented contract** ("the output must be able to store the largest label, or
this function will raise an Exception"): give the bool output the range {0, 1} in the overflow
check (a write function for `np.bool_` that reports overflow for any label above 1, keeping the
`uint8` view for reading the input). The existing retry in `_measurements.label` then raises
`RuntimeError("insufficient bit-depth in requested output type")` exactly when there is more than
one feature, and a bool output with a single feature keeps working in place (through the retry's
`int32` temporary: the working labels start at 2, so the first foreground element already
overflows, before anything is written, which keeps the in-place retry safe). Raising for every
bool output (option 2 above, gh-24579) would also refuse that single-feature case, which the
contract allows.
