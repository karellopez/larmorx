# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's own confound interfaces on tables (``fmriprep/interfaces/confounds.py``).

Ported from fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; the same in 25.2.6 on these
interfaces), with pandas' reading and writing of the tables (:mod:`larmorx.confounds._tables`,
pandas 3.0.6 by default, ``pandas_version="2.2.3"`` for fMRIPrep 25.2.6's image):

- :func:`fmriprep_framewise_displacement`: ``FramewiseDisplacement`` (fMRIPrep's, not nipype's:
  :func:`larmorx.confounds.framewise_displacement` is nipype's): the motion table's
  ``trans_*``/``rot_*`` columns differenced (pandas' ``diff``), the rotations times ``radius``,
  the absolute values summed per row from the first column on (pandas' ``sum(axis=1,
  skipna=False)``: the first row ``n/a``), written to ``<in_file's stem>_fd.tsv<ext>`` (a
  gzipped table when the input's extension is ``.gz``, as fMRIPrep's ``*_motion.tsv.nii.gz``).
- :func:`filter_dropped`: ``FilterDropped``: the CompCor metadata's rows whose ``retained`` is
  true.
- :func:`rename_acompcor`: ``RenameACompCor``: the aCompCor components of the ``CSF``, ``WM``
  and ``combined`` masks renamed ``c_comp_cor_XX``, ``w_comp_cor_XX``, ``a_comp_cor_XX`` in the
  components and the metadata.
- :func:`gather_confounds`: ``GatherConfounds``: the tables concatenated side by side (the
  shorter ones aligned at the end: their missing rows come first), the column names in snake
  case, ``confounds.tsv``.

Every table is read as pandas reads it: the correctly rounded values by default, pandas'
``precise_xstrtod`` values with ``upstream_defects="reproduce"`` (PD-01, the notice when they
differ).
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx._nipype import fname_presuffix
from larmorx.confounds import _tables as tb
from larmorx.confounds._tables import PANDAS_NEWEST, Column, Table

__all__ = [
    "GatherConfoundsResult",
    "RenameACompCorResult",
    "filter_dropped",
    "fmriprep_framewise_displacement",
    "gather_confounds",
    "rename_acompcor",
]

_MOTION = ["trans_x", "trans_y", "trans_z", "rot_x", "rot_y", "rot_z"]


def _existing(spec: str, name: str, value: Any) -> str:
    """nipype's ``File(exists=True)``: the path, or its ``TraitError`` (as ``ValueError``)."""
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


def _numbers(col: Column, name: str) -> tuple[np.ndarray, bool]:
    """A numeric column for pandas' arithmetic: ``(values, integer)``; ``TypeError`` for the
    text and boolean columns larmorx does not compute with."""
    if col.kind in ("float64", "int64", "uint64"):
        return np.asarray(col.values), col.kind != "float64"
    raise TypeError(
        f"column {name!r} holds {'booleans' if col.kind == 'bool' else 'text'}: fMRIPrep's "
        "framewise displacement is computed on numbers (larmorx does not follow pandas' "
        "arithmetic on other columns)"
    )


def _diff(values: np.ndarray, integer: bool) -> np.ndarray:
    """pandas' ``diff()`` of one column: float64, the first row NaN; integers subtracted in
    their own type (wrapping, as pandas' ``diff_2d``) before the conversion."""
    out = np.full(values.shape, np.nan)
    if len(values) > 1:
        with np.errstate(over="ignore"):
            out[1:] = (
                (values[1:] - values[:-1]).astype(np.float64)
                if integer
                else (values[1:] - values[:-1])
            )
    return out


def fmriprep_framewise_displacement(
    in_file: os.PathLike[str] | str,
    *,
    radius: float = 50.0,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """fMRIPrep's ``FramewiseDisplacement`` interface: the framewise displacement of the motion
    table ``in_file`` (``trans_x``, ``trans_y``, ``trans_z`` in mm and ``rot_x``, ``rot_y``,
    ``rot_z`` in radians, any other columns ignored; pandas' ``KeyError`` when one is missing),
    the rotations as arcs on a sphere of ``radius`` mm; the path of the table written
    (``FramewiseDisplacement``, ``n/a`` in the first row) in ``out_dir`` (fMRIPrep's node
    directory)."""
    _defects.check(upstream_defects)
    path = _existing("_FramewiseDisplacementInputSpec", "in_file", in_file)
    if isinstance(radius, bool) or not isinstance(radius, (int, float, np.integer, np.floating)):
        raise ValueError(
            "The 'radius' trait of a _FramewiseDisplacementInputSpec instance must be a float, "
            f"but a value of {radius!r} {type(radius)!r} was specified."
        )
    out = fname_presuffix(path, suffix="_fd.tsv", newpath=_out_dir(out_dir))
    motion = tb.read_tsv(path, pandas_version=pandas_version, upstream_defects=upstream_defects)
    sel = motion.select(_MOTION)
    diffs = []
    for name, col in zip(sel.names, sel.columns, strict=True):
        vals, integer = _numbers(col, name)
        d = _diff(vals, integer)
        if name.startswith("rot_"):
            d = d * float(radius)
        diffs.append(np.abs(d))
    fd = diffs[0].copy()
    for d in diffs[1:]:
        fd = fd + d
    tb.write_tsv(out, Table(["FramewiseDisplacement"], [Column("float64", fd)], motion.index))
    return out


def filter_dropped(
    in_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """fMRIPrep's ``FilterDropped`` interface: the rows of the CompCor metadata ``in_file``
    whose boolean ``retained`` is true, written as ``<stem>_filtered<ext>`` in ``out_dir``;
    the path. A ``retained`` column with missing values raises pandas' ``ValueError``, one
    of text pandas' ``KeyError``."""
    _defects.check(upstream_defects)
    path = _existing("_FilterDroppedInputSpec", "in_file", in_file)
    out = fname_presuffix(path, suffix="_filtered", newpath=_out_dir(out_dir))
    meta = tb.read_tsv(path, pandas_version=pandas_version, upstream_defects=upstream_defects)
    keep = _retained(meta, pandas_version)
    tb.write_tsv(out, Table([], [], []) if keep is None else meta.take_rows(keep))
    return out


def _retained(meta: Table, pandas_version: str) -> list[int] | None:
    """``metadata[metadata.retained]``'s rows; ``None`` for pandas' empty selection of columns
    (an empty ``retained`` column is no boolean mask: the table keeps no column and no row)."""
    if "retained" not in meta.names:
        raise AttributeError("'DataFrame' object has no attribute 'retained'")
    col = meta.column("retained")
    if col.kind == "bool":
        return np.flatnonzero(np.asarray(col.values)).tolist()
    if len(col) == 0:
        return None
    if col.kind == "object" and all(isinstance(v, bool) or v != v for v in col.values):
        raise ValueError("Cannot mask with non-boolean array containing NA / NaN values")
    dtype = "str" if tb.pandas_series(pandas_version) == "3.0" else "object"
    raise KeyError(f"None of [Index({list(col.values)!r}, dtype='{dtype}')] are in the [columns]")


@dataclass(frozen=True)
class RenameACompCorResult:
    """``RenameACompCor``'s outputs: the renamed components and metadata (the inputs
    themselves when one of them has no columns)."""

    components_file: str
    metadata_file: str


def rename_acompcor(
    components_file: os.PathLike[str] | str,
    metadata_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> RenameACompCorResult:
    """fMRIPrep's ``RenameACompCor`` interface: the retained aCompCor components of the masks
    ``CSF``, ``WM`` and ``combined`` (the metadata's ``mask`` column) renamed
    ``c_comp_cor_XX``, ``w_comp_cor_XX``, ``a_comp_cor_XX`` (numbered from 00 in the metadata's
    order) in the components' header and the metadata's ``component`` column, written as
    ``<stem>_renamed<ext>`` in ``out_dir``. A table without columns (pandas'
    ``EmptyDataError``, a short series) leaves both files as they are."""
    _defects.check(upstream_defects)
    comp_path = _existing("_RenameACompCorInputSpec", "components_file", components_file)
    meta_path = _existing("_RenameACompCorInputSpec", "metadata_file", metadata_file)
    kw = {"pandas_version": pandas_version, "upstream_defects": upstream_defects}
    try:
        components = tb.read_tsv(comp_path, **kw)
        metadata = tb.read_tsv(meta_path, **kw)
    except tb.EmptyDataError:
        return RenameACompCorResult(comp_path, meta_path)
    d = _out_dir(out_dir)
    comp_out = fname_presuffix(comp_path, suffix="_renamed", newpath=d)
    meta_out = fname_presuffix(meta_path, suffix="_renamed", newpath=d)
    keep = _retained(metadata, pandas_version)
    if keep is None:
        raise KeyError("mask")
    retained = metadata.take_rows(keep)
    masks = retained.column("mask").values
    names = retained.column("component").values
    rows = retained.index
    groups = []
    for mask, prefix in (("CSF", "c"), ("WM", "w"), ("combined", "a")):
        sel = [i for i, m in enumerate(masks) if m == mask]
        orig = [names[i] for i in sel]
        new = [f"{prefix}_comp_cor_{k:02d}" for k in range(len(orig))]
        groups.append(([rows[i] for i in sel], orig, new))
    comp_names = list(components.names)
    for _, orig, new in groups:
        mapping = dict(zip(orig, new, strict=False))
        comp_names = [mapping.get(n, n) for n in comp_names]
    tb.write_tsv(comp_out, Table(comp_names, components.columns, components.index))
    comp_col = metadata.column("component")
    values = list(comp_col.values) if comp_col.kind == "object" else comp_col.values.tolist()
    pos = {label: i for i, label in enumerate(metadata.index)}
    for labels, _, new in groups:
        for label, name in zip(labels, new, strict=True):
            values[pos[label]] = name
    cols = list(metadata.columns)
    cols[metadata.names.index("component")] = Column("object", values)
    tb.write_tsv(meta_out, Table(list(metadata.names), cols, metadata.index))
    return RenameACompCorResult(comp_out, meta_out)


@dataclass(frozen=True)
class GatherConfoundsResult:
    """``GatherConfounds``' outputs: the combined table and the names of the confound groups
    given (``confounds_list``)."""

    confounds_file: str
    confounds_list: list[str]


def _less_breakable(a_string: str) -> str:
    return "".join(a_string.split()).strip("#")


def _camel_to_snake(name: str) -> str:
    s1 = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s1).lower()


def _union(left: list[int], right: list[int]) -> list[int]:
    """pandas' ``Index.union(other, sort=False)`` of integer row labels: the left labels in
    their order, then the right's not among them."""
    if not right or left == right:
        return list(left)
    if not left:
        return list(right)
    seen = set(left)
    return list(left) + [r for r in right if r not in seen]


def _reindex(col: Column, index: list[int], labels: list[int]) -> Column:
    """A column aligned on new row labels (pandas' ``reindex``): missing rows NaN, an integer
    column then float, a boolean one booleans and NaN."""
    pos = {label: i for i, label in enumerate(index)}
    take = [pos.get(label, -1) for label in labels]
    if all(t >= 0 for t in take):
        return col.take(take)
    if col.kind in ("float64", "int64", "uint64"):
        vals = np.asarray(col.values).astype(np.float64)
        return Column("float64", np.array([vals[t] if t >= 0 else np.nan for t in take]))
    vals = col.values.tolist() if col.kind == "bool" else col.values
    return Column("object", [vals[t] if t >= 0 else np.nan for t in take])


def _concat(left: Table, right: Table) -> Table:
    labels = _union(left.index, right.index)
    cols = [_reindex(c, left.index, labels) for c in left.columns]
    cols += [_reindex(c, right.index, labels) for c in right.columns]
    return Table(left.names + right.names, cols, labels)


def gather_confounds(
    signals: os.PathLike[str] | str | None = None,
    dvars: os.PathLike[str] | str | None = None,
    std_dvars: os.PathLike[str] | str | None = None,
    fd: os.PathLike[str] | str | None = None,
    rmsd: os.PathLike[str] | str | None = None,
    tcompcor: os.PathLike[str] | str | None = None,
    acompcor: os.PathLike[str] | str | None = None,
    crowncompcor: os.PathLike[str] | str | None = None,
    cos_basis: os.PathLike[str] | str | None = None,
    motion: os.PathLike[str] | str | None = None,
    *,
    out_dir: os.PathLike[str] | str = ".",
    pandas_version: str = PANDAS_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> GatherConfoundsResult:
    """fMRIPrep's ``GatherConfounds`` interface: the tables given (``None``: not given),
    read in the order ``signals``, ``std_dvars``, ``dvars``, ``fd``, ``rmsd``, ``tcompcor``,
    ``acompcor``, ``crowncompcor``, ``cos_basis``, ``motion`` (empty files and tables without
    columns skipped), their columns renamed to snake case without white space or ``#``
    (``GlobalSignal`` → ``global_signal``), placed side by side, a shorter table's rows aligned
    with the last rows (fMRIPrep's ``_adjust_indices``: their missing rows first, in volume
    order; PIPE-63: pandas' ``concat`` writes the first volume's row last when an earlier table is
    the shorter one, ``"reproduce"``); ``confounds.tsv`` in ``out_dir``, and the names of the groups
    given."""
    _defects.check(upstream_defects)
    groups = (
        (signals, "signals", "Global signals"),
        (std_dvars, "std_dvars", "Standardized DVARS"),
        (dvars, "dvars", "DVARS"),
        (fd, "fd", "Framewise displacement"),
        (rmsd, "rmsd", "Framewise displacement (RMS)"),
        (tcompcor, "tcompcor", "tCompCor"),
        (acompcor, "acompcor", "aCompCor"),
        (crowncompcor, "crowncompcor", "crownCompCor"),
        (cos_basis, "cos_basis", "Cosine basis"),
        (motion, "motion", "Motion parameters"),
    )
    all_files = []
    confounds_list = []
    for value, trait, name in groups:
        if value is None:
            continue
        path = _existing("GatherConfoundsInputSpec", trait, value)
        confounds_list.append(name)
        if os.path.exists(path) and os.stat(path).st_size > 0:
            all_files.append(path)
    data = Table([], [], [])
    unsorted = False
    for path in all_files:
        try:
            new = tb.read_tsv(
                path, pandas_version=pandas_version, upstream_defects=upstream_defects
            )
        except tb.EmptyDataError:
            continue
        names = list(new.names)
        for column_name in list(new.names):
            target = _camel_to_snake(_less_breakable(column_name))
            names = [target if n == column_name else n for n in names]
        new = Table(names, new.columns, new.index)
        diff = data.n_rows - new.n_rows
        if diff > 0:
            new = Table(new.names, new.columns, list(range(diff, new.n_rows + diff)))
        elif diff < 0:
            data = Table(data.names, data.columns, list(range(-diff, data.n_rows - diff)))
        data = _concat(data, new)
        order = sorted(range(data.n_rows), key=data.index.__getitem__)
        if order != list(range(data.n_rows)):
            unsorted = True
            if upstream_defects == "fix":
                # Kept in volume order after every concat: a later shorter frame's relabelling
                # (``_adjust_indices``) then shifts the volumes, not pandas' order.
                data = data.take_rows(order)
    if unsorted:
        _defects.warn(
            [
                (
                    "PIPE-63",
                    "fMRIPrep GatherConfounds",
                    upstream_defects,
                    "pandas' concat puts the rows of a table longer than the earlier ones after "
                    "theirs, so the first volumes' rows would be written last: "
                    + (
                        "the rows are written in volume order"
                        if upstream_defects == "fix"
                        else "written as pandas orders them"
                    ),
                )
            ],
            stacklevel=2,
        )
    out = os.path.join(_out_dir(out_dir), "confounds.tsv")
    tb.write_tsv(out, data)
    return GatherConfoundsResult(out, confounds_list)
