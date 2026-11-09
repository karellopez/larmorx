# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: B023 (every lambda runs at once, inside the loop that defines it)
"""Binary morphology's, ``label``'s, ``sum_labels``', ``center_of_mass``' and
``watershed_ift``'s argument handling against SciPy's (relay 22 of the field maps' part B).

The cases: inputs, masks, outputs (types, strings, lists, arrays of every element type, byte
order, layout, read-only, of another shape, the input itself), origins, border values,
iterations, structures and axes of every Python and numpy kind; ``label``'s outputs (bool,
narrow types with many labels, Fortran-ordered, strided and reversed arrays), ``sum_labels``'
inputs, labels and indices; ``watershed_ift``'s inputs, markers and outputs; mask values out of
``int8``'s range. Each case's result (or exception), the output array afterwards and the
warnings are compared with SciPy's: ``upstream_defects="reproduce"`` must give SciPy's exactly;
the default may differ only with the notice of a registered defect (SCI-10, SCI-13 to SCI-15,
SCI-25, SCI-27, SCI-29, SCI-30), SCI-28's refusal (named in its message) or SCI-26 (numpy's
write-back warning, given with ``reproduce`` only). Two families besides: iterated erosions and
dilations with a mask into output arrays of every type (SCI-25), and ``label`` into Fortran,
strided and C-ordered outputs (the numbering along the output's axis of the smallest stride).

Used by ``check_ndimage.py --sections arguments``; standalone::

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P morph_cases.py [names...]
"""

from __future__ import annotations

import functools
import inspect
import sys
import warnings

import numpy as np
from scipy import ndimage as snd

from larmorx import ndimage as lnd
from larmorx._defects import UpstreamDefectWarning


def _oracle(x):
    """SciPy's side of a comparison, for the originals' step (``steplines``, when the
    checking script has loaded it)."""
    m = sys.modules.get("steplines")
    return x if m is None else m.oracle(x)


#: The entries whose notices may accompany a corrected default.
KNOWN = {"SCI-10", "SCI-13", "SCI-14", "SCI-15", "SCI-25", "SCI-27", "SCI-29", "SCI-30"}
WRITEBACK = "WRITEBACKIFCOPY detected in"


def describe(r, given):
    if any(r is g for g in given):
        return ("<the output array>",)
    if r is None:
        return ("None",)
    if isinstance(r, tuple):
        return ("tuple", *[describe(x, given) for x in r])
    if isinstance(r, list):
        return ("list", *[describe(x, given) for x in r])
    if isinstance(r, np.ndarray):
        return ("ndarray", r.dtype.str, r.shape, np.ascontiguousarray(r).tobytes().hex())
    if isinstance(r, np.generic):
        return ("numpy", r.dtype.str, np.asarray(r).tobytes().hex())
    return (type(r).__name__, repr(r))


def outcome(case, m):
    """(result or exception, the output arrays afterwards, warnings, notice entries)."""
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            ret, given = case(m)
            res = describe(ret, given)
            after = [describe(g, []) for g in given]
        except Exception as e:
            res, after = (type(e).__name__, str(e)), []
    msgs = [
        f"{x.category.__name__}: {x.message}" for x in w if x.category is not UpstreamDefectWarning
    ]
    ids = [getattr(x.message, "entry", "?") for x in w if x.category is UpstreamDefectWarning]
    return res, after, msgs, ids


def short(o):
    res, after, msgs, ids = o
    s = str(res)
    return (s[:160] + "...") if len(s) > 160 else s, [str(a)[:100] for a in after], msgs, ids


A = np.zeros((5, 6), bool)
A[1:5, 1:6] = True
A[2, 3] = False
A[0, 0] = True
A[4, 1] = False
SHAPE = A.shape
CASES: dict[str, object] = {}


def ro_array(dt):
    x = np.zeros(SHAPE, dt)
    x.flags.writeable = False
    return x


INPUTS = {
    "bool": lambda: A.copy(),
    "uint8": lambda: A.astype(np.uint8),
    "int8": lambda: A.astype(np.int8),
    "float32": lambda: A.astype(np.float32),
    "float16": lambda: A.astype(np.float16),
    "longdouble": lambda: A.astype(np.longdouble),
    "object": lambda: A.astype(object),
    "complex64": lambda: A.astype(np.complex64),
    "U3": lambda: A.astype("U3"),
    ">f8": lambda: A.astype(">f8"),
    ">i4": lambda: A.astype(">i4"),
    "longlong": lambda: A.astype(np.longlong),
    "ulonglong": lambda: A.astype(np.ulonglong),
    "strided": lambda: np.repeat(A, 2, axis=1)[:, ::2],
    "list": lambda: A.astype(int).tolist(),
    "0-d": lambda: np.array(True),
    "empty": lambda: np.zeros((0, 3), bool),
    "empty float16": lambda: np.zeros((0, 3), np.float16),
    "python scalar": lambda: 1,
}
OUTPUTS = {
    "type float16": lambda: np.float16,
    "type int32": lambda: np.int32,
    "str 'bool'": lambda: "bool",
    "str 'x'": lambda: "x",
    "dtype float32": lambda: np.dtype(np.float32),
    "list": lambda: np.zeros(SHAPE).tolist(),
    "array bool": lambda: np.zeros(SHAPE, bool),
    "array int32": lambda: np.zeros(SHAPE, np.int32),
    "array uint8": lambda: np.zeros(SHAPE, np.uint8),
    "array float32": lambda: np.zeros(SHAPE, np.float32),
    "array float64": lambda: np.zeros(SHAPE),
    "array float16": lambda: np.zeros(SHAPE, np.float16),
    "array longdouble": lambda: np.zeros(SHAPE, np.longdouble),
    "array object": lambda: np.zeros(SHAPE, object),
    "array complex64": lambda: np.zeros(SHAPE, np.complex64),
    "array >i4": lambda: np.zeros(SHAPE, ">i4"),
    "array >f8": lambda: np.zeros(SHAPE, ">f8"),
    "array longlong": lambda: np.zeros(SHAPE, np.longlong),
    "array strided bool": lambda: np.zeros((5, 12), bool)[:, ::2],
    "array strided int32": lambda: np.zeros((5, 12), np.int32)[:, ::2],
    "array Fortran int16": lambda: np.zeros(SHAPE, np.int16, order="F"),
    "array read-only bool": lambda: ro_array(bool),
    "array read-only int32": lambda: ro_array(np.int32),
    "array of another shape": lambda: np.zeros((5, 7), bool),
    "array 0-d": lambda: np.zeros((), bool),
}
BORDERS = {
    "1": 1, "True": True, "numpy True": np.True_, "numpy int 1": np.int64(1), "2.5": 2.5,
    "1.0": 1.0, "numpy float 1.0": np.float64(1.0), "str": "x", "None": None, "2**31": 2**31,
    "-2**31-1": -(2**31) - 1, "2**63": 2**63, "-1": -1, "7": 7, "0-d array 1": np.array(1),
    "1-element array": np.array([1]),
}  # fmt: skip
ORIGINS = {
    "0.0": 0.0, "True": True, "numpy True": np.True_, "numpy int 0": np.int64(0), "(0, 0)": (0, 0),
    "[0, 0]": [0, 0], "(0.0, 0)": (0.0, 0), "array [0, 0]": np.array([0, 0]),
    "array [0., 0.]": np.array([0.0, 0.0]), "(0,)": (0,), "'0'": "0", "None": None,
    "(-1, 0)": (-1, 0), "(1, 0)": (1, 0), "0-d array": np.array(0), "numpy uint8 0": np.uint8(0),
    "(True, 0)": (True, 0), "2**70": 2**70,
}  # fmt: skip
ITERS = {
    "1.0": 1.0, "True": True, "numpy int 2": np.int64(2), "0": 0, "-1": -1, "'2'": "2",
    "None": None, "2": 2, "3": 3, "2**31": 2**31, "-2**31": -(2**31), "numpy True": np.True_,
}  # fmt: skip
MASKS = {
    "bool": lambda: A | np.eye(5, 6, dtype=bool),
    "int8": lambda: (A | np.eye(5, 6, dtype=bool)).astype(np.int8),
    "float16": lambda: A.astype(np.float16),
    "longdouble": lambda: A.astype(np.longdouble),
    "object": lambda: A.astype(object),
    "complex64": lambda: A.astype(np.complex64),
    "U3": lambda: A.astype("U3"),
    ">f8": lambda: A.astype(">f8"),
    ">i2 with 256": lambda: np.where(A, 256, 1).astype(">i2"),
    "longlong": lambda: A.astype(np.longlong),
    "list": lambda: A.astype(int).tolist(),
    "strided": lambda: np.repeat(A, 2, axis=1)[:, ::2],
    "0.5": lambda: np.where(A, 0.5, 1.0),
}
STRUCTS = {
    "float16": lambda: np.ones((3, 3), np.float16),
    "object": lambda: np.ones((3, 3), object),
    "list": lambda: [[0, 1, 0], [1, 1, 1], [0, 1, 0]],
    "strided": lambda: np.ones((3, 6), bool)[:, ::2],
    "0-d": lambda: np.array(True),
    "rank 1": lambda: np.ones(3, bool),
    "U1": lambda: np.array([["1", "", "1"]] * 3),
    "(2, 2)": lambda: np.ones((2, 2), bool),
    "Fortran (3, 2)": lambda: np.asfortranarray(np.array([[1, 0], [1, 1], [0, 1]], bool)),
}


def mk(fn_name, **kw):
    """A case calling ``m.<fn_name>(input, **kw)``; values that are factories are called per
    module; an ndarray output is reported afterwards."""

    def case(m):
        k = {}
        for key, v in kw.items():
            k[key] = v() if callable(v) and not isinstance(v, type) else v
        x = k.pop("input")
        given = [k["output"]] if isinstance(k.get("output"), np.ndarray) else []
        return getattr(m, fn_name)(x, **k), given

    return case


MORPH = ["binary_erosion", "binary_dilation", "binary_opening", "binary_closing",
         "binary_propagation", "binary_fill_holes", "binary_hit_or_miss"]  # fmt: skip
HAS = {
    "iterations": {"binary_erosion", "binary_dilation", "binary_opening", "binary_closing"},
    "mask": {"binary_erosion", "binary_dilation", "binary_opening", "binary_closing",
             "binary_propagation"},
    "border_value": {"binary_erosion", "binary_dilation", "binary_opening", "binary_closing",
                     "binary_propagation"},
    "brute_force": {"binary_erosion", "binary_dilation", "binary_opening", "binary_closing"},
}  # fmt: skip
for fn in MORPH:
    org = "origin1" if fn == "binary_hit_or_miss" else "origin"
    st = "structure1" if fn == "binary_hit_or_miss" else "structure"
    for k, v in INPUTS.items():
        CASES[f"{fn} input {k}"] = mk(fn, input=v)
    for k, v in OUTPUTS.items():
        CASES[f"{fn} output {k}"] = mk(fn, input=lambda: A.copy(), output=v)
    for k, v in ORIGINS.items():
        CASES[f"{fn} {org} {k}"] = mk(fn, input=lambda: A.copy(), **{org: v})
    for k, v in STRUCTS.items():
        CASES[f"{fn} {st} {k}"] = mk(fn, input=lambda: A.copy(), **{st: v})
    CASES[f"{fn} into its input"] = lambda m, fn=fn: (lambda x: (getattr(m, fn)(x, output=x), [x]))(
        A.copy()
    )
    CASES[f"{fn} into its input (uint8)"] = lambda m, fn=fn: (
        lambda x: (getattr(m, fn)(x, output=x), [x])
    )(A.astype(np.uint8))
    CASES[f"{fn} axes (0,)"] = mk(fn, input=lambda: A.copy(), axes=(0,))
    CASES[f"{fn} axes 1.0"] = mk(fn, input=lambda: A.copy(), axes=1.0)
    CASES[f"{fn} axes (0, 0)"] = mk(fn, input=lambda: A.copy(), axes=(0, 0))
    if fn == "binary_hit_or_miss":
        CASES[f"{fn} origin2 (0, 1)"] = mk(fn, input=lambda: A.copy(), origin2=(0, 1))
        CASES[f"{fn} origin2 0.0"] = mk(fn, input=lambda: A.copy(), origin2=0.0)
        CASES[f"{fn} structure2 float16"] = mk(
            fn, input=lambda: A.copy(), structure2=STRUCTS["float16"]
        )
        CASES[f"{fn} structure2 (2, 2)"] = mk(
            fn, input=lambda: A.copy(), structure2=STRUCTS["(2, 2)"]
        )
        CASES[f"{fn} structure2 rank 1"] = mk(
            fn, input=lambda: A.copy(), structure2=STRUCTS["rank 1"]
        )
    if fn in HAS["border_value"]:
        for k, v in BORDERS.items():
            CASES[f"{fn} border_value {k}"] = mk(fn, input=lambda: A.copy(), border_value=v)
    if fn in HAS["iterations"]:
        for k, v in ITERS.items():
            CASES[f"{fn} iterations {k}"] = mk(fn, input=lambda: A.copy(), iterations=v)
    if fn in HAS["brute_force"]:
        for k, v in {"'x'": "x", "None": None, "2": 2}.items():
            CASES[f"{fn} brute_force {k}"] = mk(
                fn, input=lambda: A.copy(), iterations=2, brute_force=v
            )
    if fn in HAS["mask"]:
        for k, v in MASKS.items():
            CASES[f"{fn} mask {k}"] = mk(fn, input=lambda: A.copy(), mask=v)
            if fn != "binary_propagation":
                CASES[f"{fn} mask {k} iterations 2"] = mk(
                    fn, input=lambda: A.copy(), mask=v, iterations=2
                )
        for ok in (
            "array float32",
            "array int32",
            "array >i4",
            "array float64",
            "array strided bool",
            "array uint8",
        ):
            its = (2, -1) if fn != "binary_propagation" else (None,)
            for it in its:
                kw = {"iterations": it} if it is not None else {}
                CASES[f"{fn} mask bool {kw} output {ok}"] = mk(
                    fn, input=lambda: A.copy(), mask=MASKS["bool"], output=OUTPUTS[ok], **kw
                )
        for ok in ("array float32", "array int32", "array float64"):
            CASES[f"{fn} no mask iterations 2 output {ok}"] = mk(
                fn,
                input=lambda: A.copy(),
                output=OUTPUTS[ok],
                **({"iterations": 2} if fn in HAS["iterations"] else {}),
            )

# label
for k, v in INPUTS.items():
    CASES[f"label input {k}"] = mk("label", input=v)
LABEL_OUT = dict(OUTPUTS)
LABEL_OUT.update({
    "type int8": lambda: np.int8, "type uint8": lambda: np.uint8, "type float32": lambda: np.float32,
    "str '>i4'": lambda: ">i4", "type longlong": lambda: np.longlong, "type object": lambda: object,
    "type bool": lambda: np.bool_, "type complex64": lambda: np.complex64, "array int8": lambda: np.zeros(SHAPE, np.int8),
})  # fmt: skip
for k, v in LABEL_OUT.items():
    CASES[f"label output {k}"] = mk("label", input=lambda: A.copy(), output=v)
for k, v in STRUCTS.items():
    CASES[f"label structure {k}"] = mk("label", input=lambda: A.copy(), structure=v)
MANY = np.zeros((40, 40), bool)
MANY[::2, ::2] = True  # 400 labels
for k, v in {"type int8": np.int8, "type uint8": np.uint8, "type bool": np.bool_, "type int16": np.int16,
             "array int8": lambda: np.zeros((40, 40), np.int8), "array bool": lambda: np.zeros((40, 40), bool),
             "array uint8": lambda: np.zeros((40, 40), np.uint8), "type float16": np.float16}.items():  # fmt: skip
    CASES[f"label 400 labels output {k}"] = mk("label", input=lambda: MANY.copy(), output=v)
CASES["label into its input (int32)"] = lambda m: (lambda x: (m.label(x, output=x), [x]))(
    A.astype(np.int32)
)
CASES["label into its input (bool)"] = lambda m: (lambda x: (m.label(x, output=x), [x]))(A.copy())
CASES["label into its input (float64)"] = lambda m: (lambda x: (m.label(x, output=x), [x]))(
    A.astype(np.float64)
)
CASES["label 0-d output array"] = lambda m: (lambda o: (m.label(np.array(3), output=o), [o]))(
    np.zeros((), np.int16)
)
CASES["label empty output array"] = lambda m: (
    lambda o: (m.label(np.zeros((0, 3)), output=o), [o])
)(np.zeros((0, 3), np.int16))
CASES["label empty read-only output"] = lambda m: (
    lambda o: (m.label(np.zeros((0, 3)), output=o), [o])
)((lambda z: (z.flags.__setattr__("writeable", False), z)[1])(np.zeros((0, 3), np.int16)))
CASES["label 0-d read-only output"] = lambda m: (lambda o: (m.label(np.array(2), output=o), [o]))(
    (lambda z: (z.flags.__setattr__("writeable", False), z)[1])(np.zeros((), np.int16))
)
CASES["label 0-d output type float16"] = mk("label", input=lambda: np.array(2), output=np.float16)
CASES["label empty output str 'x'"] = mk("label", input=lambda: np.zeros((0, 3)), output="x")

# sum_labels, sum, center_of_mass
LAB = (np.arange(30).reshape(5, 6) % 3).astype(np.int32)
VALS = {
    "float64": lambda: np.arange(30.0).reshape(5, 6) * 0.25,
    "float16": lambda: (np.arange(30.0).reshape(5, 6) * 0.25).astype(np.float16),
    "float32": lambda: (np.arange(30.0).reshape(5, 6) * 0.25).astype(np.float32),
    "longdouble": lambda: (np.arange(30.0).reshape(5, 6) * 0.25).astype(np.longdouble),
    "complex128": lambda: np.arange(30.0).reshape(5, 6) * (1 + 2j),
    "object": lambda: np.arange(30).reshape(5, 6).astype(object),
    "bool": lambda: A.copy(),
    ">f8": lambda: (np.arange(30.0).reshape(5, 6) * 0.25).astype(">f8"),
    "int8": lambda: np.arange(30).reshape(5, 6).astype(np.int8),
    "uint64": lambda: np.arange(30).reshape(5, 6).astype(np.uint64),
    "list": lambda: np.arange(30).reshape(5, 6).tolist(),
    "U3": lambda: np.arange(30).reshape(5, 6).astype("U3"),
    "strided": lambda: np.repeat(np.arange(30.0).reshape(5, 6), 2, axis=1)[:, ::2],
}
LABELS = {
    "int32": lambda: LAB.copy(),
    "None": lambda: None,
    "float64": lambda: LAB.astype(np.float64),
    "float with 1.5": lambda: np.where(LAB == 2, 1.5, LAB),
    "negative": lambda: LAB - 1,
    "beyond size": lambda: LAB * 20,
    "uint64": lambda: LAB.astype(np.uint64),
    "uint32": lambda: LAB.astype(np.uint32),
    "bool": lambda: LAB.astype(bool),
    "object": lambda: LAB.astype(object),
    ">i4": lambda: LAB.astype(">i4"),
    "longlong": lambda: LAB.astype(np.longlong),
    "broadcast (1, 6)": lambda: LAB[:1],
    "another shape": lambda: LAB[:2, :2],
    "list": lambda: LAB.tolist(),
    "complex": lambda: LAB.astype(np.complex128),
    "U3": lambda: LAB.astype("U3"),
    "float16": lambda: LAB.astype(np.float16),
    "strided": lambda: np.repeat(LAB, 2, axis=1)[:, ::2],
}
INDEX = {
    "None": None, "1": 1, "1.5": 1.5, "numpy int 1": np.int64(1), "[0, 1, 2, 5]": [0, 1, 2, 5],
    "[1.5, 2]": [1.5, 2], "[[0, 1], [2, 3]]": [[0, 1], [2, 3]], "0-d array": np.array(2),
    "uint64 array": np.array([1, 2], np.uint64), "'1'": "1", "[-1, 1]": [-1, 1], "range(4)": range(4),
    "[]": [], "tuple (1, 2)": (1, 2), "float array": np.array([1.0, 2.0]), "True": True,
    "[True, False]": [True, False], "2**63": 2**63, "[2**63]": [2**63], "complex 1j": 1j,
}  # fmt: skip
for fn in ("sum_labels", "sum", "center_of_mass"):
    for k, v in VALS.items():
        CASES[f"{fn} input {k} labels int32 index [0, 1, 2, 5]"] = mk(
            fn, input=v, labels=LABELS["int32"], index=[0, 1, 2, 5]
        )
        CASES[f"{fn} input {k} labels int32 index 1"] = mk(
            fn, input=v, labels=LABELS["int32"], index=1
        )
        CASES[f"{fn} input {k} labels None"] = mk(fn, input=v)
        CASES[f"{fn} input {k} labels int32 index None"] = mk(fn, input=v, labels=LABELS["int32"])
    for k, v in LABELS.items():
        CASES[f"{fn} labels {k} index [0, 1, 2, 5]"] = mk(
            fn, input=VALS["float64"], labels=v, index=[0, 1, 2, 5]
        )
        CASES[f"{fn} labels {k} index 1"] = mk(fn, input=VALS["float64"], labels=v, index=1)
        CASES[f"{fn} labels {k} index None"] = mk(fn, input=VALS["float64"], labels=v)
    for k, v in INDEX.items():
        CASES[f"{fn} index {k}"] = mk(fn, input=VALS["float64"], labels=LABELS["int32"], index=v)
        CASES[f"{fn} index {k} labels float64"] = mk(
            fn, input=VALS["float64"], labels=LABELS["float64"], index=v
        )
        CASES[f"{fn} index {k} labels uint64"] = mk(
            fn, input=VALS["float64"], labels=LABELS["uint64"], index=v
        )
    CASES[f"{fn} 0-d input"] = mk(fn, input=lambda: np.array(3.0))
    CASES[f"{fn} 0-d input labels 0-d"] = mk(
        fn, input=lambda: np.array(3.0), labels=lambda: np.array(1), index=[1]
    )
    CASES[f"{fn} empty input"] = mk(fn, input=lambda: np.zeros((0, 3)))
    CASES[f"{fn} empty input index [1]"] = mk(
        fn, input=lambda: np.zeros((0, 3)), labels=lambda: np.zeros((0, 3), int), index=[1]
    )
    CASES[f"{fn} a label with nothing"] = mk(
        fn, input=VALS["float64"], labels=LABELS["int32"], index=[7]
    )
    CASES[f"{fn} zero mass"] = mk(
        fn, input=lambda: np.zeros(SHAPE), labels=LABELS["int32"], index=[1, 2]
    )
    CASES[f"{fn} 1-D input"] = mk(
        fn, input=lambda: np.arange(7.0), labels=lambda: np.arange(7) % 2, index=[0, 1]
    )
    CASES[f"{fn} 3-D input"] = mk(
        fn,
        input=lambda: np.arange(24.0).reshape(2, 3, 4),
        labels=lambda: np.arange(24).reshape(2, 3, 4) % 3,
        index=[0, 1, 2],
    )
    CASES[f"{fn} large labels uint32 2**31"] = mk(
        fn,
        input=VALS["float64"],
        labels=lambda: LAB.astype(np.uint32) * 2**30,
        index=[0, 2**30, 2**31],
    )
    CASES[f"{fn} int64 labels 2**40"] = mk(
        fn, input=VALS["float64"], labels=lambda: LAB.astype(np.int64) * 2**40, index=[0, 2**40]
    )

# watershed_ift
IMG = (np.arange(30).reshape(5, 6) * 7 % 11).astype(np.uint8)
MRK = np.zeros(SHAPE, np.int32)
MRK[0, 0] = 1
MRK[4, 5] = 2
MRK[2, 3] = -1
W_IN = {
    "uint8": lambda: IMG.copy(), "uint16": lambda: IMG.astype(np.uint16), ">u2": lambda: IMG.astype(">u2"),
    "int8": lambda: IMG.astype(np.int8), "float32": lambda: IMG.astype(np.float32), "list": lambda: IMG.tolist(),
    "bool": lambda: IMG > 3, "strided uint8": lambda: np.repeat(IMG, 2, axis=1)[:, ::2],
    "ushort": lambda: IMG.astype(np.ushort), "Fortran uint16": lambda: np.asfortranarray(IMG.astype(np.uint16)),
}  # fmt: skip
W_MRK = {
    "int8": lambda: MRK.astype(np.int8), "int16": lambda: MRK.astype(np.int16), "int64": lambda: MRK.astype(np.int64),
    "intc": lambda: MRK.astype(np.intc), "intp": lambda: MRK.astype(np.intp), "longlong": lambda: MRK.astype(np.longlong),
    "uint8": lambda: MRK.astype(np.uint8), ">i4": lambda: MRK.astype(">i4"), "bool": lambda: MRK != 0,
    "float64": lambda: MRK.astype(np.float64), "list": lambda: MRK.tolist(), "strided int32": lambda: np.repeat(MRK, 2, axis=1)[:, ::2],
    "another shape": lambda: MRK[:2],
}  # fmt: skip
W_OUT = {
    "type int16": lambda: np.int16, "str 'x'": lambda: "x", "array int32": lambda: np.zeros(SHAPE, np.int32),
    "array int8": lambda: np.zeros(SHAPE, np.int8), "array int64": lambda: np.zeros(SHAPE, np.int64),
    "array >i4": lambda: np.zeros(SHAPE, ">i4"), "array uint8": lambda: np.zeros(SHAPE, np.uint8),
    "array read-only int32": lambda: ro_array(np.int32), "array of another shape": lambda: np.zeros((5, 7), np.int32),
    "array strided int32": lambda: np.zeros((5, 12), np.int32)[:, ::2], "array longlong": lambda: np.zeros(SHAPE, np.longlong),
    "array bool": lambda: np.zeros(SHAPE, bool), "list": lambda: np.zeros(SHAPE, int).tolist(),
    "array Fortran int16": lambda: np.zeros(SHAPE, np.int16, order="F"), "array float64": lambda: np.zeros(SHAPE),
}  # fmt: skip
for k, v in W_IN.items():
    CASES[f"watershed_ift input {k}"] = mk("watershed_ift", input=v, markers=lambda: MRK.copy())
for k, v in W_MRK.items():
    CASES[f"watershed_ift markers {k}"] = mk("watershed_ift", input=lambda: IMG.copy(), markers=v)
for k, v in W_OUT.items():
    CASES[f"watershed_ift output {k}"] = mk(
        "watershed_ift", input=lambda: IMG.copy(), markers=lambda: MRK.copy(), output=v
    )
for k, v in STRUCTS.items():
    CASES[f"watershed_ift structure {k}"] = mk(
        "watershed_ift", input=lambda: IMG.copy(), markers=lambda: MRK.copy(), structure=v
    )
CASES["watershed_ift 0-d"] = mk(
    "watershed_ift", input=lambda: np.array(3, np.uint8), markers=lambda: np.array(1, np.int32)
)
CASES["watershed_ift empty"] = mk(
    "watershed_ift",
    input=lambda: np.zeros((0, 3), np.uint8),
    markers=lambda: np.zeros((0, 3), np.int32),
)
CASES["watershed_ift 8-D"] = mk(
    "watershed_ift",
    input=lambda: np.zeros((1,) * 8, np.uint8),
    markers=lambda: np.zeros((1,) * 8, np.int32),
)
CASES["watershed_ift 8-D read-only output"] = mk(
    "watershed_ift",
    input=lambda: np.zeros((1,) * 8, np.uint8),
    markers=lambda: np.zeros((1,) * 8, np.int32),
    output=lambda: (lambda z: (z.flags.__setattr__("writeable", False), z)[1])(
        np.zeros((1,) * 8, np.int32)
    ),
)
CASES["watershed_ift markers into output (the same array)"] = lambda m: (
    lambda mk_: (m.watershed_ift(IMG.copy(), mk_, output=mk_), [mk_])
)(MRK.copy())

# relay 22's extra cases: mask values out of int8's range, F-ordered label outputs, bool outputs
for k, v in {"nan": np.nan, "inf": np.inf, "1e10": 1e10, "5e9+1": 5e9 + 1, "300": 300.0,
             "-129": -129.0, "2**63": 2.0**63}.items():  # fmt: skip
    for it in (2, -1):
        CASES[f"binary_erosion mask with {k} iterations {it}"] = mk(
            "binary_erosion",
            input=lambda: A.copy(),
            iterations=it,
            mask=lambda v=v: np.where(A | np.eye(5, 6, dtype=bool), v, 0.0),
        )
        CASES[f"binary_dilation mask with {k} iterations {it}"] = mk(
            "binary_dilation",
            input=lambda: A.copy(),
            iterations=it,
            mask=lambda v=v: np.where(np.eye(5, 6, dtype=bool), 1.0, v),
        )
_RNG = np.random.default_rng(22)
for t in range(12):
    _x = _RNG.random((6, 7, 5)) < 0.45
    CASES[f"label F output {t}"] = lambda m, x=_x: (lambda o: (m.label(x, output=o), [o]))(
        np.zeros(x.shape, np.int32, order="F")
    )
    CASES[f"label strided axis-0 output {t}"] = lambda m, x=_x: (
        lambda o: (m.label(x, output=o), [o])
    )(
        np.zeros((x.shape[0], x.shape[1], 2 * x.shape[2]), np.int16)[:, :, ::2]
        .transpose(2, 1, 0)
        .transpose(2, 1, 0)
    )
    CASES[f"label reversed output {t}"] = lambda m, x=_x: (lambda o: (m.label(x, output=o), [o]))(
        np.zeros(x.shape, np.int32)[:, ::-1]
    )
    CASES[f"label F bool output {t}"] = lambda m, x=_x: (lambda o: (m.label(x, output=o), [o]))(
        np.zeros(x.shape, bool, order="F")
    )
    CASES[f"label bool output {t}"] = lambda m, x=_x: (lambda o: (m.label(x, output=o), [o]))(
        np.zeros(x.shape, bool)
    )
    _y = _RNG.random((40, 50)) < 0.5
    CASES[f"label bool output many {t}"] = lambda m, x=_y: (lambda o: (m.label(x, output=o), [o]))(
        np.zeros(x.shape, bool)
    )
    _z = np.zeros((60, 60), bool)
    _z[::2, ::3] = _RNG.random((30, 20)) < (0.2 + 0.05 * t)
    CASES[f"label bool output near 255 {t}"] = lambda m, x=_z: (
        lambda o: (m.label(x, output=o), [o])
    )(np.zeros(x.shape, bool))
    CASES[f"label uint8 output near 255 {t}"] = lambda m, x=_z: (
        lambda o: (m.label(x, output=o), [o])
    )(np.zeros(x.shape, np.uint8))


class _Repro:
    """``lx.ndimage`` with ``upstream_defects="reproduce"`` wherever a function takes it."""

    def __getattr__(self, name):
        f = getattr(lnd, name)
        if "upstream_defects" in inspect.signature(f).parameters:
            return functools.partial(f, upstream_defects="reproduce")
        return f


def default_ok(s, f) -> tuple[bool, str]:
    """Whether larmorx's default outcome ``f`` is SciPy's ``s`` or a registered correction, and
    which."""
    if s[:3] == f[:3] and not f[3]:
        return True, "same"
    if f[3]:
        return set(f[3]) <= KNOWN, "+".join(sorted(set(f[3])))
    if "SCI-28" in str(f[0]):
        return True, "SCI-28"
    if s[:2] == f[:2] and [m for m in s[2] if WRITEBACK not in m] == f[2]:
        return True, "SCI-26"
    return False, "no notice"


def cases(names=()):
    """(name, reproduce = SciPy, default ok, the default's category, detail) per case."""
    for name, case in CASES.items():
        if names and not any(o in name for o in names):
            continue
        s, f, r = _oracle(outcome(case, snd)), outcome(case, lnd), outcome(case, _Repro())
        ok, cat = default_ok(s, f)
        yield (
            name,
            s[:3] == r[:3],
            ok,
            cat,
            f"scipy {short(s)}; larmorx {short(f)}; reproduce {short(r)}",
        )


def sci25_family(trials: int = 60):
    """Iterated erosions and dilations with a mask into output arrays of every type: reproduce
    = SciPy's bytes and warnings; the default = SciPy's result into a bool output, cast, with
    SCI-25's notice exactly where SciPy's differs. Yields (name, ok, detail)."""
    rng = np.random.default_rng(5)
    for _ in range(trials):
        shape = (6, 7)
        x = rng.random(shape) < 0.6
        m = rng.random(shape) < 0.7
        it = int(rng.choice([2, 3, -1, 0]))
        for dt in (np.float32, np.float64, np.int32, np.uint8, np.int16, ">i4", ">f4", np.float16):
            for fn in ("binary_erosion", "binary_dilation", "binary_closing"):
                for bf in (False, True):
                    kw = {"iterations": it, "mask": m, "brute_force": bf}
                    label = f"{fn} {np.dtype(dt).str} iterations={it} brute_force={bf}"
                    o1, o2, o3 = (np.zeros(shape, dt) for _ in range(3))
                    s = _oracle(
                        outcome(lambda md, o=o1: (getattr(md, fn)(x, output=o, **kw), [o]), snd)
                    )
                    r = outcome(
                        lambda md, o=o2: (
                            getattr(md, fn)(x, output=o, **kw, upstream_defects="reproduce"),
                            [o],
                        ),
                        lnd,
                    )
                    yield (
                        "SCI-25 family: reproduce = SciPy",
                        s[:3] == r[:3],
                        f"{label}: {short(s)} / {short(r)}",
                    )
                    f = outcome(lambda md, o=o3: (getattr(md, fn)(x, output=o, **kw), [o]), lnd)
                    if s[0][0] in ("<the output array>",):
                        ref = getattr(snd, fn)(x, **kw)
                        want = np.zeros(shape, dt)
                        np.copyto(want, ref, casting="unsafe")
                        differs = o1.tobytes() != want.tobytes()
                        ok = (
                            o3.tobytes() == want.tobytes()
                            and (("SCI-25" in f[3]) == differs)
                            and set(f[3]) <= {"SCI-25"}
                        )
                        yield (
                            "SCI-25 family: default = SciPy's bool result cast, notice where it differs",
                            ok,
                            f"{label}: {f[3]}",
                        )
                    else:
                        yield (
                            "SCI-25 family: default = SciPy's error",
                            s[:3] == f[:3],
                            f"{label}: {short(f)}",
                        )


def label_order_family(trials: int = 100):
    """``label`` into Fortran-ordered, strided and C-ordered outputs of 2-D and 3-D inputs:
    SciPy's numbering (along the output's axis of the smallest stride). Yields (name, ok,
    detail)."""
    rng = np.random.default_rng(3)
    for _ in range(trials):
        shape = tuple(int(v) for v in rng.integers(2, 9, size=int(rng.integers(2, 4))))
        x = rng.random(shape) < 0.45
        for kind in (
            "F output",
            "F input",
            "strided output",
            "reversed output",
            "transposed output",
        ):
            if kind == "F output":
                o1, o2, xi = (
                    np.zeros(shape, np.int32, order="F"),
                    np.zeros(shape, np.int32, order="F"),
                    x,
                )
            elif kind == "F input":
                o1, o2, xi = (
                    np.zeros(shape, np.int32),
                    np.zeros(shape, np.int32),
                    np.asfortranarray(x),
                )
            elif kind == "strided output":
                o1, o2 = (np.zeros((*shape, 2), np.int32)[..., 0] for _ in range(2))
                xi = x
            elif kind == "reversed output":
                o1, o2, xi = (
                    np.zeros(shape, np.int32)[..., ::-1],
                    np.zeros(shape, np.int32)[..., ::-1],
                    x,
                )
            else:
                o1 = np.zeros(shape[::-1], np.int16).T
                o2, xi = np.zeros(shape[::-1], np.int16).T, x
            a, b = _oracle((snd.label(xi, output=o1), o1)), lnd.label(xi, output=o2)
            a = a[0]
            yield (
                f"label numbering: {kind}",
                a == b and o1.tobytes() == o2.tobytes() and np.array_equal(o1, o2),
                f"{shape}: {a} / {b}",
            )


def main() -> int:
    names = [a for a in sys.argv[1:] if not a.startswith("--")]
    n = bad_r = bad_f = 0
    cats: dict[str, int] = {}
    for name, rep, ok, cat, detail in cases(names):
        n += 1
        cats[cat] = cats.get(cat, 0) + 1
        if not rep:
            bad_r += 1
            print(f"DIFF reproduce {name}: {detail}")
        if not ok:
            bad_f += 1
            print(f"DIFF default {name}: {detail}")
    fam = {}
    if not names:
        for name, ok, detail in [*sci25_family(), *label_order_family()]:
            t = fam.setdefault(name, [0, 0])
            t[0] += 1
            t[1] += ok
            if not ok:
                print(f"DIFF {name}: {detail}")
    print(
        f"{n} cases: reproduce differs in {bad_r}, the default unexplained in {bad_f}; the default: {cats}"
    )
    for name, (t, k) in fam.items():
        print(f"{name}: {t} cases, identical {k}")
    return 0 if bad_r == bad_f == 0 and all(t == k for t, k in fam.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
