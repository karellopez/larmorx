# SPDX-License-Identifier: Apache-2.0
"""nipy 0.6.1's motion-parameter helpers, compiled from the pinned tag's own source, for nipype's
``normalize_mc_params(params, "NIPY")`` (``FramewiseDisplacement(parameter_source="NIPY")``).

nipy is installed in no environment (its registration package imports compiled parts), and
nipype imports ``to_matrix44`` and ``aff2euler`` from ``nipy.algorithms.registration`` when it
needs them. :func:`install` puts a module of that name into ``sys.modules`` holding nipy 0.6.1's
``threshold``, ``rotation_vec2mat``, ``to_matrix44``, ``aff2rot_zooms`` and ``aff2euler`` and the
constants they read (``MAX_ANGLE``, ``SMALL_ANGLE``, ``MAX_DIST``, ``LOG_MAX_DIST``), compiled
from ``<workspace>/tags/nipy-0.6.1/nipy/algorithms/registration/affine.py`` and
``scripting.py`` as written (the files' own function and constant definitions, nothing else;
nipy 0.6.1, BSD-3-Clause, Copyright (c) 2006-2024, NIPY Developers), with numpy,
``numpy.linalg`` and nibabel's ``eulerangles`` as their globals, or with the C library's
functions swapped (``variant``):

- ``"plain"``: as this machine computes them: numpy's float64 ``sin`` and ``cos`` (glibc's),
  ``exp`` (Intel's SVML on CPUs with AVX-512 unless ``NPY_DISABLE_CPU_FEATURES`` turns it off,
  glibc's otherwise: NP-04), CPython's ``math.atan2`` (glibc's).
- ``"reproduce"``: what larmorx's ``upstream_defects="reproduce"`` follows: numpy's ``exp``
  through glibc's (``math.exp``: numpy without AVX-512) and ``atan2`` correctly rounded
  (CORE-MATH's ``cr_atan2`` behind CPython's special cases: the orchestrator's D4).
- ``"reproduce-npexp"``: ``"reproduce"`` with numpy's own ``exp`` (to tell an ``exp`` difference
  from an ``atan2`` one).
- ``"fix"``: larmorx's default: ``sin``, ``cos``, ``exp`` and ``atan2`` correctly rounded
  (CORE-MATH's ``cr_*``).

CORE-MATH's functions come from ``<workspace>/oracles/core-math-preload/libcrmath.so``
(``scripts/build_crmath_preload.sh``), called through ``ctypes`` (not preloaded): an independent
build of the correctly rounded functions larmorx ports.
"""

from __future__ import annotations

import ast
import ctypes
import math
import os
import sys
import types
from pathlib import Path

import nibabel.eulerangles as nib_euler
import numpy as np
import numpy.linalg as npl

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
NIPY = WORKSPACE / "tags" / "nipy-0.6.1" / "nipy" / "algorithms" / "registration"
CRMATH = WORKSPACE / "oracles" / "core-math-preload" / "libcrmath.so"
VARIANTS = ("plain", "reproduce", "reproduce-npexp", "fix")
FUNCTIONS = {
    "affine.py": ("threshold", "rotation_vec2mat", "to_matrix44"),
    "scripting.py": ("aff2euler", "aff2rot_zooms"),
}
CONSTANTS = ("MAX_ANGLE", "SMALL_ANGLE", "MAX_DIST", "LOG_MAX_DIST")

_cr = ctypes.CDLL(str(CRMATH))
for _name, _n in (("cr_sin", 1), ("cr_cos", 1), ("cr_exp", 1), ("cr_atan2", 2)):
    getattr(_cr, _name).restype = ctypes.c_double
    getattr(_cr, _name).argtypes = [ctypes.c_double] * _n


def cr(name: str, *args: float) -> float:
    """CORE-MATH's correctly rounded ``name`` (``sin``, ``cos``, ``exp``, ``atan2``)."""
    return getattr(_cr, f"cr_{name}")(*args)


def py_atan2(atan2):
    """CPython 3.12's ``math.atan2`` (``m_atan2``) around another C ``atan2``."""

    def f(y, x):
        y, x = float(y), float(x)
        if math.isnan(x) or math.isnan(y):
            return math.nan
        if math.isinf(y):
            if math.isinf(x):
                return math.copysign((0.25 if math.copysign(1.0, x) == 1.0 else 0.75) * math.pi, y)
            return math.copysign(0.5 * math.pi, y)
        if math.isinf(x) or y == 0.0:
            return math.copysign(0.0 if math.copysign(1.0, x) == 1.0 else math.pi, y)
        return atan2(y, x)

    return f


def _elementwise(fn):
    """numpy's float64 ufunc ``fn`` of one argument, value by value through a C function."""

    def f(x):
        a = np.asarray(x, dtype=np.float64)
        out = np.array([fn(float(v)) for v in a.ravel()], dtype=np.float64).reshape(a.shape)
        return out[()] if a.ndim == 0 else out

    return f


class _Numpy(types.ModuleType):
    """numpy with some of its functions replaced."""

    def __init__(self, **swap):
        super().__init__("numpy")
        self.__dict__.update(swap)

    def __getattr__(self, name):
        return getattr(np, name)


def _numpy(variant: str):
    if variant == "plain":
        return np
    if variant in ("reproduce", "reproduce-npexp"):
        return np if variant == "reproduce-npexp" else _Numpy(exp=_elementwise(math.exp))
    return _Numpy(
        sin=_elementwise(lambda v: cr("sin", v)),
        cos=_elementwise(lambda v: cr("cos", v)),
        exp=_elementwise(lambda v: cr("exp", v)),
    )


def _euler(variant: str):
    if variant == "plain":
        return nib_euler
    shim = types.SimpleNamespace(sqrt=math.sqrt, atan2=py_atan2(lambda y, x: cr("atan2", y, x)))
    g = dict(vars(nib_euler))
    g["math"] = shim
    mod = types.ModuleType("nibabel.eulerangles")
    f = nib_euler.mat2euler
    mod.mat2euler = types.FunctionType(f.__code__, g, "mat2euler", f.__defaults__, f.__closure__)
    return mod


def _definitions(path: Path, names: tuple[str, ...], constants: tuple[str, ...]) -> ast.Module:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    keep = []
    for node in tree.body:
        function = isinstance(node, ast.FunctionDef) and node.name in names
        constant = isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id in constants for t in node.targets
        )
        if function or constant:
            keep.append(node)
    found = {n.name for n in keep if isinstance(n, ast.FunctionDef)}
    missing = set(names) - found
    if missing:
        raise RuntimeError(f"nipy 0.6.1's {path.name} has no {sorted(missing)}")
    return ast.Module(body=keep, type_ignores=[])


def build(variant: str = "plain") -> types.ModuleType:
    """A module ``nipy.algorithms.registration`` with nipy 0.6.1's functions in ``variant``."""
    if variant not in VARIANTS:
        raise ValueError(f"variant must be one of {VARIANTS}")
    mod = types.ModuleType("nipy.algorithms.registration")
    g = mod.__dict__
    g.update(np=_numpy(variant), npl=npl, euler=_euler(variant))
    for fname, names in FUNCTIONS.items():
        path = NIPY / fname
        code = compile(_definitions(path, names, CONSTANTS), str(path), "exec")
        exec(code, g)
    return mod


def install(variant: str = "plain") -> types.ModuleType:
    """Put :func:`build`'s module (and its parents) into ``sys.modules``; return it."""
    mod = build(variant)
    for parent in ("nipy", "nipy.algorithms"):
        sys.modules.setdefault(parent, types.ModuleType(parent))
    sys.modules["nipy.algorithms.registration"] = mod
    return mod


def numpy_exp_is_glibc(n: int = 4000) -> bool:
    """Whether numpy's float64 ``exp`` gives glibc's bits here (no SVML: NP-04)."""
    x = np.random.default_rng(3).uniform(-23.1, 23.1, n)
    a = np.exp(x)
    b = np.array([math.exp(v) for v in x])
    return bool(np.array_equal(a.view(np.uint64), b.view(np.uint64)))
