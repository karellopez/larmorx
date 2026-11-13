# SPDX-License-Identifier: Apache-2.0
"""niworkflows' confound model steps: ``ExpandModel`` (``parse_formula``, ``temporal_derivatives``,
``exponential_terms``), ``SpikeRegressors`` (``spike_regressors``) and ``NormalizeMotionParams``,
larmorx against this environment's niworkflows, pandas and numpy, bit for bit.

Sections:

- ``power``: ``_core.numpy_power_f64`` against numpy's float64 ``x ** o`` (exponents 0 to 9 and
  larger, values of every magnitude, NaN, infinities, -0.0): ``"reproduce"`` is numpy's on CPUs
  without AVX-512 (glibc's ``pow``: run with ``NPY_DISABLE_CPU_FEATURES`` naming the AVX-512
  features, as the other sections; without it the differences are SVML's, counted apart: NP-04),
  ``"fix"`` against the exact power rounded once (Python's ``Fraction``).
- ``expand``: ``parse_formula`` (``unscramble`` off and on), ``temporal_derivatives``,
  ``exponential_terms`` on tables (fMRIPrep-like confound tables of 1 to 600 volumes, integer,
  unsigned, text and boolean columns, NaN, infinities, huge values) and the ``ExpandModel``
  interface on files, over every operator form (``dd<n>``, ``d<n>``, ``d<n>-<m>``, ``^^<n>``,
  ``^<n>``, ``^<n>-<m>``, nested groups, both orders of a derivative and a power), the
  shorthands and ``others``, and the failures (a missing name, an empty term, operators without a
  group). ``upstream_defects="reproduce"`` against niworkflows by name (PIPE-43: niworkflows'
  order follows Python's per-process string hashes) and byte for byte against niworkflows run
  with insertion-ordered sets (the order larmorx keeps, one niworkflows can produce); the default
  against niworkflows with ordered sets, pandas' ``round_trip`` parser (PD-01) and correctly
  rounded powers (GLIBC-07), and against equivalent formulas niworkflows reads right for the
  cases of PIPE-64, PIPE-65 and PIPE-66.
- ``spikes``: ``spike_regressors`` and the ``SpikeRegressors`` interface: thresholds, ``'<'`` and
  ``'>'``, every ``lags`` form, ``minimum_contiguous``, both outputs, ``concatenate``, header
  prefixes, more than 100 spikes, integer and boolean criteria, NaN, a table without rows, a
  missing column, an unknown criterion. ``reproduce`` against niworkflows byte for byte, the
  default against a model with PIPE-67's, PIPE-68's and PIPE-69's corrections and pandas'
  ``round_trip`` parser.
- ``normalize``: ``NormalizeMotionParams`` (``FSL``, ``AFNI`` with 6 and 7 columns, ``FSFAST``,
  ``NIPY`` with nipy 0.6.1's own functions: ``nipy_inject.py``), one row (PIPE-70), one value,
  one column, an empty file, comments, too few columns, a format niworkflows refuses.
- ``real``: fMRIPrep's own ``model_expansion`` and ``spike_regressors`` nodes in the oracle runs'
  working directories (every ``bold_confounds_wf``), with their own inputs: larmorx with
  ``pandas_version="2.2.3"`` (fMRIPrep 25.2.6's image) and ``upstream_defects="reproduce"``:
  the expansion by name against fMRIPrep's file (fMRIPrep's order checked to be one niworkflows
  can produce, larmorx's the formula's), the spike regressors byte for byte against the node's
  file and the run's ``desc-confounds_timeseries.tsv``, and the chain (larmorx's expansion
  through larmorx's spike regressors) by name against the derivative; the default's changed
  cells counted.

Run it in each environment with the worktree's extension and numpy's AVX-512 dispatch off::

    NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL
    AVX512_ICL AVX512_SPR" PYTHONPATH=<worktree>/python python -P check_model.py
    [--tier quick|full] [--sections ...]

Exit status 1 when any case differs.
"""

# ruff: noqa: B023 (every lambda here is called at once, inside its own loop iteration)

from __future__ import annotations

import argparse
import contextlib
import hashlib
import itertools
import math
import os
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_tables as ct
import nipy_inject as ni

import larmorx as lx
from larmorx import _core
from larmorx.niworkflows import confounds as lc

COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])
APART: dict[str, int] = defaultdict(int)
NOTICES: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
SECONDS: dict[str, float] = defaultdict(float)
PANDAS = pd.__version__
NUMPY = np.__version__
RUNS = ct.RUNS
NPY_OFF = "AVX512_SKX" in os.environ.get("NPY_DISABLE_CPU_FEATURES", "")


def count(key: str, ok: bool, what: str) -> None:
    COUNTS[key][0 if ok else 1] += 1
    if not ok and len(FAILS) < 80:
        FAILS.append(f"{key}: {what}")


def last_line(e: BaseException) -> str:
    return ct.last_line(e)


def entries(caught) -> list[str]:
    return [w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)]


# --- power -----------------------------------------------------------------------------------


def exact_power(x: float, o: int) -> float:
    """``x ** o`` correctly rounded (Python's ``Fraction``; IEEE's results for 0, NaN and the
    infinities)."""
    if o == 0:
        return 1.0
    if not math.isfinite(x) or x == 0.0:
        return float(np.float64(x) ** np.float64(o))
    try:
        return float(Fraction(x) ** o)
    except OverflowError:
        return math.copysign(math.inf, x) if o % 2 else math.inf


def power_values(rng: np.random.Generator, n: int) -> np.ndarray:
    parts = [
        rng.normal(0, 1e-3, n),  # motion parameters
        rng.normal(0, 0.05, n),  # their derivatives
        rng.normal(3000, 500, n),  # signals
        rng.normal(0, 1, n),
        np.sign(rng.normal(size=n)) * 10.0 ** rng.uniform(-300, 300, n),
        rng.uniform(0, 1, n) * 1e-310,
        np.round(rng.normal(0, 1e6, n)),
        np.array([0.0, -0.0, np.nan, np.inf, -np.inf, 1.0, -1.0, 2.0, 0.5, 1e-200, 1e200]),
    ]
    return np.concatenate(parts)


def power_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(41)
    vals = power_values(rng, 2000 if tier == "quick" else 20000)
    exps = (0, 2, 3, 4, 5, 6, 7, 8, 9, 13, 64) if tier == "full" else (0, 2, 3, 4, 6)
    for o in exps:
        with np.errstate(all="ignore"):
            npv = vals**o
        rep, d_rep = _core.numpy_power_f64(vals, float(o), "reproduce")
        fix, d_fix = _core.numpy_power_f64(vals, float(o), "fix")
        same_np = ct.bits_equal(npv, rep)
        if NPY_OFF or o in (0, 2):
            count("power reproduce = numpy (AVX-512 off: glibc's pow)", same_np, f"o={o}")
        else:
            n = int(np.count_nonzero(npv.view(np.uint64) != rep.view(np.uint64)))
            APART[f"power: numpy with AVX-512 (SVML) differs from glibc's pow, o={o}"] += n
        want = np.array([exact_power(float(x), o) for x in vals])
        count("power fix = the exact power rounded once", ct.bits_equal(want, fix), f"o={o}")
        n = int(np.count_nonzero(fix.view(np.uint64) != rep.view(np.uint64)))
        count("power differ count", n == d_rep == d_fix, f"o={o}: {n} {d_rep} {d_fix}")
        APART[f"power: glibc's pow is not correctly rounded, o={o}"] += n


# --- tables ----------------------------------------------------------------------------------


def fmriprep_like(rng: np.random.Generator, t: int, n_acc: int = 5, n_cos: int = 3) -> pd.DataFrame:
    """A table like fMRIPrep's ``GatherConfounds`` output (its columns, magnitudes and leading
    ``n/a``)."""
    cols: dict[str, np.ndarray] = {}
    for name in ("global_signal", "csf", "white_matter", "csf_wm", "tcompcor"):
        cols[name] = rng.normal(3000, 300, t)
    for name in ("std_dvars", "dvars", "framewise_displacement"):
        v = np.abs(rng.normal(1, 0.5, t))
        if t:
            v[0] = np.nan
        cols[name] = v
    cols["rmsd"] = np.abs(rng.normal(0.1, 0.05, t))
    if t:
        cols["rmsd"][0] = np.nan
    for i in range(3):
        cols[f"t_comp_cor_{i:02d}"] = rng.normal(0, 0.05, t)
    for p in ("c", "w", "a"):
        for i in range(n_acc):
            cols[f"{p}_comp_cor_{i:02d}"] = rng.normal(0, 0.05, t)
    for i in range(4):
        cols[f"edge_comp_{i:02d}"] = rng.normal(0, 0.05, t)
    for i in range(n_cos):
        cols[f"cosine{i:02d}"] = rng.normal(0, 0.1, t)
    nss = np.zeros(t)
    if t > 1:
        nss[0] = 1
    cols["non_steady_state_outlier00"] = nss
    for name in ("trans_x", "trans_y", "trans_z"):
        cols[name] = rng.normal(0, 0.3, t)
    for name in ("rot_x", "rot_y", "rot_z"):
        cols[name] = rng.normal(0, 3e-3, t)
    return pd.DataFrame(cols)


def odd_table(rng: np.random.Generator, t: int) -> pd.DataFrame:
    return pd.DataFrame(
        {
            "a": rng.normal(0, 1, t),
            "big": np.sign(rng.normal(size=t)) * 10.0 ** rng.uniform(100, 300, t),
            "tiny": rng.uniform(-1, 1, t) * 1e-200,
            "special": np.resize(np.array([np.nan, np.inf, -np.inf, -0.0, 0.0, 1.5]), t),
            "n": rng.integers(-1000, 1000, t).astype(np.int64),
            "huge": np.resize(np.array([2**62, -(2**62), 3, 2**31], dtype=np.int64), t),
            "u": np.resize(np.array([2**64 - 1, 2**63, 1], dtype=np.uint64), t),
            "text": np.resize(np.array(["x", "y", "z w"], dtype=object), t),
            "flag": np.resize(np.array([True, False]), t),
            "trans_x": rng.normal(0, 0.3, t),
            "std_dvars": rng.normal(1, 0.3, t),
            "dvars": rng.normal(30, 3, t),
            "non_std_dvars": rng.normal(30, 3, t),
        }
    )


class OrderedSet:
    """``set`` in insertion order: niworkflows run with it orders the formula's variables and the
    ``others`` as larmorx does (PIPE-43's model)."""

    def __init__(self, items=()):
        self.d = dict.fromkeys(items)

    def __iter__(self):
        return iter(self.d)

    def __len__(self):
        return len(self.d)

    def __contains__(self, x):
        return x in self.d

    def __sub__(self, other):
        return OrderedSet(x for x in self.d if x not in other)


def exact_pow_frame(df: pd.DataFrame, o) -> pd.DataFrame:
    out = {}
    for c in df.columns:
        s = df[c]
        if s.dtype == np.float64 and o not in (0, 2):
            out[c] = np.array([exact_power(float(x), int(o)) for x in s.to_numpy()])
        else:
            with np.errstate(all="ignore"):
                out[c] = s.to_numpy() ** o
    return pd.DataFrame(out, columns=df.columns) if len(set(df.columns)) == len(df.columns) else (
        df**o
    )  # fmt: skip


@contextlib.contextmanager
def niworkflows_model(default: bool):
    """niworkflows with insertion-ordered sets (larmorx's order); with ``default`` also pandas'
    correctly rounded parser and correctly rounded powers (larmorx's default)."""
    from niworkflows.interfaces import confounds as nwc

    nwc.set = OrderedSet
    pow_ = pd.DataFrame.__pow__
    if default:
        pd.DataFrame.__pow__ = exact_pow_frame
    try:
        with ct.model_default() if default else contextlib.nullcontext():
            yield nwc
    finally:
        del nwc.set
        pd.DataFrame.__pow__ = pow_


def frame_columns(df) -> list[tuple[str, bytes]]:
    """A table's columns as (name, the column's bits or text) pairs, sorted (order ignored)."""
    out = []
    for j in range(df.shape[1]):
        s = df.iloc[:, j]
        v = s.to_numpy()
        key = v.astype(np.float64).tobytes() if v.dtype.kind in "fiu" else repr(list(v)).encode()
        out.append((str(df.columns[j]), str(v.dtype).encode() + key))
    return sorted(out)


def table_columns(t: lc.ConfoundTable) -> list[tuple[str, bytes]]:
    out = []
    for n, v in zip(t.names, t.columns, strict=True):
        if isinstance(v, list):
            out.append((n, b"object" + repr(v).encode()))
        else:
            v = np.asarray(v)
            key = (
                v.astype(np.float64).tobytes() if v.dtype.kind in "fiu" else repr(list(v)).encode()
            )
            out.append((n, str(v.dtype).encode() + key))
    return sorted(out)


def outcome_orig(fn):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            return fn()
        except Exception as e:
            return last_line(e)


def outcome_lx(fn):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = fn()
        except Exception as e:
            out = last_line(e)
    return out, entries(caught)


def same_frames(want, got, ordered: bool) -> bool:
    if isinstance(want, str) or isinstance(got, str):
        if not (isinstance(want, str) and isinstance(got, str)):
            return False
        # By name, an error is compared by its type: which name niworkflows meets first follows
        # its set order (PIPE-43); the ordered comparison checks the message.
        return want == got if ordered else want.split(":")[0] == got.split(":")[0]
    a, b = frame_columns(want), table_columns(got)
    if ordered:
        return [n for n in want.columns] == list(got.names) and a == b
    return a == b


# fMRIPrep's formula, the docstring's examples, every operator form, shorthands, failures.
FORMULAS = [
    lc.DEFAULT_FORMULA,
    "rps + wm + csf + gsr",
    "(dd1(rps + wm + csf + gsr))^^2",
    "(dd1(rps))^^2",
    "(dd1(rps + wm + csf + gsr))^^2 + others",
    "dd6(trans_x)", "d4-6(trans_x + rot_z)", "d0(csf)", "d2(rps)", "dd0(gsr)", "d1-1(wm)",
    "(trans_x)^^6", "(rot_y)^4-6", "(wm)^3", "(csf)^0", "(csf)^1", "(csf)^0-2", "(csf)^^1",
    "d1(csf)^2", "(dd1(csf))^2", "(dd2(rps))^^3 + fd", "dd1((rps)^^2)", "(dd1((wm + csf)^2))^^2",
    "dd1(dd1(gsr))", "((csf + wm))^^2", "(dd1(acc))^^2 + tcc", "dd1(dv) + nss", "(fd)^^2 + spikes",
    "a_comp_cor_00 + (cosine00)^2 + others", "others", "(others)^2", "d7-9(gsr)", "d3-1(gsr)",
    "missing_column + csf", "csf +", "csf + + wm", "()", "(csf", "csf)", "d1-(csf)",
    "dd1 (csf)", "(csf) ^^2", " csf  +  wm ",
]  # fmt: skip

ODD_FORMULAS = [
    "(a + n)^^3", "dd1(n + a)", "(dd1(n))^^2", "(huge)^^3", "(n)^0-3", "dd1(huge)", "dd2(u)",
    "(u)^^2", "(a + u)^2", "dd1(a + u)", "(big)^^3", "(tiny)^^4", "(special)^^5", "dd1(special)",
    "(n)^1", "dd0(n)", "(text)^2", "dd1(flag)", "text + flag + a", "(a)^^2 + others",
    "(trans_x)^^9", "(a)^13", "(a)^64",
]  # fmt: skip

# The defects' cases: (formula, table, an equivalent formula niworkflows reads right on the
# table with the columns renamed, the renaming, larmorx's default expected to raise).
DEFECT_CASES = [
    ("dvall", {"std_dvars": "Q0", "dvars": "Q1", "non_std_dvars": "Q2"}, "Q0 + Q1 + Q2", "PIPE-64"),
    ("(dd1(std_dvars + a))^^2 + n", {"std_dvars": "Q0"}, "(dd1(Q0 + a))^^2 + n", "PIPE-64"),
    ("(dvall)^2 + a", {"std_dvars": "Q0", "dvars": "Q1", "non_std_dvars": "Q2"},
     "(Q0 + Q1 + Q2)^2 + a", "PIPE-64"),
    ("(acc)^^2 + a", {}, "a", "PIPE-64"),
    ("(dd1(nss + a))^^2", {}, "(dd1(a))^^2", "PIPE-64"),
]  # fmt: skip


def defect_tables(rng: np.random.Generator, t: int) -> list[tuple[str, pd.DataFrame, str, str]]:
    """More of PIPE-64's, PIPE-65's and PIPE-66's cases, each with its own table: (formula,
    table, equivalent formula on the renamed table, renaming)."""
    x = rng.normal(0, 1, t)
    y = rng.normal(0, 1, t)
    z = rng.normal(0, 1, t)
    out = [
        # others when there are none
        ("x + others", pd.DataFrame({"x": x}), "x", {}),
        ("(dd1(x))^^2 + others", pd.DataFrame({"x": x}), "(dd1(x))^^2", {}),
        # a column named like a shorthand inside others (re-expanded in the group)
        ("(others)^^2", pd.DataFrame({"x": x, "wm": y}), "(others)^^2", {"wm": "Q0"}),
        # names holding a shorthand's letters
        ("x + brothers + others", pd.DataFrame({"x": x, "brothers": y, "z": z}),
         "x + Q0 + others", {"brothers": "Q0"}),
        ("(swm)^2 + others", pd.DataFrame({"swm": x, "z": z}), "(Q0)^2 + others", {"swm": "Q0"}),
        # a name with d<digits>: niworkflows also counts it among the others
        ("head1 + others", pd.DataFrame({"head1": x, "z": z}), "Q0 + others", {"head1": "Q0"}),
        # PIPE-65: names read as operators, nested exponents
        ("d1a + z", pd.DataFrame({"d1a": x, "z": z}), "Q0 + z", {"d1a": "Q0"}),
        ("x^2 + z", pd.DataFrame({"x^2": x, "z": z}), "Q0 + z", {"x^2": "Q0"}),
        # PIPE-66: a variable reached twice
        ("x + (x)^^2", pd.DataFrame({"x": x, "z": z}), "(x)^^2", {}),
        ("dd1(x + (x))", pd.DataFrame({"x": x}), "dd1(x)", {}),
        ("(dd1(x))^^2 + dd1(x) + x", pd.DataFrame({"x": x}), "(dd1(x))^^2", {}),
    ]  # fmt: skip
    return out


RAISES = [  # PIPE-65: larmorx's default refuses the text niworkflows ignores
    ("foo(x)", "ValueError"), ("(x)b", "ValueError"), ("d1x(x)", "ValueError"),
    ("d1(x)(y)", "ValueError"), ("dd1-2(x)", "ValueError"), ("(x)^ ^2", "ValueError"),
]  # fmt: skip


def write_df(path: Path, df: pd.DataFrame) -> str:
    df.to_csv(path, sep="\t", index=False, na_rep="n/a")
    return str(path)


def run_iface(klass, d: Path, model=None, **inputs):
    d.mkdir(parents=True)
    with ct.cwd(d), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        cm = niworkflows_model(model == "default") if model else contextlib.nullcontext()
        try:
            with cm:
                res = klass(**inputs).run()
        except Exception as e:
            return last_line(e)
    return ct.files_of(d), res.outputs.get()


def run_lx_file(fn, d: Path, *args, **kw):
    d.mkdir(parents=True)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = fn(*args, out_dir=d, **kw)
        except Exception as e:
            return last_line(e), entries(caught)
    return (ct.files_of(d), out), entries(caught)


def tsv_by_name(data: bytes) -> list[tuple[str, tuple[str, ...]]]:
    lines = data.decode().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    rows = [line.split("\t") for line in lines]
    if not rows:
        return []
    names = rows[0]
    return sorted(
        (n, tuple(r[j] if j < len(r) else "" for r in rows[1:])) for j, n in enumerate(names)
    )


def files_by_name(want, got) -> bool:
    if isinstance(want, str) or isinstance(got, str):
        both = isinstance(want, str) and isinstance(got, str)
        return both and want.split(":")[0] == got.split(":")[0]
    wf, gf = want[0], got[0]
    if sorted(wf) != sorted(gf):
        return False
    return all(tsv_by_name(wf[k]) == tsv_by_name(gf[k]) for k in wf)


def files_exact(want, got) -> bool:
    if isinstance(want, str) or isinstance(got, str):
        return want == got
    return want[0] == got[0]


def expand_section(tier: str, tmp: Path) -> None:
    from niworkflows.interfaces import confounds as nwc

    rng = np.random.default_rng(23)
    inputs = tmp / "inputs"
    inputs.mkdir()
    k = [0]
    ts = (1, 2, 3, 10, 240, 600) if tier == "full" else (1, 3, 40)
    tables = [(f"fmriprep-like T={t}", fmriprep_like(rng, t), FORMULAS) for t in ts]
    tables += [(f"odd T={t}", odd_table(rng, t), ODD_FORMULAS + FORMULAS[-8:]) for t in (1, 7, 60)]
    for name, df, formulas in tables:
        p = write_df(inputs / f"t{k[0]}.tsv", df)
        k[0] += 1
        parent = pd.read_csv(p, sep="\t")
        mapping = {c: (list(parent[c]) if parent[c].dtype == object else parent[c].to_numpy())
                   for c in parent.columns}  # fmt: skip
        for f in formulas:
            # functions on the table (as read)
            for unscramble in (False, True):
                what = f"{name}: {f!r} unscramble={unscramble}"
                want = outcome_orig(lambda f=f, u=unscramble: nwc.parse_formula(f, parent, u))
                got, notes = outcome_lx(
                    lambda f=f, u=unscramble: lc.parse_formula(
                        f, mapping, u, upstream_defects="reproduce"
                    )
                )
                ok = same_frames(
                    want if isinstance(want, str) else want[1],
                    got if isinstance(got, str) else got.data,
                    ordered=False,
                )
                if ok and not isinstance(want, str):
                    ok = sorted(want[0]) == sorted(got.variables)
                count("parse_formula reproduce (by name)", ok, what)
                with niworkflows_model(False) as m:
                    model = outcome_orig(lambda f=f, u=unscramble: m.parse_formula(f, parent, u))
                ok = same_frames(
                    model if isinstance(model, str) else model[1],
                    got if isinstance(got, str) else got.data,
                    ordered=True,
                )
                if ok and not isinstance(model, str):
                    ok = model[0] == got.variables
                count("parse_formula reproduce = niworkflows with ordered sets", ok, what)
                for e in set(notes):
                    NOTICES[f"parse_formula reproduce: {e}"] += 1
            # the interface
            k[0] += 1
            d = tmp / f"e{k[0]}"
            want = run_iface(nwc.ExpandModel, d / "orig", confounds_file=p, model_formula=f)
            got, notes = run_lx_file(
                lc.expand_model, d / "lx", p, f, pandas_version=PANDAS, upstream_defects="reproduce"
            )
            count("ExpandModel reproduce (by name)", files_by_name(want, got), f"{name}: {f!r}")
            want = run_iface(
                nwc.ExpandModel, d / "orig-ordered", model="ordered", confounds_file=p,
                model_formula=f,
            )  # fmt: skip
            count("ExpandModel reproduce = ordered sets (bytes)", files_exact(want, got), f"{f!r}")
            want = run_iface(
                nwc.ExpandModel, d / "model", model="default", confounds_file=p, model_formula=f
            )
            got, notes = run_lx_file(lc.expand_model, d / "fix", p, f, pandas_version=PANDAS)
            defect = any(n in ("PIPE-64", "PIPE-65", "PIPE-66") for n in notes)
            if defect:
                APART[
                    f"ExpandModel default: a defect case outside the model ({sorted(set(notes))})"
                ] += 1
            else:
                count("ExpandModel default = model (bytes)", files_exact(want, got), f"{f!r}")
            for e in set(notes):
                NOTICES[f"ExpandModel default: {e}"] += 1
    # temporal_derivatives and exponential_terms called directly
    df = fmriprep_like(rng, 50)
    mapping = {c: df[c].to_numpy() for c in df.columns}
    for order in ([0], [1], [0, 1], [2], [0, 1, 2, 3], range(4), range(2, 5), [5, 3], [60]):
        for fn, lfn in (
            (nwc.temporal_derivatives, lc.temporal_derivatives),
            (nwc.exponential_terms, lc.exponential_terms),
        ):
            what = f"{fn.__name__} {list(order)}"
            v = ["csf", "trans_x", "rot_z"]
            want = outcome_orig(lambda fn=fn, o=order: fn(o, v, df))
            for mode in ("reproduce", "fix"):
                got, _ = outcome_lx(
                    lambda lfn=lfn, o=order, m=mode: lfn(o, v, mapping, upstream_defects=m)
                )
                if mode == "fix" and fn is nwc.exponential_terms and not isinstance(want, str):
                    with niworkflows_model(True):
                        want2 = outcome_orig(lambda o=order: nwc.exponential_terms(o, v, df))
                else:
                    want2 = want
                ok = same_frames(
                    want2 if isinstance(want2, str) else want2[1],
                    got if isinstance(got, str) else got.data,
                    ordered=True,
                )
                if ok and not isinstance(want2, str):
                    ok = list(want2[0]) == list(got.variables)
                count(f"{fn.__name__} {mode}", ok, what)
    # the defects' cases
    base = odd_table(rng, 30)
    cases = [(f, base, eq, ren) for f, ren, eq, _ in DEFECT_CASES]
    cases += [(f, t, eq, ren) for f, t, eq, ren in defect_tables(rng, 30)]
    for f, table, eq, ren in cases:
        p = write_df(inputs / f"d{k[0]}.tsv", table)
        q = write_df(inputs / f"d{k[0]}-renamed.tsv", table.rename(columns=ren))
        k[0] += 1
        back = {v: kk for kk, v in ren.items()}
        for unscramble in (False, True):
            renamed = pd.read_csv(q, sep="\t")
            with niworkflows_model(False) as m:
                want = outcome_orig(lambda u=unscramble: m.parse_formula(eq, renamed, u))
            if not isinstance(want, str):
                cols = []
                for c in want[1].columns:
                    for a, b in back.items():
                        c = c.replace(a, b)
                    cols.append(c)
                want = (want[0], want[1].set_axis(cols, axis=1))
            parent = pd.read_csv(p, sep="\t")
            mapping = {c: (list(parent[c]) if parent[c].dtype == object else parent[c].to_numpy())
                       for c in parent.columns}  # fmt: skip
            got, notes = outcome_lx(lambda u=unscramble: lc.parse_formula(f, mapping, u))
            ok = same_frames(
                want if isinstance(want, str) else want[1],
                got if isinstance(got, str) else got.data,
                ordered=False,
            )
            count("defects: default = the equivalent formula (by name)", ok, f"{f!r} ~ {eq!r}")
            for e in set(notes):
                NOTICES[f"defect cases default: {e}"] += 1
            want_r = outcome_orig(lambda u=unscramble: nwc.parse_formula(f, parent, u))
            got_r, _ = outcome_lx(
                lambda u=unscramble: lc.parse_formula(f, mapping, u, upstream_defects="reproduce")
            )
            ok = same_frames(
                want_r if isinstance(want_r, str) else want_r[1],
                got_r if isinstance(got_r, str) else got_r.data,
                ordered=False,
            )
            count("defects: reproduce = niworkflows (by name)", ok, f"{f!r}")
    t = pd.DataFrame({"x": rng.normal(0, 1, 9), "y": rng.normal(0, 1, 9)})
    mapping = {c: t[c].to_numpy() for c in t.columns}
    for f, kind in RAISES:
        got, _ = outcome_lx(lambda f=f: lc.parse_formula(f, mapping))
        count("defects: PIPE-65's refusals", isinstance(got, str) and got.startswith(kind), f)
        want = outcome_orig(lambda f=f: nwc.parse_formula(f, t))
        got, notes = outcome_lx(
            lambda f=f: lc.parse_formula(f, mapping, upstream_defects="reproduce")
        )
        ok = same_frames(
            want if isinstance(want, str) else want[1],
            got if isinstance(got, str) else got.data,
            ordered=False,
        )
        count("defects: PIPE-65 reproduce = niworkflows", ok and "PIPE-65" in notes, f)
    # ((x)^2)^3: the cube of the square (larmorx), niworkflows' TypeError (reproduce)
    got, notes = outcome_lx(lambda: lc.parse_formula("((x)^2)^3", mapping))
    sq = t["x"].to_numpy() ** 2
    want = np.array([exact_power(float(v), 3) for v in sq])
    ok = not isinstance(got, str) and list(got.data.names) == ["x_power2_power3"]
    count("defects: ((x)^2)^3", ok and ct.bits_equal(got.data.columns[0], want), str(got)[:80])


# --- spikes ----------------------------------------------------------------------------------


def corrected_spikes(data, criteria=None, header_prefix="motion_outlier", lags=None,
                     minimum_contiguous=None, concatenate=True, output="spikes"):  # fmt: skip
    """niworkflows' ``spike_regressors`` with PIPE-67's, PIPE-68's and PIPE-69's corrections
    (written apart from larmorx's: the default's model)."""
    n = data.shape[0]
    lags = lags or [0]
    criteria = criteria or {"framewise_displacement": (">", 0.5), "std_dvars": (">", 1.5)}
    flagged: set[int] = set()
    for metric, (criterion, threshold) in criteria.items():
        if criterion == "<":
            flagged |= set(np.where(data[metric] < threshold)[0].tolist())
        elif criterion == ">":
            flagged |= set(np.where(data[metric] > threshold)[0].tolist())
        else:
            raise ValueError(f"the criterion {criterion!r}")
    mask = set(flagged)
    for lag in lags:
        mask |= {m + lag for m in flagged}
    mask = {m for m in mask if 0 <= m < n}
    if minimum_contiguous is not None:
        bounds = sorted(mask | {-1, n})
        for a, b in itertools.pairwise(bounds):
            if b - a - 1 < minimum_contiguous:
                mask |= set(range(a + 1, b))
        mask = {m for m in mask if 0 <= m < n}
    if output == "mask":
        spikes = np.zeros(n)
        spikes[list(mask)] = 1
        spikes = pd.DataFrame(data=spikes, columns=[header_prefix])
    else:
        spikes = np.zeros((max(range(n)) + 1, len(mask)))
        for i, m in enumerate(sorted(mask)):
            spikes[m, i] = 1
        spikes = pd.DataFrame(
            data=spikes, columns=[f"{header_prefix:s}{v:02d}" for v in range(len(mask))]
        )
    return pd.concat((data, spikes), axis=1) if concatenate else spikes


def spike_table(rng: np.random.Generator, t: int, rate: float) -> pd.DataFrame:
    fd = np.abs(rng.normal(0.2, 0.3, t))
    dv = np.abs(rng.normal(1.0, 0.5, t))
    hit = rng.uniform(size=t) < rate
    fd[hit] += 1.0
    if t:
        fd[0] = np.nan
        dv[0] = np.nan
    return pd.DataFrame({"x": rng.normal(0, 1, t), "framewise_displacement": fd, "std_dvars": dv})


def spikes_section(tier: str, tmp: Path) -> None:
    from niworkflows.interfaces import confounds as nwc

    rng = np.random.default_rng(29)
    inputs = tmp / "inputs"
    inputs.mkdir()
    ts = (1, 2, 10, 300, 1200) if tier == "full" else (1, 10, 300)
    lag_forms = ([0], [1], [0, 1], [-1, 0, 1], [1, 2], [0, 1, 2], [-2, -1], [], [3], [0, 0])
    mins = (None, 0, 1, 2, 5)
    k = [0]
    for t in ts:
        df = spike_table(rng, t, 0.1 if t < 1000 else 0.15)
        p = write_df(inputs / f"s{t}.tsv", df)
        data = pd.read_csv(p, sep="\t")
        mapping = {c: data[c].to_numpy() for c in data.columns}
        combos = [(lags, m) for lags in lag_forms for m in (*mins, t, t + 1, t - 1)]
        if tier == "quick":
            combos = combos[::3]
        for lags, m in combos:
            for output in ("spikes", "mask"):
                for concatenate in (True, False):
                    kw = dict(
                        lags=lags, minimum_contiguous=m, concatenate=concatenate, output=output
                    )
                    what = f"T={t} {kw}"
                    want = outcome_orig(lambda kw=kw: nwc.spike_regressors(data, **kw))
                    got, notes = outcome_lx(
                        lambda kw=kw: lc.spike_regressors(
                            mapping, upstream_defects="reproduce", **kw
                        )
                    )
                    count("spike_regressors reproduce", same_frames(want, got, ordered=True), what)
                    want = outcome_orig(lambda kw=kw: corrected_spikes(data, **kw))
                    got, notes = outcome_lx(lambda kw=kw: lc.spike_regressors(mapping, **kw))
                    count(
                        "spike_regressors default = model",
                        same_frames(want, got, ordered=True),
                        what,
                    )
                    for e in set(notes):
                        NOTICES[f"spike_regressors default: {e}"] += 1
        # the interface
        for fd, dv, prefix in ((0.5, 1.5, "motion_outlier"), (0.2, 1.0, "spike_"), (2.0, 9.0, "o")):
            for lags, m, out_fmt, conc in (([0], None, "spikes", True), ([0, 1], 2, "mask", False),
                                           ([1, 2], t - 1, "spikes", True)):  # fmt: skip
                k[0] += 1
                d = tmp / f"n{k[0]}"
                inp = dict(confounds_file=p, fd_thresh=fd, dvars_thresh=dv, header_prefix=prefix,
                           lags=lags, minimum_contiguous=m, concatenate=conc, output_format=out_fmt)  # fmt: skip
                want = run_iface(nwc.SpikeRegressors, d / "orig", **inp)
                kw = {key: v for key, v in inp.items() if key != "confounds_file"}
                got, _ = run_lx_file(
                    lc.spike_regressors_node, d / "lx", p, pandas_version=PANDAS,
                    upstream_defects="reproduce", **kw,
                )  # fmt: skip
                count("SpikeRegressors reproduce (bytes)", files_exact(want, got), f"T={t} {kw}")
                orig = nwc.spike_regressors
                nwc.spike_regressors = corrected_spikes
                try:
                    want = run_iface(nwc.SpikeRegressors, d / "model", model="default", **inp)
                finally:
                    nwc.spike_regressors = orig
                got, _ = run_lx_file(
                    lc.spike_regressors_node, d / "fix", p, pandas_version=PANDAS, **kw
                )
                count(
                    "SpikeRegressors default = model (bytes)", files_exact(want, got), f"T={t} {kw}"
                )
    # criteria, column types, failures
    df = spike_table(rng, 40, 0.2)
    df["n"] = rng.integers(0, 5, 40).astype(np.int64)
    df["flag"] = rng.uniform(size=40) < 0.2
    df["text"] = ["a"] * 40
    mapping = {c: (list(df[c]) if df[c].dtype == object else df[c].to_numpy()) for c in df.columns}
    crits = [
        {"framewise_displacement": ("<", 0.1)},
        {"n": (">", 2), "flag": (">", 0.5)},
        {"n": ("<", 1.0)},
        {"framewise_displacement": (">", np.nan)},
        {"missing": (">", 1.0)},
        {"framewise_displacement": (">", 0.5), "std_dvars": (">=", 0.0)},
        {"framewise_displacement": ("=", 0.5)},
        {},
        None,
        {"std_dvars": (">", -np.inf)},
    ]
    for c in crits:
        for kw in ({}, {"header_prefix": "s", "lags": [0, 1]}):
            what = f"criteria {c} {kw}"
            want = outcome_orig(lambda c=c, kw=kw: nwc.spike_regressors(df, criteria=c, **kw))
            got, notes = outcome_lx(
                lambda c=c, kw=kw: lc.spike_regressors(
                    mapping, c, upstream_defects="reproduce", **kw
                )
            )
            count("spike_regressors criteria reproduce", same_frames(want, got, ordered=True), what)
            want = outcome_orig(lambda c=c, kw=kw: corrected_spikes(df, criteria=c, **kw))
            got, notes = outcome_lx(lambda c=c, kw=kw: lc.spike_regressors(mapping, c, **kw))
            ok = same_frames(want, got, ordered=True)
            if isinstance(want, str) and isinstance(got, str):
                ok = want.split(":")[0] == got.split(":")[0]
            count("spike_regressors criteria default = model", ok, what)
    empty = df.iloc[:0][["framewise_displacement", "std_dvars"]]
    emap = {c: empty[c].to_numpy() for c in empty.columns}
    for output in ("spikes", "mask"):
        want = outcome_orig(lambda o=output: nwc.spike_regressors(empty, output=o))
        for mode in ("reproduce", "fix"):
            got, _ = outcome_lx(
                lambda o=output, m=mode: lc.spike_regressors(emap, output=o, upstream_defects=m)
            )
            count(
                "spike_regressors without rows",
                same_frames(want, got, ordered=True),
                f"{output} {mode}",
            )


# --- normalize -------------------------------------------------------------------------------


def motion_text(rng: np.random.Generator, t: int, k: int, fmt: str = "%.10f") -> str:
    a = rng.normal(0, 0.5, (t, k))
    return "".join(" ".join(fmt % v for v in row) + "\n" for row in a)


def normalize_section(tier: str, tmp: Path) -> None:
    from niworkflows.interfaces import confounds as nwc

    rng = np.random.default_rng(37)
    inputs = tmp / "inputs"
    inputs.mkdir()
    files = []
    for t in (1, 2, 50) if tier == "quick" else (1, 2, 3, 50, 600):
        for k in (6, 7):
            files.append((f"T={t} k={k}", motion_text(rng, t, k)))
    files += [
        ("12 columns", motion_text(rng, 20, 12)),
        ("%.18e", motion_text(rng, 20, 6, "%.18e")),
        (
            "ints",
            "".join(" ".join(str(int(v)) for v in rng.integers(-5, 5, 6)) + "\n" for _ in range(9)),
        ),
        ("comments", "# a header\n" + motion_text(rng, 5, 6) + "# end\n"),
        ("one value", "0.5\n"),
        ("one column", "0.1\n0.2\n0.3\n"),
        ("three columns", motion_text(rng, 5, 3)),
        ("empty", ""),
        ("nan", "nan 0 0 0 0 0\n0 0 0 0 0 0\n"),
        ("ragged", "1 2 3 4 5 6\n1 2 3\n"),
    ]
    k = [0]
    for what, text in files:
        p = inputs / f"m{k[0]}.txt"
        k[0] += 1
        p.write_text(text)
        for fmt in ("FSL", "AFNI", "FSFAST", "NIPY"):
            for mode in ("reproduce", "fix"):
                if fmt == "NIPY":
                    ni.install(mode)
                k[0] += 1
                d = tmp / f"m{k[0]}"
                if mode == "fix":
                    load = np.loadtxt

                    def ndmin_one_row(path, *a, load=load, **kw):
                        r = load(path, *a, **kw)
                        if r.ndim == 1 and r.size > 1:
                            with warnings.catch_warnings():
                                warnings.simplefilter("ignore")
                                s = load(path, *a, ndmin=2, **kw)
                            if s.shape[0] == 1:
                                return s
                        return r

                    nwc.np.loadtxt = ndmin_one_row
                try:
                    want = run_iface(
                        nwc.NormalizeMotionParams, d / "orig", in_file=str(p), format=fmt
                    )
                finally:
                    if mode == "fix":
                        nwc.np.loadtxt = load
                got, notes = run_lx_file(
                    lc.normalize_motion_params, d / "lx", str(p), format=fmt, numpy_version=NUMPY,
                    upstream_defects=mode,
                )  # fmt: skip
                ok = files_exact(want, got)
                if isinstance(want, str) and isinstance(got, str) and not ok:
                    # niworkflows' message holds the file path (in a temporary folder).
                    ok = want.split(":")[0] == got.split(":")[0]
                count(
                    f"NormalizeMotionParams {mode}",
                    ok,
                    f"{what} {fmt}: {str(want)[:90]} | {str(got)[:90]}",
                )
                for e in set(notes):
                    NOTICES[f"NormalizeMotionParams {mode}: {e}"] += 1
    p = inputs / "bad.txt"
    p.write_text(motion_text(rng, 4, 6))
    for fmt in ("SPM", "fsl", 3):
        want = outcome_orig(lambda f=fmt: nwc.NormalizeMotionParams(in_file=str(p), format=f).run())
        got, _ = outcome_lx(
            lambda f=fmt: lc.normalize_motion_params(str(p), format=f, out_dir=tmp / "bad")
        )
        count("NormalizeMotionParams refused formats", want == got, f"{fmt!r}: {want} | {got}")


# --- real ------------------------------------------------------------------------------------

QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256", "ds005454")


def blocks(names: list[str], parent: list[str]) -> list[list[str]] | None:
    """The expansion's columns in groups (a parent column and its expansions after it), or
    ``None`` when the order is not the parent's."""
    import re

    out: list[list[str]] = []
    base_of = {}
    for n in names:
        b = re.sub("_derivative[0-9]+", "", re.sub("_power[0-9]+", "", n))
        base_of[n] = b
        if out and base_of[out[-1][0]] == b and b != n:
            out[-1].append(n)
        elif b == n:
            out.append([n])
        else:
            return None
    bases = [g[0] for g in out]
    pos = {c: i for i, c in enumerate(parent)}
    if any(b not in pos for b in bases) or [pos[b] for b in bases] != sorted(pos[b] for b in bases):
        return None
    return out


def possible_order(names: list[str], ours: list[str], parent: list[str]) -> bool:
    """Whether ``names`` is an order niworkflows can produce for fMRIPrep's formula given larmorx's
    columns ``ours``: the same groups in the parent's order, each starting with its variable and
    its first derivative, the two squares in either order."""
    a, b = blocks(names, parent), blocks(ours, parent)
    if a is None or b is None or len(a) != len(b):
        return False
    for ga, gb in zip(a, b, strict=True):
        if sorted(ga) != sorted(gb) or ga[0] != gb[0]:
            return False
        if len(ga) > 1 and ga[1] != gb[1]:
            return False
    return True


def real_section(tier: str, tmp: Path) -> None:
    from nipype.utils.filemanip import loadpkl

    wfs = sorted(RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if tier == "quick":
        wfs = [w for w in wfs if any(str(w.relative_to(RUNS)).startswith(q) for q in QUICK_REAL)]
    print(f"real: {len(wfs)} confounds workflows", flush=True)
    for wf in wfs:
        run = wf.relative_to(RUNS).parts[0]
        label = f"{run}/{wf.parts[-2]}"

        def host(v, run=run):
            if isinstance(v, str) and v.startswith("/work/"):
                return str(RUNS / run / v.lstrip("/"))
            return v

        derivatives = {
            hashlib.sha256(p.read_bytes()).hexdigest(): p
            for p in (RUNS / run / "out").rglob("*desc-confounds_timeseries.tsv")
        }
        me = wf / "model_expansion"
        inp = {key: host(v) for key, v in loadpkl(me / "_inputs.pklz").items()}
        want = (me / "confounds_expansion.tsv").read_bytes()
        parent = pd.read_csv(inp["confounds_file"], sep="\t", nrows=0).columns.tolist()
        out = tmp / label.replace("/", "-")
        t0 = time.perf_counter()
        got_files = {}
        for mode in ("reproduce", "fix"):
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                path = lc.expand_model(
                    inp["confounds_file"], inp["model_formula"], out_dir=out / f"me-{mode}",
                    pandas_version="2.2.3", upstream_defects=mode,
                )  # fmt: skip
            got = Path(path).read_bytes()
            got_files[mode] = path
            for e in sorted(set(entries(caught))):
                NOTICES[f"real model_expansion {mode}: {e}"] += 1
            if mode == "reproduce":
                count(
                    "real model_expansion reproduce (by name)",
                    tsv_by_name(want) == tsv_by_name(got),
                    label,
                )
                names = want.decode().split("\n", 1)[0].split("\t")
                ours = got.decode().split("\n", 1)[0].split("\t")
                count("real model_expansion: fMRIPrep's order is one niworkflows can produce",
                      possible_order(names, ours, parent), label)  # fmt: skip
                count("real model_expansion: larmorx's order is the formula's",
                      possible_order(ours, ours, parent) and blocks(ours, parent) is not None, label)  # fmt: skip
                APART["real model_expansion: fMRIPrep's order = larmorx's (by chance)"] += (
                    names == ours
                )
                for g in blocks(names, parent) or []:
                    sq = [c for c in g if c.endswith("_power2")]
                    if len(sq) == 2:
                        first = "x_power2" if sq[0] == g[0] + "_power2" else "x_derivative1_power2"
                        APART[f"real model_expansion: fMRIPrep's groups with {first} first"] += 1
            else:
                cells, changed = ct.cells_changed({"x": want}, {"x": _reorder(got, want)})
                APART["real model_expansion default: cells changed"] += changed
                APART["real model_expansion default: cells"] += cells
        SECONDS["model_expansion"] += time.perf_counter() - t0
        sr = wf / "spike_regressors"
        inp = {key: host(v) for key, v in loadpkl(sr / "_inputs.pklz").items()}
        want = (sr / "confounds_expansion_desc-motion_outliers.tsv").read_bytes()
        kw = {key: v for key, v in inp.items() if key != "confounds_file"}
        kw["lags"] = list(kw["lags"])
        t0 = time.perf_counter()
        for mode in ("reproduce", "fix"):
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                path = lc.spike_regressors_node(
                    inp["confounds_file"], out_dir=out / f"sr-{mode}", pandas_version="2.2.3",
                    upstream_defects=mode, **kw,
                )  # fmt: skip
                chain = lc.spike_regressors_node(
                    got_files[mode], out_dir=out / f"chain-{mode}", pandas_version="2.2.3",
                    upstream_defects=mode, **kw,
                )  # fmt: skip
            got = Path(path).read_bytes()
            for e in sorted(set(entries(caught))):
                NOTICES[f"real spike_regressors {mode}: {e}"] += 1
            if mode == "reproduce":
                count("real spike_regressors reproduce (bytes)", got == want, label)
                h = hashlib.sha256(want).hexdigest()
                count("real: the node's file is the run's desc-confounds_timeseries.tsv",
                      h in derivatives, label)  # fmt: skip
                c = Path(chain).read_bytes()
                count(
                    "real chain reproduce = the derivative (by name)",
                    tsv_by_name(c) == tsv_by_name(want),
                    label,
                )
            else:
                cells, changed = ct.cells_changed({"x": want}, {"x": got})
                APART["real spike_regressors default: cells changed"] += changed
                APART["real spike_regressors default: cells"] += cells
                c = Path(chain).read_bytes()
                cells, changed = ct.cells_changed({"x": want}, {"x": _reorder(c, want)})
                APART["real chain default: cells changed against the derivative"] += changed
        SECONDS["spike_regressors"] += time.perf_counter() - t0


def _reorder(data: bytes, like: bytes) -> bytes:
    """``data``'s columns in ``like``'s order (by name; the same names), for counting cells."""
    rows = [r.split("\t") for r in data.decode().split("\n") if r != ""]
    names = like.decode().split("\n", 1)[0].split("\t")
    if sorted(rows[0]) != sorted(names) or len(set(names)) != len(names):
        return data
    idx = [rows[0].index(n) for n in names]
    return "".join("\t".join(r[i] for i in idx) + "\n" for r in rows).encode()


# ---------------------------------------------------------------------------------------------


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="power,expand,spikes,normalize,real")
    a = p.parse_args()
    import nipype
    import niworkflows

    print(
        f"pandas {PANDAS}, numpy {NUMPY}, nipype {nipype.__version__}, niworkflows "
        f"{niworkflows.__version__}, AVX-512 off: {NPY_OFF}, tier {a.tier}",
        flush=True,
    )
    sections = {
        "power": power_section,
        "expand": expand_section,
        "spikes": spikes_section,
        "normalize": normalize_section,
        "real": real_section,
    }
    with tempfile.TemporaryDirectory(prefix="lx-check-model-") as tmp:
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = Path(tmp) / s
            d.mkdir()
            sections[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, d) in COUNTS.items():
        print(f"  {key:66s} identical {i:6d}  differ {d:5d}")
    if SECONDS:
        print("  larmorx's seconds per node (both modes): "
              + ", ".join(f"{k} {v:.1f}" for k, v in SECONDS.items()))  # fmt: skip
    for key, n in sorted(APART.items()):
        print(f"  apart: {key} ({n})")
    for key, n in sorted(NOTICES.items()):
        print(f"  notice: {key} ({n})")
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[1] for c in COUNTS.values())
    print(f"CHECK {'PASS' if differ == 0 else 'DIFFER'}: {differ} cases differ")
    return 0 if differ == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
