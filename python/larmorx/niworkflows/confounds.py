# SPDX-License-Identifier: Apache-2.0
"""niworkflows' confound model interfaces (``niworkflows/interfaces/confounds.py``).

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's, and 1.15.3 on these functions), with pandas' reading and writing of the
tables (:mod:`larmorx.confounds._tables`: pandas 3.0.6 by default, ``pandas_version="2.2.3"``
for fMRIPrep 25.2.6's image) and numpy's arithmetic under them:

- :func:`expand_model`: ``ExpandModel``: a confound model expanded by a formula
  (:func:`parse_formula`; fMRIPrep's ``(dd1(rps + wm + csf + gsr))^^2 + others``: the motion
  parameters and the white-matter, CSF and global signals with their first temporal derivatives
  and the squares of both, then every other column), written as ``<stem>_expansion.tsv``.
- :func:`spike_regressors_node`: ``SpikeRegressors``: one spike regressor per volume whose
  framewise displacement or standardised DVARS exceeds its threshold (:func:`spike_regressors`),
  appended to the table; fMRIPrep's ``desc-confounds_timeseries.tsv`` is this node's output.
- :func:`normalize_motion_params`: ``NormalizeMotionParams``: motion parameters in SPM's order
  and units (nipype's ``normalize_mc_params``, :func:`larmorx.confounds.normalize_mc_params`).
- :func:`fsl_motion_params`, :func:`fsl_rms_deviation`, :func:`framewise_displacement`:
  niworkflows' copies of fMRIPrep's ``FSLMotionParams`` (fMRIPrep 25.2.7's form in 1.15.4,
  25.2.6's before: ``niworkflows_version``), ``FSLRMSDeviation`` and ``FramewiseDisplacement``
  (:mod:`larmorx.confounds.motion`, :func:`larmorx.confounds.fmriprep_framewise_displacement`).
- :func:`parse_formula`, :func:`temporal_derivatives`, :func:`exponential_terms`,
  :func:`spike_regressors`: niworkflows' functions on tables (:class:`ConfoundTable`, or a
  mapping of column names to arrays as pandas' ``read_csv`` types them: float64, int64, uint64,
  booleans, text).

The arithmetic: a derivative is numpy's ``diff`` (``n`` times the difference of consecutive
values), the first rows NaN; a power ``o`` is numpy's ``x ** o`` (``x * x`` for 2, 1 for 0,
integers in their own type, which wraps around); for other exponents on floats glibc 2.35's
``pow`` with ``upstream_defects="reproduce"`` (numpy's on CPUs without AVX-512; with AVX-512
numpy's wheels take Intel's SVML ``pow``, which larmorx does not follow: NP-04's class) and the
correctly rounded ``pow`` by default (GLIBC-07, the notice when a value differs). Integer,
boolean and text columns follow numpy's own arithmetic (``diff`` of booleans is their
difference as numpy's ``not_equal``; text raises Python's ``TypeError``), as pandas hands them
to numpy; the columns of a group are put side by side in pandas' common type first.

The defects (``upstream_defects="fix"`` corrects them, ``"reproduce"`` gives niworkflows'
result; :mod:`larmorx._defects`):

- PIPE-43 (never reproduced: nondeterminism): ``parse_formula`` orders the variables of each
  group, and the ``others``, as a Python set of strings, whose order changes between processes
  (Python's per-process hash seed), so the expansion's columns of one variable come in another
  order from one fMRIPrep run to the next (``x_power2`` before or after
  ``x_derivative1_power2``). larmorx keeps the formula's order in both modes (the variable, its
  derivatives, their powers; ``others`` in the table's order), one niworkflows can produce.
- PIPE-64: the shorthands (``wm``, ``gsr``, ``rps``, ``fd``, ``acc``, ``tcc``, ``dv``,
  ``dvall``, ``nss``, ``spikes``, ``others``) are replaced as text, anywhere and again inside
  every group: a name holding one (``std_dvars``) is rewritten (``std_std_dvarsars``),
  ``dvall`` becomes ``std_dvarsall`` (``dv`` is replaced first), a shorthand that matches no
  column leaves an empty term (``IndexError: string index out of range``), and the names that
  ``others`` leaves out are the formula's with every ``d<digits>`` and ``^<digits>`` cut out of
  them. larmorx replaces whole names only, once, and drops a term that matches nothing.
- PIPE-65: a term's operators are read with regular expressions over the whole term: text
  around a group other than a derivative before it and an exponent after it is ignored
  (``foo(a)``, ``(a)b``, ``d1x(a)``), an exponent after a group that holds another single-caret
  one fails (``((a)^2)^3``: ``TypeError``), and a name that starts like a derivative or ends
  like an exponent (``d1a``, ``a^2``) is taken for an operator without its group
  (``TypeError``). larmorx reads the derivative before the group and the exponent after it,
  refuses other text with ``ValueError`` and reads a term without a group as a name.
- PIPE-66: a variable reached by two terms (``a + (a)^^2``) is repeated: twice in
  ``parse_formula``'s table, four times in ``ExpandModel``'s file (pandas selects every column
  of a repeated name for each time it is named), and a group that repeats one fails with
  pandas' ``ValueError: Shape of passed values ...``. larmorx keeps each column once.
- PIPE-67: ``spike_regressors`` adds each lag to the volumes the earlier lags added, so lags
  ``[1, 2]`` flag the third volume after a spike too. larmorx adds every lag to the flagged
  volumes only.
- PIPE-68: ``minimum_contiguous`` counts the last run of unflagged volumes one volume longer
  than it is, so a final run one volume shorter than the minimum is kept. larmorx counts it.
- PIPE-69: a criterion other than ``'<'`` and ``'>'`` is skipped without an error (all of them:
  ``TypeError: reduce() of empty iterable``). larmorx refuses it (``ValueError``).
- PIPE-70: ``NormalizeMotionParams`` reads with numpy's ``loadtxt``, which squeezes a table of
  one row to a vector: numpy's ``AxisError``. larmorx keeps the row.

The tables are read as pandas reads them (PD-01: the correctly rounded values by default).
"""

from __future__ import annotations

import operator
import os
import re
from collections import OrderedDict, deque
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass, field
from functools import reduce
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx._nipype import fname_presuffix
from larmorx.confounds import _tables as tb
from larmorx.confounds._tables import PANDAS_NEWEST, Column, Table

__all__ = [
    "DEFAULT_FORMULA",
    "NIWORKFLOWS_NEWEST",
    "ConfoundTable",
    "ModelTerms",
    "expand_model",
    "exponential_terms",
    "framewise_displacement",
    "fsl_motion_params",
    "fsl_rms_deviation",
    "normalize_motion_params",
    "parse_formula",
    "spike_regressors",
    "spike_regressors_node",
    "temporal_derivatives",
]

#: ``ExpandModel``'s default formula (fMRIPrep's): the 36-parameter expansion's terms and every
#: other column.
DEFAULT_FORMULA = "(dd1(rps + wm + csf + gsr))^^2 + others"

NUMPY_NEWEST = "2.5.4"

# A term that a shorthand or ``others`` filled with nothing (PIPE-64's correction): no column.
_EMPTY = "\x00"
_TOKEN = re.compile(r"[^\s+()]+")


# --- tables ----------------------------------------------------------------------------------


@dataclass(frozen=True)
class ConfoundTable:
    """A table as niworkflows' functions return it (a pandas ``DataFrame``): column names in
    order (a name may repeat, as in pandas) and one 1-D column per name: a numpy array of
    float64, int64, uint64 or booleans, or a list (text, Python integers, NaN)."""

    names: tuple[str, ...]
    columns: tuple[Any, ...]

    @property
    def n_rows(self) -> int:
        return len(self.columns[0]) if self.columns else 0

    def __getitem__(self, name: str) -> Any:
        """The first column named ``name`` (``KeyError`` otherwise)."""
        for n, c in zip(self.names, self.columns, strict=True):
            if n == name:
                return c
        raise KeyError(name)

    def as_dict(self) -> dict[str, Any]:
        """The columns by name (a repeated name: its first column)."""
        out: dict[str, Any] = {}
        for n, c in zip(self.names, self.columns, strict=True):
            out.setdefault(n, c)
        return out


@dataclass(frozen=True)
class ModelTerms:
    """The two results of :func:`parse_formula`, :func:`temporal_derivatives` and
    :func:`exponential_terms` (niworkflows returns them as a tuple: this unpacks the same way):
    the model's variables and their table."""

    variables: list[str]
    data: ConfoundTable

    def __iter__(self) -> Iterator[Any]:
        return iter((self.variables, self.data))


_KINDS = {"f8": "float64", "i8": "int64", "u8": "uint64", "b1": "bool"}


def _column(name: str, values: Any) -> Column:
    if isinstance(values, Column):
        return values
    if isinstance(values, np.ndarray):
        if values.dtype == object and values.ndim == 1:
            return Column("object", values.tolist())
        kind = _KINDS.get(values.dtype.str[1:])
        if kind is None or values.ndim != 1:
            raise TypeError(
                f"column {name!r}: larmorx takes the column types pandas' read_csv gives (1-D "
                f"float64, int64, uint64 or boolean arrays, or lists), not {values.dtype} of "
                f"shape {values.shape}"
            )
        return Column(kind, values)
    if isinstance(values, (list, tuple)):
        return Column("object", list(values))
    return _column(name, np.asarray(values))


def _as_table(data: Any) -> Table:
    if isinstance(data, Table):
        return data
    if isinstance(data, ConfoundTable):
        names, values = list(data.names), list(data.columns)
    elif isinstance(data, Mapping):
        names, values = [str(k) for k in data], list(data.values())
    else:
        raise TypeError(
            "data must be a ConfoundTable or a mapping of column names to columns, not "
            f"{type(data).__name__}"
        )
    cols = [_column(n, v) for n, v in zip(names, values, strict=True)]
    lengths = {len(c) for c in cols}
    if len(lengths) > 1:
        raise ValueError("All arrays must be of the same length")
    n = lengths.pop() if lengths else 0
    return Table(names, cols, list(range(n)))


def _public(table: Table) -> ConfoundTable:
    return ConfoundTable(
        tuple(table.names),
        tuple(c.values if c.kind == "object" else np.asarray(c.values) for c in table.columns),
    )


def _one(table: Table, name: str) -> Table:
    """``parent_data[name]`` (a pandas ``Series``) as a one-column table; ``KeyError``."""
    return Table([name], [table.column(name)], list(table.index))


def _concat(parts: list[Table], index: list[int]) -> Table:
    """``pd.concat(parts, axis=1)`` of tables on the same rows."""
    names: list[str] = []
    cols: list[Column] = []
    for p in parts:
        names += p.names
        cols += p.columns
    return Table(names, cols, list(index))


def _array(c: Column) -> np.ndarray:
    """A column as numpy holds it in pandas' block (text and Python integers as objects)."""
    if c.kind == "object":
        out = np.empty(len(c.values), dtype=object)
        out[:] = list(c.values)
        return out
    return np.asarray(c.values)


def _values(table: Table) -> np.ndarray:
    """``numpy.asarray(frame)``: the columns side by side in pandas' common type (numpy's
    promotion: float64 for int64 beside float64 or uint64; objects for booleans beside other
    types, and for text)."""
    n = table.n_rows
    if not table.columns:
        return np.empty((n, 0))
    arrays = [_array(c) for c in table.columns]
    kinds = {a.dtype.kind for a in arrays}
    if "O" in kinds or ("b" in kinds and len(kinds) > 1):
        dtype = np.dtype(object)
    else:
        dtype = np.result_type(*arrays)
    out = np.empty((n, len(arrays)), dtype=dtype)
    for j, a in enumerate(arrays):
        out[:, j] = a
    return out


def _kind(dtype: np.dtype) -> str:
    return {"f": "float64", "i": "int64", "u": "uint64", "b": "bool"}.get(dtype.kind, "object")


def _frame(names: list[str], data: np.ndarray, index: list[int]) -> Table:
    """``pd.DataFrame(columns=names, data=data)`` of a 2-D array: pandas' ``ValueError`` when the
    shapes disagree."""
    if data.shape[1] != len(names):
        raise ValueError(
            f"Shape of passed values is {data.shape}, indices imply {(data.shape[0], len(names))}"
        )
    kind = _kind(data.dtype)
    cols = [
        Column(kind, data[:, j].tolist() if kind == "object" else np.ascontiguousarray(data[:, j]))
        for j in range(data.shape[1])
    ]
    return Table(list(names), cols, list(index))


# --- the model formula -----------------------------------------------------------------------


@dataclass
class _Run:
    """One top-level call: the mode, the order of niworkflows' sets (``"first"``: the formula's;
    ``"reversed"``: to tell whether niworkflows' order depends on them, PIPE-43) and what the
    notices need."""

    mode: str
    order: str = "first"
    pipe64: list[str] = field(default_factory=list)
    pipe65: list[str] = field(default_factory=list)
    pipe66: list[str] = field(default_factory=list)
    pow_values: int = 0
    pow_differ: int = 0

    @property
    def fix(self) -> bool:
        return self.mode == "fix"

    def ordered(self, items: Any) -> list[Any]:
        """``list(set(items))`` in the formula's order (or reversed)."""
        out = list(dict.fromkeys(items))
        return out[::-1] if self.order == "reversed" else out


def _matches(regex: str, variables: Sequence[str]) -> str:
    m = re.compile(regex)
    return " + ".join(v for v in variables if m.match(v))


def _shorthands(variables: Sequence[str]) -> dict[str, str]:
    return {
        "wm": "white_matter",
        "gsr": "global_signal",
        "rps": "trans_x + trans_y + trans_z + rot_x + rot_y + rot_z",
        "fd": "framewise_displacement",
        "acc": _matches("a_comp_cor_[0-9]+", variables),
        "tcc": _matches("t_comp_cor_[0-9]+", variables),
        "dv": _matches("^std_dvars$", variables),
        "dvall": _matches(".*dvars", variables),
        "nss": _matches("non_steady_state_outlier[0-9]+", variables),
        "spikes": _matches("motion_outlier[0-9]+", variables),
    }


def _get_variables_from_formula(model_formula: str) -> list[str]:
    """niworkflows' names of a formula: the operators, spaces and parentheses cut out of the
    text (wherever they occur), then split at ``+``."""
    for symbol in (
        " ", r"\(", r"\)", "dd[0-9]+", r"d[0-9]+[\-]?[0-9]*", r"\^\^[0-9]+", r"\^[0-9]+[\-]?[0-9]*",
    ):  # fmt: skip
        model_formula = re.sub(symbol, "", model_formula)
    return model_formula.split("+")


def _formula_names(model_formula: str) -> set[str]:
    """The names of a formula as larmorx reads it: its words that are neither an exponent
    (``^...``) nor the text before a group."""
    out = set()
    for m in _TOKEN.finditer(model_formula):
        tok = m.group(0)
        if tok.startswith("^") or tok == _EMPTY:
            continue
        if re.match(r"\s*\(", model_formula[m.end() :]):
            continue
        out.add(tok)
    return out


def _expand_niworkflows(model_formula: str, variables: Sequence[str], run: _Run) -> str:
    """niworkflows' ``_expand_shorthand``: text replacements in this order, then ``others``."""
    sh = _shorthands(variables)
    for key in ("wm", "gsr", "rps", "fd", "acc", "tcc", "dv", "dvall", "nss", "spikes"):
        model_formula = model_formula.replace(key, sh[key])
    formula_variables = set(_get_variables_from_formula(model_formula))
    others = " + ".join(run.ordered(v for v in variables if v not in formula_variables))
    return model_formula.replace("others", others)


def _expand_names(model_formula: str, variables: Sequence[str], run: _Run) -> str:
    """PIPE-64's correction: the shorthands and ``others`` replaced where they are whole names;
    one that matches no column leaves no term."""
    sh = _shorthands(variables)

    def repl(m: re.Match[str]) -> str:
        tok = m.group(0)
        return (sh[tok] or _EMPTY) if tok in sh else tok

    model_formula = _TOKEN.sub(repl, model_formula)
    names = _formula_names(model_formula)
    others = " + ".join(run.ordered(v for v in variables if v not in names)) or _EMPTY
    return _TOKEN.sub(lambda m: others if m.group(0) == "others" else m.group(0), model_formula)


def _order_as_range(order: str) -> Any:
    parts = [int(o) for o in order.split("-")]
    if len(parts) > 1:
        return range(parts[0], (parts[-1] + 1))
    return parts


def _check_and_expand_exponential(expr: str, variables: Any, data: Any, parent: Table, run: _Run):
    if re.search(r"\^\^[0-9]+$", expr):
        order: Any = re.compile(r"\^\^([0-9]+)$").findall(expr)
        order = range(1, int(*order) + 1)
        variables, data = _exponential_terms(order, variables, data, parent, run)
    elif re.search(r"\^[0-9]+[\-]?[0-9]*$", expr):
        order = re.compile(r"\^([0-9]+[\-]?[0-9]*)").findall(expr)
        order = _order_as_range(*order)
        variables, data = _exponential_terms(order, variables, data, parent, run)
    return variables, data


def _check_and_expand_derivative(expr: str, variables: Any, data: Any, parent: Table, run: _Run):
    if re.search(r"^dd[0-9]+", expr):
        order: Any = re.compile(r"^dd([0-9]+)").findall(expr)
        order = range(int(*order) + 1)
        variables, data = _temporal_derivatives(order, variables, data, parent, run)
    elif re.search(r"^d[0-9]+[\-]?[0-9]*", expr):
        order = re.compile(r"^d([0-9]+[\-]?[0-9]*)").findall(expr)
        order = _order_as_range(*order)
        variables, data = _temporal_derivatives(order, variables, data, parent, run)
    return variables, data


def _check_and_expand_subformula(expression: str, parent: Table, variables: Any, data: Any, run):
    grouping_depth = 0
    for i, char in enumerate(expression):
        if char == "(":
            if grouping_depth == 0:
                formula_delimiter = i + 1
            grouping_depth += 1
        elif char == ")":
            grouping_depth -= 1
            if grouping_depth == 0:
                expr = expression[formula_delimiter:i].strip()
                return _parse_formula(expr, parent, False, run, top=False)
    return variables, data


def _group(expression: str) -> tuple[str, str, str] | None:
    """The first group of a term: ``(text before, inside, text after)``, or ``None`` when the
    term has no complete group (larmorx then reads it as a name, as niworkflows)."""
    start = expression.find("(")
    if start < 0:
        return None
    depth = 0
    for i in range(start, len(expression)):
        if expression[i] == "(":
            depth += 1
        elif expression[i] == ")":
            depth -= 1
            if depth == 0:
                return (
                    expression[:start].strip(),
                    expression[start + 1 : i].strip(),
                    expression[i + 1 :].strip(),
                )
    return None


_DERIV = re.compile(r"dd[0-9]+|d[0-9]+[\-]?[0-9]*")
_EXPON = re.compile(r"\^\^[0-9]+|\^[0-9]+[\-]?[0-9]*")


def _pipe65(expression: str) -> str | None:
    """What niworkflows reads otherwise than the term says (PIPE-65), or ``None``."""
    g = _group(expression)
    if g is None:
        if re.search(r"^d[0-9]+", expression) or re.search(r"\^[0-9]+[\-]?[0-9]*$", expression):
            return f"{expression!r} is read as an operator without its group (TypeError)"
        return None
    before, _, after = g
    if (before and not _DERIV.fullmatch(before)) or (after and not _EXPON.fullmatch(after)):
        return f"the text around the group of {expression!r} is ignored"
    single = after.startswith("^") and not after.startswith("^^")
    if single and len(re.findall(r"\^([0-9]+[\-]?[0-9]*)", expression)) > 1:
        return f"the exponent of {expression!r} is read with the group's own (TypeError)"
    return None


def _parse_expression(expression: str, parent: Table, run: _Run):
    note = _pipe65(expression)
    if note is not None:
        run.pipe65.append(note)
    if not run.fix:
        variables: Any = None
        data: Any = None
        variables, data = _check_and_expand_subformula(expression, parent, variables, data, run)
        variables, data = _check_and_expand_exponential(expression, variables, data, parent, run)
        variables, data = _check_and_expand_derivative(expression, variables, data, parent, run)
        if variables is None:
            expr = expression.strip()
            variables = [expr]
            data = _one(parent, expr)
        return variables, data
    g = _group(expression)
    if g is None:
        expr = expression.strip()
        return [expr], _one(parent, expr)
    before, inside, after = g
    if (before and not _DERIV.fullmatch(before)) or (after and not _EXPON.fullmatch(after)):
        raise ValueError(
            f"the term {expression!r} of the model formula is not a name, nor a group with a "
            "derivative (dd<n>, d<n>, d<n>-<m>) before it and an exponent (^^<n>, ^<n>, ^<n>-<m>) "
            "after it (PIPE-65: niworkflows ignores the rest)"
        )
    variables, data = _parse_formula(inside, parent, False, run, top=False)
    if after.startswith("^^"):
        variables, data = _exponential_terms(
            range(1, int(after[2:]) + 1), variables, data, parent, run
        )
    elif after:
        variables, data = _exponential_terms(
            _order_as_range(after[1:]), variables, data, parent, run
        )
    if before.startswith("dd"):
        variables, data = _temporal_derivatives(
            range(int(before[2:]) + 1), variables, data, parent, run
        )
    elif before:
        variables, data = _temporal_derivatives(
            _order_as_range(before[1:]), variables, data, parent, run
        )
    return variables, data


def _unique_columns(table: Table, run: _Run) -> Table:
    """PIPE-66's correction: a repeated column name kept once (its first column)."""
    if len(set(table.names)) == len(table.names):
        return table
    keep = []
    seen: set[str] = set()
    for i, n in enumerate(table.names):
        if n in seen:
            run.pipe66.append(n)
            continue
        seen.add(n)
        keep.append(i)
    return Table([table.names[i] for i in keep], [table.columns[i] for i in keep], table.index)


def _unscramble(parent: Table, data: Table) -> Table:
    """niworkflows' ``_unscramble_regressor_columns``: the table's columns in the parent's
    order, each variable's expansions after it."""
    matches = ["_power[0-9]+", "_derivative[0-9]+"]
    var: OrderedDict[str, deque[str]] = OrderedDict((c, deque()) for c in parent.names)
    for c in data.names:
        col = c
        for m in matches:
            col = re.sub(m, "", col)
        if col == c:
            var[col].appendleft(c)
        else:
            var[col].append(c)
    unscrambled = reduce(operator.add, var.values())
    return data.select([*unscrambled])


def _parse_formula(model_formula: str, parent: Table, unscramble: bool, run: _Run, top: bool):
    # niworkflows expands the shorthands again in every group; larmorx once, at the top.
    niworkflows = _expand_niworkflows(model_formula, parent.names, run)
    ours = _expand_names(model_formula, parent.names, run) if top else model_formula
    if ours != niworkflows:
        run.pipe64.append(model_formula)
    model_formula = ours if run.fix else niworkflows
    variables: dict[str, Any] = {}
    data: dict[str, Any] = {}
    expr_delimiter = 0
    grouping_depth = 0
    for i, char in enumerate(model_formula):
        if char == "(":
            grouping_depth += 1
        elif char == ")":
            grouping_depth -= 1
        elif grouping_depth == 0 and char == "+":
            expression = model_formula[expr_delimiter:i].strip()
            variables[expression] = None
            data[expression] = None
            expr_delimiter = i + 1
    expression = model_formula[expr_delimiter:].strip()
    variables[expression] = None
    data[expression] = None
    for expression in list(variables):
        if run.fix and expression == _EMPTY:
            variables[expression], data[expression] = [], None
        elif expression[0] == "(" and expression[-1] == ")":
            variables[expression], data[expression] = _parse_formula(
                expression[1:-1], parent, False, run, top=False
            )
        else:
            variables[expression], data[expression] = _parse_expression(expression, parent, run)
    out_vars = run.ordered(reduce(operator.add, variables.values()))
    out = _concat([d for d in data.values() if d is not None], parent.index)
    if run.fix:
        out = _unique_columns(out, run)
    elif len(set(out.names)) != len(out.names):
        run.pipe66 += [n for i, n in enumerate(out.names) if n in out.names[:i]]
    if unscramble:
        out = _unscramble(parent, out)
    return out_vars, out


def _temporal_derivatives(order: Any, variables: Any, data: Any, parent: Table, run: _Run):
    """niworkflows' ``temporal_derivatives``: numpy's ``diff`` of the variables, ``n`` times for
    the ``n``-th derivative, its first ``n`` rows NaN; the order 0 the variables themselves."""
    if variables is not None and not variables and run.fix:
        return [], None
    variables_deriv: OrderedDict[int, list[str]] = OrderedDict()
    data_deriv: OrderedDict[int, Any] = OrderedDict()
    if 0 in order:
        if data is None:
            raise TypeError("'NoneType' object is not subscriptable")
        data_deriv[0] = data.select(variables)
        variables_deriv[0] = variables
        order = set(order) - {0}
    for o in order:
        variables_deriv[o] = [f"{v}_derivative{o}" for v in variables]
        sel = _values(data.select(variables))
        arr = np.tile(np.nan, sel.shape)
        with np.errstate(all="ignore"):
            arr[o:, :] = np.diff(sel, n=o, axis=0)
        data_deriv[o] = arr
    names = reduce(operator.add, variables_deriv.values())
    parts = [_values(d) if isinstance(d, Table) else d for d in data_deriv.values()]
    return names, _frame(names, np.concatenate(parts, axis=1), parent.index)


def _power(table: Table, o: int, run: _Run) -> Table:
    """``frame ** o`` column by column (pandas' blocks): float64 through numpy's ``power``
    (``larmorx_confounds::expansion``: glibc's or the correctly rounded ``pow``), the other
    columns with numpy's own ``**`` (integers and booleans in their types, wrapping around; text
    and Python integers as Python computes them)."""
    cols = []
    for c in table.columns:
        if c.kind == "float64":
            vals, differ = _core.numpy_power_f64(
                np.ascontiguousarray(c.values, dtype=np.float64), float(o), run.mode
            )
            if o not in (0, 2):
                run.pow_values += len(vals)
                run.pow_differ += differ
            cols.append(Column("float64", vals))
        else:
            with np.errstate(all="ignore"):
                vals = _array(c) ** o
            kind = _kind(vals.dtype)
            cols.append(Column(kind, vals.tolist() if kind == "object" else vals))
    return Table(list(table.names), cols, list(table.index))


def _exponential_terms(order: Any, variables: Any, data: Any, parent: Table, run: _Run):
    """niworkflows' ``exponential_terms``: the variables to the powers in ``order``, the power 1
    the variables themselves."""
    if variables is not None and not variables and run.fix:
        return [], None
    variables_exp: OrderedDict[int, list[str]] = OrderedDict()
    data_exp: OrderedDict[int, Table] = OrderedDict()
    if 1 in order:
        if data is None:
            raise TypeError("'NoneType' object is not subscriptable")
        data_exp[1] = data.select(variables)
        variables_exp[1] = variables
        order = set(order) - {1}
    for o in order:
        variables_exp[o] = [f"{v}_power{o}" for v in variables]
        data_exp[o] = _power(data.select(variables), o, run)
    names = reduce(operator.add, variables_exp.values())
    parts = [_values(d) for d in data_exp.values()]
    return names, _frame(names, np.concatenate(parts, axis=1), parent.index)


def _notices(run: _Run, order_varies: bool, stacklevel: int) -> None:
    mode = run.mode
    fix = run.fix
    notices = []
    if order_varies:
        notices.append(
            (
                "PIPE-43",
                "niworkflows parse_formula",
                "fix",
                "the expanded columns are in the formula's order (each variable, its "
                "derivatives, then their powers): niworkflows orders some of them as a Python set "
                "of strings, which changes between processes (never reproduced)",
            )
        )
    if run.pipe64:
        notices.append(
            (
                "PIPE-64",
                "niworkflows parse_formula",
                mode,
                f"niworkflows replaces the shorthands of {run.pipe64[0]!r} as text (inside names, "
                "inside every group, an empty term when one matches no column): "
                + ("whole names are replaced, once" if fix else "replaced as niworkflows does"),
            )
        )
    if run.pipe65:
        notices.append(
            (
                "PIPE-65",
                "niworkflows parse_formula",
                mode,
                run.pipe65[0]
                + (": larmorx reads the term as written" if fix else ", as niworkflows"),
            )
        )
    if run.pipe66:
        notices.append(
            (
                "PIPE-66",
                "niworkflows parse_formula",
                mode,
                f"the columns {sorted(set(run.pipe66))} are reached by more than one term: "
                + ("kept once" if fix else "repeated, as niworkflows"),
            )
        )
    if run.pow_differ:
        notices.append(
            (
                "GLIBC-07",
                "glibc 2.35",
                mode,
                f"{run.pow_differ} of {run.pow_values} powers differ between glibc's pow (numpy's "
                "on CPUs without AVX-512) and the correctly rounded one: "
                + ("correctly rounded" if fix else "glibc's"),
            )
        )
    _defects.warn(notices, stacklevel=stacklevel + 1)


def _parse_top(
    model_formula: str,
    parent: Table,
    unscramble: bool,
    mode: str,
    stacklevel: int,
    columns_only: bool = False,
) -> tuple[list[str], Table]:
    if not isinstance(model_formula, str):
        raise TypeError(f"the model formula must be a string, not {type(model_formula).__name__}")
    run = _Run(mode)
    variables, data = _parse_formula(model_formula, parent, unscramble, run, top=True)
    other = _Run(mode, order="reversed")
    try:
        v2, d2 = _parse_formula(model_formula, parent, unscramble, other, top=True)
        order_varies = d2.names != data.names or (v2 != variables and not columns_only)
    except Exception:
        order_varies = False
    _notices(run, order_varies, stacklevel + 1)
    return variables, data


def parse_formula(
    model_formula: str,
    parent_data: Any,
    unscramble: bool = False,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ModelTerms:
    """niworkflows' ``parse_formula(model_formula, parent_data, unscramble)``: the model's
    variables and their table from ``parent_data`` (a :class:`ConfoundTable` or a mapping of
    names to columns). The formula adds terms with ``+``; a term is a column name, a shorthand
    (``rps`` the six motion parameters, ``wm``, ``csf``, ``gsr`` the white-matter, CSF and global
    signals, ``fd`` the framewise displacement, ``acc``, ``tcc`` the a/tCompCor components,
    ``dv`` the standardised DVARS, ``dvall`` every DVARS, ``nss`` the non-steady-state
    outliers, ``spikes`` the motion outliers, ``others`` every column the formula does not name)
    or a group ``(...)`` with temporal derivatives before it (``dd<n>``: orders 0 to n, ``d<n>``,
    ``d<n>-<m>``) and powers after it (``^^<n>``: 1 to n, ``^<n>``, ``^<n>-<m>``). With
    ``unscramble`` the columns follow ``parent_data``'s order, each variable's expansions after
    it. The defects PIPE-43, PIPE-64, PIPE-65 and PIPE-66 (the module's documentation) follow
    ``upstream_defects``; pandas' and Python's errors as niworkflows (``KeyError`` for a name
    that is not a column)."""
    mode = _defects.check(upstream_defects)
    variables, data = _parse_top(model_formula, _as_table(parent_data), unscramble, mode, 2)
    return ModelTerms(variables, _public(data))


def temporal_derivatives(
    order: Sequence[int],
    variables: Sequence[str],
    data: Any,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ModelTerms:
    """niworkflows' ``temporal_derivatives(order, variables, data)``: the ``variables`` of
    ``data`` (a :class:`ConfoundTable` or a mapping) with their temporal derivatives of the
    orders in ``order`` (``<v>_derivative<o>``: numpy's ``diff`` ``o`` times, the first ``o``
    rows NaN; order 0 keeps the variable itself), as float64 columns."""
    mode = _defects.check(upstream_defects)
    table = _as_table(data)
    run = _Run(mode)
    names, out = _temporal_derivatives(order, list(variables), table, table, run)
    return ModelTerms(names, _public(out))


def exponential_terms(
    order: Sequence[int],
    variables: Sequence[str],
    data: Any,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ModelTerms:
    """niworkflows' ``exponential_terms(order, variables, data)``: the ``variables`` of ``data``
    (a :class:`ConfoundTable` or a mapping) with their powers of the orders in ``order``
    (``<v>_power<o>``: numpy's ``x ** o``, the module's documentation; order 1 keeps the
    variable itself); GLIBC-07's notice when a power differs between the two modes."""
    mode = _defects.check(upstream_defects)
    table = _as_table(data)
    run = _Run(mode)
    names, out = _exponential_terms(order, list(variables), table, table, run)
    _notices(run, False, 2)
    return ModelTerms(names, _public(out))


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


def _trait(spec: str, name: str, info: str, value: Any) -> ValueError:
    return ValueError(
        f"The '{name}' trait of a {spec} instance must be {info}, but a value of {value!r} "
        f"{type(value)!r} was specified."
    )


def _output_path(output_file: Any, default: str, out_dir: str) -> str:
    if output_file is None:
        return default
    return os.path.abspath(os.path.join(out_dir, os.fspath(output_file)))


def expand_model(
    confounds_file: os.PathLike[str] | str,
    model_formula: str = DEFAULT_FORMULA,
    *,
    output_file: os.PathLike[str] | str | None = None,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``ExpandModel`` interface: the table ``confounds_file`` expanded by
    ``model_formula`` (:func:`parse_formula` with ``unscramble``: the columns in the table's
    order, each variable's derivatives and powers after it), written as pandas writes it to
    ``output_file`` (relative to ``out_dir``) or ``<stem>_expansion.tsv`` in ``out_dir`` (the
    node directory); the path. The table is read as pandas reads it (PD-01)."""
    mode = _defects.check(upstream_defects)
    path = _existing("_ExpandModelInputSpec", "confounds_file", confounds_file)
    if not isinstance(model_formula, str):
        raise _trait("_ExpandModelInputSpec", "model_formula", "a string", model_formula)
    d = _out_dir(out_dir)
    out = _output_path(
        output_file,
        fname_presuffix(path, suffix="_expansion.tsv", newpath=d, use_ext=False),
        d,
    )
    table = tb.read_tsv(path, pandas_version=pandas_version, upstream_defects=mode, stacklevel=2)
    _, data = _parse_top(model_formula, table, True, mode, 2, columns_only=True)
    tb.write_tsv(out, data)
    return out


# --- spike regressors ------------------------------------------------------------------------


def _spikes(
    data: Table,
    criteria: Mapping[str, tuple[str, float]] | None,
    header_prefix: str,
    lags: Sequence[int] | None,
    minimum_contiguous: int | None,
    concatenate: bool,
    output: str,
    mode: str,
    stacklevel: int,
) -> Table:
    fix = mode == "fix"
    notices: list[tuple[str, str, str, str]] = []
    tool = "niworkflows spike_regressors"
    mask_by: dict[str, set[int]] = {}
    indices = range(data.n_rows)
    lags = lags or [0]
    criteria = criteria or {
        "framewise_displacement": (">", 0.5),
        "std_dvars": (">", 1.5),
    }
    for metric, (criterion, threshold) in criteria.items():
        if criterion not in ("<", ">"):
            if fix:
                raise ValueError(
                    f"the criterion {criterion!r} of {metric!r} is not '<' or '>' (PIPE-69: "
                    "niworkflows skips it)"
                )
            notices.append(
                ("PIPE-69", tool, mode, f"the criterion {criterion!r} of {metric!r} is skipped")
            )
            continue
        col = data.column(metric)
        if col.kind == "object":
            raise TypeError(
                f"the column {metric!r} holds text: larmorx compares numbers only (it does not "
                "follow pandas' comparisons of other columns)"
            )
        values = np.asarray(col.values)
        hit = values < threshold if criterion == "<" else values > threshold
        mask_by[metric] = {int(i) for i in np.where(hit)[0]}
    mask = reduce(operator.or_, mask_by.values())

    flagged = set(mask)
    for lag in lags:
        mask = {m + lag for m in mask} | mask
    corrected = flagged.union(*({m + lag for m in flagged} for lag in lags))
    if mask.intersection(indices) != corrected.intersection(indices):
        notices.append(
            (
                "PIPE-67",
                tool,
                mode,
                f"the lags {list(lags)} are added to the volumes the earlier lags flagged: "
                + ("each lag is added to the flagged volumes only" if fix else "as niworkflows"),
            )
        )
        if fix:
            mask = corrected
    mask = mask.intersection(indices)

    if minimum_contiguous is not None:
        results = {}
        for post_final in (data.n_rows + 1, data.n_rows):
            m2 = set(mask)
            epoch_length = np.diff(sorted(m2 | {-1, post_final})) - 1
            epoch_end = sorted(m2 | {post_final})
            for end, length in zip(epoch_end, epoch_length, strict=False):
                if length < minimum_contiguous:
                    m2 = m2 | set(range(end - length, end))
            results[post_final] = m2.intersection(indices)
        original, ours = results[data.n_rows + 1], results[data.n_rows]
        if original != ours:
            notices.append(
                (
                    "PIPE-68",
                    tool,
                    mode,
                    f"the last {data.n_rows - 1 - max(mask, default=-1)} unflagged volumes are "
                    "counted one volume longer, so they stay under minimum_contiguous="
                    f"{minimum_contiguous}: "
                    + ("they are flagged" if fix else "kept, as niworkflows"),
                )
            )
        mask = ours if fix else original

    _defects.warn(notices, stacklevel=stacklevel + 1)
    if output == "mask":
        spikes = np.zeros(data.n_rows)
        spikes[list(mask)] = 1
        table = Table([header_prefix], [Column("float64", spikes)], list(data.index))
    else:
        spikes = np.zeros((max(indices) + 1, len(mask)))
        for i, m in enumerate(sorted(mask)):
            spikes[m, i] = 1
        header = [f"{header_prefix:s}{vol:02d}" for vol in range(len(mask))]
        table = _frame(header, spikes, list(data.index))
    if concatenate:
        return _concat([data, table], data.index)
    return table


def spike_regressors(
    data: Any,
    criteria: Mapping[str, tuple[str, float]] | None = None,
    header_prefix: str = "motion_outlier",
    lags: Sequence[int] | None = None,
    minimum_contiguous: int | None = None,
    concatenate: bool = True,
    output: str = "spikes",
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ConfoundTable:
    """niworkflows' ``spike_regressors``: the volumes of ``data`` (a :class:`ConfoundTable` or a
    mapping) whose column exceeds a criterion (``criteria``: column → (``'>'`` or ``'<'``,
    threshold); by default framewise displacement > 0.5 or standardised DVARS > 1.5, Power et al.
    2014), with the volumes ``lags`` after each (the flagged volume always) and, with
    ``minimum_contiguous``, every run of fewer unflagged volumes; as one regressor per volume
    (``<header_prefix><kk>``, 1 at that volume) or, with ``output="mask"``, one column
    ``header_prefix`` (1 at every flagged volume); appended to ``data`` with ``concatenate``.
    PIPE-67, PIPE-68, PIPE-69 follow ``upstream_defects``; a table without rows raises Python's
    ``ValueError`` for the regressors, as niworkflows."""
    mode = _defects.check(upstream_defects)
    table = _spikes(
        _as_table(data),
        criteria,
        header_prefix,
        lags,
        minimum_contiguous,
        concatenate,
        output,
        mode,
        2,
    )
    return _public(table)


def _is_number(v: Any) -> bool:
    return isinstance(v, (int, float, np.integer, np.floating)) and not isinstance(v, bool)


def spike_regressors_node(
    confounds_file: os.PathLike[str] | str,
    *,
    fd_thresh: float = 0.5,
    dvars_thresh: float = 1.5,
    header_prefix: str = "motion_outlier",
    lags: Sequence[int] = (0,),
    minimum_contiguous: int | None = None,
    concatenate: bool = True,
    output_format: str = "spikes",
    output_file: os.PathLike[str] | str | None = None,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``SpikeRegressors`` interface: :func:`spike_regressors` of the table
    ``confounds_file`` with the criteria ``framewise_displacement > fd_thresh`` and
    ``std_dvars > dvars_thresh``, written as pandas writes it to ``output_file`` (relative to
    ``out_dir``) or ``<stem>_desc-motion_outliers.tsv`` in ``out_dir``; the path. fMRIPrep's
    ``desc-confounds_timeseries.tsv`` is this file."""
    mode = _defects.check(upstream_defects)
    spec = "_SpikeRegressorsInputSpec"
    path = _existing(spec, "confounds_file", confounds_file)
    for name, v in (("fd_thresh", fd_thresh), ("dvars_thresh", dvars_thresh)):
        if not _is_number(v):
            raise _trait(spec, name, "a float", v)
    if not isinstance(header_prefix, str):
        raise _trait(spec, "header_prefix", "a string", header_prefix)
    lags = list(lags)
    if not all(isinstance(x, (int, np.integer)) and not isinstance(x, bool) for x in lags):
        raise _trait(spec, "lags", "a list of items which are an integer", lags)
    if minimum_contiguous is not None and not (
        isinstance(minimum_contiguous, (int, np.integer))
        and not isinstance(minimum_contiguous, bool)
    ):
        raise _trait(spec, "minimum_contiguous", "None or an integer", minimum_contiguous)
    if not isinstance(concatenate, bool):
        raise _trait(spec, "concatenate", "a boolean", concatenate)
    if output_format not in ("spikes", "mask"):
        raise _trait(spec, "output_format", "'spikes' or 'mask'", output_format)
    d = _out_dir(out_dir)
    out = _output_path(
        output_file,
        fname_presuffix(path, suffix="_desc-motion_outliers.tsv", newpath=d, use_ext=False),
        d,
    )
    criteria = {
        "framewise_displacement": (">", float(fd_thresh)),
        "std_dvars": (">", float(dvars_thresh)),
    }
    table = tb.read_tsv(path, pandas_version=pandas_version, upstream_defects=mode, stacklevel=2)
    table = _spikes(
        table,
        criteria,
        header_prefix,
        [int(x) for x in lags],
        None if minimum_contiguous is None else int(minimum_contiguous),
        concatenate,
        output_format,
        mode,
        2,
    )
    tb.write_tsv(out, table)
    return out


# --- motion parameters -----------------------------------------------------------------------

#: ``NormalizeMotionParams``' formats (its ``traits.Enum``).
FORMATS = ("FSL", "AFNI", "FSFAST", "NIPY")


def normalize_motion_params(
    in_file: os.PathLike[str] | str,
    *,
    format: str = "FSL",
    out_dir: os.PathLike[str] | str = ".",
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``NormalizeMotionParams`` interface: the motion parameters of ``in_file``
    (numpy's ``loadtxt``: one row per volume) in SPM's order and units, row by row
    (:func:`larmorx.confounds.normalize_mc_params` with ``source=format``: ``"FSL"``,
    ``"AFNI"``, ``"FSFAST"`` or ``"NIPY"``), written with numpy's ``savetxt`` (``%.18e``) as
    ``motion_params.txt`` in ``out_dir``; the path. A table of one row stays a row (PIPE-70;
    ``"reproduce"``: numpy's ``AxisError``, as niworkflows); a table of one column or none
    raises numpy's ``AxisError``, too few columns numpy's ``IndexError``."""
    from larmorx.confounds import framewise

    mode = _defects.check(upstream_defects)
    spec = "_NormalizeMotionParamsInputSpec"
    path = _existing(spec, "in_file", in_file)
    if not isinstance(format, str) or format not in FORMATS:
        raise _trait(spec, "format", "'FSL' or 'AFNI' or 'FSFAST' or 'NIPY'", format)
    d = _out_dir(out_dir)
    mpars = np.loadtxt(path)  # mpars is N_t x 6
    if mpars.ndim < 2 and mpars.size:
        import warnings

        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            shaped = np.loadtxt(path, ndmin=2)
        if shaped.shape[0] == 1 and shaped.shape[1] > 1:
            _defects.warn(
                [
                    (
                        "PIPE-70",
                        "niworkflows NormalizeMotionParams",
                        mode,
                        f"numpy's loadtxt squeezes the table of one row ({mpars.size} values) to "
                        "a vector, which numpy's apply_along_axis refuses (AxisError): "
                        + ("the row is converted" if mode == "fix" else "as niworkflows"),
                    )
                ],
                stacklevel=2,
            )
            if mode == "fix":
                mpars = shaped
    if format == "NIPY" and mpars.ndim == 2 and mpars.shape[0] and mpars.dtype == np.float64:
        mpars, other = framewise._rust_rows(mpars, str(numpy_version), mode)
        if other is not None:
            framewise._glibc07(mode, "change the motion parameters")
    else:
        mpars = np.apply_along_axis(
            func1d=framewise.normalize_mc_params,
            axis=1,
            arr=mpars,
            source=format,
            numpy_version=numpy_version,
            upstream_defects=mode,
        )
    out = os.path.join(d, "motion_params.txt")
    np.savetxt(out, mpars)
    return out


# --- niworkflows' copies of fMRIPrep's motion interfaces ---------------------------------------

#: The newest niworkflows release, whose ``FSLMotionParams`` (fMRIPrep 25.2.7's) is the default.
NIWORKFLOWS_NEWEST = "1.15.4"


def _fmriprep_form(niworkflows_version: str) -> str:
    """The fMRIPrep release whose ``FSLMotionParams`` niworkflows ``niworkflows_version``
    carries: 25.2.6's up to 1.15.3, 25.2.7's from 1.15.4."""
    try:
        key = tuple(int(p) for p in str(niworkflows_version).split(".")[:3])
    except ValueError:
        raise ValueError(f"not a niworkflows version: {niworkflows_version!r}") from None
    return "25.2.6" if key <= (1, 15, 3) else "25.2.7"


def fsl_motion_params(
    xfm_file: os.PathLike[str] | str,
    boldref_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    niworkflows_version: str = NIWORKFLOWS_NEWEST,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
):
    """niworkflows' ``FSLMotionParams`` (its copy of fMRIPrep's: 25.2.7's form in niworkflows
    1.15.4, 25.2.6's in 1.15.2 and 1.15.3): :func:`larmorx.confounds.fsl_motion_params` with
    that form; a :class:`larmorx.confounds.MotionParamsResult`."""
    from larmorx.confounds.motion import fsl_motion_params as fmp

    return fmp(
        xfm_file,
        boldref_file,
        out_dir=out_dir,
        fmriprep_version=_fmriprep_form(niworkflows_version),
        numpy_version=numpy_version,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )


def fsl_rms_deviation(
    xfm_file: os.PathLike[str] | str,
    boldref_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    upstream_defects: UpstreamDefectsName = "fix",
):
    """niworkflows' ``FSLRMSDeviation`` (fMRIPrep's, the same in 1.15.2 to 1.15.4):
    :func:`larmorx.confounds.fsl_rms_deviation`."""
    from larmorx.confounds.motion import fsl_rms_deviation as frd

    return frd(xfm_file, boldref_file, out_dir=out_dir, upstream_defects=upstream_defects)


def framewise_displacement(
    in_file: os.PathLike[str] | str,
    *,
    radius: float = 50.0,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``FramewiseDisplacement`` (fMRIPrep's, the same in 1.15.2 to 1.15.4):
    :func:`larmorx.confounds.fmriprep_framewise_displacement`."""
    from larmorx.confounds.fmriprep import fmriprep_framewise_displacement as ffd

    return ffd(
        in_file,
        radius=radius,
        out_dir=out_dir,
        pandas_version=pandas_version,
        upstream_defects=upstream_defects,
    )
