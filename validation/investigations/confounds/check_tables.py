# SPDX-License-Identifier: Apache-2.0
"""pandas' text round trips and fMRIPrep's and niworkflows' table steps: larmorx against this
environment's pandas, fMRIPrep and niworkflows, bit for bit.

Sections:

- ``parse``: ``_core.pandas_try_double`` (pandas' ``precise_xstrtod``, larmorx told this
  pandas' series) against ``pandas.read_csv(sep="\\t")`` on columns of numbers written in the
  forms the confounds chain writes (Python's repr: ``to_csv``; ``%.10f``: nipype's CompCor;
  ``%.18e``: niworkflows' ``AddTSVHeader``; ``%0.6f``: nipype's DVARS) and others (``%.17g``,
  ``%.25g``, ``%.3e``, white space, signs, ``E``), over magnitudes from subnormals to 1e308;
  the correctly rounded values against Python's ``float``. Odd tokens one by one (exponent
  forms, overflow and underflow, missing-value words in other cases, infinity words, integers
  of every size, booleans, text) through ``larmorx.confounds._tables.read_tsv``: the type pandas
  infers and the value.
- ``read``: whole files through ``read_tsv`` and ``pandas.read_csv``: repeated and empty names,
  a byte-order mark, ``\\r\\n`` and ``\\r`` line ends, blank lines and lines of spaces, quoted
  fields (tabs, doubled quotes, line breaks), short rows, a long row (pandas' ``ParserError``),
  empty files and files of blank lines (``EmptyDataError``), a header alone, integer columns at
  int64's limits with and without missing values (PD-03), unsigned and larger integers,
  booleans with missing values, gzip, bzip2 and xz. ``upstream_defects="reproduce"`` against
  pandas' default parser, the default against pandas' ``float_precision="round_trip"`` (the
  correctly rounded parser).
- ``write``: ``_tables.tsv_text`` against ``DataFrame.to_csv(sep="\\t", index=False,
  na_rep="n/a")`` byte for byte: floats of every magnitude with NaN, infinities and -0.0,
  integers, booleans, text with tabs, quotes and line breaks, empty strings, names that need
  quotes, a table without columns or rows; ``.gz`` files by their content.
- ``ujson``: ``_tables.ujson_double`` against ``Series.to_json(orient="values")`` (pandas'
  ujson, 10 decimals): magnitudes, ties (k / 2**n), the 1e16 - 1 and 1e-15 thresholds,
  carries (0.99999999995), -0.0, integers; NaN and infinities ``null``; PD-02 counted.
- ``iface``: fMRIPrep's ``FramewiseDisplacement``, ``FilterDropped``, ``RenameACompCor``,
  ``GatherConfounds`` and niworkflows' ``AddTSVHeader``, ``DictMerge``, ``TSV2JSON`` (run in a
  temporary folder) against ``lx.confounds`` and ``lx.niworkflows`` with
  ``upstream_defects="reproduce"``: every file (gzipped tables by their content) and output,
  errors by type and last line; the default against the same interfaces run with pandas'
  ``round_trip`` parser and correctly rounded JSON numbers (the default's model), and against
  PIPE-61's and PIPE-62's corrections.
- ``real``: fMRIPrep's own nodes in the oracle runs' working directories (every
  ``bold_confounds_wf``: ``fdisp``, ``add_dvars_header``, ``add_std_dvars_header``,
  ``rename_acompcor``, ``acc_metadata_filter``, ``tcc_metadata_filter``, ``acc_metadata_fmt``,
  ``tcc_metadata_fmt``, ``crowncc_metadata_fmt``, ``merge_confound_metadata2``, ``concat``)
  with their own inputs: larmorx with ``pandas_version="2.2.3"`` (fMRIPrep 25.2.6's image) and
  ``upstream_defects="reproduce"`` against the node's files and results; the default's values
  that differ counted (PD-01, PD-02).

Run it in each environment with the worktree's extension::

    PYTHONPATH=<worktree>/python python -P check_tables.py [--tier quick|full] [--sections ...]

(fMRIPrep's interfaces: ``PYTHONPATH`` also holds ``tags/fmriprep-25.2.7`` outside fMRIPrep's
image). Exit status 1 when any case differs.
"""

from __future__ import annotations

import argparse
import contextlib
import functools
import gzip
import io
import json
import logging
import math
import os
import re
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from pathlib import Path

import numpy as np
import pandas as pd

import larmorx as lx
from larmorx import _core
from larmorx.confounds import _tables as tb

PANDAS = pd.__version__
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])  # identical, differ
SECONDS: dict[str, float] = defaultdict(float)
APART: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
logging.getLogger("nipype.interface").setLevel(logging.ERROR)
logging.getLogger("nipype.workflow").setLevel(logging.ERROR)


def count(key: str, ok: bool, what: str) -> None:
    COUNTS[key][0 if ok else 1] += 1
    if not ok and len(FAILS) < 80:
        FAILS.append(f"{key}: {what}")


def last_line(e: BaseException) -> str:
    msg = str(e).strip().splitlines()
    kind = type(e).__name__
    kind = "ValueError" if kind in ("TraitError", "ParserError", "EmptyDataError") else kind
    return f"{kind}: {msg[-1] if msg else ''}"


def bits_equal(a: np.ndarray, b: np.ndarray) -> bool:
    a = np.asarray(a, dtype=np.float64)
    b = np.asarray(b, dtype=np.float64)
    if a.shape != b.shape:
        return False
    nan = np.isnan(a) & np.isnan(b)
    return bool(np.all(nan | (a.view(np.uint64) == b.view(np.uint64))))


@contextlib.contextmanager
def cwd(path: Path):
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


# --- parse -----------------------------------------------------------------------------------


def number_values(rng: np.random.Generator, n: int) -> dict[str, np.ndarray]:
    return {
        "motion": rng.normal(0, 1e-3, n),
        "component": rng.normal(0, 0.1, n),
        "signal": rng.normal(3000, 500, n),
        "power": rng.normal(3000, 500, n) ** 2,
        "wide": np.sign(rng.normal(size=n)) * 10.0 ** rng.uniform(-300, 300, n),
        "subnormal": rng.uniform(0, 1, n) * 1e-310,
        "integral": np.round(rng.normal(0, 1e6, n)),
        "dyadic": rng.integers(-(2**20), 2**20, n) / 2.0 ** rng.integers(0, 40, n),
    }


FORMATS = {
    "repr": repr,
    "%.10f": lambda v: "%.10f" % v,  # noqa: UP031 (printf formats are what is checked)
    "%.18e": lambda v: "%.18e" % v,  # noqa: UP031
    "%0.6f": lambda v: "%0.6f" % v,  # noqa: UP031
    "%.17g": lambda v: "%.17g" % v,  # noqa: UP031
    "%.25g": lambda v: "%.25g" % v,  # noqa: UP031
    "%.3E": lambda v: "%.3E" % v,  # noqa: UP031
    "spaced": lambda v: f"  {v!r} ",
    "signed": lambda v: f"+{v!r}" if v >= 0 else repr(v),
}

ODD_TOKENS = [
    "1e 5", "1e", "1e+", "1e-", ".", "-", "+", ".5", "5.", "-.5", "+.5e-3", "1.e5", "1.E-5",
    "1e400", "-1e400", "2e308", "1.7976931348623157e308", "1.7976931348623159e308", "1e-320",
    "1e-400", "1e-700", "-1e-700", "-0e400", "0e-999", "-0", "-0.0", "0.0", "00012.5000",
    "123456789012345678901234567890", "0.00000000000000000000001234567890123456789",
    "1e0000000000000000005", "1e00000000000000000005", "1e9999999999999999999",
    "1e-9999999999999999999", "9" * 400 + "e-400", "0." + "0" * 330 + "1e330",
    "inf", "INF", "Inf", "+inf", "-inf", "infinity", "-Infinity", "+INFINITY", "infinit",
    "nan", "NaN", "NAN", "-NaN", "n/a", "N/A", "NA", "null", "NULL", "None", "#N/A", "<NA>",
    " n/a", "n/a ", "1.5x", "0x10", "1_000", "1,5", "1.5.6", "\v1.5", "1.5\f", "1e5 ", " 1e5",
    "e5", "E5", "12", "-12", "+12", " 12 ", "9223372036854775807", "-9223372036854775808",
    "9223372036854775808", "-9223372036854775809", "18446744073709551615",
    "18446744073709551616", "True", "false", "TRUE", "tRuE", "yes", "abc", "é", "1e5x",
]  # fmt: skip


def kind_of(series: pd.Series) -> str:
    dt = series.dtype
    if dt == np.float64:
        return "float64"
    if dt == np.int64:
        return "int64"
    if dt == np.uint64:
        return "uint64"
    if dt == np.bool_:
        return "bool"
    return "object"


def column_equal(series: pd.Series, col: tb.Column) -> bool:
    if kind_of(series) != col.kind:
        return False
    if col.kind == "float64":
        return bits_equal(series.to_numpy(), np.asarray(col.values))
    want = list(series.to_numpy(dtype=object)) if col.kind == "object" else series.tolist()
    got = list(col.values) if col.kind == "object" else np.asarray(col.values).tolist()
    if len(want) != len(got):
        return False
    for w, g in zip(want, got, strict=True):
        w_nan = isinstance(w, float) and math.isnan(w)
        g_nan = isinstance(g, float) and math.isnan(g)
        if w_nan or g_nan:
            if not (w_nan and g_nan):
                return False
        elif type(w) is not type(g) and not (isinstance(w, str) and isinstance(g, str)):
            if not (isinstance(w, (int, np.integer)) and isinstance(g, int)):
                return False
            if int(w) != g:
                return False
        elif w != g:
            return False
    return True


def parse_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(20261112)
    n = 2000 if tier == "quick" else 20000
    series = tb.pandas_series(PANDAS)
    for vname, vals in number_values(rng, n).items():
        for fname, fmt in FORMATS.items():
            tokens = [fmt(float(v)) for v in vals]
            text = "x\n" + "\n".join(tokens) + "\n"
            col = pd.read_csv(io.StringIO(text), sep="\t")["x"]
            got = _core.pandas_try_double(tokens, series)
            key = f"parse {fname}"
            if col.dtype == np.int64 and all(re.fullmatch(r"[ +-]*[0-9]+ *", x) for x in tokens):
                APART[f"parse: integer texts, an int64 column ({vname}, {fname})"] += 1
                continue
            if col.dtype != np.float64 or got is None:
                count(key, False, f"{vname}: pandas {col.dtype}, larmorx {got is not None}")
                continue
            count(key, bits_equal(col.to_numpy(), got[0]), f"{vname}: pandas' values")
            exact = np.array([float(t) for t in tokens])
            count(f"parse exact {fname}", bits_equal(exact, got[1]), f"{vname}: rounded")
            APART[f"PD-01 values {fname}"] += int(np.count_nonzero(~(got[0] == got[1])))
    for i, tok in enumerate(ODD_TOKENS):
        p = tmp / f"odd{i}.tsv"
        p.write_text("x\n" + tok + "\n", encoding="utf-8")
        want = pd.read_csv(p, sep="\t")["x"]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            got = tb.read_tsv(p, pandas_version=PANDAS, upstream_defects="reproduce")
        count("parse odd token", column_equal(want, got.columns[0]), f"{tok!r}: {want.dtype}")


# --- read ------------------------------------------------------------------------------------


def read_files(tmp: Path, rng: np.random.Generator) -> list[tuple[str, Path]]:
    files: dict[str, bytes] = {
        "mixed": b"i\tf\tb\ts\tni\tnb\tns\n1\t1.5\tTrue\tCSF\t\tFalse\tn/a\n"
        b"2\t-0.25\tfalse\tWM\t3\tn/a\tx\n-3\t1e-05\tTRUE\tcombined\t4\ttrue\t\n",
        "names": b"a\ta\ta.1\t\ta\tUnnamed: 4\n1\t2\t3\t4\t5\t6\n",
        "bom": "﻿x\ty\n1\t2\n".encode(),
        "crlf": b"x\ty\r\n1\t2\r\n\r\n3\t4\r\n",
        "cr": b"x\ty\r1\t2\r3\t4\r",
        "blank": b"\n\nx\ty\n\n1\t2\n   \n\t\n3\t4\n\n",
        "quotes": b'x\ty\n"a\tb"\t"c""d"\n"multi\nline"\t"x"y\n"\t"\n',
        "short": b"x\ty\tz\n1\n2\t3\n4\t5\t6\n",
        "long": b"x\ty\n1\t2\n3\t4\t5\n",
        "empty": b"",
        "blanklines": b"\n\n   \n",
        "header": b"x\ty\n",
        "int64": b"a\tb\tc\n9223372036854775807\t-9223372036854775808\t-9223372036854775808\n"
        b"1\t2\tn/a\n",
        "uint64": b"a\tb\tc\td\n18446744073709551615\t18446744073709551616\t9223372036854775808"
        b"\t-9223372036854775809\n1\t2\t-1\t5\n",
        "uintna": b"a\n9223372036854775808\nn/a\n",
        "floats": b"x\ty\n0.1\tinf\n1e5\t-Infinity\nn/a\tINF\n",
        "ints-then-text": b"x\n1\n99999999999999999999\nabc\n",
        "text-then-big": b"x\nabc\n99999999999999999999\n",
    }
    motion = pd.DataFrame(rng.normal(0, 1e-3, (40, 6)), columns=list("abcdef"))
    raw = motion.to_csv(sep="\t", index=False, na_rep="n/a").encode()
    files["motion.tsv.gz"] = gzip.compress(raw)
    import bz2
    import lzma

    files["motion.tsv.bz2"] = bz2.compress(raw)
    files["motion.tsv.xz"] = lzma.compress(raw)
    out = []
    for name, data in files.items():
        p = tmp / (name if "." in name else f"{name}.tsv")
        p.write_bytes(data)
        out.append((name, p))
    return out


def read_outcome(path: Path, **kw):
    try:
        return pd.read_csv(path, sep="\t", **kw)
    except Exception as e:
        return last_line(e)


def lx_outcome(path: Path, mode: str):
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            return tb.read_tsv(path, pandas_version=PANDAS, upstream_defects=mode)
    except NotImplementedError as e:
        return f"refused: {e}"
    except Exception as e:
        return last_line(e)


def tables_equal(df, t) -> tuple[bool, str]:
    if isinstance(df, str) or isinstance(t, str):
        if isinstance(t, str) and t.startswith("refused"):
            return True, "refused"
        return (
            df == t
        ), f"{df if isinstance(df, str) else 'table'} | {t if isinstance(t, str) else 'table'}"
    if list(map(str, df.columns)) != t.names:
        return False, f"names {list(df.columns)} | {t.names}"
    for j, name in enumerate(t.names):
        if not column_equal(df.iloc[:, j], t.columns[j]):
            return (
                False,
                f"column {name}: {df.iloc[:, j].dtype} {df.iloc[:, j].tolist()} | {t.columns[j]}",
            )
    return True, ""


def read_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(7)
    for name, p in read_files(tmp, rng):
        ok, what = tables_equal(read_outcome(p), lx_outcome(p, "reproduce"))
        count("read reproduce", ok, f"{name}: {what}")
        if what == "refused":
            APART[f"read refused ({name})"] += 1
        ok, what = tables_equal(read_outcome(p, float_precision="round_trip"), lx_outcome(p, "fix"))
        if name == "int64" and not ok:
            APART["read fix: PD-03 (int64's minimum kept beside a missing value)"] += 1
            continue
        count("read fix (round_trip)", ok, f"{name}: {what}")


# --- write -----------------------------------------------------------------------------------


def to_table(df: pd.DataFrame) -> tb.Table:
    cols = []
    for j in range(df.shape[1]):
        s = df.iloc[:, j]
        k = kind_of(s)
        if k == "object":
            cols.append(tb.Column("object", list(s.to_numpy(dtype=object))))
        else:
            cols.append(tb.Column(k, s.to_numpy()))
    return tb.Table([str(c) for c in df.columns], cols, list(range(len(df))))


def write_frames(rng: np.random.Generator, n: int) -> dict[str, pd.DataFrame]:
    vals = np.concatenate(list(number_values(rng, n).values()))
    vals[::97] = np.nan
    vals[1::101] = np.inf
    vals[2::103] = -np.inf
    vals[3::107] = -0.0
    vals = vals[: (len(vals) // 4) * 4]
    texts = ["a\tb", 'q"uote', "line\nbreak", "cr\rx", "", "n/a", "é ü", " lead", "trail ", "#x"]
    return {
        "floats": pd.DataFrame(vals.reshape(-1, 4), columns=["a", "b", "c d", "e\tf"]),
        "ints": pd.DataFrame(
            {"i": np.array([0, -1, 2**63 - 1, -(2**63)] * 5, dtype=np.int64), "q": ['x"'] * 20}
        ),
        "bools": pd.DataFrame({"b": [True, False] * 5, "s": texts}),
        "object": pd.DataFrame({"o": [1.5, np.nan, "x", True, 3] * 2}),
        "empty-string": pd.DataFrame({"e": [""]}),
        "no-rows": pd.DataFrame({"x": [], "y": []}),
        "no-columns": pd.DataFrame(),
    }


def write_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(11)
    for name, df in write_frames(rng, 400 if tier == "quick" else 4000).items():
        want = df.to_csv(sep="\t", index=False, na_rep="n/a", lineterminator="\n")
        got = tb.tsv_text(to_table(df))
        count("write text", want == got, f"{name}: {want[:80]!r} | {got[:80]!r}")
        a, b = tmp / f"{name}-pd.tsv.gz", tmp / f"{name}-lx.tsv.gz"
        df.to_csv(a, sep="\t", index=False, na_rep="n/a")
        tb.write_tsv(b, to_table(df))
        same = gzip.decompress(a.read_bytes()) == gzip.decompress(b.read_bytes())
        count("write .gz content", same, name)


# --- ujson -----------------------------------------------------------------------------------


def ujson_values(rng: np.random.Generator, n: int) -> np.ndarray:
    special = [
        0.0, -0.0, 1.0, -1.0, 0.5, 1 / 2048, 3 / 2048, 5 / 2**12, 0.99999999995, 0.99999999994,
        9.99999999995, 1e16 - 2, 9999999999999998.0, 1e16, 1e-15, 9.999999999999999e-16,
        1.0000000000000002e-15, 1e15 + 0.5, 2.0**53, 2.0**53 + 2, 123456789.123456789,
        0.00000000005, 0.00000000015, 0.00000000025, 4.5e-11, 1e300, -1e-300, 5e-324,
        np.nan, np.inf, -np.inf,
    ]  # fmt: skip
    vals = np.concatenate(list(number_values(rng, n).values()))
    ties = rng.integers(-(2**30), 2**30, n) / 2.0 ** rng.integers(1, 45, n)
    tenth = np.round(rng.normal(0, 10, n), 10) + rng.choice([-1, 1], n) * 5e-11
    return np.concatenate([np.array(special), vals, ties, tenth])


def ujson_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(13)
    vals = ujson_values(rng, 1000 if tier == "quick" else 20000)
    want = pd.Series(vals).to_json(orient="values")[1:-1].split(",")
    stats = [0, 0]
    for v, w in zip(vals.tolist(), want, strict=True):
        got = "null" if not math.isfinite(v) else tb.ujson_double(v)
        count("ujson text", got == w, f"{v!r}: {w} | {got}")
        tb.json_value(v, "fix", stats)
    APART[f"PD-02 values (of {stats[0]})"] += stats[1]


# --- iface -----------------------------------------------------------------------------------


def file_content(p: Path) -> bytes:
    data = p.read_bytes()
    return gzip.decompress(data) if p.name.endswith(".gz") else data


def files_of(d: Path) -> dict[str, bytes]:
    return {
        p.name: file_content(p)
        for p in sorted(d.iterdir())
        if p.is_file() and not p.name.startswith(("_", "result"))
    }


@contextlib.contextmanager
def model_default():
    """pandas' correctly rounded parser and correctly rounded 10-decimal JSON numbers: the
    model of larmorx's default (PD-01, PD-02)."""
    read_csv = pd.read_csv
    to_json = pd.DataFrame.to_json

    def conv(v):
        if isinstance(v, (bool, np.bool_)):
            return bool(v)
        if isinstance(v, (int, np.integer)):
            return int(v)
        if isinstance(v, (float, np.floating)):
            v = float(v)
            if not math.isfinite(v):
                return None
            a_ = abs(v)
            return v if a_ > 1e16 - 1 or (a_ != 0 and a_ < 1e-15) else float(f"{v:.10f}")
        if isinstance(v, dict):
            return {str(k): conv(x) for k, x in v.items()}
        if isinstance(v, (list, tuple)):
            return [conv(x) for x in v]
        return v

    def rounded(df, *a, **kw):
        if kw.get("orient") != "index" or not df.columns.is_unique or not df.index.is_unique:
            return to_json(df, *a, **kw)
        return json.dumps(
            {
                str(i): {str(c): conv(v) for c, v in zip(df.columns, row, strict=True)}
                for i, row in zip(df.index, df.itertuples(index=False, name=None), strict=True)
            }
        )

    pd.read_csv = functools.partial(read_csv, float_precision="round_trip")
    pd.DataFrame.to_json = rounded
    try:
        yield
    finally:
        pd.read_csv = read_csv
        pd.DataFrame.to_json = to_json


def run_orig(klass, d: Path, model: bool = False, **inputs):
    d.mkdir(parents=True)
    with cwd(d), warnings.catch_warnings(), model_default() if model else contextlib.nullcontext():
        warnings.simplefilter("ignore")
        try:
            res = klass(**inputs).run()
        except Exception as e:
            return last_line(e)
    return files_of(d), res.outputs.get()


def run_lx(fn, d: Path, *args, **kw):
    d.mkdir(parents=True)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = fn(*args, out_dir=d, **kw)
        except Exception as e:
            return last_line(e), []
    entries = [w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)]
    return (files_of(d), out), entries


def outputs_equal(want, got) -> bool:
    """Interface outputs against larmorx's return value: paths by file name, dictionaries by
    their JSON text (floats' repr), lists as they are."""

    def norm(v):
        if isinstance(v, (str, os.PathLike)) and os.path.sep in os.fspath(v):
            return os.path.basename(os.fspath(v))
        if isinstance(v, dict):
            return json.dumps(v)
        return v

    w = [norm(v) for v in want if v is not None]
    if hasattr(got, "__dataclass_fields__"):
        g = [norm(getattr(got, f)) for f in got.__dataclass_fields__]
    else:
        g = [norm(got)]
    return w == g


def compare(key: str, want, got, what: str) -> None:
    if isinstance(want, str) or isinstance(got, str):
        count(
            key,
            want == got,
            f"{what}: {want if isinstance(want, str) else 'ok'} | {got if isinstance(got, str) else 'ok'}",
        )
        return
    ok = want[0] == got[0] and outputs_equal(list(want[1].values()), got[1])
    what2 = what
    if want[0] != got[0]:
        for k in sorted(set(want[0]) | set(got[0])):
            if want[0].get(k) != got[0].get(k):
                what2 += f": {k} {want[0].get(k, b'')[:120]!r} | {got[0].get(k, b'')[:120]!r}"
                break
    count(key, ok, what2)


def write_df(path: Path, df: pd.DataFrame) -> str:
    df.to_csv(path, sep="\t", index=False, na_rep="n/a")
    return str(path)


def motion_frame(rng: np.random.Generator, t: int) -> pd.DataFrame:
    df = pd.DataFrame(
        rng.normal(0, 1e-3, (t, 6)),
        columns=["trans_x", "trans_y", "trans_z", "rot_x", "rot_y", "rot_z"],
    )
    df[["trans_x", "trans_y", "trans_z"]] *= 300
    return df


def metadata_frame(
    rng: np.random.Generator, n: int, masks=("CSF", "WM", "combined")
) -> pd.DataFrame:
    rows = []
    for m in masks:
        sv = np.sort(np.abs(rng.normal(100, 50, n)))[::-1]
        ve = sv**2 / np.sum(sv**2)
        cve = np.cumsum(ve)
        for k in range(n):
            rows.append(
                {
                    "component": f"a_comp_cor_{len(rows):02d}",
                    "mask": m,
                    "singular_value": float(f"{sv[k]:.10f}"),
                    "variance_explained": float(f"{ve[k]:.10f}"),
                    "cumulative_variance_explained": float(f"{cve[k]:.10f}"),
                    "retained": bool(cve[k] <= 0.5 or k == 0),
                }
            )
    return pd.DataFrame(rows)


def iface_section(tier: str, tmp: Path) -> None:
    from fmriprep.interfaces import confounds as fc
    from niworkflows.interfaces import utility as nu

    rng = np.random.default_rng(17)
    inputs = tmp / "inputs"
    inputs.mkdir()
    k = [0]

    def case(key, klass, fn, args, orig_inputs, lx_kw, defaults_expected=True, model=True):
        k[0] += 1
        d = tmp / f"c{k[0]}"
        want = run_orig(klass, d / "orig", **orig_inputs)
        got, _ = run_lx(
            fn, d / "lx", *args, pandas_version=PANDAS, upstream_defects="reproduce", **lx_kw
        )
        compare(f"{key} reproduce", want, got, str(orig_inputs)[:160])
        if model:
            want = run_orig(klass, d / "model", model=True, **orig_inputs)
            got, entries = run_lx(fn, d / "fix", *args, pandas_version=PANDAS, **lx_kw)
            for e in entries:
                APART[f"{key} default notice {e}"] += 1
            compare(f"{key} default (model)", want, got, str(orig_inputs)[:160])

    # fMRIPrep's FramewiseDisplacement
    ts = (1, 2, 3, 50, 600) if tier == "full" else (1, 2, 50)
    for t in ts:
        for ext in (".tsv", ".tsv.nii.gz"):
            df = motion_frame(rng, t)
            p = write_df(inputs / f"m{t}{ext.replace('.', '_')}{ext}", df)
            for radius in (50.0, 0.0, 1.5):
                case(
                    "fd",
                    fc.FramewiseDisplacement,
                    lx.confounds.fmriprep_framewise_displacement,
                    [p],
                    {"in_file": p, "radius": radius},
                    {"radius": radius},
                )
    df = motion_frame(rng, 20)
    df["extra"] = 1
    df.loc[3, "rot_y"] = np.nan
    df.loc[5, "trans_x"] = np.inf
    p = write_df(inputs / "m_extra.tsv", df[["rot_z", "extra", *df.columns[:5]]])
    case(
        "fd",
        fc.FramewiseDisplacement,
        lx.confounds.fmriprep_framewise_displacement,
        [p],
        {"in_file": p},
        {},
    )
    zero = motion_frame(rng, 10)
    zero["trans_y"] = 0
    p = write_df(inputs / "m_int.tsv", zero)
    case(
        "fd",
        fc.FramewiseDisplacement,
        lx.confounds.fmriprep_framewise_displacement,
        [p],
        {"in_file": p},
        {},
    )
    p = write_df(inputs / "m_missing.tsv", motion_frame(rng, 10).drop(columns=["rot_z"]))
    case(
        "fd",
        fc.FramewiseDisplacement,
        lx.confounds.fmriprep_framewise_displacement,
        [p],
        {"in_file": p},
        {},
    )

    # FilterDropped and RenameACompCor
    for n in (1, 5, 30):
        meta = metadata_frame(rng, n)
        pm = write_df(inputs / f"meta{n}.tsv", meta)
        case("filter", fc.FilterDropped, lx.confounds.filter_dropped, [pm], {"in_file": pm}, {})
        retained = meta[meta.retained]
        comp = pd.DataFrame(
            rng.normal(0, 0.1, (40, len(retained))), columns=retained.component.tolist()
        )
        pc = write_df(inputs / f"acompcor{n}.tsv", comp)
        case(
            "rename",
            fc.RenameACompCor,
            lx.confounds.rename_acompcor,
            [pc, pm],
            {"components_file": pc, "metadata_file": pm},
            {},
        )
    none = metadata_frame(rng, 4)
    none["retained"] = False
    pm = write_df(inputs / "meta_none.tsv", none)
    case("filter", fc.FilterDropped, lx.confounds.filter_dropped, [pm], {"in_file": pm}, {})
    header = inputs / "meta_header.tsv"
    header.write_text("component\tmask\tretained\n")
    case(
        "filter",
        fc.FilterDropped,
        lx.confounds.filter_dropped,
        [str(header)],
        {"in_file": str(header)},
        {},
    )
    na = inputs / "meta_na.tsv"
    na.write_text("component\tretained\na\tTrue\nb\tn/a\n")
    case(
        "filter", fc.FilterDropped, lx.confounds.filter_dropped, [str(na)], {"in_file": str(na)}, {}
    )
    blank = inputs / "blank.tsv"
    blank.write_text("\n\n")
    case(
        "rename",
        fc.RenameACompCor,
        lx.confounds.rename_acompcor,
        [str(blank), str(blank)],
        {"components_file": str(blank), "metadata_file": str(blank)},
        {},
    )

    # GatherConfounds
    t = 30
    sig = pd.DataFrame(
        rng.normal(3000, 500, (t, 5)),
        columns=["global_signal", "csf", "white_matter", "csf_wm", "tcompcor"],
    )
    files = {
        "signals": write_df(inputs / "signals.tsv", sig),
        "std_dvars": write_df(
            inputs / "std_dvars.tsv", pd.DataFrame({"std_dvars": rng.normal(1, 0.2, t - 1)})
        ),
        "dvars": write_df(inputs / "dvars.tsv", pd.DataFrame({"dvars": rng.normal(20, 4, t - 1)})),
        "fd": write_df(
            inputs / "fd.tsv.nii.gz",
            pd.DataFrame({"FramewiseDisplacement": [np.nan, *rng.normal(0.1, 0.05, t - 1)]}),
        ),
        "rmsd": write_df(
            inputs / "rmsd.tsv.nii.gz",
            pd.DataFrame({"rmsd": [np.nan, *rng.normal(0.05, 0.02, t - 1)]}),
        ),
        "tcompcor": write_df(
            inputs / "tcompcor.tsv",
            pd.DataFrame(
                rng.normal(0, 0.1, (t, 4)), columns=[f"t_comp_cor_{i:02d}" for i in range(4)]
            ),
        ),
        "acompcor": write_df(
            inputs / "acompcor.tsv",
            pd.DataFrame(
                rng.normal(0, 0.1, (t, 3)),
                columns=["c_comp_cor_00", "w_comp_cor_00", "a_comp_cor_00"],
            ),
        ),
        "crowncompcor": write_df(
            inputs / "crown.tsv",
            pd.DataFrame(rng.normal(0, 0.1, (t, 2)), columns=["edge_comp_00", "edge_comp_01"]),
        ),
        "cos_basis": write_df(
            inputs / "pre_filter.tsv",
            pd.DataFrame(
                {
                    "cosine00": rng.normal(0, 0.1, t),
                    "non_steady_state_outlier00": [1, *([0] * (t - 1))],
                }
            ),
        ),
        "motion": write_df(inputs / "motion.tsv.nii.gz", motion_frame(rng, t)),
    }
    (inputs / "empty.tsv").write_text("")
    (inputs / "blanks.tsv").write_text("\n\n")
    (inputs / "head.tsv").write_text("only_header\n")
    write_df(
        inputs / "camel.tsv",
        pd.DataFrame(
            {
                "GlobalSignal": rng.normal(size=t),
                "# std DVARS": rng.normal(size=t),
                "aCompCor": rng.normal(size=t),
                "X1Y2": rng.normal(size=t),
            }
        ),
    )
    write_df(inputs / "ints_short.tsv", pd.DataFrame({"flag": [0, 1] * ((t - 2) // 2)}))
    write_df(inputs / "long.tsv", pd.DataFrame({"long": rng.normal(size=t + 2)}))
    combos = [
        dict(files),
        {k: v for k, v in files.items() if k in ("std_dvars", "dvars", "fd")},
        {"std_dvars": files["std_dvars"], "fd": files["fd"], "motion": files["motion"]},
        {
            "signals": str(inputs / "camel.tsv"),
            "dvars": str(inputs / "ints_short.tsv"),
            "fd": str(inputs / "long.tsv"),
        },
        {
            "signals": str(inputs / "empty.tsv"),
            "dvars": str(inputs / "blanks.tsv"),
            "fd": str(inputs / "head.tsv"),
            "motion": files["motion"],
        },
        {"signals": files["signals"], "dvars": files["signals"]},
        {},
    ]
    for combo in combos:
        case("gather", fc.GatherConfounds, lx.confounds.gather_confounds, [], combo, combo)

    # AddTSVHeader
    for name, text, columns in (
        ("dvars", "".join(f"{v:0.6f}\n" for v in rng.normal(20, 4, 29)), ["dvars"]),
        (
            "multi",
            "".join(" ".join(f"{v:.18e}" for v in row) + "\n" for row in rng.normal(size=(5, 3))),
            ["a", "b", "c"],
        ),
        ("one-row", "1.5 2.5 3.5\n", ["a", "b", "c"]),
        ("one-value", "1.5\n", ["dvars"]),
        ("empty", "", ["dvars"]),
        ("mismatch", "1 2\n3 4\n", ["a"]),
        ("comments", "# a comment\n1.0\n2.0\n", ["x"]),
    ):
        p = inputs / f"add_{name}.tsv"
        p.write_text(text)
        k[0] += 1
        d = tmp / f"c{k[0]}"
        want = run_orig(nu.AddTSVHeader, d / "orig", in_file=str(p), columns=columns)
        got, _ = run_lx(
            lx.niworkflows.add_tsv_header, d / "lx", str(p), columns, upstream_defects="reproduce"
        )
        compare("add_tsv_header reproduce", want, got, name)
        got, entries = run_lx(lx.niworkflows.add_tsv_header, d / "fix", str(p), columns)
        if name in ("one-row", "one-value"):
            ok = (
                not isinstance(got, str)
                and entries == ["PIPE-61"]
                and next(iter(got[0].values()))
                == (
                    "\t".join(columns)
                    + "\n"
                    + "\t".join(f"{float(v):.18e}" for v in text.split())
                    + "\n"
                ).encode()
            )
            count("add_tsv_header default (PIPE-61)", ok, f"{name}: {got}")
        elif name == "mismatch":
            count(
                "add_tsv_header default (PIPE-62)",
                isinstance(got, str) and "PIPE-62" in got,
                f"{got}",
            )
        else:
            compare("add_tsv_header default", want, got, name)

    # DictMerge
    for dicts in ([{"a": 1}, {"b": 2.5, "a": 3}], [], [{"x": {"y": 1}}, {}]):
        r = nu.DictMerge(in_dicts=dicts).run().outputs.out_dict
        count(
            "dict_merge", json.dumps(r) == json.dumps(lx.niworkflows.dict_merge(dicts)), str(dicts)
        )

    # TSV2JSON
    meta = metadata_frame(rng, 12)
    pm = write_df(inputs / "meta_json.tsv", meta)
    dup = inputs / "dup.tsv"
    dup.write_text("component\tx\na_b\t1\naB\t2\n")
    numeric = inputs / "numeric.tsv"
    numeric.write_text("component\tvalue\n1\t0.5\n2\t0.25\n")
    for kw in (
        {
            "index_column": "component",
            "output": None,
            "additional_metadata": {"Method": "tCompCor"},
            "drop_columns": ["mask"],
        },
        {
            "index_column": "component",
            "output": None,
            "additional_metadata": {"Method": "aCompCor"},
        },
        {"index_column": "component", "output": None, "enforce_case": False},
        {"index_column": "nothere", "output": None},
        {"index_column": "component", "output": None, "drop_columns": ["missing"]},
        {"index_column": "component"},
        {
            "index_column": "component",
            "output": None,
            "additional_metadata": {"method": 1.123456789012345, "Method": [1.5, 2]},
        },
    ):
        case("tsv2json", nu.TSV2JSON, lx.niworkflows.tsv2json, [pm], {"in_file": pm, **kw}, kw)
    for p in (dup, numeric, blank):
        kw = {"index_column": "component", "output": None}
        case(
            "tsv2json",
            nu.TSV2JSON,
            lx.niworkflows.tsv2json,
            [str(p)],
            {"in_file": str(p), **kw},
            kw,
        )


# --- real ------------------------------------------------------------------------------------

REAL_NODES = (
    "fdisp", "add_dvars_header", "add_std_dvars_header", "rename_acompcor", "acc_metadata_filter",
    "tcc_metadata_filter", "acc_metadata_fmt", "tcc_metadata_fmt", "crowncc_metadata_fmt",
    "merge_confound_metadata2", "concat",
)  # fmt: skip
QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256", "ds005454")


def real_section(tier: str, tmp: Path) -> None:
    from nipype.utils.filemanip import loadpkl

    wfs = sorted(RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if tier == "quick":
        wfs = [w for w in wfs if any(str(w.relative_to(RUNS)).startswith(q) for q in QUICK_REAL)]
    print(f"real: {len(wfs)} confounds workflows")
    for wf in wfs:
        run = wf.relative_to(RUNS).parts[0]

        def host(v, run=run):
            if isinstance(v, str) and v.startswith("/work/"):
                return str(RUNS / run / v.lstrip("/"))
            if isinstance(v, list):
                return [host(x) for x in v]
            return v

        for node in REAL_NODES:
            nd = wf / node
            inputs = {k: host(v) for k, v in loadpkl(nd / "_inputs.pklz").items()}
            result = {}
            if node.endswith("_fmt") or node == "merge_confound_metadata2":
                # The other nodes' results hold files that exist only in fMRIPrep's container.
                result = loadpkl(next(nd.glob("result_*.pklz"))).outputs.get()
            want_files = files_of(nd)
            out = tmp / f"{run}-{wf.parts[-2]}-{node}"
            kw = {"pandas_version": "2.2.3"}
            t0 = time.perf_counter()
            for mode in ("reproduce", "fix"):
                d = out / mode
                d.mkdir(parents=True)
                with warnings.catch_warnings(record=True) as caught:
                    warnings.simplefilter("always")
                    got = real_call(node, inputs, d, mode, kw)
                entries = [
                    w.message.entry
                    for w in caught
                    if isinstance(w.message, lx.UpstreamDefectWarning)
                ]
                got_files = files_of(d)
                if mode == "reproduce":
                    ok = real_equal(node, want_files, result, got_files, got)
                    count(f"real {node}", ok, f"{run}/{wf.parts[-2]}")
                else:
                    differ = not real_equal(node, want_files, result, got_files, got)
                    for e in sorted(set(entries)):
                        APART[f"real default {node}: {e}"] += 1
                    if differ:
                        APART[f"real default {node}: files differ from fMRIPrep's"] += 1
                        cells, changed = cells_changed(want_files, got_files)
                        APART[f"real default {node}: cells changed"] += changed
                        APART[f"real default {node}: cells in the files that differ"] += cells
            SECONDS[node] += time.perf_counter() - t0


def cells_changed(want: dict, got: dict) -> tuple[int, int]:
    """The cells of the tables two runs wrote, and how many hold another text."""
    cells = changed = 0
    for name, data in want.items():
        a = [line.split("\t") for line in data.decode().splitlines()]
        b = [line.split("\t") for line in got.get(name, b"").decode().splitlines()]
        for ra, rb in zip(a[1:], b[1:], strict=False):
            cells += len(ra)
            changed += sum(x != y for x, y in zip(ra, rb, strict=False))
    return cells, changed


def real_call(node: str, inputs: dict, d: Path, mode: str, kw: dict):
    kw = dict(kw, upstream_defects=mode)
    if node == "fdisp":
        return lx.confounds.fmriprep_framewise_displacement(
            inputs["in_file"], radius=inputs["radius"], out_dir=d, **kw
        )
    if node.startswith("add_"):
        return lx.niworkflows.add_tsv_header(
            inputs["in_file"], inputs["columns"], out_dir=d, upstream_defects=mode
        )
    if node == "rename_acompcor":
        return lx.confounds.rename_acompcor(
            inputs["components_file"], inputs["metadata_file"], out_dir=d, **kw
        )
    if node.endswith("_filter"):
        return lx.confounds.filter_dropped(inputs["in_file"], out_dir=d, **kw)
    if node.endswith("_fmt"):
        rest = {k: v for k, v in inputs.items() if k != "in_file"}
        return lx.niworkflows.tsv2json(inputs["in_file"], out_dir=d, **rest, **kw)
    if node == "merge_confound_metadata2":
        return lx.niworkflows.dict_merge(inputs["in_dicts"])
    if node == "concat":
        return lx.confounds.gather_confounds(**inputs, out_dir=d, **kw)
    raise KeyError(node)


def real_equal(node: str, want_files: dict, result: dict, got_files: dict, got) -> bool:
    if node.endswith("_fmt") or node == "merge_confound_metadata2":
        want = result["output" if node.endswith("_fmt") else "out_dict"]
        return json.dumps(want) == json.dumps(got)
    if want_files != got_files:
        return False
    if node == "concat":
        return True  # confounds_list is not set by fMRIPrep's node outputs
    return True


# ---------------------------------------------------------------------------------------------


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="parse,read,write,ujson,iface,real")
    a = p.parse_args()
    import nipype
    import niworkflows

    print(
        f"pandas {PANDAS}, numpy {np.__version__}, nipype {nipype.__version__}, "
        f"niworkflows {niworkflows.__version__}, tier {a.tier}",
        flush=True,
    )
    with tempfile.TemporaryDirectory(prefix="lx-check-tables-") as tmp:
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = Path(tmp) / s
            d.mkdir()
            {
                "parse": parse_section,
                "read": read_section,
                "write": write_section,
                "ujson": ujson_section,
                "iface": iface_section,
                "real": real_section,
            }[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, d) in COUNTS.items():
        print(f"  {key:52s} identical {i:6d}  differ {d:5d}")
    if SECONDS:
        print(
            "  larmorx's seconds per node (both modes): "
            + ", ".join(f"{k} {v:.1f}" for k, v in SECONDS.items())
        )
    for key, n in sorted(APART.items()):
        print(f"  apart: {key} ({n})")
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[1] for c in COUNTS.values())
    print(f"CHECK {'PASS' if differ == 0 else 'DIFFER'}: {differ} cases differ")
    return 0 if differ == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
