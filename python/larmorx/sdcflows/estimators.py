# SPDX-License-Identifier: Apache-2.0
"""sdcflows' field-map estimators: the files, the estimation strategy each set of files gives,
and the registries that name them (``lx.sdcflows``).

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers): ``fieldmaps.py``
(``MetadataError``, ``EstimatorType``, ``MODALITIES``, ``FieldmapFile``, ``FieldmapEstimation``,
``get_identifier``, ``clear_registry``) and ``utils/bimap.py`` (``bidict``,
``EstimatorRegistry``); niworkflows 1.15.4's ``utils/bids.py`` (``relative_to_root``; the same in
1.15.2). sdcflows 2.15.1/2.15.2 (fMRIPrep 25.2.6's) differ on these paths only in
``FieldmapFile``'s path check (2.15 refuses a broken link; 2.17.1 accepts it, as DataLad leaves
files not yet fetched): ``sdcflows_version="2.15"``.

sdcflows keeps its registries in two module globals: every estimator built in a process is
registered there, and its automatic name (``auto_00000``, ...) depends on those already there.
:class:`FieldmapRegistry` is that pair as one object; :data:`REGISTRY`, the default of every
function, behaves as sdcflows' globals (fMRIPrep's subjects share it in one process).

Upstream behaviours kept (rule C, documented): SDC-18, :meth:`bidict.add` takes the *last* free
``auto_`` index in ``range(len(used) + 1)``, not the first (the names are in fMRIPrep's and
sdcflows' derivatives). Corrected by default (``upstream_defects="reproduce"`` gives sdcflows'):
SDC-20, :func:`get_identifier` names the unknown index table in its ``KeyError`` (sdcflows' raw
string gives the text ``'{by}'``); SDC-45, :class:`FieldmapFile` takes its entities from the
file's path from its participant's folder (sdcflows parses the absolute path, where pybids'
derivatives patterns ``den-``, ``res-``, ``desc-``, ... have no separator before them: below a
folder such as ``golden-clone`` or ``andres-lab`` every file gets ``den`` or ``res``, and the
fieldmap-less search queries the layout with them: ``ValueError: 'den' is not a recognized
entity``). Not reproduced (nondeterminism): the order of the common
``B0FieldIdentifier`` values in ``FieldmapEstimation``'s "Multiple common" message (a Python
``set``'s order in sdcflows; sorted here).
"""

from __future__ import annotations

import json
import logging
import re
from collections import defaultdict
from collections.abc import Iterable, Mapping
from contextlib import suppress
from enum import Enum, auto
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "MODALITIES",
    "REGISTRY",
    "SDCFLOWS_VERSIONS",
    "EstimatorRegistry",
    "EstimatorType",
    "FieldmapEstimation",
    "FieldmapFile",
    "FieldmapRegistry",
    "ImageFileError",
    "MetadataError",
    "bidict",
    "clear_registry",
    "create_logger",
    "get_identifier",
    "relative_to_root",
]

#: The sdcflows versions whose ``FieldmapFile`` path check can be chosen.
SDCFLOWS_VERSIONS = ("2.17.1", "2.15")


def create_logger(name: str, level: int = 40) -> logging.Logger:
    """sdcflows' ``create_logger``: a logger at ``level`` (``ERROR``) with its own handler, so
    the estimators' warnings and debug messages are silent unless the level is lowered."""
    logger = logging.getLogger(name)
    logger.setLevel(level)
    logger.handlers.clear()
    handler = logging.StreamHandler()
    handler.setLevel(level)
    handler.setFormatter(logging.Formatter("[%(name)s - %(levelname)s]: %(message)s"))
    logger.addHandler(handler)
    return logger


logger = create_logger("larmorx.sdcflows.fieldmaps")


class MetadataError(ValueError):
    """Missing metadata that an estimation needs (sdcflows' ``MetadataError``)."""


class ImageFileError(Exception):
    """An image file whose header cannot be read (nibabel's ``ImageFileError``, which is not a
    ``ValueError``: sdcflows' readout-time checks do not swallow it)."""


class EstimatorType(Enum):
    """The field-map estimation strategies (sdcflows' ``EstimatorType``)."""

    UNKNOWN = auto()
    PEPOLAR = auto()
    PHASEDIFF = auto()
    MAPPED = auto()
    ANAT = auto()


#: The estimation strategy each BIDS suffix belongs to.
MODALITIES: dict[str, EstimatorType | None] = {
    "bold": EstimatorType.PEPOLAR,
    "dwi": EstimatorType.PEPOLAR,
    "epi": EstimatorType.PEPOLAR,
    "asl": EstimatorType.PEPOLAR,
    "m0scan": EstimatorType.PEPOLAR,
    "fieldmap": EstimatorType.MAPPED,
    "magnitude": None,
    "magnitude1": None,
    "magnitude2": None,
    "phase1": EstimatorType.PHASEDIFF,
    "phase2": EstimatorType.PHASEDIFF,
    "phasediff": EstimatorType.PHASEDIFF,
    "sbref": EstimatorType.PEPOLAR,
    "T1w": EstimatorType.ANAT,
    "T2w": EstimatorType.ANAT,
}

_autokey_pat = re.compile(r"^auto_(\d+)$")


def _listify(obj: Any) -> Any:
    # pybids' listify
    return obj if isinstance(obj, (list, tuple, type(None))) else [obj]


class bidict(dict):
    """A bidirectional dictionary: values are keys of the inverse map (sdcflows' ``bidict``)."""

    _inverse: dict[Any, Any]

    def __init__(self, *args: Any, **kwargs: Any):
        super().__init__(*args, **kwargs)
        self._inverse = {v: k for k, v in self.items()}
        if len(self) != len(self._inverse):
            raise TypeError("Bidirectional dictionary cannot contain repeated values")

    def __setitem__(self, key: Any, value: Any) -> None:
        if key == value:
            raise TypeError(f"'{key}' <> '{value}' is a self-mapping")
        try:
            hash(value)
        except TypeError as exc:
            raise TypeError(f"value '{value}' of {exc}") from None
        try:
            hash(key)
        except TypeError as exc:
            raise TypeError(f"key '{key}' of {exc}") from None
        if self.__contains__(key):
            raise KeyError(f"'{key}' is already {'a value' * (key in self._inverse)} in mapping")
        if self.__contains__(value):
            raise ValueError(
                f"'{value}' is already {'a key' * (value not in self._inverse)} in mapping"
            )
        super().__setitem__(key, value)
        self._inverse[value] = key

    def __delitem__(self, key: Any) -> None:
        if not self.__contains__(key):
            raise KeyError(f"'{key}")
        if super().__contains__(key):
            del self._inverse[super().__getitem__(key)]
            super().__delitem__(key)
        else:
            super().__delitem__(self._inverse[key])
            del self._inverse[key]

    def __getitem__(self, key: Any) -> Any:
        if key in self._inverse:
            return self._inverse[key]
        return super().__getitem__(key)

    def __contains__(self, key: object) -> bool:
        if super().__contains__(key):
            return True
        return key in self._inverse

    def add(self, value: Any) -> str:
        """Insert ``value`` under an automatic key ``auto_NNNNN``: the last index of
        ``range(len(used) + 1)`` not used yet (SDC-18, kept)."""
        used = {
            int(i.groups()[0])
            for i in [_autokey_pat.match(k) for k in self.keys() if k.startswith("auto_")]
            if i is not None
        }
        newkey = ""
        for i in range(len(used) + 1):
            if i not in used:
                newkey = f"auto_{i:05d}"
        self.__setitem__(newkey, value)
        return newkey

    def clear(self) -> None:
        """Remove every pair."""
        self._inverse.clear()
        super().clear()


class EstimatorRegistry(bidict):
    """The estimators by name: each name maps to its sorted tuple of source paths."""

    @property
    def sources(self) -> list[str]:
        """Every source path of every estimator, sorted."""
        return sorted({el for group in self.values() for el in group})

    def get_key(self, value: Any) -> tuple[str, ...]:
        """The names of the estimators that have ``value`` among their sources."""
        if value not in self.sources:
            return ()
        return tuple(sorted(k for k, v in self.items() if value in v))


class FieldmapRegistry:
    """sdcflows' two registries as one object: ``estimators`` (an :class:`EstimatorRegistry`,
    name to sources) and ``intents`` (each ``IntendedFor`` path to the estimator names
    intended for it)."""

    def __init__(self) -> None:
        self.estimators = EstimatorRegistry()
        self.intents: defaultdict[str, set[str]] = defaultdict(set)

    def clear(self) -> None:
        """Empty both registries (sdcflows' ``clear_registry``)."""
        self.estimators.clear()
        self.intents.clear()


#: The registry every function uses by default: sdcflows' module globals.
REGISTRY = FieldmapRegistry()


def _registry(registry: FieldmapRegistry | None) -> FieldmapRegistry:
    return REGISTRY if registry is None else registry


def relative_to_root(path: Any) -> Path:
    """niworkflows' ``relative_to_root``: the path from the nearest ``sub-*`` folder above a file
    named ``sub-*`` (a ``ValueError`` for other names)."""
    path = Path(path)
    if path.name.startswith("sub-"):
        parents = [path.name]
        for p in path.parents:
            parents.insert(0, p.name)
            if p.name.startswith("sub-"):
                return Path(*parents)
        return path
    raise ValueError(
        f"Could not determine the BIDS root of <{path}>. "
        "Only files under a subject directory are currently supported."
    )


def _bids_part(path: Path) -> str | None:
    """SDC-45: a file's BIDS part, ``/``-rooted: its path from the nearest ``sub-*`` folder above
    it (as niworkflows' :func:`relative_to_root` and ``DerivativesDataSink`` take it); ``None``
    when no such folder is above it (where the dataset begins is not known)."""
    parts = path.parts
    for i in range(len(parts) - 2, -1, -1):
        if parts[i].startswith("sub-"):
            return "/" + "/".join(parts[i:])
    return None


def _get_trt(metadata: Mapping[str, Any], in_file: Any, use_estimate: bool = False) -> Any:
    """sdcflows' ``get_trt`` with nibabel's error for an unreadable header (not a
    ``ValueError``)."""
    from larmorx.io.nifti import NiftiError
    from larmorx.sdcflows.pepolar import get_trt

    try:
        return get_trt(metadata, in_file=in_file, use_estimate=use_estimate)
    except FileNotFoundError as err:  # nibabel's message
        raise FileNotFoundError(f"No such file or no access: '{in_file}'") from err
    except NiftiError as err:
        if Path(in_file).is_file() and Path(in_file).stat().st_size <= 0:
            raise ImageFileError(f"Empty file: '{in_file}'") from err
        raise ImageFileError(str(err)) from err


class FieldmapFile:
    """A file that can take part in a field-map estimation (sdcflows' ``FieldmapFile``): its
    path, BIDS entities, suffix, the BIDS root above it and its metadata (the sidecar beside it,
    updated with ``metadata``), checked for what its suffix requires.

    Parameters
    ----------
    path
        A NIfTI file whose suffix is one of :data:`MODALITIES`.
    metadata
        Metadata that override the sidecar's (the same dictionary object is kept when no
        sidecar is read, as sdcflows keeps it).
    find_meta
        Read the sidecar beside the file (``<name>.json``; no BIDS inheritance).
    sdcflows_version
        ``"2.17.1"`` (default: a broken link is accepted) or ``"2.15"`` (fMRIPrep 25.2.6's: it
        is refused).
    upstream_defects
        ``"fix"`` (SDC-45: the entities from the file's path from its participant's folder; a
        file below no ``sub-*`` folder keeps sdcflows' parse) or ``"reproduce"`` (sdcflows': from
        the path as given, the folders above the dataset included).
    """

    __slots__ = ("bids_root", "entities", "find_meta", "metadata", "path", "suffix")

    def __init__(
        self,
        path: Any,
        *,
        metadata: Any = None,
        find_meta: bool = True,
        sdcflows_version: str = "2.17.1",
        upstream_defects: UpstreamDefectsName = "fix",
    ):
        if sdcflows_version not in SDCFLOWS_VERSIONS:
            raise ValueError(
                f"sdcflows_version must be one of {SDCFLOWS_VERSIONS}, not {sdcflows_version!r}"
            )
        mode = _defects.check(upstream_defects)
        self.path = Path(path)
        self.metadata = {} if metadata is None else metadata
        self.find_meta = find_meta
        self.bids_root: Path | None = None
        self._check_path(self.path, sdcflows_version)
        self._post_init(mode)

    @staticmethod
    def _check_path(value: Path, sdcflows_version: str) -> None:
        if sdcflows_version == "2.15":
            if not value.is_file():
                raise FileNotFoundError(
                    f"File path <{value}> does not exist, is a broken link, or it is not a file"
                )
        elif not value.is_file() and not value.is_symlink():
            raise FileNotFoundError(f"File path <{value}> does not exist, or it is not a file")
        if not value.name.endswith((".nii", ".nii.gz")):
            raise ValueError(f"File path <{value}> does not look like a NIfTI file.")
        suffix = re.search(r"(?<=_)\w+(?=\.nii)", value.name).group()  # type: ignore[union-attr]
        if suffix not in tuple(MODALITIES.keys()):
            raise ValueError(
                f"File path <{value}> with suffix <{suffix}> is not a valid fieldmap sourcefile."
            )

    def _post_init(self, mode: str) -> None:
        from larmorx.bids import parse_file_entities

        whole = parse_file_entities(str(self.path))  # sdcflows: the path as given
        part = _bids_part(self.path)
        below = whole if part is None else parse_file_entities(part)
        self.entities = whole if mode == "reproduce" else below
        if part is not None and whole != below:
            extra = {k: v for k, v in whole.items() if below.get(k, None) != v}
            _defects.warn(
                [
                    (
                        "SDC-45",
                        "sdcflows",
                        mode,
                        f"the folders above <{part.lstrip('/')}> give it the entities {extra} "
                        "(sdcflows parses the absolute path)",
                    )
                ],
                stacklevel=4,
            )
        self.suffix = self.entities.pop("suffix")
        extension = self.entities.pop("extension").lstrip(".")
        if self.find_meta:
            sidecar = Path(str(self.path).replace(extension, "json"))
            if sidecar.is_file():
                _meta = self.metadata or {}
                self.metadata = json.loads(sidecar.read_text(encoding="utf-8"))
                self.metadata.update(_meta)
        relative_path = relative_to_root(self.path)
        self.bids_root = (
            Path(str(self.path)[: -len(str(relative_path))])
            if str(relative_path) != str(self.path)
            else None
        )
        if self.suffix in ("T1w", "T2w"):
            self.metadata["TotalReadoutTime"] = 0.0
        if self.suffix in ("bold", "dwi", "epi", "sbref", "asl", "m0scan"):
            if "PhaseEncodingDirection" not in self.metadata:
                raise MetadataError(f"Missing 'PhaseEncodingDirection' for <{self.path}>.")
            msg = "Missing readout timing information for <%s>. %s"
            extra = "Explicit fallback must be provided."
            have_trt = False
            try:
                _get_trt(self.metadata, self.path)
                have_trt = True
            except ValueError:
                with suppress(ValueError):
                    _get_trt(self.metadata, self.path, use_estimate=True)
                    extra = "Estimated timing is available."
            if not have_trt:
                logger.warning(msg, self.path, extra)
        elif self.suffix == "fieldmap" and "Units" not in self.metadata:
            raise MetadataError(f"Missing 'Units' for <{self.path}>.")
        elif self.suffix == "phasediff" and (
            "EchoTime1" not in self.metadata or "EchoTime2" not in self.metadata
        ):
            raise MetadataError(f"Missing 'EchoTime1' and/or 'EchoTime2' for <{self.path}>.")
        elif self.suffix in ("phase1", "phase2") and ("EchoTime" not in self.metadata):
            raise MetadataError(f"Missing 'EchoTime' for <{self.path}>.")

    def __repr__(self) -> str:
        return f"FieldmapFile(path={self.path}, metadata={self.metadata!r})"

    def _key(self) -> tuple[Any, ...]:
        return (
            self.path,
            self.entities,
            self.suffix,
            self.bids_root,
            self.metadata,
            self.find_meta,
        )

    def __eq__(self, other: object) -> bool:
        if other.__class__ is not self.__class__:
            return NotImplemented
        return self._key() == other._key()  # type: ignore[attr-defined]

    __hash__ = None  # type: ignore[assignment]  (attrs: eq without frozen)


class FieldmapEstimation:
    """A field-map estimation (sdcflows' ``FieldmapEstimation``): its sources, the strategy they
    give (:class:`EstimatorType`: missing phase or magnitude files found by name), its
    identifier (the sources' common ``B0FieldIdentifier``, or an automatic ``auto_NNNNN``) and
    its registration in ``registry`` (by default :data:`REGISTRY`).

    Parameters
    ----------
    sources
        :class:`FieldmapFile` objects or paths (made into :class:`FieldmapFile`).
    bids_id
        The identifier; by default the sources' ``B0FieldIdentifier``, else automatic.
    registry
        The registry it is recorded in.
    sdcflows_version
        For the :class:`FieldmapFile` objects it makes.
    upstream_defects
        For the :class:`FieldmapFile` objects it makes (SDC-45).
    """

    __slots__ = ("_method", "_wf", "bids_id", "registry", "sanitized_id", "sources")

    def __init__(
        self,
        sources: Any = None,
        *,
        bids_id: str | None = None,
        registry: FieldmapRegistry | None = None,
        sdcflows_version: str = "2.17.1",
        upstream_defects: UpstreamDefectsName = "fix",
    ):
        mode = _defects.check(upstream_defects)
        self.sources = [
            FieldmapFile(f, sdcflows_version=sdcflows_version, upstream_defects=mode)
            if not isinstance(f, FieldmapFile)
            else f
            for f in _listify(sources)  # type: ignore[union-attr]
        ]
        object.__setattr__(self, "_method", EstimatorType.UNKNOWN)
        object.__setattr__(self, "_wf", None)
        object.__setattr__(self, "bids_id", bids_id)
        self.registry = _registry(registry)
        self._post_init(sdcflows_version, mode)

    # attrs' on_setattr hooks: the method cannot change once set, the identifier never
    @property
    def method(self) -> EstimatorType:
        """The estimation strategy."""
        return self._method

    @method.setter
    def method(self, value: EstimatorType) -> None:
        if self._method == value:
            return
        if self._method != EstimatorType.UNKNOWN:
            raise TypeError(f"Cannot change determined method {self._method} to {value}.")
        if value not in (
            EstimatorType.PEPOLAR,
            EstimatorType.PHASEDIFF,
            EstimatorType.MAPPED,
            EstimatorType.ANAT,
        ):
            raise ValueError(f"Invalid estimation method type {value}.")
        object.__setattr__(self, "_method", value)

    def __setattr__(self, name: str, value: Any) -> None:
        if name == "bids_id":
            if self.bids_id and self.bids_id == value:
                return
            raise ValueError("Cannot edit the bids_id")
        object.__setattr__(self, name, value)

    def _post_init(self, sdcflows_version: str, mode: str) -> None:
        reg = self.registry
        suffix_list = [f.suffix for f in self.sources]
        suffix_set = set(suffix_list)

        fmap_types = suffix_set.intersection(("fieldmap", "phasediff", "phase1", "phase2"))
        if len(fmap_types) > 1 and fmap_types - {"phase1", "phase2"}:
            raise TypeError(f"Incompatible suffices found: <{','.join(sorted(fmap_types))}>.")

        if fmap_types:
            sources = sorted(
                f.path
                for f in self.sources
                if f.suffix in ("fieldmap", "phasediff", "phase1", "phase2")
            )
            missing_phases = ("phase1" not in fmap_types, "phase2" not in fmap_types)
            if sum(missing_phases) == 1:
                mis_ph = "phase1" if missing_phases[0] else "phase2"
                hit_ph = "phase2" if missing_phases[0] else "phase1"
                new_source = sources[0].parent / sources[0].name.replace(hit_ph, mis_ph)
                self.sources.append(
                    FieldmapFile(
                        new_source, sdcflows_version=sdcflows_version, upstream_defects=mode
                    )
                )
                sources.insert(int(missing_phases[1]), new_source)

            self.method = MODALITIES[sorted(fmap_types)[0]]  # type: ignore[assignment]

            magnitude = f"magnitude{'' if self.method == EstimatorType.MAPPED else '1'}"
            if magnitude not in suffix_set:
                try:
                    new_path = sources[0].parent / sources[0].name.replace(
                        "_fieldmap", "_magnitude"
                    ).replace("_phasediff", "_phase1").replace("_phase", "_magnitude")
                    self.sources.append(
                        FieldmapFile(
                            new_path, sdcflows_version=sdcflows_version, upstream_defects=mode
                        )
                    )
                except Exception:
                    raise ValueError(
                        "A fieldmap or phase-difference estimation type was found, "
                        f"but an anatomical reference ({magnitude} file) is missing."
                    ) from None

            if self.method == EstimatorType.PHASEDIFF and "magnitude2" not in suffix_set:
                try:
                    new_path = sources[-1].parent / sources[-1].name.replace("diff", "2").replace(
                        "phase", "magnitude"
                    )
                    self.sources.append(
                        FieldmapFile(
                            new_path, sdcflows_version=sdcflows_version, upstream_defects=mode
                        )
                    )
                except Exception:
                    if "phase2" in suffix_set:
                        raise ValueError(
                            "A phase-difference estimation (phase1/2) type was found, "
                            "but an anatomical reference (magnitude2 file) is missing."
                        ) from None

        pepolar_types = suffix_set.intersection(("bold", "dwi", "epi", "sbref", "asl", "m0scan"))
        anat_types = suffix_set.intersection(("T1w", "T2w"))
        _pepolar_estimation = (
            len([f for f in suffix_list if f in ("bold", "dwi", "epi", "sbref", "asl", "m0scan")])
            > 1
        )
        if _pepolar_estimation and not anat_types:
            self.method = MODALITIES[sorted(pepolar_types)[0]]  # type: ignore[assignment]
            _pe = {f.metadata["PhaseEncodingDirection"] for f in self.sources}
            if len(_pe) == 1:
                raise ValueError(
                    f"Only one phase-encoding direction <{_pe.pop()}> found across sources."
                )
        elif anat_types:
            self.method = MODALITIES[sorted(anat_types)[0]]  # type: ignore[assignment]
            if not pepolar_types:
                raise ValueError("Only anatomical sources were found, cannot estimate fieldmap.")

        if self.method == EstimatorType.UNKNOWN:
            raise ValueError("Insufficient sources to estimate a fieldmap.")

        intents_meta = {
            el for f in self.sources for el in _listify(f.metadata.get("IntendedFor") or [])
        }

        if not self.bids_id:
            b0_ids = [
                _listify(f.metadata.get("B0FieldIdentifier"))
                for f in self.sources
                if f.metadata.get("B0FieldIdentifier")
            ]
            if b0_ids:
                bids_ids = set(b0_ids[0]).intersection(*b0_ids[1:])
                if not bids_ids:
                    raise ValueError(
                        f"No common ``B0FieldIdentifier`` found: <{', '.join(map(str, b0_ids))}>"
                    )
                elif len(bids_ids) > 1:
                    raise ValueError(
                        "Multiple common ``B0FieldIdentifier``s found: "
                        f"<{', '.join(sorted(bids_ids))}>"
                    )
                object.__setattr__(self, "bids_id", bids_ids.pop())

        if self.bids_id:
            reg.estimators[self.bids_id] = self.paths()
        else:
            object.__setattr__(self, "bids_id", reg.estimators.add(self.paths()))

        for intent_file in intents_meta:
            reg.intents[intent_file].add(self.bids_id)  # type: ignore[arg-type]

        object.__setattr__(self, "sanitized_id", re.sub(r"[^a-zA-Z0-9]", "_", self.bids_id))

    def paths(self) -> tuple[str, ...]:
        """The sources' paths, sorted."""
        return tuple(sorted(str(f.path) for f in self.sources))

    def get_workflow(self, set_inputs: bool = True, **kwargs: Any) -> Any:
        """The estimator's workflow (sdcflows' ``get_workflow``):
        :func:`larmorx.sdcflows.estimator_workflow`."""
        from larmorx.sdcflows.base import estimator_workflow

        return estimator_workflow(self, set_inputs, **kwargs)

    def __repr__(self) -> str:
        return (
            f"FieldmapEstimation(sources=<{len(self.sources)} files>, method={self.method!r}, "
            f"bids_id={self.bids_id!r})"
        )


def get_identifier(
    filename: str,
    by: str = "IntendedFor",
    *,
    registry: FieldmapRegistry | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
) -> tuple[str, ...]:
    """The estimator names for ``filename``: those intended for it (``by='IntendedFor'``, the
    path as written in ``IntendedFor``) or those that have it among their sources
    (``'sources'``). Another ``by`` is a ``KeyError`` naming it (SDC-20: sdcflows' message is
    the literal text ``'{by}'``, given with ``upstream_defects="reproduce"``)."""
    _defects.check(upstream_defects)
    reg = _registry(registry)
    if by == "IntendedFor":
        return tuple(sorted(reg.intents.get(filename, ())))
    if by == "sources":
        return reg.estimators.get_key(filename)
    if upstream_defects == "reproduce":
        raise KeyError(r"'{by}'")
    raise KeyError(f"'{by}'")


def clear_registry(registry: FieldmapRegistry | None = None) -> None:
    """Empty the registries (by default :data:`REGISTRY`, sdcflows' globals)."""
    _registry(registry).clear()


def _paths(files: Iterable[Any]) -> list[str]:
    return [str(getattr(f, "path", f)) for f in files]
