# SPDX-License-Identifier: Apache-2.0 AND MIT
"""A BIDS dataset index with pybids' entities, metadata inheritance and queries, in memory.

Ported from pybids 0.22.0 (MIT, Copyright (c) 2016-2026 The PyBIDS Developers; tag
``pybids-0.22.0``): ``layout/layout.py`` (``BIDSLayout``: ``get``, ``get_file``,
``get_metadata``, ``get_entities``, ``parse_file_entities``, the ``get_<entity>s`` queries,
``add_derivatives``, ``_build_file_query``, ``_sanitize_query_dtypes``), ``layout/index.py``
(``BIDSLayoutIndexer``: the directory walk, the ignore and force patterns, the per-directory
``layout_config.json``, the entities of each file name, the metadata of the JSON sidecars by
pybids' inheritance), ``layout/models.py`` (``Config``, ``Entity``, ``Tag``, ``BIDSFile``),
``layout/validation.py`` (the root's checks, the ignore patterns), ``layout/utils.py``
(``parse_file_entities``, ``PaddedInt``, ``BIDSMetadata``), ``utils.py`` (``listify``,
``natural_sort``, ``hashablefy``, ``bids_sort``), ``layout/db.py`` (SQLite's ``REGEXP``:
case-insensitive ``re.search``), ``exceptions.py``; the configurations ``config/bids.json`` and
``config/derivatives.json`` (pybids', unchanged) and ``config/nipreps.json`` (niworkflows 1.15.4,
Apache-2.0, unchanged). pybids 0.20.0 (fMRIPrep 25.2.6's image) differs on these paths only in
the order of :meth:`BIDSFile.get_entities` (``pybids_version="0.20.0"``) and in the warning about
``IntendedFor`` URIs of named datasets (0.22.0 only).

pybids keeps its index in an SQLite database through SQLAlchemy; this index keeps the same
records in Python dictionaries and reproduces the database's semantics where they show: the
``IN`` comparison of the stored text values (an integer filter casts them as SQLite's ``CAST``
does), ``IS NULL`` for absent entities, ``REGEXP`` as pybids registers it, the order of the rows
(by path, by entity name). No database file is written (``database_path`` has no counterpart).

Differences, each on purpose:

- pybids matches its ignore patterns against the relative path with the platform's separator, so
  on Windows a pattern such as ``^/code`` never matches; here the path is always written with
  ``/`` (pybids' behaviour on Linux and macOS, on every platform).
- pybids' ``natural_sort`` of unique values (``get_<entity>s``) breaks ties (``'01'`` and ``'1'``)
  in the order of a Python ``set``, which changes between processes; here ties keep the order of
  plain string comparison.
- ``validate=True`` with the default indexer filters files with the BIDS validator, which is not
  ported: :class:`BIDSLayoutIndexer` raises ``NotImplementedError`` (fMRIPrep and sdcflows index
  with ``validate=False``; the root's checks of ``BIDSLayout(validate=True)`` are ported).
- pybids' file associations (``get_associations``, ``get_fieldmap``, ``get_nearest``), its BIDS
  schema configuration (``config='bids-schema'``), ``write_to_file`` (``build_path`` is in
  :mod:`larmorx.bids.writing`),
  ``get_collections``, ``to_df`` and database files are not ported: nothing on fMRIPrep's and
  sdcflows' field-map paths calls them.
"""

from __future__ import annotations

import difflib
import enum
import json
import os
import re
import warnings
from collections.abc import Iterable, Mapping
from functools import cache, partial
from pathlib import Path, PurePath
from typing import Any

__all__ = [
    "ALWAYS_IGNORE",
    "BIDS_CONFIG",
    "DEFAULT_LOCATIONS_TO_IGNORE",
    "DERIVATIVES_CONFIG",
    "NIPREPS_CONFIG",
    "PYBIDS_VERSIONS",
    "BIDSConflictingValuesError",
    "BIDSDataFile",
    "BIDSDerivativesValidationError",
    "BIDSEntityError",
    "BIDSFile",
    "BIDSImageFile",
    "BIDSJSONFile",
    "BIDSLayout",
    "BIDSLayoutIndexer",
    "BIDSMetadata",
    "BIDSValidationError",
    "Config",
    "Entity",
    "PaddedInt",
    "PyBIDSError",
    "Query",
    "TargetError",
    "listify",
    "natural_sort",
    "parse_file_entities",
]

_CONFIG_DIR = Path(__file__).resolve().parent / "config"
#: pybids 0.22.0's ``bids.json`` (the entities of BIDS raw file names).
BIDS_CONFIG = _CONFIG_DIR / "bids.json"
#: pybids 0.22.0's ``derivatives.json``.
DERIVATIVES_CONFIG = _CONFIG_DIR / "derivatives.json"
#: niworkflows 1.15.4's ``nipreps.json`` (fMRIPrep's derivatives, with ``fmapid``).
NIPREPS_CONFIG = _CONFIG_DIR / "nipreps.json"
_CONFIG_PATHS = {"bids": BIDS_CONFIG, "derivatives": DERIVATIVES_CONFIG}

#: The pybids versions whose behaviour can be chosen: 0.22.0 (newest, both kept environments)
#: and 0.20.0 (fMRIPrep 25.2.6's image).
PYBIDS_VERSIONS = ("0.22.0", "0.20.0")

#: pybids' default ignore pattern (``validation.DEFAULT_LOCATIONS_TO_IGNORE``).
DEFAULT_LOCATIONS_TO_IGNORE = (re.compile(r"^/(code|models|sourcedata|stimuli)"),)
#: Always ignored: dot files and dot folders anywhere (``validation.ALWAYS_IGNORE``).
ALWAYS_IGNORE = (re.compile(r"/\."),)

# bidsschematools 2.0.0's ``rules.entities`` (both kept environments), then suffix, extension and
# datatype: the order pybids 0.22.0's ``bids_sort`` gives a file's entities.
_SCHEMA_ENTITIES = (
    "subject", "template", "session", "cohort", "sample", "task", "tracksys", "acquisition",
    "nucleus", "volume", "ceagent", "tracer", "stain", "reconstruction", "direction", "run",
    "modality", "echo", "flip", "inversion", "mtransfer", "part", "processing", "hemisphere",
    "space", "split", "recording", "chunk", "atlas", "segmentation", "scale", "resolution",
    "density", "label", "description", "suffix", "extension", "datatype",
)  # fmt: skip
_SCHEMA_INDEX = {name: i for i, name in enumerate(_SCHEMA_ENTITIES)}

_EXAMPLE_BIDS_DESCRIPTION = {"Name": "Example dataset", "BIDSVersion": "1.0.2"}
_MANDATORY_BIDS_FIELDS = {
    "Name": {"Name": "Example dataset"},
    "BIDSVersion": {"BIDSVersion": "1.0.2"},
}
_MANDATORY_DERIVATIVES_FIELDS = {
    **_MANDATORY_BIDS_FIELDS,
    "GeneratedBy": {"GeneratedBy": [{"Name": "Example pipeline"}]},
}
_EXAMPLE_DERIVATIVES_DESCRIPTION = {
    k: val[k] for val in _MANDATORY_DERIVATIVES_FIELDS.values() for k in val
}


# --- exceptions (pybids' ``exceptions.py``) -------------------------------------------------


class PyBIDSError(Exception):
    """Base class of pybids' errors."""


class BIDSEntityError(AttributeError, PyBIDSError):
    """An unknown entity in a ``get_<entity>s`` query."""


class TargetError(ValueError, PyBIDSError):
    """An unknown or missing target of a query."""


class BIDSValidationError(ValueError, PyBIDSError):
    """An invalid dataset root."""


class BIDSDerivativesValidationError(BIDSValidationError):
    """An invalid derivatives dataset."""


class BIDSConflictingValuesError(BIDSValidationError):
    """An entity's value in a file name differs from its sidecar's."""


class Query(enum.Enum):
    """The special values of :meth:`BIDSLayout.get` filters (pybids' ``Query``)."""

    NONE = 1  # the entity must not be present
    REQUIRED = ANY = 2  # the entity must be present, with any value
    OPTIONAL = 3  # the entity may or may not be present


# --- small utilities (pybids' ``utils.py``, ``layout/utils.py``) ----------------------------


def listify(obj: Any) -> Any:
    """Wrap every object that is not a list, a tuple or ``None`` in a list."""
    return obj if isinstance(obj, (list, tuple, type(None))) else [obj]


def _alphanum_key(key: Any, field: str | None = None) -> list[Any]:
    if field is not None:
        key = getattr(key, field)
    if not isinstance(key, str):
        key = str(key)
    return [int(c) if c.isdigit() else c.lower() for c in re.split("([0-9]+)", key)]


def natural_sort(items: Iterable[Any], field: str | None = None) -> list[Any]:
    """pybids' ``natural_sort``: digits compared as numbers, letters without case."""
    return sorted(items, key=partial(_alphanum_key, field=field))


def _hashablefy(obj: Any) -> Any:
    if isinstance(obj, list):
        return tuple(_hashablefy(o) for o in obj)
    if isinstance(obj, dict):
        return _FrozenDict({k: _hashablefy(v) for k, v in obj.items()})
    return obj


class _FrozenDict(dict):
    """A hashable dictionary (pybids' ``frozendict``), printed as a dictionary."""

    def __hash__(self) -> int:  # type: ignore[override]
        return hash(frozenset(self.items()))


class BIDSMetadata(dict):
    """A metadata dictionary that names its file when a key is missing."""

    def __init__(self, source_file: str):
        self._source_file = source_file
        super().__init__()

    def __getitem__(self, key: Any) -> Any:
        try:
            return super().__getitem__(key)
        except KeyError:
            raise KeyError(
                f"Metadata term {key!r} unavailable for file {self._source_file}."
            ) from None


class PaddedInt(int):
    """An integer that keeps its zero padding as a string (pybids' ``PaddedInt``)."""

    def __new__(cls, val: Any) -> PaddedInt:
        return super().__new__(cls, val)

    def __init__(self, val: Any):
        self.sval = str(val)

    def __eq__(self, val: object) -> bool:  # type: ignore[override]
        try:
            return val == self.sval or super().__eq__(val)  # type: ignore[return-value]
        except ValueError:
            return val == self  # type: ignore[no-any-return]

    def __str__(self) -> str:
        return self.sval

    def __repr__(self) -> str:
        return self.sval

    def __format__(self, format_spec: str) -> str:
        try:
            return format(self.sval, format_spec)
        except ValueError:
            return super().__format__(format_spec)

    def __hash__(self) -> int:
        return super().__hash__()


def _bids_sort(unsorted: dict[str, Any]) -> dict[str, Any]:
    n = len(_SCHEMA_ENTITIES)
    return {k: unsorted[k] for k in sorted(unsorted, key=lambda k: _SCHEMA_INDEX.get(k, n))}


def _sqlite_int(text: str) -> int:
    """SQLite's ``CAST(text AS INTEGER)``: the longest integer prefix (after spaces), else 0."""
    m = re.match(r"\s*([+-]?\d+)", text)
    return int(m.group(1)) if m else 0


@cache
def _regexp(expr: str) -> re.Pattern[str]:
    return re.compile(expr, re.I)


# --- configuration, entities, tags (pybids' ``layout/models.py``) ---------------------------


class Entity:
    """An entity of a configuration: its name, the pattern of its value in a path (searched in
    the whole path; the first group is the value), its value type and directory template."""

    __slots__ = ("_dtype", "directory", "dtype", "mandatory", "name", "pattern", "regex")

    def __init__(
        self,
        name: str,
        pattern: str | None = None,
        mandatory: bool = False,
        directory: str | None = None,
        dtype: Any = "str",
    ):
        self.name = name
        self.pattern = pattern
        self.mandatory = mandatory
        self.directory = directory
        if not isinstance(dtype, str):
            dtype = dtype.__name__
        if dtype not in ("str", "float", "int", "bool"):
            raise ValueError(
                f"Invalid dtype '{dtype}'. Must be one of 'int', 'float', 'bool', or 'str'."
            )
        self._dtype = dtype
        self.dtype = {"int": PaddedInt, "str": str, "float": float, "bool": bool}[dtype]
        self.regex = re.compile(pattern) if pattern is not None else None

    def __repr__(self) -> str:
        return f"<Entity {self.name} (pattern={self.pattern}, dtype={self.dtype})>"

    def match_file(self, path: str) -> Any:
        """The entity's value in ``path`` (a ``BIDSFile`` or a string), or ``None``."""
        if self.regex is None:
            return None
        m = self.regex.search(path if isinstance(path, str) else path.path)
        val = m.group(1) if m is not None else None
        return self._astype(val)

    def _astype(self, val: Any) -> Any:
        if val is not None and self.dtype is not None:
            val = self.dtype(val)
        return val


class Config:
    """A pybids configuration: a name, its entities in order, its path patterns."""

    def __init__(
        self,
        name: str,
        entities: Iterable[Mapping[str, Any]] | None = None,
        default_path_patterns: Any = None,
    ):
        self.name = name
        self.default_path_patterns = default_path_patterns
        self.entities: dict[str, Entity] = {}
        for ent in entities or ():
            e = Entity(**ent)
            self.entities[e.name] = e

    def __repr__(self) -> str:
        return f"<Config {self.name}>"

    @classmethod
    def load(cls, config: Any) -> Config:
        """A configuration from a name (``'bids'``, ``'derivatives'``), a JSON file's path or a
        dictionary. ``'bids-schema'`` (the BIDS schema's rules) is not ported."""
        if isinstance(config, Config):
            return config
        if config == "bids-schema" or (isinstance(config, dict) and "schema_version" in config):
            raise NotImplementedError(
                "pybids' 'bids-schema' configuration (bidsschematools' rules) is not ported; "
                "use 'bids' (pybids' default)"
            )
        if isinstance(config, (str, PurePath)):
            if config in _CONFIG_PATHS:
                config = _CONFIG_PATHS[config]  # type: ignore[index]
            if not Path(config).exists():
                raise ValueError(f"{config} is not a valid path.")
            with open(config) as f:
                config = json.load(f)
        return _config_from_dict(config)


@cache
def _shipped_config(path: str) -> Config:
    with open(path) as f:
        return _config_from_dict(json.load(f))


def _config_from_dict(config: Mapping[str, Any]) -> Config:
    return Config(**config)


def _load_config(config: Any) -> Config:
    if isinstance(config, str) and config in _CONFIG_PATHS:
        return _shipped_config(str(_CONFIG_PATHS[config]))
    return Config.load(config)


class _Tag:
    """A file's entity value as pybids stores it: the text ``_value``, its type ``_dtype`` and
    the value read back from them (``value``)."""

    __slots__ = ("_dtype", "_value", "is_metadata", "value")

    def __init__(self, value: Any, dtype: Any = None, is_metadata: bool = False):
        if dtype is None:
            dtype = type(value)
        if not isinstance(dtype, str):
            dtype = dtype.__name__
        if dtype in ("list", "dict"):
            _dtype, _value = "json", json.dumps(value)
        else:
            _dtype, _value = dtype, str(value)
        if _dtype not in ("str", "float", "int", "bool", "json"):
            raise ValueError(
                f"Passed value has an invalid dtype ({dtype}). Must be one of int, float, bool, "
                "or str."
            )
        self._dtype = _dtype
        self._value = _value
        self.is_metadata = is_metadata
        # pybids reads every value back from the database (``Tag._init_on_load``)
        if _dtype == "json":
            self.value = json.loads(_value)
        elif _dtype == "bool":
            self.value = _value == "True"
        else:
            self.value = {"str": str, "int": PaddedInt, "float": float}[_dtype](_value)


# --- files (pybids' ``BIDSFile`` and its subclasses) ----------------------------------------


class BIDSFile:
    """A file of a :class:`BIDSLayout`: its path, its entities from the file name and its
    metadata from the sidecars (``entities`` holds both, ordered by entity name)."""

    def __init__(self, filename: str, layout: BIDSLayout | None = None):
        self.path = str(filename)
        p = Path(self.path)
        self.filename = p.name
        self.dirname = str(p.parent)
        self.is_dir = not self.filename
        self._layout = layout
        self._tags: dict[str, _Tag] = {}

    @property
    def _path(self) -> Path:
        return Path(self.path)

    @property
    def _dirname(self) -> Path:
        return Path(self.dirname)

    def __repr__(self) -> str:
        return f"<{type(self).__name__} filename='{self.path}'>"

    def __fspath__(self) -> str:
        return self.path

    @property
    def relpath(self) -> str:
        """The path relative to the layout's root."""
        if self._layout is None:
            raise ValueError(f"{self.path} belongs to no layout")
        return str(Path(self.path).relative_to(self._layout.root))

    @property
    def entities(self) -> dict[str, Any]:
        """Every entity of the file, from its name and its metadata, ordered by name."""
        return {name: self._tags[name].value for name in sorted(self._tags)}

    def get_entities(self, metadata: Any = False, values: str = "tags") -> dict[str, Any]:
        """The entities of the file name (``metadata=False``, the default), of the metadata
        (``True``) or both (``None``); ``values='objects'`` gives the :class:`Entity`
        objects."""
        if metadata is None and values == "tags":
            return self.entities
        layout = self._layout
        version = layout._pybids_version if layout is not None else PYBIDS_VERSIONS[0]
        names = sorted(self._tags)
        if metadata not in (None, "all"):
            names = [n for n in names if self._tags[n].is_metadata == bool(metadata)]
        if values.startswith("obj"):
            if layout is None:
                raise ValueError(f"{self.path} belongs to no layout")
            out = {n: layout._entities[n] for n in names}
        else:
            out = {n: self._tags[n].value for n in names}
        return _bids_sort(out) if version != "0.20.0" else out

    def get_metadata(self) -> BIDSMetadata:
        """The metadata of the file (its sidecars, merged by pybids' inheritance)."""
        md = BIDSMetadata(self.path)
        md.update(self.get_entities(metadata=True))
        return md


class BIDSDataFile(BIDSFile):
    """A ``.tsv`` or ``.tsv.gz`` file."""


class BIDSImageFile(BIDSFile):
    """A NIfTI or GIFTI file (``.nii``, ``.nii.gz``, ``.gii``)."""


class BIDSJSONFile(BIDSFile):
    """A JSON file."""

    def get_dict(self) -> dict[str, Any]:
        """The file's contents as a dictionary."""
        d = json.loads(self.get_json())
        if not isinstance(d, dict):
            raise ValueError(
                f"File {self.path} is a json containing {type(d)}, not a dict which was expected"
            )
        return d

    def get_json(self) -> str:
        """The file's contents as text."""
        with open(self.path) as f:
            return f.read()


def _make_bidsfile(filename: str, layout: BIDSLayout | None = None) -> BIDSFile:
    ext = "".join(PurePath(filename).suffixes)
    if ext.endswith((".nii", ".nii.gz", ".gii")):
        cls: type[BIDSFile] = BIDSImageFile
    elif ext in (".tsv", ".tsv.gz"):
        cls = BIDSDataFile
    elif ext == ".json":
        cls = BIDSJSONFile
    else:
        cls = BIDSFile
    return cls(filename, layout)


def parse_file_entities(
    filename: Any,
    entities: Iterable[Entity] | None = None,
    config: Any = None,
    include_unmatched: bool = False,
) -> dict[str, Any]:
    """The entities in ``filename``'s path (pybids' ``parse_file_entities``): by default those
    of the ``'bids'`` and ``'derivatives'`` configurations, searched in the whole path."""
    if entities is None:
        if config is None:
            config = ["bids", "derivatives"]
        configs = [_load_config(c) for c in listify(config)]
        merged: dict[str, Entity] = {}
        for c in configs:
            merged.update(c.entities)
        entities = merged.values()
    path = str(filename)  # pybids matches str(filename) (a BIDSFile's repr, for a BIDSFile)
    ent_vals: dict[str, Any] = {}
    for ent in entities:
        match = ent.match_file(path)
        if match is not None or include_unmatched:
            ent_vals[ent.name] = match
    return ent_vals


# --- the indexer (pybids' ``layout/index.py``, ``layout/validation.py``) --------------------


def _regexfy(patt: Any, root: Path) -> re.Pattern[str]:
    if hasattr(patt, "search"):
        return patt  # type: ignore[no-any-return]
    p = Path(patt)
    patt = p.relative_to(root).as_posix() if p.is_absolute() else PurePath(patt).as_posix()
    return re.compile(r"^/" + str(patt) + r".*")


def _sort_patterns(patterns: Any, root: Path) -> list[Any]:
    regexes = [patt for patt in patterns if hasattr(patt, "search")]
    paths = [
        str((root / patt).absolute()) for patt in listify(patterns) if not hasattr(patt, "search")
    ]
    paths.sort(key=len)
    return [Path(p) for p in reversed(paths)] + regexes


def _validate_indexing_args(
    ignore: Any, force_index: Any, root: Path
) -> tuple[list[Any], list[Any]]:
    if ignore is None:
        ignore = [p for p in DEFAULT_LOCATIONS_TO_IGNORE if p not in set(force_index or [])]
    ignore = list(listify(ignore))
    ignore.extend(ALWAYS_IGNORE)
    return _sort_patterns(ignore, root), _sort_patterns(force_index or [], root)


def _relative_posix(path: Path, root: Path) -> str:
    # pybids: str(Path("/") / path.relative_to(root)) (the platform's separator); "/" here
    return "/" + Path(path).absolute().relative_to(root).as_posix().lstrip("/")


def _matches(rel: str, patterns: list[re.Pattern[str]]) -> bool:
    return any(p.search(rel) for p in patterns)


def _validate_path(
    rel: str, incl: list[re.Pattern[str]], excl: list[re.Pattern[str]]
) -> bool | None:
    if incl and _matches(rel, incl):
        return True
    if excl and _matches(rel, excl):
        return False
    return None


def _resolve_intent(intent: str, root: Path, subject: str) -> str | None:
    if not intent.startswith("bids:"):
        return str(root / f"sub-{subject}" / intent)
    if intent.startswith("bids::"):
        return str(root / intent[6:])
    return None


class BIDSLayoutIndexer:
    """pybids' indexer: which files of the dataset are indexed (``ignore`` and ``force_index``:
    paths relative to the root, or compiled regular expressions searched in ``/``-rooted
    relative paths), and whether their metadata are read (``index_metadata``). ``validate=True``
    (the BIDS validator's file rules) is not ported."""

    def __init__(
        self,
        validate: bool = False,
        ignore: Any = None,
        force_index: Any = None,
        index_metadata: bool = True,
        config_filename: str = "layout_config.json",
        **filters: Any,
    ):
        if validate:
            raise NotImplementedError(
                "BIDSLayoutIndexer(validate=True) filters files with the BIDS validator, which "
                "larmorx does not port; index with validate=False (as fMRIPrep and sdcflows do)"
            )
        self.validate = validate
        self.ignore = ignore
        self.force_index = force_index
        self.index_metadata = index_metadata
        self.config_filename = config_filename
        self.filters = filters

    def __call__(self, layout: BIDSLayout) -> None:
        root = layout._root
        ignore, force = _validate_indexing_args(self.ignore, self.force_index, root)
        incl = [_regexfy(p, root) for p in force]
        excl = [_regexfy(p, root) for p in ignore]
        configs = list(layout.config.values())
        self._index_dir(layout, root, configs, incl, excl, None)
        if self.index_metadata:
            self._index_metadata(layout)

    def _index_dir(
        self,
        layout: BIDSLayout,
        path: Path,
        config: list[Config],
        incl: list[re.Pattern[str]],
        excl: list[re.Pattern[str]],
        force: bool | None,
    ) -> None:
        root = layout._root
        if root / "derivatives" in path.parents:
            return
        config = list(config)
        config_file = path / self.config_filename
        if config_file.exists():
            cfg = Config.load(config_file)
            existing = layout.config.get(cfg.name)
            if existing is not None:
                cfg = existing
            else:
                layout.config[cfg.name] = cfg
                for ent in cfg.entities.values():
                    layout._entities.setdefault(ent.name, ent)
            config.append(cfg)
        config_entities: dict[str, Entity] = {}
        for c in config:
            config_entities.update(c.entities)
        try:
            entries = list(os.scandir(path))
        except OSError:
            return
        dirnames = [e.name for e in entries if e.is_dir(follow_symlinks=False)]
        filenames = [e.name for e in entries if not e.is_dir(follow_symlinks=False)]
        links_to_dirs = [f for f in filenames if (path / f).is_dir()]
        dirnames.extend(links_to_dirs)
        for link in links_to_dirs:
            filenames.remove(link)
        zarrnames = [d for d in dirnames if PurePath(d).suffix == ".zarr"]
        dirnames = [d for d in dirnames if PurePath(d).suffix != ".zarr"]
        filenames.extend(zarrnames)
        if self.config_filename in filenames:
            filenames.remove(self.config_filename)
        for f in filenames:
            abs_fn = path / f
            if force or self._validate_file(_relative_posix(abs_fn, root), incl, excl):
                layout._add_file(abs_fn, config_entities)
        for d in dirnames:
            sub = path / d
            sub_force = _validate_path(_relative_posix(sub, root), incl, excl)
            if sub_force is not False:
                self._index_dir(layout, sub, config, incl, excl, sub_force)

    @staticmethod
    def _validate_file(rel: str, incl: list[re.Pattern[str]], excl: list[re.Pattern[str]]) -> bool:
        matched = _validate_path(rel, incl, excl)
        return True if matched is None else matched

    def _index_metadata(self, layout: BIDSLayout) -> None:
        filters = dict(self.filters)
        if filters:
            filters["return_type"] = "object"
            if filters.get("extension"):
                filters["extension"] = list(listify(filters["extension"]))
                if ".json" not in filters["extension"]:
                    filters["extension"].append(".json")
        all_files = layout.get(**filters)
        all_tags = {
            f"{bf.path}_{name}": str(tag.value)
            for bf in layout._files.values()
            for name, tag in bf._tags.items()
        }
        file_data: dict[str, dict[Path, list[tuple[dict[str, Any], Any, str]]]] = {}

        @cache
        def load_json(path: str, label: str) -> Any:
            with open(path, encoding="utf-8") as handle:
                try:
                    return json.load(handle)
                except (UnicodeDecodeError, json.JSONDecodeError) as e:
                    raise OSError(
                        f"Error occurred while trying to decode JSON from file {label}"
                    ) from e

        filenames = []
        for bf in all_files:
            ents = bf.entities
            if "suffix" in ents and "extension" in ents:
                file_ents = dict(ents)
                suffix = file_ents.pop("suffix")
                ext = file_ents.pop("extension")
                key = f"{ext}/{suffix}"
                payload = None
                if ext == ".json":
                    payload = partial(load_json, bf.path, repr(bf))
                else:
                    filenames.append(bf)
                file_data.setdefault(key, {}).setdefault(bf._dirname, []).append(
                    (file_ents, payload, bf.path)
                )

        for bf in filenames:
            file_ents = dict(bf.entities)
            suffix = file_ents.pop("suffix", None)
            ext = file_ents.pop("extension", None)
            file_ent_keys = set(file_ents)
            if suffix is None or ext is None:
                continue
            json_key = f".json/{suffix}"
            dirname = bf._dirname
            payloads = []
            while True:
                for js_ents, js_md, js_path in file_data.get(json_key, {}).get(dirname, []):
                    js_keys = set(js_ents)
                    if js_keys - file_ent_keys:
                        continue
                    if all(js_ents[name] == file_ents[name] for name in js_keys):
                        payloads.append((js_md, js_path))
                parent = dirname.parent
                if parent == dirname:
                    break
                dirname = parent
            if not payloads:
                continue
            virtual_datafile = not bf._path.exists()
            file_md: dict[str, Any] = {}
            for pl, _js_file in payloads[::-1]:
                try:
                    file_md.update(pl())
                except FileNotFoundError:
                    if not virtual_datafile:
                        raise
                    file_md = {}
                    break
            # pybids links IntendedFor targets (file associations, not ported): its checks run
            for target in listify(file_md.get("IntendedFor", [])):
                if layout._pybids_version == "0.20.0":
                    layout._root.joinpath(f"sub-{bf.entities['subject']}", target)
                    continue
                resolved = _resolve_intent(target, layout._root, bf.entities["subject"])
                if resolved is None:
                    warnings.warn(f"Skipping association for {resolved}", stacklevel=2)
            for md_key, md_val in file_md.items():
                if md_val is None:
                    continue
                tag_string = f"{bf.path}_{md_key}"
                if tag_string in all_tags:
                    file_val = all_tags[tag_string]
                    if str(md_val) != file_val:
                        raise BIDSConflictingValuesError(
                            f"Conflicting values found for entity '{md_key}' in filename "
                            f"{bf.path} (value='{file_val}') versus its JSON sidecar "
                            f"(value='{md_val}'). Please reconcile this discrepancy."
                        )
                    continue
                if md_key not in layout._entities:
                    layout._entities[md_key] = Entity(md_key)
                bf._tags[md_key] = _Tag(md_val, is_metadata=True)
                layout._seen(md_key, True)


# --- the layout (pybids' ``BIDSLayout``) ----------------------------------------------------


def _validate_root(root: Any, validate: bool) -> tuple[Path, dict[str, Any] | None]:
    try:
        root = Path(root)
    except TypeError:
        raise TypeError(
            "root argument must be a pathlib.Path (or a type that supports casting to "
            "pathlib.Path, such as string) specifying the directory containing the BIDS "
            "dataset."
        ) from None
    root = root.absolute()
    if not root.exists():
        raise ValueError(f"BIDS root does not exist: {root}")
    target = root / "dataset_description.json"
    description = None
    if not target.exists():
        if validate:
            raise BIDSValidationError(
                "'dataset_description.json' is missing from project root. Every valid BIDS "
                "dataset must have this file.\nExample contents of "
                f"'dataset_description.json': \n{json.dumps(_EXAMPLE_BIDS_DESCRIPTION)}"
            )
    else:
        err = None
        try:
            with open(target, encoding="utf-8") as desc_fd:
                description = json.load(desc_fd)
        except (UnicodeDecodeError, json.JSONDecodeError) as e:
            description = None
            err = e
        if validate:
            if description is None:
                raise BIDSValidationError(
                    "'dataset_description.json' is not a valid json file. There is likely a "
                    f"typo in your 'dataset_description.json' at {target.resolve()}. \nExample "
                    "contents of 'dataset_description.json': \n"
                    f"{json.dumps(_EXAMPLE_BIDS_DESCRIPTION)}"
                ) from err
            for k in _MANDATORY_BIDS_FIELDS:
                if k not in description:
                    raise BIDSValidationError(
                        f"Mandatory {k!r} field missing from 'dataset_description.json' at "
                        f"{target.resolve()}. \nExample: {_MANDATORY_BIDS_FIELDS[k]}"
                    )
    return root, description


def _validate_derivative_path(path: Path) -> str:
    dd = Path(path) / "dataset_description.json"
    description = json.loads(dd.read_text(encoding="utf-8"))
    names = [p["Name"] for p in description.get("GeneratedBy", []) if "Name" in p]
    if names:
        name = names[0]
    elif "PipelineDescription" in description:
        warnings.warn(
            "The PipelineDescription field was superseded by GeneratedBy in BIDS 1.4.0. You can "
            "use ``pybids upgrade`` to update your derivative dataset.",
            stacklevel=2,
        )
        name = description["PipelineDescription"].get("Name")
    else:
        name = None
    if name is None:
        raise BIDSDerivativesValidationError(
            "Every valid BIDS-derivatives dataset must have a GeneratedBy.Name field set inside "
            f"'dataset_description.json', here {dd.resolve()}. \nExample: "
            f"{_MANDATORY_DERIVATIVES_FIELDS['GeneratedBy']}"
        )
    return name  # type: ignore[no-any-return]


class BIDSLayout:
    """A BIDS dataset indexed as pybids 0.22.0 indexes it (``bids.layout.BIDSLayout``).

    Parameters
    ----------
    root
        The dataset's folder.
    validate
        Check ``dataset_description.json`` (pybids' root checks); with the default indexer it
        would also filter files with the BIDS validator, which is not ported: pass
        ``indexer=BIDSLayoutIndexer(validate=False, ...)`` or ``validate=False``.
    derivatives
        Also index derivatives datasets: ``True`` (``<root>/derivatives``), a path or a list.
    config
        The configurations (``'bids'``, ``'derivatives'``, a JSON file such as
        :data:`NIPREPS_CONFIG`, a dictionary); by default ``'bids'``, with ``'derivatives'``
        for a derivatives dataset.
    sources
        The layouts this one derives from.
    regex_search
        Stored, as pybids stores it (its queries take their own ``regex_search``).
    indexer
        A :class:`BIDSLayoutIndexer` (which files, which patterns).
    is_derivative
        Index as a derivatives dataset.
    pybids_version
        ``"0.22.0"`` (default) or ``"0.20.0"`` (fMRIPrep 25.2.6's image): the order of
        :meth:`BIDSFile.get_entities` and the warning about named-dataset URIs.
    """

    def __init__(
        self,
        root: Any = None,
        validate: bool = True,
        *,
        derivatives: Any = False,
        config: Any = None,
        sources: Any = None,
        regex_search: bool = False,
        indexer: BIDSLayoutIndexer | None = None,
        is_derivative: bool = False,
        pybids_version: str = "0.22.0",
    ):
        if pybids_version not in PYBIDS_VERSIONS:
            raise ValueError(
                f"pybids_version must be one of {PYBIDS_VERSIONS}, not {pybids_version!r}"
            )
        self._pybids_version = pybids_version
        root, description = _validate_root(root, validate)
        if is_derivative or (description and description.get("DatasetType") == "derivative"):
            try:
                self.source_pipeline: str | None = _validate_derivative_path(root)
            except (BIDSValidationError, FileNotFoundError):
                if validate:
                    raise
                self.source_pipeline = None
            default_config = ["bids", "derivatives"]
            self.is_derivative = True
        else:
            self.source_pipeline = None
            default_config = ["bids"]
            self.is_derivative = False
        self._root = root
        self.description = description
        self.derivatives: dict[str, BIDSLayout] = {}
        self.sources = sources
        self.regex_search = regex_search
        self.config: dict[str, Config] = {}
        self._entities: dict[str, Entity] = {}
        self._files: dict[str, BIDSFile] = {}
        self._seen_order: dict[tuple[str, bool], None] = {}
        for c in listify(config if config else default_config):
            cfg = _load_config(c)
            self.config[cfg.name] = cfg
            for ent in cfg.entities.values():
                self._entities[ent.name] = ent
        _indexer = indexer or BIDSLayoutIndexer(validate=validate and not is_derivative)
        _indexer(self)
        if derivatives:
            if derivatives is True:
                derivatives = root / "derivatives"
            self.add_derivatives(
                derivatives,
                validate=validate,
                derivatives=None,
                sources=self,
                config=None,
                regex_search=regex_search,
                indexer=indexer,
            )

    # -- indexing helpers --
    def _seen(self, name: str, is_metadata: bool) -> None:
        self._seen_order.setdefault((name, is_metadata), None)

    def _add_file(self, abs_fn: Path, entities: Mapping[str, Entity]) -> None:
        bf = _make_bidsfile(str(abs_fn), self)
        for e in entities.values():
            m = e.match_file(bf.path)
            if m is None and e.mandatory:
                break
            if m is not None:
                bf._tags[e.name] = _Tag(m, e._dtype)
                self._seen(e.name, False)
        self._files[bf.path] = bf

    # -- pybids' API --
    @property
    def root(self) -> str:
        """The dataset's folder (absolute, as given: links not resolved)."""
        return str(self._root)

    @property
    def entities(self) -> dict[str, Entity]:
        """Every entity of the layouts (:meth:`get_entities`)."""
        return self.get_entities()

    @property
    def files(self) -> dict[str, BIDSFile]:
        """Every file of the layouts, by path (:meth:`get_files`)."""
        return self.get_files()

    def get_files(self, scope: Any = "all") -> dict[str, BIDSFile]:
        """Every file of the layouts in ``scope``, by path."""
        out: dict[str, BIDSFile] = {}
        for lay in self._get_layouts_in_scope(scope):
            out.update(lay._files)
        return out

    def __repr__(self) -> str:
        subjects = {f._tags["subject"]._value for f in self._files.values() if "subject" in f._tags}
        sessions = {
            (f._tags["session"].value, f._tags["subject"].value)
            for f in self._files.values()
            if "session" in f._tags and "subject" in f._tags
        }
        runs = {
            (f._tags["run"].value, f._tags["subject"].value)
            for f in self._files.values()
            if "run" in f._tags and "subject" in f._tags and isinstance(f._tags["run"].value, int)
        }
        return (
            f"BIDS Layout: ...{self.root[-30:]} | Subjects: {len(subjects)} | Sessions: "
            f"{len(sessions)} | Runs: {len(runs)}"
        )

    def __getattr__(self, key: str) -> Any:
        if key.startswith("get_"):
            ent_name = key.replace("get_", "")
            entities = self.get_entities()
            if ent_name not in entities:
                sing = _singular(ent_name, entities)
                if sing is not None:
                    ent_name = sing
                else:
                    raise BIDSEntityError(
                        f"'get_{ent_name}' can't be called because '{ent_name}' isn't a "
                        "recognized entity name."
                    )
            return partial(self.get, return_type="id", target=ent_name)
        raise AttributeError(f"{type(self).__name__} object has no attribute named {key!r}")

    def _in_scope(self, scope: Any) -> bool:
        scope = listify(scope)
        if "all" in scope:
            return True
        return any(
            [
                "raw" in scope and not self.is_derivative,
                ("derivatives" in scope or self.source_pipeline in scope) and self.is_derivative,
            ]
        )

    def _get_layouts_in_scope(self, scope: Any) -> list[BIDSLayout]:
        if scope == "self":
            return [self]

        def collect(layout: BIDSLayout) -> list[BIDSLayout]:
            out = [layout]
            for d in layout.derivatives.values():
                out.extend(collect(d))
            return out

        seen: dict[int, BIDSLayout] = {}
        for lay in collect(self):
            if lay._in_scope(scope):
                seen.setdefault(id(lay), lay)
        return list(seen.values())

    def get_entities(self, scope: Any = "all", metadata: Any = None) -> dict[str, Entity]:
        """The entities of the layouts in ``scope``: all (``metadata=None``), those found in
        file names (``False``) or in metadata only (``True``)."""
        entities: dict[str, Entity] = {}
        for lay in self._get_layouts_in_scope(scope):
            if metadata is None:
                entities.update(lay._entities)
            else:
                names = [n for (n, md) in lay._seen_order if md == bool(metadata)]
                entities.update({n: lay._entities[n] for n in names})
        return entities

    def parse_file_entities(
        self,
        filename: Any,
        scope: Any = "all",
        entities: Iterable[Entity] | None = None,
        config: Any = None,
        include_unmatched: bool = False,
    ) -> dict[str, Any]:
        """The entities of ``filename`` with this layout's configurations."""
        if entities is None and config is None:
            configs: dict[str, Config] = {}
            for lay in self._get_layouts_in_scope(scope):
                configs.update(lay.config)
            config = list(configs.values())
        return parse_file_entities(filename, entities, config, include_unmatched)

    def add_derivatives(self, path: Any, **kwargs: Any) -> None:
        """Index derivatives datasets (each a folder with ``dataset_description.json``, or a
        folder of such folders) as children of this layout."""
        paths = listify(path)
        deriv_paths = []
        for p in paths:
            p = Path(p).absolute()
            if (p / "dataset_description.json").exists():
                deriv_paths.append((p.name, p))
            else:
                deriv_paths.extend(
                    (f"{p.name}/{dd.parent.name}", dd.parent)
                    for dd in p.glob("*/dataset_description.json")
                )
        if not deriv_paths:
            warnings.warn(
                "Derivative indexing was requested, but no valid datasets were found in the "
                f"specified locations ({paths}). Note that all BIDS-Derivatives datasets must "
                "meet all the requirements for BIDS-Raw datasets (a common problem is to fail "
                "to include a 'dataset_description.json' file in derivatives datasets).\n"
                "Example contents of 'dataset_description.json':\n"
                f"{json.dumps(_EXAMPLE_DERIVATIVES_DESCRIPTION)}",
                stacklevel=2,
            )
        kwargs["sources"] = kwargs.get("sources") or self
        kwargs.setdefault("pybids_version", self._pybids_version)
        for name, p in deriv_paths:
            if name in self.derivatives:
                raise BIDSDerivativesValidationError(
                    f"Pipeline name {name} ({p!s}) has already been added to this BIDSLayout. "
                    "Every added pipeline must have a unique name!"
                )
            self.derivatives[name] = BIDSLayout(p, is_derivative=True, **kwargs)

    def get(
        self,
        return_type: str = "object",
        target: str | None = None,
        scope: Any = "all",
        regex_search: bool = False,
        invalid_filters: str = "error",
        **filters: Any,
    ) -> list[Any]:
        """Files (``'object'``), file names (``'file'``/``'filename'``), the unique values of
        an entity (``'id'``, with ``target``) or folders (``'dir'``) matching ``filters``:
        entity names with a value, a list of values (any of them), ``None`` or
        :attr:`Query.NONE` (absent), :attr:`Query.REQUIRED` (present), :attr:`Query.OPTIONAL`
        (no filter); ``regex_search`` matches values as case-insensitive regular expressions."""
        if not return_type.startswith(("obj", "file")) and return_type not in ("id", "dir"):
            raise ValueError(
                f"Invalid return_type <{return_type}> specified (must be one of 'object', "
                "'file', 'filename', 'id', or 'dir')."
            )
        layouts = self._get_layouts_in_scope(scope)
        entities = self.get_entities()
        if isinstance(filters.get("filters"), dict):
            raise RuntimeError(
                "You passed in filters as a dictionary named filters; please pass the keys in "
                "as named keywords to the `get()` call. For example: `layout.get(**filters)`."
            )
        if "extension" in filters and "bids" in self.config:
            filters["extension"] = [
                "." + x.lstrip(".") if isinstance(x, str) else x
                for x in listify(filters["extension"])
            ]
        if invalid_filters != "allow":
            bad_filters = set(filters) - set(entities)
            if bad_filters:
                if invalid_filters == "drop":
                    for bad in bad_filters:
                        filters.pop(bad)
                elif invalid_filters == "error":
                    first_bad = sorted(bad_filters)[0]
                    msg = f"'{first_bad}' is not a recognized entity. "
                    suggestions = difflib.get_close_matches(first_bad, list(entities))
                    if suggestions:
                        msg += f"Did you mean {suggestions}? "
                    raise ValueError(
                        msg + "If you're sure you want to impose this constraint, set "
                        "invalid_filters='allow'."
                    )
        if target is not None and target not in entities:
            potential = list(entities)
            suggestions = difflib.get_close_matches(target, potential)
            if suggestions:
                message = f"Did you mean one of: {suggestions}?"
            else:
                message = f"Valid targets are: {potential}"
            raise TargetError(f"Unknown target '{target}'. " + message)
        elif target is None and return_type in ("id", "dir"):
            raise TargetError(
                'If return_type is "id" or "dir", a valid target entity must also be specified.'
            )
        results: list[BIDSFile] = []
        for lay in layouts:
            results.extend(lay._query(filters, regex_search))
        if return_type.startswith("file"):
            return natural_sort(sorted(f.path for f in results))
        if return_type == "id":
            metadata = target not in self.get_entities(metadata=False)
            values = []
            for res in results:
                if target in res._tags:
                    ents = _hashablefy(res.get_entities(metadata=metadata))
                    if target in ents:
                        values.append(ents[target])
            unique = list(dict.fromkeys(values))
            if any(_str_hashed(v) for v in unique):
                # pybids: natural_sort(list(set(...))): a set of strings has a per-process
                # order; ties are broken by plain string order here
                return natural_sort(sorted(unique, key=str))
            return natural_sort(list(set(unique)))  # numbers: the set's order is pybids'

        if return_type == "dir":
            template = entities[target].directory  # type: ignore[index]
            if template is None:
                raise ValueError(
                    "Return type set to directory, but no directory template is defined for "
                    f'the target entity ("{target}").'
                )
            template = self._root.as_posix() + template
            for ent in re.findall(r"{(.*?)\}", template):
                template = template.replace(f"{{{ent}}}", entities[ent].pattern or "")
            template += r"[^/]*$"
            matches = [f.dirname for f in results if re.search(template, f._dirname.as_posix())]
            return natural_sort(sorted(set(matches)))
        return natural_sort(sorted(results, key=lambda f: f.path), "path")

    def _sanitize_query_dtypes(self, filters: Mapping[str, Any]) -> dict[str, Any]:
        out = dict(filters)
        for name, val in filters.items():
            if isinstance(val, enum.Enum):
                continue
            try:
                ent = self._entities[name]
                if isinstance(val, (list, tuple)):
                    out[name] = [v if isinstance(v, Query) else ent._astype(v) for v in val]
                else:
                    out[name] = ent._astype(val)
            except Exception:
                pass
        return out

    def _query(self, filters: Mapping[str, Any], regex: bool) -> list[BIDSFile]:
        # pybids' GROUP BY path: the rows in path order
        files = sorted((f for f in self._files.values() if not f.is_dir), key=lambda f: f.path)
        if not filters:
            return files
        filters = self._sanitize_query_dtypes(filters)
        for name, val in filters.items():
            val_list = list(listify(val)) if val is not None else [None]
            if not val_list:
                continue
            if Query.OPTIONAL in val_list:
                continue
            none = required = False
            if None in val_list:
                none = True
                val_list.remove(None)
            if Query.NONE in val_list:
                none = True
                val_list.remove(Query.NONE)
            if Query.REQUIRED in val_list:
                required = True
                val_list.remove(Query.REQUIRED)
            if none and required:
                continue
            files = [
                f
                for f in files
                if _filter_match(f._tags.get(name), none, required, regex, val_list)
            ]
        return files

    def get_file(self, filename: Any, scope: Any = "all") -> BIDSFile | None:
        """The file with this path (absolute, or relative to the root), or ``None``."""
        path = str(self._root.joinpath(filename).absolute())
        for lay in self._get_layouts_in_scope(scope):
            if path in lay._files:
                return lay._files[path]
        return None

    def get_metadata(
        self, path: Any, include_entities: bool = False, scope: Any = "all"
    ) -> BIDSMetadata:
        """The metadata of the file at ``path`` (its sidecars by pybids' inheritance), ordered by
        key; with ``include_entities``, its file-name entities too. A path the layout does not
        hold gives an empty dictionary."""
        md = BIDSMetadata(str(path))
        for lay in self._get_layouts_in_scope(scope):
            bf = lay._files.get(str(path))
            if bf is None:
                continue
            names = sorted(n for n, t in bf._tags.items() if include_entities or t.is_metadata)
            if names:
                md.update({n: bf._tags[n].value for n in names})
                return md
        return md


def _filter_match(
    tag: _Tag | None, none: bool, required: bool, regex: bool, val_list: list[Any]
) -> bool:
    """pybids' SQL condition of one filter for one file (``_build_file_query``)."""
    clauses = []
    if not none and tag is None:  # the inner join
        return False
    if none:
        clauses.append(tag is None)
    if required:
        clauses.append(tag is not None)
    if regex:
        clauses.extend(
            tag is not None and _regexp(str(v)).search(tag._value) is not None for v in val_list
        )
    elif val_list:
        if tag is None:
            clauses.append(False)
        elif isinstance(val_list[0], int):
            clauses.append(_sqlite_int(tag._value) in {_as_int(v) for v in val_list})
        else:
            clauses.append(tag._value in {_as_text(v) for v in val_list})
    return any(clauses)


def _as_int(v: Any) -> Any:
    # the IN list compared with an INTEGER expression: text that looks like an integer counts
    if isinstance(v, bool):
        return int(v)
    if isinstance(v, int):
        return int(v)
    if isinstance(v, str) and re.fullmatch(r"\s*[+-]?\d+\s*", v):
        return int(v)
    return v


def _as_text(v: Any) -> Any:
    # the IN list compared with the TEXT column (values the layout's entities did not convert)
    if isinstance(v, str):
        return v
    if isinstance(v, (bool, int)):
        return str(int(v))
    return str(v)


def _str_hashed(v: Any) -> bool:
    """Whether ``v``'s hash depends on string hashing (randomized per process)."""
    if isinstance(v, (str, bytes)):
        return True
    if isinstance(v, tuple):
        return any(_str_hashed(x) for x in v)
    return isinstance(v, dict)


def _singular(name: str, entities: Mapping[str, Any]) -> str | None:
    """The singular of ``name`` that is an entity (pybids asks ``inflect``; the regular English
    plurals here)."""
    candidates = []
    if name.endswith("ies"):
        candidates.append(name[:-3] + "y")
    if name.endswith("es"):
        candidates.append(name[:-2])
    if name.endswith("s") and not name.endswith("ss"):
        candidates.append(name[:-1])
    for c in candidates:
        if c in entities:
            return c
    return None
