# SPDX-License-Identifier: Apache-2.0
"""What pybids, sdcflows and fMRIPrep find in a BIDS dataset, as JSON-ready data (relay 11).

Runs with the original libraries only (pybids, sdcflows, nibabel; no larmorx), so it also runs
inside fMRIPrep's image (pybids 0.20.0, sdcflows 2.15.1). ``check_estimators.py`` calls the same
functions on larmorx's ports (they share the duck-typed serializer) and compares the results.

- ``serialize``: values with their types (pybids' ``PaddedInt``, dictionaries in order, files
  as their class and path, estimators with their sources and metadata), the dataset root and
  the scratch folder written as ``<root>``/``<tmp>``.
- ``dump_index``: every file's entities and metadata (``BIDSFile.entities``, ``get_entities``
  in each mode, ``get_metadata``, the layout's ``get_metadata``), the layout's entities, the
  ``get_<entity>s`` values and a battery of queries (``QUERIES``, with ``Query`` markers).
- ``Recorder``: a layout proxy that records every call sdcflows' search makes, with its result,
  for the other side to replay.
- ``dump_wrangler``: ``find_estimators`` over a list of argument sets (each from an empty
  registry), with the estimators, the registries, the debug log and any error.
- ``fmriprep_functions``: fMRIPrep 25.2.7's ``map_fieldmap_estimation`` and ``get_estimator``
  taken out of ``workflows/base.py`` (``ast``), with a stub ``config`` whose logger records.

As a script (inside fMRIPrep's image): ``python -I estimators_dump.py <cases.json> <out.json>``
(the cases are written by ``check_estimators.py --sections image``).
"""

from __future__ import annotations

import ast
import enum
import io
import json
import logging
import re
import sys
import traceback
from pathlib import Path, PurePath
from types import SimpleNamespace
from typing import Any

QUERIES: list[dict[str, Any]] = [
    {},
    {"suffix": "bold"},
    {"suffix": "bold", "extension": "nii.gz"},
    {"suffix": ["bold", "sbref"], "extension": [".nii", ".nii.gz"]},
    {"run": 1},
    {"run": "01"},
    {"run": [1, 2]},
    {"run": ["01", "x"]},
    {"session": None},
    {"session": "@NONE"},
    {"session": "@REQUIRED"},
    {"session": "@OPTIONAL"},
    {"session": ["@NONE", "@REQUIRED"]},
    {"echo": [None, "1"]},
    {"echo": []},
    {"part": ["mag", None], "extension": [".nii", ".nii.gz"]},
    {"part": "mag", "regex_search": True},
    {"subject": "0", "regex_search": True},
    {"suffix": "BOLD", "regex_search": True},
    {"suffix": "epi", "IntendedFor": "@REQUIRED"},
    {"B0FieldIdentifier": "@REQUIRED"},
    {"B0FieldIdentifier": '"pe"', "regex_search": True},
    {"PhaseEncodingDirection": "j-"},
    {"PhaseEncodingDirection": ["j", "j-"]},
    {"TotalReadoutTime": 0.05},
    {"EchoTime": "@REQUIRED", "suffix": "bold"},
    {"datatype": "fmap", "scope": "raw"},
    {"datatype": "fmap", "scope": "derivatives"},
    {"subjekt": "01"},
    {"subjekt": "01", "invalid_filters": "drop"},
    {"subjekt": "01", "invalid_filters": "allow"},
    {"return_type": "filename", "suffix": "T1w"},
    {"return_type": "id", "target": "subject"},
    {"return_type": "id", "target": "run"},
    {"return_type": "id", "target": "B0FieldIdentifier"},
    {"return_type": "id", "target": "IntendedFor"},
    {"return_type": "id", "target": "nonexistent"},
    {"return_type": "id"},
    {"return_type": "dir", "target": "subject"},
    {"return_type": "dir", "target": "session"},
    {"return_type": "dir", "target": "run"},
    {"return_type": "bogus"},
]

TARGETS = (
    "subjects", "sessions", "tasks", "acquisitions", "ceagents", "directions", "runs", "echos",
    "echoes", "parts", "suffixes", "datatypes", "extensions", "fmaps", "B0FieldIdentifiers",
    "B0FieldSources", "PhaseEncodingDirections", "TotalReadoutTimes", "EchoTimes", "fmapids",
    "descs", "unknowns",
)  # fmt: skip


def _markers(value: Any, query_cls: Any) -> Any:
    if isinstance(value, str) and value.startswith("@"):
        return getattr(query_cls, value[1:])
    if isinstance(value, list):
        return [_markers(v, query_cls) for v in value]
    return value


def with_markers(kwargs: dict[str, Any], query_cls: Any) -> dict[str, Any]:
    return {k: _markers(v, query_cls) for k, v in kwargs.items()}


class Ser:
    """The serializer: ``roots`` are replaced by their labels in every string."""

    def __init__(self, roots: dict[str, str]):
        self.roots = sorted(roots.items(), key=lambda kv: -len(kv[0]))

    def s(self, text: str) -> str:
        for root, label in self.roots:
            text = text.replace(root, label)
        # sdcflows names the identifiers in a set's order (per process): compared sorted
        return re.sub(
            r"(Multiple common ``B0FieldIdentifier``s found: <)([^>]*)(>)",
            lambda m: m.group(1) + ", ".join(sorted(m.group(2).split(", "))) + m.group(3),
            text,
        )

    def __call__(self, v: Any) -> Any:
        name = type(v).__name__
        if v is None:
            return None
        if name == "PaddedInt":
            return ["PaddedInt", str(v)]
        if isinstance(v, bool):
            return ["bool", v]
        if isinstance(v, int):
            return ["int", int(v)]
        if isinstance(v, float):
            return ["float", repr(v)]
        if isinstance(v, str):
            return ["str", self.s(v)]
        if isinstance(v, enum.Enum):
            return ["enum", type(v).__name__, v.name]
        if isinstance(v, PurePath):
            return ["path", self.s(str(v))]
        if hasattr(v, "relpath") and hasattr(v, "path") and hasattr(v, "get_entities"):
            return ["file", name, self.s(v.path)]
        if name == "FieldmapFile":
            return [
                "FieldmapFile", self.s(str(v.path)), v.suffix, self(v.entities), self(v.metadata),
                self(v.bids_root), self.s(repr(v)),
            ]  # fmt: skip
        if name == "FieldmapEstimation":
            return [
                "FieldmapEstimation", v.method.name, v.bids_id, v.sanitized_id,
                [self(f) for f in v.sources], self.s(repr(v)),
            ]  # fmt: skip
        if isinstance(v, dict) or name == "_AssociationDict":
            # pybids' BIDSFile.entities is SQLAlchemy's association proxy (a dict-like view)
            kind = "dict" if name in ("dict", "_AssociationDict") else name
            return ["dict", kind, [[self(k), self(x)] for k, x in v.items()]]
        if isinstance(v, (list, tuple)):
            return [type(v).__name__, [self(x) for x in v]]
        if isinstance(v, (set, frozenset)):
            return ["set", sorted(json.dumps(self(x)) for x in v)]
        return ["repr", name, self.s(repr(v))]

    def error(self, err: BaseException) -> list[Any]:
        # a call's qualified name in Python's TypeError differs between the layouts
        text = re.sub(r"[\w.<>]+\(\) got multiple values", "get() got multiple values", str(err))
        return ["error", type(err).__name__, self.s(text)]


def _call(ser: Ser, fn: Any, *args: Any, **kwargs: Any) -> Any:
    try:
        return ser(fn(*args, **kwargs))
    except Exception as err:
        return ser.error(err)


def dump_index(layout: Any, ser: Ser, query_cls: Any, parse: Any) -> dict[str, Any]:
    """Every file's entities and metadata, the layout's entities and the queries."""
    out: dict[str, Any] = {"files": {}, "queries": [], "targets": {}}
    files = layout.get(return_type="filename")
    out["filenames"] = [ser.s(f) for f in files]
    for bf in layout.get():
        key = ser.s(bf.path)
        out["files"][key] = {
            "class": type(bf).__name__,
            "relpath": bf.relpath,
            "entities": ser(bf.entities),
            "get_entities": ser(bf.get_entities()),
            "get_entities_md": ser(bf.get_entities(metadata=True)),
            "get_entities_all": ser(bf.get_entities(metadata="all")),
            "get_entities_none": ser(bf.get_entities(metadata=None)),
            "get_metadata": ser(bf.get_metadata()),
            "layout_md": ser(layout.get_metadata(bf.path)),
            "layout_md_ents": ser(layout.get_metadata(bf.path, include_entities=True)),
            "parse": _call(ser, layout.parse_file_entities, bf.path),
            "parse_module": _call(ser, parse, bf.path),
            "parse_bids": _call(ser, parse, bf.path, config=["bids"]),
            "get_file_rel": _call(ser, layout.get_file, bf.relpath),
        }
    out["entities"] = list(layout.get_entities())
    out["entities_fn"] = sorted(layout.get_entities(metadata=False))
    out["entities_md"] = sorted(layout.get_entities(metadata=True))
    out["get_file_missing"] = _call(ser, layout.get_file, "nothing/here.nii.gz")
    for t in TARGETS:
        try:
            fn = getattr(layout, f"get_{t}")
        except Exception as err:
            out["targets"][t] = ser.error(err)
            continue
        out["targets"][t] = _call(ser, fn)
    for q in QUERIES:
        out["queries"].append(_call(ser, layout.get, **with_markers(q, query_cls)))
    return out


class Recorder:
    """A layout proxy that records every method call and its serialized result."""

    def __init__(self, layout: Any, ser: Ser, query_cls: Any):
        self._layout = layout
        self._ser = ser
        self._query = query_cls
        self.calls: list[Any] = []

    @property
    def root(self) -> str:
        return self._layout.root  # type: ignore[no-any-return]

    def _encode(self, v: Any) -> Any:
        if isinstance(v, enum.Enum):
            return "@" + v.name
        if isinstance(v, (list, tuple)):
            return [self._encode(x) for x in v]
        if isinstance(v, (set, frozenset)):
            return sorted(self._encode(x) for x in v)
        if hasattr(v, "path") and hasattr(v, "relpath"):
            return {"@file": self._ser.s(v.path)}
        if isinstance(v, PurePath):
            return {"@path": self._ser.s(str(v))}
        if type(v).__name__ == "PaddedInt":
            return {"@padded": str(v)}
        return self._ser.s(v) if isinstance(v, str) else v

    def __getattr__(self, name: str) -> Any:
        target = getattr(self._layout, name)
        if not callable(target):
            return target

        def call(*args: Any, **kwargs: Any) -> Any:
            entry = [
                name,
                [self._encode(a) for a in args],
                {k: self._encode(v) for k, v in kwargs.items()},
            ]
            try:
                result = target(*args, **kwargs)
            except Exception as err:
                self.calls.append([*entry, self._ser.error(err)])
                raise
            self.calls.append([*entry, self._ser(result)])
            return result

        return call


class _Capture(logging.Handler):
    def __init__(self) -> None:
        super().__init__(logging.DEBUG)
        self.lines: list[str] = []

    def emit(self, record: logging.LogRecord) -> None:
        self.lines.append(f"{record.levelname}: {record.getMessage()}")


def capture_logger(name: str) -> tuple[logging.Logger, _Capture]:
    logger = logging.getLogger(name)
    logger.handlers.clear()
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    cap = _Capture()
    logger.addHandler(cap)
    return logger, cap


def registry_state(estimators: Any, intents: Any, ser: Ser) -> dict[str, Any]:
    return {
        "estimators": [[k, [ser.s(p) for p in v]] for k, v in sorted(estimators.items())],
        "sources": [ser.s(p) for p in estimators.sources],
        "intents": [[ser.s(k), sorted(v)] for k, v in sorted(intents.items())],
    }


def dump_wrangler(
    find: Any, clear: Any, state: Any, layout: Any, ser: Ser, cases: list[dict[str, Any]], *,
    record: bool = False, query_cls: Any = None, keep_registry: bool = False,
) -> list[dict[str, Any]]:  # fmt: skip
    """``find(layout=..., **case)`` for each case from an empty registry (unless
    ``keep_registry``): estimators, registry, debug log, error, and the layout calls."""
    out = []
    for case in cases:
        if not keep_registry:
            clear()
        logger, cap = capture_logger("estimators_dump.wrangler")
        lay = Recorder(layout, ser, query_cls) if record else layout
        kwargs = dict(case)
        try:
            result = ser(find(layout=lay, logger=logger, **kwargs))
        except Exception as err:
            result = ser.error(err)
        entry = {
            "case": ser(case),
            "result": result,
            "registry": state(),
            "log": [ser.s(x) for x in cap.lines],
        }
        if record:
            entry["calls"] = lay.calls  # type: ignore[union-attr]
        out.append(entry)
    return out


def natural_reduce(func: Any, iterable: Any, initial: Any) -> list[Any]:
    """``functools.reduce`` for sdcflows' ``B0FieldIdentifier`` set, returned in pybids'
    natural order: sdcflows iterates the set, whose order changes between processes (SDC-19);
    patched into a wrangler module (``module.reduce = natural_reduce``) so that the comparison
    is exact."""
    import functools

    values = functools.reduce(func, iterable, initial)

    def key(t: Any) -> list[Any]:
        return [int(c) if c.isdigit() else c.lower() for c in re.split("([0-9]+)", str(t))]

    return sorted(sorted(values, key=str), key=key)


def fmriprep_functions(
    base_py: str, logger: logging.Logger, sdcflows_modules: dict[str, Any] | None = None
) -> dict[str, Any]:
    """fMRIPrep's ``map_fieldmap_estimation`` and ``get_estimator`` from its source file, with a
    stub ``config`` (``config.loggers.workflow`` = ``logger``)."""
    tree = ast.parse(Path(base_py).read_text(encoding="utf-8"))
    wanted = [
        n
        for n in tree.body
        if isinstance(n, ast.FunctionDef) and n.name in ("map_fieldmap_estimation", "get_estimator")
    ]
    module = ast.Module(body=wanted, type_ignores=[])
    namespace: dict[str, Any] = {
        "config": SimpleNamespace(loggers=SimpleNamespace(workflow=logger)),
        "bids": SimpleNamespace(BIDSLayout=object),
        "__name__": "fmriprep_base_extract",
    }
    exec(compile(module, base_py, "exec"), namespace)
    return {
        "map_fieldmap_estimation": namespace["map_fieldmap_estimation"],
        "get_estimator": namespace["get_estimator"],
    }


def _main() -> int:
    """Inside fMRIPrep's image: pybids 0.20.0, sdcflows 2.15.1, fMRIPrep 25.2.6."""
    import re

    import fmriprep.workflows.base as fbase
    import sdcflows.utils.wrangler as wr
    from bids.layout import BIDSLayout, BIDSLayoutIndexer, Query
    from bids.layout import parse_file_entities as parse
    from sdcflows import fieldmaps as fm
    from sdcflows.utils.wrangler import find_estimators

    wr.reduce = natural_reduce  # type: ignore[attr-defined]  (SDC-19: one fixed order)

    cases = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    out: dict[str, Any] = {}
    for item in cases:
        ser = Ser({item["root"]: "<root>"})
        ignore = [
            "code", "stimuli", "sourcedata", "models", re.compile(r"^\."),
            re.compile(r"sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|dwi|eeg|ieeg|meg|perf)"),
            re.compile(r"sub-(?!(" + item["subject"] + r")(\b|_))"),
        ]  # fmt: skip
        try:
            layout = BIDSLayout(
                item["root"], indexer=BIDSLayoutIndexer(validate=False, ignore=ignore)
            )
        except Exception as err:
            out[item["name"]] = {"error": ser.error(err)}
            continue
        entry: dict[str, Any] = {"index": dump_index(layout, ser, Query, parse)}

        def state(ser: Ser = ser) -> dict[str, Any]:
            return registry_state(fm._estimators, fm._intents, ser)

        entry["wrangler"] = dump_wrangler(
            find_estimators, fm.clear_registry, state, layout, ser,
            [{"subject": item["subject"], **c} for c in item["wrangler"]],
        )  # fmt: skip
        logger, cap = capture_logger("estimators_dump.fmriprep")
        fbase.config.loggers.workflow = logger  # type: ignore[attr-defined]
        runs = []
        for combo in item["fmriprep"]:
            fm.clear_registry()
            cap.lines.clear()
            try:
                ests, emap = fbase.map_fieldmap_estimation(
                    layout=layout, subject_id=item["subject"], **combo
                )
                res = [ser(ests), [[ser.s(k), v] for k, v in emap.items()]]
            except Exception as err:
                res = ser.error(err)
            runs.append(
                {
                    "combo": combo,
                    "result": res,
                    "log": [ser.s(x) for x in cap.lines],
                    "registry": state(),
                }
            )
        entry["fmriprep"] = runs
        out[item["name"]] = entry
    Path(sys.argv[2]).write_text(json.dumps(out, indent=0), encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(_main())
    except SystemExit:
        raise
    except BaseException:
        traceback.print_exc(file=io.StringIO())
        traceback.print_exc()
        raise SystemExit(1) from None
