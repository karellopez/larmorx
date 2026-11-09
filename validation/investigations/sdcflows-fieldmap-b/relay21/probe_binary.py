"""Relay 21: binary morphology, label and sum_labels argument handling against SciPy's."""
import warnings
import numpy as np
from scipy import ndimage as snd
from larmorx import ndimage as lnd

def run(f):
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            r = f()
            if isinstance(r, tuple):
                r = r[0]
            r = np.asarray(r)
            return ("ok", r.dtype.str, r.tobytes().hex()[:40], [str(x.message) for x in w])
        except Exception as e:
            return (type(e).__name__, str(e), [str(x.message) for x in w])

a = (np.arange(30).reshape(5, 6) % 4) > 1
ro = np.zeros((5, 6), bool); ro.flags.writeable = False
inputs = {"f16": a.astype(np.float16), "ld": a.astype(np.longdouble), "obj": a.astype(object), "c64": a.astype(np.complex64), "U": a.astype("U3"), ">f8": a.astype(">f8")}
kws = {"origin float": {"origin": 0.0}, "origin True": {"origin": True}, "output str bool": {"output": "bool"}, "output f16": {"output": np.float16},
       "output ro": {"output": ro}, "output list": {"output": [[0]*6]*5}, "border str": {"border_value": "x"}, "border 2.5": {"border_value": 2.5},
       "iterations 1.0": {"iterations": 1.0}, "iterations True": {"iterations": True}, "mask f16": {"mask": a.astype(np.float16)}, "structure f16": {"structure": np.ones((3,3), np.float16)}}
fns = {"binary_erosion": lambda m, x, **k: m.binary_erosion(x, **k), "binary_dilation": lambda m, x, **k: m.binary_dilation(x, **k),
       "binary_opening": lambda m, x, **k: m.binary_opening(x, **k), "binary_closing": lambda m, x, **k: m.binary_closing(x, **k),
       "binary_propagation": lambda m, x, **k: m.binary_propagation(x, **{kk: v for kk, v in k.items() if kk != "iterations"}),
       "binary_fill_holes": lambda m, x, **k: m.binary_fill_holes(x, **{kk: v for kk, v in k.items() if kk in ("output", "origin", "structure")}),
       "label": lambda m, x, **k: m.label(x, **{kk: v for kk, v in k.items() if kk in ("output", "structure")}),
       "sum_labels": lambda m, x, **k: m.sum_labels(x, labels=x.astype(int) if x.dtype.kind in "biuf" else None)}
d = n = 0
for fn, call in fns.items():
    for k, x in inputs.items():
        n += 1; r1 = run(lambda: call(snd, x)); r2 = run(lambda: call(lnd, x))
        if r1 != r2: d += 1; print(f"DIFF {fn} input {k}\n   scipy {r1}\n   lx    {r2}")
    for k, kw in kws.items():
        n += 1; r1 = run(lambda: call(snd, a, **kw)); r2 = run(lambda: call(lnd, a, **kw))
        if r1 != r2: d += 1; print(f"DIFF {fn} {k}\n   scipy {r1}\n   lx    {r2}")
print(n, "cases", d, "differ")
