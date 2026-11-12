# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""The tables of fMRIPrep's confound steps as pandas reads and writes them, without pandas.

fMRIPrep's confound interfaces (and niworkflows') read their inputs with
``pandas.read_csv(path, sep="\\t")`` and write with ``DataFrame.to_csv(path, sep="\\t",
index=False, na_rep="n/a")``; ``TSV2JSON`` turns a table into a dictionary with ``to_json``.
This module follows pandas 3.0.6 (and 2.2.3, fMRIPrep 25.2.6's: pandas is BSD-3-Clause,
Copyright (c) 2008-2011 AQR Capital Management, LLC, Lambda Foundry, Inc. and PyData Development
Team, and 2011-2026 Open source contributors) on those calls:

- **Reading** (the C engine's defaults): UTF-8 text (a byte-order mark dropped), gzip, bzip2 and
  xz files by their extension (``compression="infer"``), lines ended by ``\\n``, ``\\r\\n`` or
  ``\\r``, blank lines and lines of spaces skipped, tab-separated fields with ``"`` quoting
  (``QUOTE_MINIMAL``, ``""`` inside quotes), the first line the header (an empty name becomes
  ``Unnamed: <i>``, a repeated one ``<name>.<k>``), short rows padded with missing values, a
  longer row pandas' ``ParserError``; each column's type as pandas infers it: 64-bit integers
  (unsigned or Python integers beyond them), else floats, else booleans (``True``/``TRUE``/
  ``true`` and their ``False`` forms, any case), else strings, with pandas' missing-value words
  (``n/a``, ``NaN``, the empty field, ...: NaN; an integer column with one becomes float, a
  boolean one strings and booleans). The floats are read by larmorx's Rust port of pandas'
  ``precise_xstrtod`` (``larmorx_confounds::pandas``): pandas' value, or by default the
  correctly rounded value of the same text (PD-01: pandas reads at most 17 digit characters,
  the leading zeros of ``0.00...`` included, and divides by one power of ten, so most values
  written by ``to_csv`` come back one or more units in the last place off).
- **Writing**: the header and rows through Python's ``csv`` writer as pandas sets it up
  (``QUOTE_MINIMAL``, ``"``), floats as numpy's ``astype(str)`` (their shortest repr), missing
  values ``n/a``; the line terminator is ``\\n`` on every platform (pandas writes
  ``os.linesep``, ``\\r\\n`` on Windows; larmorx's files are Linux's on every platform); a
  ``.gz`` name is written with Python's ``gzip.GzipFile`` as pandas does (the same content; the
  header's time stamp is the time of writing).
- **JSON** (``to_json(orient="index")`` then Python's ``json``): numbers as ujson writes them
  with 10 decimals (:func:`ujson_double`), or by default correctly rounded to 10 decimals
  (PD-02: ujson multiplies the fraction by 1e10 in floating point, truncates and rounds the
  remainder, ties of an even last digit down and of a 0 up); NaN and infinities ``null``.
"""

from __future__ import annotations

import bz2
import csv
import gzip
import io
import lzma
import math
import os
import re
from dataclasses import dataclass, field
from typing import Any

import numpy as np

from larmorx import _core, _defects

__all__ = [
    "PANDAS_NEWEST",
    "Column",
    "EmptyDataError",
    "ParserError",
    "Table",
    "pandas_series",
    "read_tsv",
    "ujson_double",
    "write_tsv",
]

#: The pandas release followed by default (the newest); ``"2.2.3"`` is fMRIPrep 25.2.6's.
PANDAS_NEWEST = "3.0.6"

#: pandas' default missing values (``parsers.pyx``'s ``STR_NA_VALUES``, both releases).
NA_VALUES = frozenset(
    {
        "-1.#IND", "1.#QNAN", "1.#IND", "-1.#QNAN", "#N/A N/A", "#N/A", "N/A", "n/a", "NA",
        "<NA>", "#NA", "NULL", "null", "NaN", "-NaN", "nan", "-nan", "", "None",
    }
)  # fmt: skip
_TRUE = frozenset({"True", "TRUE", "true"})
_FALSE = frozenset({"False", "FALSE", "false"})
_SPACE = " \t\n\v\f\r"
_INT = re.compile(r"[ \t\n\v\f\r]*[+-]?[0-9]+[ \t\n\v\f\r]*")
_UINT = re.compile(r"[ \t\n\v\f\r]*\+?[0-9]+[ \t\n\v\f\r]*")
_INT64_MIN = -(2**63)
_INT64_MAX = 2**63 - 1
_UINT64_MAX = 2**64 - 1


class EmptyDataError(ValueError):
    """pandas' ``EmptyDataError`` (``No columns to parse from file``)."""


class ParserError(ValueError):
    """pandas' ``ParserError`` (a row with more fields than the header)."""


def pandas_series(version: str) -> str:
    """``"3.0"`` or ``"2.2"`` for a pandas version this module follows; ``ValueError``
    otherwise."""
    parts = str(version).split(".")
    if parts[:2] in (["3", "0"], ["2", "2"]):
        return ".".join(parts[:2])
    raise ValueError(f"pandas_version must be a pandas 3.0 or 2.2 release, not {version!r}")


@dataclass
class Column:
    """One column: ``kind`` is ``"float64"``, ``"int64"``, ``"uint64"``, ``"bool"`` (numpy
    arrays) or ``"object"`` (a list of strings, booleans, Python integers and NaN)."""

    kind: str
    values: Any

    def __len__(self) -> int:
        return len(self.values)

    def floats(self) -> np.ndarray:
        """The values as float64 (pandas' ``astype(float)``); ``TypeError`` for strings."""
        if self.kind == "object":
            out = np.empty(len(self.values))
            for i, v in enumerate(self.values):
                if isinstance(v, str):
                    raise TypeError(f"unsupported operand type(s) for -: 'str' and 'str' ({v!r})")
                out[i] = float(v)
            return out
        return np.asarray(self.values, dtype=np.float64)

    def take(self, rows: list[int] | np.ndarray) -> Column:
        if self.kind == "object":
            return Column("object", [self.values[i] for i in rows])
        return Column(self.kind, np.asarray(self.values)[np.asarray(rows, dtype=np.intp)])


@dataclass
class Table:
    """A table as pandas holds it here: column names (repeats allowed), columns, and the row
    labels (``index``: 0, 1, ... after reading; ``GatherConfounds`` shifts them)."""

    names: list[str]
    columns: list[Column]
    index: list[int] = field(default_factory=list)

    @property
    def n_rows(self) -> int:
        return len(self.index)

    def column(self, name: str) -> Column:
        """The first column of that name; ``KeyError`` (pandas' message) otherwise."""
        for n, c in zip(self.names, self.columns, strict=True):
            if n == name:
                return c
        raise KeyError(name)

    def select(self, names: list[str]) -> Table:
        """``df[names]``: ``KeyError("[...] not in index")`` for the names missing."""
        missing = [n for n in names if n not in self.names]
        if missing:
            raise KeyError(f"{missing} not in index")
        cols = []
        out_names = []
        for n in names:
            for m, c in zip(self.names, self.columns, strict=True):
                if m == n:
                    out_names.append(m)
                    cols.append(c)
        return Table(out_names, cols, list(self.index))

    def take_rows(self, rows: list[int]) -> Table:
        """The rows at these positions, with their labels."""
        return Table(
            list(self.names), [c.take(rows) for c in self.columns], [self.index[i] for i in rows]
        )


# --- reading ---------------------------------------------------------------------------------


def _open_text(path: str) -> str:
    low = path.lower()
    for ext in (".tar", ".tar.gz", ".tar.xz", ".tar.bz2", ".zip", ".zst"):
        if low.endswith(ext):
            raise NotImplementedError(
                f"{os.path.basename(path)}: pandas would read it as a {ext[1:]} archive; "
                "larmorx reads plain, gzip, bzip2 and xz tables only"
            )
    with open(path, "rb") as f:
        data = f.read()
    if low.endswith(".gz"):
        data = gzip.decompress(data)
    elif low.endswith(".bz2"):
        data = bz2.decompress(data)
    elif low.endswith(".xz"):
        data = lzma.decompress(data)
    text = data.decode("utf-8")
    return text[1:] if text.startswith("﻿") else text


def _records(text: str) -> list[tuple[int, list[str]]]:
    """The C tokenizer's records with their 1-based line numbers: blank lines and lines of
    spaces skipped; a field that starts with ``"`` is quoted (it may hold tabs, line breaks
    and doubled quotes; what follows its closing quote up to the next tab is appended)."""
    out: list[tuple[int, list[str]]] = []
    if '"' not in text:
        lines = re.split(r"\r\n|\r|\n", text)
        if lines and lines[-1] == "":
            lines.pop()
        for no, line in enumerate(lines, 1):
            if line.strip(" ") == "":
                continue
            out.append((no, line.split("\t")))
        return out
    i, n, no = 0, len(text), 1
    while i < n:
        # START_RECORD: an empty line or a line of spaces is skipped (WHITESPACE_LINE).
        j = i
        while j < n and text[j] == " ":
            j += 1
        if j == n:
            break
        if text[j] in "\r\n":
            no += 1
            i = j + (2 if text.startswith("\r\n", j) else 1)
            continue
        rec_no = no
        fields: list[str] = []
        buf: list[str] = []
        state = "start_field"
        while True:
            if i == n:
                if state in ("quoted",):
                    raise ParserError(
                        f"Error tokenizing data. C error: EOF inside string starting at row {rec_no - 1}"
                    )
                fields.append("".join(buf))
                break
            c = text[i]
            if state == "quoted":
                if c == '"':
                    state = "quote_in_quoted"
                else:
                    if c == "\n" or (c == "\r" and not text.startswith("\r\n", i)):
                        no += 1
                    buf.append(c)
                i += 1
                continue
            if state == "quote_in_quoted" and c == '"':
                buf.append('"')
                state = "quoted"
                i += 1
                continue
            if c in "\r\n":
                fields.append("".join(buf))
                no += 1
                i += 2 if text.startswith("\r\n", i) else 1
                break
            if c == "\t":
                fields.append("".join(buf))
                buf = []
                state = "start_field"
                i += 1
                continue
            if state == "start_field" and c == '"':
                state = "quoted"
                i += 1
                continue
            buf.append(c)
            state = "in_field"
            i += 1
        out.append((rec_no, fields))
    return out


def _dedup(names: list[str]) -> list[str]:
    """pandas' ``_dedup_names`` (``mangle_dupe_cols``): ``x``, ``x.1``, ``x.2``, ..."""
    counts: dict[str, int] = {}
    out = []
    for col in names:
        cur = counts.get(col, 0)
        while cur > 0:
            counts[col] = cur + 1
            col = f"{col}.{cur}"
            cur = counts.get(col, 0)
        out.append(col)
        counts[col] = cur + 1
    return out


def _int_or_none(tokens: list[str]) -> tuple[str, Any]:
    """``_try_int64``: ``("ok", ints, na_count)``, ``("invalid", None, 0)`` (a token with
    other characters: "Number is not int"), ``("nodigits", None, 0)`` or ``("overflow", ...)``,
    stopping at the first token that is not an int64 (as pandas)."""
    ints = []
    na = 0
    for t in tokens:
        if t in NA_VALUES:
            na += 1
            ints.append(None)
            continue
        if not _INT.fullmatch(t):
            stripped = t.lstrip(_SPACE)
            digit_start = stripped[1:] if stripped[:1] in "+-" else stripped
            if not digit_start[:1].isascii() or not digit_start[:1].isdigit():
                return "nodigits", None, 0
            return "invalid", None, 0
        v = int(t.strip(_SPACE))
        if v < _INT64_MIN or v > _INT64_MAX:
            return "overflow", None, 0
        ints.append(v)
    return "ok", ints, na


def _uint_path(tokens: list[str]) -> tuple[str, Any]:
    """``_try_uint64`` after an int64 overflow: ``("uint64", values)``, ``("pylong",)``,
    ``("object",)`` (the strings, missing values kept as text), ``("invalid",)`` or
    ``("nodigits",)``."""
    seen_sint = seen_null = seen_uint = False
    vals = []
    for t in tokens:
        if t in NA_VALUES:
            seen_null = True
            vals.append(0)
            continue
        s = t.lstrip(_SPACE)
        if s.startswith("-"):
            seen_sint = True
            vals.append(0)
            continue
        body = s[1:] if s.startswith("+") else s
        if not body[:1].isascii() or not body[:1].isdigit():
            return ("nodigits",)
        if not _UINT.fullmatch(t):
            return ("invalid",)
        v = int(t.strip(_SPACE))
        if v > _UINT64_MAX:
            return ("pylong",)
        if v > _INT64_MAX:
            seen_uint = True
        vals.append(v)
    if seen_uint and (seen_sint or seen_null):
        return ("object",)
    if seen_sint:
        return ("pylong",)
    return ("uint64", vals)


def _pylong(tokens: list[str]) -> Column | None:
    out: list[Any] = []
    for t in tokens:
        if t in NA_VALUES:
            out.append(math.nan)
            continue
        try:
            out.append(int(t))
        except ValueError:
            return None
    return Column("object", out)


def _bool(tokens: list[str]) -> Column | None:
    out: list[Any] = []
    na = False
    for t in tokens:
        if t in NA_VALUES:
            na = True
            out.append(math.nan)
        elif t in _TRUE or (t.isascii() and t.upper() == "TRUE"):
            out.append(True)
        elif t in _FALSE or (t.isascii() and t.upper() == "FALSE"):
            out.append(False)
        else:
            return None
    if na:
        return Column("object", out)
    return Column("bool", np.array(out, dtype=bool))


@dataclass
class _ReadStats:
    floats: int = 0
    differ: int = 0
    pd03: bool = False


def _floats(tokens: list[str], series: str, mode: str, stats: _ReadStats) -> Column | None:
    got = _core.pandas_try_double(tokens, series)
    if got is None:
        return None
    pd_vals, exact = got
    same = (pd_vals.view(np.uint64) == exact.view(np.uint64)) | (
        np.isnan(pd_vals) & np.isnan(exact)
    )
    stats.floats += int(np.count_nonzero(~np.isnan(exact)))
    stats.differ += int(np.count_nonzero(~same))
    return Column("float64", exact if mode == "fix" else pd_vals)


def _convert(tokens: list[str], series: str, mode: str, stats: _ReadStats) -> Column:
    """pandas' ``_convert_tokens`` without a requested type (int64, float64, bool, object)."""
    if not tokens:
        return Column("object", [])
    status, ints, na = _int_or_none(tokens)
    if status == "ok":
        if na:
            fix = mode == "fix"
            stats.pd03 = stats.pd03 or _INT64_MIN in ints
            # PD-03: pandas marks the missing values of an int64 column with int64's minimum
            # and turns every value equal to it into NaN.
            return Column(
                "float64",
                np.array(
                    [
                        math.nan if v is None or (not fix and v == _INT64_MIN) else float(v)
                        for v in ints
                    ]
                ),
            )
        return Column("int64", np.array(ints, dtype=np.int64))
    if status == "overflow":
        u = _uint_path(tokens)
        if u[0] == "uint64":
            return Column("uint64", np.array(u[1], dtype=np.uint64))
        if u[0] == "pylong":
            col = _pylong(tokens)
            return col if col is not None else Column("object", list(tokens))
        if u[0] == "object":
            return Column("object", list(tokens))
    col = _floats(tokens, series, mode, stats)
    if col is not None:
        return col
    col = _bool(tokens)
    if col is not None:
        return col
    return Column("object", [math.nan if t in NA_VALUES else t for t in tokens])


def read_tsv(
    path: str | os.PathLike[str],
    *,
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: str = "fix",
    stacklevel: int = 2,
) -> Table:
    """``pandas.read_csv(path, sep="\\t")`` (see the module's documentation): the table, with
    PD-01's notice when pandas' parser would read a value as another double (``"fix"``: the
    correctly rounded values are kept) or did (``"reproduce"``: pandas' values)."""
    mode = _defects.check(upstream_defects)
    series = pandas_series(pandas_version)
    path = os.fspath(path)
    text = _open_text(path)
    recs = _records(text)
    if not recs:
        raise EmptyDataError("No columns to parse from file")
    header = recs[0][1]
    names = _dedup([n if n != "" else f"Unnamed: {i}" for i, n in enumerate(header)])
    width = len(names)
    rows = recs[1:]
    if rows and len(rows[0][1]) > width:
        raise NotImplementedError(
            f"{os.path.basename(path)}: the first row has more fields than the header, so "
            "pandas would take the first column as the row index; larmorx reads tables whose "
            "rows have at most the header's fields"
        )
    for no, fields in rows:
        if len(fields) > width:
            raise ParserError(
                f"Error tokenizing data. C error: Expected {width} fields in line {no}, "
                f"saw {len(fields)}\n"
            )
    tokens = [[(f[j] if j < len(f) else "") for _, f in rows] for j in range(width)]
    stats = _ReadStats()
    cols = [_convert(t, series, mode, stats) for t in tokens]
    notices = []
    if stats.differ:
        what = (
            f"pandas' read_csv would read {stats.differ} of {stats.floats} values of "
            f"{os.path.basename(path)} as other doubles than the correctly rounded ones "
            "(precise_xstrtod keeps 17 digit characters, leading zeros included): "
        )
        notices.append(
            (
                "PD-01",
                "pandas read_csv",
                mode,
                what
                + (
                    "the correctly rounded values are read"
                    if mode == "fix"
                    else "they are read as pandas reads them"
                ),
            )
        )
    if stats.pd03:
        notices.append(
            (
                "PD-03",
                "pandas read_csv",
                mode,
                "an integer column with missing values holds int64's minimum, which pandas "
                "takes for a missing value: "
                + ("it is kept" if mode == "fix" else "it is read as NaN, as pandas"),
            )
        )
    if notices:
        _defects.warn(notices, stacklevel=stacklevel + 1)
    return Table(names, cols, list(range(len(rows))))


# --- writing ---------------------------------------------------------------------------------


def _cells(col: Column) -> list[str]:
    if col.kind == "float64":
        arr = np.asarray(col.values, dtype=np.float64)
        out = arr.astype(str).tolist()
        for i in np.flatnonzero(np.isnan(arr)).tolist():
            out[i] = "n/a"
        return out
    if col.kind in ("int64", "uint64"):
        return [str(int(v)) for v in np.asarray(col.values).tolist()]
    if col.kind == "bool":
        return ["True" if v else "False" for v in np.asarray(col.values).tolist()]
    out = []
    for v in col.values:
        if v is None or (isinstance(v, float) and math.isnan(v)):
            out.append("n/a")
        else:
            out.append(str(v))
    return out


def tsv_text(table: Table) -> str:
    """``to_csv(sep="\\t", index=False, na_rep="n/a")``'s text, ``\\n`` line ends."""
    buf = io.StringIO(newline="")
    w = csv.writer(
        buf,
        delimiter="\t",
        quotechar='"',
        quoting=csv.QUOTE_MINIMAL,
        doublequote=True,
        lineterminator="\n",
    )
    w.writerow([str(n) for n in table.names])
    cells = [_cells(c) for c in table.columns]
    for i in range(table.n_rows):
        w.writerow([c[i] for c in cells])
    return buf.getvalue()


def write_tsv(path: str | os.PathLike[str], table: Table) -> str:
    """Write ``table`` as pandas' ``to_csv(path, sep="\\t", index=False, na_rep="n/a")`` does
    (gzip for a ``.gz`` name); the path."""
    path = os.fspath(path)
    data = tsv_text(table).encode("utf-8")
    if path.lower().endswith(".gz"):
        with gzip.GzipFile(filename=path, mode="wb") as f:
            f.write(data)
    else:
        with open(path, "wb") as f:
            f.write(data)
    return path


# --- JSON ------------------------------------------------------------------------------------

_POW10 = [float(10**k) for k in range(16)]


def ujson_double(value: float, precision: int = 10) -> str:
    """ujson's ``Buffer_AppendDoubleUnchecked`` (pandas' vendored ujson, the same in 3.0.6 and
    2.2.3) of a finite double: fixed notation with at most ``precision`` decimals (trailing
    zeros dropped, at least one), C's ``%.<precision>g`` above 1e16 - 1 and below 1e-15."""
    thres_max = 1e16 - 1
    thres_min = 1e-15
    if math.isinf(value) or math.isnan(value):
        raise ValueError("Invalid Inf or Nan value when encoding double")
    neg = value < 0
    if neg:
        value = -value
    if value > thres_max or (value != 0.0 and abs(value) < thres_min):
        return "%.*g" % (precision, -value if neg else value)  # noqa: UP031 (C's printf)
    pow10 = _POW10[precision]
    whole = int(value)
    tmp = (value - float(whole)) * pow10
    frac = int(tmp)
    diff = tmp - float(frac)
    if diff > 0.5 or (diff == 0.5 and (frac == 0 or frac & 1)):
        frac += 1
    if float(frac) >= pow10:
        frac = 0
        whole += 1
    if precision == 0:
        diff = value - float(whole)
        if diff > 0.5 or (diff == 0.5 and whole & 1):
            whole += 1
        text = f"{whole}"
    elif frac:
        count = precision
        while frac % 10 == 0:
            count -= 1
            frac //= 10
        text = f"{whole}." + str(frac).rjust(count, "0")
    else:
        text = f"{whole}.0"
    return "-" + text if neg else text


def json_value(v: Any, mode: str, stats: list[int]) -> Any:
    """A table value as ``to_json`` writes it and Python's ``json`` reads it back: floats by
    :func:`ujson_double` (``"reproduce"``) or correctly rounded to 10 decimals (``"fix"``),
    NaN and infinities ``None``, booleans, integers and strings unchanged."""
    if isinstance(v, (bool, np.bool_)):
        return bool(v)
    if isinstance(v, (int, np.integer)):
        return int(v)
    if isinstance(v, (float, np.floating)):
        x = float(v)
        if math.isnan(x) or math.isinf(x):
            return None
        uj = float(ujson_double(x))
        a = abs(x)
        exact = uj if a > 1e16 - 1 or (a != 0.0 and a < 1e-15) else float(f"{x:.10f}")
        stats[0] += 1
        if uj != exact:
            stats[1] += 1
        return exact if mode == "fix" else uj
    return v
