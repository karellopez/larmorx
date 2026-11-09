"""Relay 21's probe: lx.ndimage's filters' argument handling against SciPy's, per case."""

import sys
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
                return ("ok", tuple(np.asarray(x).dtype.name for x in r), [str(x.message) for x in w])
            r = np.asarray(r)
            return ("ok", r.dtype.name, r.tobytes().hex()[:64], [str(x.message) for x in w])
        except Exception as e:
            return (type(e).__name__, str(e), [str(x.message) for x in w])


a = np.arange(12.0).reshape(3, 4) % 5
a1 = np.array([3.0, 1, 4, 1, 5, 9, 2, 6])
ro = np.zeros((3, 4))
ro.flags.writeable = False

INPUTS = {
    "f16": a.astype(np.float16),
    "ld": a.astype(np.longdouble),
    "obj": a.astype(object),
    "c64": a.astype(np.complex64),
    "bool": a > 2,
    "U": a.astype("U3"),
    "i8": a.astype(np.int8),
    ">f8": a.astype(">f8"),
}
OUTPUTS = {
    "type f16": np.float16,
    "str bool": "bool",
    "str object": "object",
    "str U3": "U3",
    "str float16": "float16",
    "str f4": "f4",
    "type bool": np.bool_,
    "type c64": np.complex64,
    "arr f16": lambda: np.zeros((3, 4), np.float16),
    "arr shape": lambda: np.zeros((3, 5)),
    "arr ro": lambda: ro,
    "arr i8": lambda: np.zeros((3, 4), np.int8),
    "arr c64": lambda: np.zeros((3, 4), np.complex64),
    "dtype obj": np.dtype(np.float32),
    "list": [0.0],
}
CVALS = {
    "1j": 1j,
    "npc": np.complex128(1 + 0j),
    "str": "x",
    "None": None,
    "npint": np.int64(3),
    "npstr": np.str_("1"),
    "True": True,
    "nan": float("nan"),
    "arr0d": np.array(2.0),
    "arr1": np.array([2.0]),
}
ORIGINS = {"float": 1.0, "npint": np.int64(1), "True": True, "neg": -1, "big": 5, "f-1": -1.0}


def out(v):
    return v() if callable(v) else v


def filters():
    """name -> (call(input, **kw), base input, default kwargs)"""
    return {
        "minimum_filter1d": (lambda m, x, **k: m.minimum_filter1d(x, 3, **k), a),
        "maximum_filter1d": (lambda m, x, **k: m.maximum_filter1d(x, 3, **k), a),
        "minimum_filter size": (lambda m, x, **k: m.minimum_filter(x, 3, **k), a),
        "maximum_filter fp": (
            lambda m, x, **k: m.maximum_filter(x, footprint=[[1, 0, 1], [0, 1, 0]], **k),
            a,
        ),
        "grey_erosion st": (
            lambda m, x, **k: m.grey_erosion(x, structure=np.ones((2, 2)), **k),
            a,
        ),
        "grey_dilation size": (lambda m, x, **k: m.grey_dilation(x, size=2, **k), a),
        "grey_opening": (lambda m, x, **k: m.grey_opening(x, size=2, **k), a),
        "grey_closing": (lambda m, x, **k: m.grey_closing(x, size=2, **k), a),
        "rank_filter": (lambda m, x, **k: m.rank_filter(x, 1, 3, **k), a),
        "median_filter": (lambda m, x, **k: m.median_filter(x, 3, **k), a),
        "percentile_filter": (lambda m, x, **k: m.percentile_filter(x, 40, 3, **k), a),
        "median_filter 1d": (lambda m, x, **k: m.median_filter(x, 3, **k), None),
        "rank_filter 1d": (lambda m, x, **k: m.rank_filter(x, 1, 3, **k), None),
    }


def main():
    diffs = 0
    total = 0
    for fname, (call, base) in filters().items():
        def cmp(label, x, **kw):
            nonlocal diffs, total
            total += 1
            r1 = run(lambda: call(snd, x, **{k: out(v) for k, v in kw.items()}))
            r2 = run(lambda: call(lnd, x, **{k: out(v) for k, v in kw.items()}))
            if r1 != r2:
                diffs += 1
                print(f"DIFF {fname} {label}\n   scipy {r1}\n   lx    {r2}")
            elif "-v" in sys.argv:
                print(f"SAME {fname} {label} {r1}")

        b = a1 if base is None else a
        ins = {k: (v.ravel()[:8] if base is None else v) for k, v in INPUTS.items()}
        for k, v in ins.items():
            cmp(f"input {k}", v)
        for k, v in OUTPUTS.items():
            if base is None:
                continue
            cmp(f"output {k}", b, output=v)
        for k, v in CVALS.items():
            cmp(f"cval {k}", b, mode="constant", cval=v)
        for k, v in ORIGINS.items():
            cmp(f"origin {k}", b, origin=v)
        for k, v in {"bogus": "bogus", "grid-wrap": "grid-wrap", "seq": ["reflect", "nearest"]}.items():
            if base is None and k == "seq":
                continue
            cmp(f"mode {k}", b, mode=v)
        if "1d" in fname and base is not None:
            for k, v in {"float": 1.0, "npint": np.int64(0), "big": 5, "neg": -3, "True": True}.items():
                cmp(f"axis {k}", b, axis=v)
        if "axes" not in fname and base is not None and "1d" not in fname:
            for k, v in {"float": (1.0,), "dup": (0, 0), "big": (5,), "int": 1}.items():
                cmp(f"axes {k}", b, axes=v)
    print(f"{total} cases, {diffs} differ")


main()
