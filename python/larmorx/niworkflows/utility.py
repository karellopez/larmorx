# SPDX-License-Identifier: Apache-2.0
"""niworkflows' table utilities under fMRIPrep's confounds (``niworkflows/interfaces/utility.py``).

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's):

- :func:`add_tsv_header`: ``AddTSVHeader``: a whitespace-separated table of numbers read with
  numpy's ``loadtxt`` and written with ``savetxt`` (``%.18e``, tab-separated) under a header of
  column names, as ``<stem>_motion.tsv``. PIPE-61: niworkflows' ``loadtxt`` squeezes a table of
  one row to a vector, which ``savetxt`` writes as one column, and a table of one value to a
  scalar, which ``savetxt`` refuses (``ValueError``); larmorx keeps the table's shape by
  default. PIPE-62: a header with another number of names than the table's columns is written
  as it is by niworkflows; larmorx refuses it by default.
- :func:`dict_merge`: ``DictMerge``: dictionaries merged in order (later ones win).
- :func:`tsv2json`: ``TSV2JSON``: a table turned into a dictionary keyed by one of its columns
  (pandas' ``read_csv`` and ``to_json(orient="index")`` then Python's ``json``), the keys in snake
  case and the fields in camel case (``enforce_case``), extra metadata added to every entry;
  returned, or written as JSON (indent 4). The numbers are rounded to 10 decimals as pandas'
  ujson writes them with ``upstream_defects="reproduce"``, correctly by default (PD-02); the
  table is read as pandas reads it (PD-01).
"""

from __future__ import annotations

import json
import os
import re
import warnings
from collections import OrderedDict
from collections.abc import Iterable, Mapping, Sequence
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx._nipype import UNDEFINED, fname_presuffix, isdefined
from larmorx.confounds import _tables as tb
from larmorx.confounds._tables import PANDAS_NEWEST, Column, Table

__all__ = ["add_tsv_header", "dict_merge", "tsv2json"]


def _existing(spec: str, name: str, value: Any) -> str:
    path = os.fspath(value)
    if not os.path.isfile(path):
        raise ValueError(
            f"The trait '{name}' of a {spec} instance is an existing file name, but the path "
            f"'{path}' does not exist."
        )
    return path


def _out_dir(out_dir: os.PathLike[str] | str) -> str:
    d = os.path.abspath(os.fspath(out_dir))
    os.makedirs(d, exist_ok=True)
    return d


def add_tsv_header(
    in_file: os.PathLike[str] | str,
    columns: Sequence[str],
    *,
    out_dir: os.PathLike[str] | str = ".",
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``AddTSVHeader`` interface: the numbers of ``in_file`` (numpy's
    ``loadtxt``: whitespace-separated, ``#`` comments) written with the header
    ``"\\t".join(columns)`` (numpy's ``savetxt``: ``%.18e``, tabs) as ``<stem>_motion.tsv`` in
    ``out_dir``; the path. A table of one row stays one row and a table of one value is written
    (PIPE-61; ``"reproduce"``: niworkflows' column and numpy's ``ValueError``); a header whose
    number of names differs from the table's columns raises ``ValueError`` (PIPE-62;
    ``"reproduce"``: written as niworkflows writes it)."""
    mode = _defects.check(upstream_defects)
    path = _existing("_AddTSVHeaderInputSpec", "in_file", in_file)
    if isinstance(columns, str) or not all(isinstance(c, str) for c in columns):
        raise ValueError(
            "The 'columns' trait of an _AddTSVHeaderInputSpec instance must be a list of items "
            f"which are a string, but a value of {columns!r} {type(columns)!r} was specified."
        )
    columns = list(columns)
    out = fname_presuffix(path, suffix="_motion.tsv", newpath=_out_dir(out_dir), use_ext=False)
    data = np.loadtxt(path)
    table = data
    notices = []
    if data.ndim < 2 and data.size:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            shaped = np.loadtxt(path, ndmin=2)
        if shaped.shape[0] == 1:
            notices.append(
                (
                    "PIPE-61",
                    "niworkflows AddTSVHeader",
                    mode,
                    f"numpy's loadtxt squeezes the table of one row ({data.size} values) to "
                    + (
                        "a scalar, which savetxt refuses"
                        if data.ndim == 0
                        else "a vector, written as one column"
                    )
                    + (": the row is written as a row" if mode == "fix" else ", as niworkflows"),
                )
            )
            if mode == "fix":
                table = shaped
    ncols = len(columns) if table.size == 0 else 1 if table.ndim < 2 else table.shape[1]
    if ncols != len(columns) and mode == "reproduce":
        notices.append(
            (
                "PIPE-62",
                "niworkflows AddTSVHeader",
                mode,
                f"the header has {len(columns)} names for a table of {ncols} columns: written "
                "as niworkflows writes it",
            )
        )
    _defects.warn(notices, stacklevel=2)
    if ncols != len(columns) and mode == "fix":
        raise ValueError(
            f"AddTSVHeader: {len(columns)} column names for a table of {ncols} columns (PIPE-62)"
        )
    np.savetxt(out, table, delimiter="\t", header="\t".join(columns), comments="")
    return out


def dict_merge(in_dicts: Iterable[Mapping[Any, Any]]) -> dict[Any, Any]:
    """niworkflows' ``DictMerge`` interface: one dictionary with the keys of ``in_dicts`` in
    order, a later dictionary's value winning."""
    in_dicts = list(in_dicts)
    for d in in_dicts:
        if not isinstance(d, dict):
            raise ValueError(
                "Each element of the 'in_dicts' trait of a _DictMergeInputSpec instance must be "
                "a dict or an OrderedDict, but a value of "
                f"{d!r} {type(d)!r} was specified."
            )
    out: dict[Any, Any] = {}
    for d in in_dicts:
        out.update(d)
    return out


_RE_TO_CAMEL = r"(.*?)_([a-zA-Z0-9])"
_RE_TO_SNAKE = r"(^.+?|.*?)((?<![_A-Z])[A-Z]|(?<![_0-9])[0-9]+)"


def _snake(match: re.Match[str]) -> str:
    return f"{match.group(1).lower()}_{match.group(2).lower()}"


def _camel(match: re.Match[str]) -> str:
    return f"{match.group(1)}{match.group(2).upper()}"


def _less_breakable(a_string: str) -> str:
    return "".join(a_string.split()).strip("#")


def _python_values(col: Column) -> list[Any]:
    if col.kind == "object":
        return list(col.values)
    return np.asarray(col.values).tolist()


def tsv2json(
    in_file: os.PathLike[str] | str,
    index_column: str,
    *,
    output: os.PathLike[str] | str | Any | None = UNDEFINED,
    additional_metadata: Mapping[str, Any] | None = None,
    drop_columns: Sequence[str] | None = None,
    enforce_case: bool = True,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> dict[str, Any] | str:
    """niworkflows' ``TSV2JSON`` interface (``_tsv2json``): the table ``in_file`` keyed by its
    column ``index_column`` (a table without it keeps its row numbers), ``additional_metadata``
    added to every row as columns and again to every entry, ``drop_columns`` removed (pandas'
    ``KeyError`` for a missing one), with ``enforce_case`` the keys in snake case and the fields
    in title-then-camel case (``Csf`` → ``CSF``); returned as an ordered dictionary when
    ``output`` is ``None``, else written as JSON (indent 4) to ``output`` (left undefined:
    ``<stem>.json`` in ``out_dir``) and the path returned. pandas' errors for an index or
    columns that are not unique."""
    mode = _defects.check(upstream_defects)
    path = _existing("_TSV2JSONInputSpec", "in_file", in_file)
    if not isinstance(index_column, str):
        raise ValueError(
            "The 'index_column' trait of a _TSV2JSONInputSpec instance must be a string, but a "
            f"value of {index_column!r} {type(index_column)!r} was specified."
        )
    if not isdefined(output):
        output = fname_presuffix(path, suffix=".json", newpath=_out_dir(out_dir), use_ext=False)
    drop_columns = list(drop_columns or [])
    additional_metadata = dict(additional_metadata or {})
    try:
        table = tb.read_tsv(path, pandas_version=pandas_version, upstream_defects=mode)
    except tb.EmptyDataError:
        table = Table([], [], [])
    names = list(table.names)
    cols = list(table.columns)
    n = table.n_rows
    for k, v in additional_metadata.items():
        col = Column("object", [v] * n)
        if k in names:
            cols[names.index(k)] = col
        else:
            names.append(k)
            cols.append(col)
    for c in drop_columns:
        if c not in names:
            raise KeyError(f"[{c!r}] not found in axis")
        keep = [i for i, m in enumerate(names) if m != c]
        names = [names[i] for i in keep]
        cols = [cols[i] for i in keep]
    index: list[Any] = list(range(n))
    if index_column in names:
        i = names.index(index_column)
        index = _python_values(cols[i])
        names = names[:i] + names[i + 1 :]
        cols = cols[:i] + cols[i + 1 :]
    if enforce_case:
        index = [re.sub(_RE_TO_SNAKE, _snake, _less_breakable(i), count=0).lower() for i in index]
        names = [
            re.sub(_RE_TO_CAMEL, _camel, _less_breakable(i).title(), count=0).replace("Csf", "CSF")
            for i in names
        ]
    if len(set(index)) != len(index):
        raise ValueError("DataFrame index must be unique for orient='index'.")
    if len(set(names)) != len(names):
        raise ValueError("DataFrame columns must be unique for orient='index'.")
    stats = [0, 0]
    values = [_python_values(c) for c in cols]
    json_data: OrderedDict[str, Any] = OrderedDict()
    for r, label in enumerate(index):
        if not isinstance(label, (str, int)) or isinstance(label, bool):
            raise NotImplementedError(
                f"TSV2JSON: the row label {label!r} is not a string or an integer"
            )
        json_data[str(label)] = OrderedDict(
            (name, _json(vals[r], mode, stats)) for name, vals in zip(names, values, strict=True)
        )
    if stats[1]:
        _defects.warn(
            [
                (
                    "PD-02",
                    "pandas to_json",
                    mode,
                    f"pandas' to_json (ujson, 10 decimals) rounds {stats[1]} of {stats[0]} values "
                    f"of {os.path.basename(path)} to another 10-decimal value than the correctly "
                    "rounded one: "
                    + (
                        "they are rounded correctly"
                        if mode == "fix"
                        else "they are rounded as ujson rounds them"
                    ),
                )
            ],
            stacklevel=2,
        )
    for i in json_data:
        json_data[i].update(additional_metadata)
    if output is None:
        return json_data
    out = os.fspath(output)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(json_data, f, indent=4)
    return out


def _json(v: Any, mode: str, stats: list[int]) -> Any:
    """``to_json`` then ``json.loads`` of one value (dictionaries and lists element by
    element)."""
    if isinstance(v, dict):
        return OrderedDict((str(k), _json(x, mode, stats)) for k, x in v.items())
    if isinstance(v, (list, tuple)):
        return [_json(x, mode, stats) for x in v]
    if v is None:
        return None
    return tb.json_value(v, mode, stats)
