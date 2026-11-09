# SPDX-License-Identifier: Apache-2.0
"""sdcflows' search for field-map estimators in a BIDS dataset (``lx.sdcflows``).

Ported from sdcflows 2.17.1 and 2.15.2 (Apache-2.0, the NiPreps developers): ``utils/wrangler.py``
(``find_estimators``, ``find_anatomical_estimators`` and their helpers). The dataset is a
:class:`larmorx.bids.BIDSLayout` (pybids' index without its database); the estimators are
:class:`larmorx.sdcflows.FieldmapEstimation` objects recorded in a
:class:`larmorx.sdcflows.FieldmapRegistry` (by default sdcflows' globals).

The search, in sdcflows' order: (1) the files that carry ``B0FieldIdentifier`` metadata, one
estimator per identifier; if none, (2) heuristics: every ``fieldmap``, ``phasediff`` and
``phase1`` file; EPI field maps in two or more phase-encoding directions, per session,
acquisition and contrast agent, grouped by their ``IntendedFor``; single-direction EPI field
maps with ``IntendedFor`` paired with each target (its single-band reference when there is
one); then (3) the fieldmap-less (SyN) estimators of the anatomical reference with every EPI
run (its single-band reference first, the shortest echo first, every echo intended), when
asked for (``fmapless``) and either nothing else was found or ``force_fmapless``.

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows 2.17.1's search exactly;
:class:`~larmorx.UpstreamDefectWarning` when the correction changed what was found):

- SDC-17 (rule A, upstream's own correction): sdcflows 2.17.1 lacks three fixes sdcflows 2.15.2
  made (published after 2.17.1): ``bids_filters`` restrict the field maps but no longer the
  anatomical reference and the targets of the fieldmap-less estimation (``session`` excepted);
  a sessionless anatomical reference serves the sessions when they have none; and the
  ``acquisition`` and ``ceagent`` filters apply to the EPI heuristics (2.17.1 looks up
  ``'acquisitions'``, a key no filter has, and so drops them). By default the search is
  2.15.2's.
- SDC-21 (rule A): the query for files whose ``B0FieldIdentifier`` is a list runs every filter
  as a case-insensitive regular expression: subject ``01`` also matches ``010`` and ``101``,
  and the identifier ``pe`` matches ``"PE"`` and ``"pe2"`` written in other files' lists, so
  their files join the estimator. By default the subject and the other filters match exactly
  and the identifier is a member of the list.

Not reproduced (nondeterminism, SDC-19): sdcflows takes the identifiers of step (1) from a
Python ``set``, so the estimators' order changes between processes; here they follow pybids'
natural order of the identifiers, in both modes.

On Windows the paths written into ``IntendedFor`` use ``/``, as on Linux (sdcflows writes the
platform's separator, which fMRIPrep's lookup does not match).
"""

from __future__ import annotations

import logging
import warnings
from collections.abc import Iterable, Mapping
from contextlib import suppress
from itertools import product
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.sdcflows import estimators as fm
from larmorx.sdcflows.estimators import (
    FieldmapEstimation,
    FieldmapFile,
    FieldmapRegistry,
    _get_trt,
    _listify,
    create_logger,
)

__all__ = ["find_anatomical_estimators", "find_estimators"]

_TOOL = "sdcflows"


def _normalize_intent(intent: str, subject: str) -> str | None:
    """A BIDS URI ``bids::sub-<subject>/...`` as a subject-relative path."""
    if intent.startswith("bids::"):
        return intent[11 + len(subject) :]
    return intent


def _resolve_intent(intent: str, layout: Any, subject: str) -> str | None:
    root = Path(layout.root)
    if intent.startswith("bids::"):
        return str(root / intent[6:])
    if not intent.startswith("bids:"):
        return str(root / f"sub-{subject}" / intent)
    return intent


def _filter_metadata(metadata: Mapping[str, Any], subject: str) -> Any:
    intents = metadata.get("IntendedFor")
    if intents:
        updated = [_normalize_intent(intent, subject) for intent in _listify(intents)]
        return {**metadata, "IntendedFor": updated}
    return metadata


def _rel(path: Any, root: Path) -> str:
    # sdcflows: str(Path(path).relative_to(root)) (the platform's separator); "/" everywhere
    return Path(path).relative_to(root).as_posix()


def _warn(entry: str, detail: str) -> None:
    warnings.warn(_defects.UpstreamDefectWarning(entry, _TOOL, "fix", detail), stacklevel=3)


def _b0_ids(layout: Any, base_entities: Mapping[str, Any]) -> list[str]:
    from larmorx.bids import BIDSEntityError, natural_sort

    ids: dict[str, None] = {}
    with suppress(BIDSEntityError):
        for entry in layout.get_B0FieldIdentifiers(**base_entities):
            for i in _listify(entry):
                ids.setdefault(i, None)
    return natural_sort(sorted(ids))


def _listed_ids(layout: Any, base_entities: Mapping[str, Any], b0_id: str, mode: str) -> list[Any]:
    """The files whose ``B0FieldIdentifier`` is a list holding ``b0_id`` (SDC-21)."""
    regex = layout.get(**base_entities, B0FieldIdentifier=f'"{b0_id}"', regex_search=True)
    if mode == "reproduce":
        return regex  # type: ignore[no-any-return]
    exact = []
    for f in layout.get(**base_entities):
        ids = f.get_metadata().get("B0FieldIdentifier")
        if isinstance(ids, list) and b0_id in ids:
            exact.append(f)
    extra = [f.relpath for f in regex if f not in exact]
    missing = [f.relpath for f in exact if f not in regex]
    if extra or missing:
        _warn(
            "SDC-21",
            f"B0FieldIdentifier {b0_id!r}: the files that list it are "
            f"{[f.relpath for f in exact]} (sdcflows' regular-expression query also takes "
            f"{extra} and misses {missing})",
        )
    return exact


def _acq_ce(
    layout: Any, base_entities: Mapping[str, Any], subject: str, version: str
) -> tuple[list[Any], list[Any]]:
    from larmorx.bids import listify

    if version == "2.17.1":
        acqs = base_entities.get("acquisitions") or [
            *layout.get_acquisitions(subject=subject, suffix="epi"),
            None,
        ]
        contrasts = base_entities.get("ceagent") or [
            *layout.get_ceagents(subject=subject, suffix="epi"),
            None,
        ]
        return acqs, contrasts
    acqs = (
        listify(base_entities["acquisition"])
        if "acquisition" in base_entities
        else [*layout.get_acquisitions(subject=subject, suffix="epi"), None]
    )
    contrasts = (
        listify(base_entities["ceagent"])
        if "ceagent" in base_entities
        else [*layout.get_ceagents(subject=subject, suffix="epi"), None]
    )
    return acqs, contrasts


def _pepolar_collections(
    layout: Any,
    base_entities: Mapping[str, Any],
    sessions: list[Any],
    acqs: list[Any],
    contrasts: list[Any],
    subject: str,
    sdcflows_version: str,
) -> list[list[FieldmapFile]]:
    """Step 2's EPI field maps in two or more directions, grouped by ``IntendedFor``."""
    out = []
    for ses, acq, ce in product(sessions, acqs, contrasts):
        entities = dict(base_entities)
        entities.update({"suffix": "epi", "session": ses, "acquisition": acq, "ceagent": ce})
        dirs = layout.get_directions(**entities)
        if len(dirs) > 1:
            by_intent: dict[tuple[Any, ...], list[FieldmapFile]] = {}
            for fmap in layout.get(**{**entities, **{"direction": dirs}}):
                fmapfile = FieldmapFile(
                    fmap.path,
                    metadata=_filter_metadata(fmap.get_metadata(), subject),
                    sdcflows_version=sdcflows_version,
                )
                by_intent.setdefault(tuple(fmapfile.metadata.get("IntendedFor", ())), []).append(
                    fmapfile
                )
            out.extend(by_intent.values())
    return out


def _anat_query(
    layout: Any,
    entities: Mapping[str, Any],
    anat_suffix: Any,
    sessions: list[Any],
    version: str,
) -> list[Any]:
    from larmorx.bids import Query

    anat_file = layout.get(**{**entities, **{"suffix": anat_suffix, "session": sessions}})
    if version != "2.17.1" and not anat_file:
        anat_file = layout.get(**{**entities, **{"suffix": anat_suffix, "session": Query.NONE}})
    return anat_file  # type: ignore[no-any-return]


def find_estimators(
    *,
    layout: Any,
    subject: str,
    sessions: list[str] | None = None,
    fmapless: bool | Iterable[str] = True,
    force_fmapless: bool = False,
    logger: logging.Logger | None = None,
    bids_filters: Mapping[str, Any] | None = None,
    anat_suffix: str | list[str] = "T1w",
    registry: FieldmapRegistry | None = None,
    sdcflows_version: str = "2.17.1",
    upstream_defects: UpstreamDefectsName = "fix",
) -> list[FieldmapEstimation]:
    """sdcflows' ``find_estimators``: the field-map estimators of one subject in a BIDS dataset.

    Parameters
    ----------
    layout
        A :class:`larmorx.bids.BIDSLayout`.
    subject
        The participant label (without ``sub-``).
    sessions
        Session labels; by default every session of the subject (or none).
    fmapless
        Also find fieldmap-less (SyN) estimators: ``True`` (BOLD, DWI and ASL runs), a set of
        suffixes (``"bold"``, ``"dwi"``, ``"asl"``) or ``False``.
    force_fmapless
        Find them even when other estimators were found.
    logger
        Where the debug messages go (by default a silent ``ERROR``-level logger, as sdcflows').
    bids_filters
        Entities that restrict the files (``session`` among them sets ``sessions``).
    anat_suffix
        The anatomical reference's suffix(es).
    registry
        Where the estimators are recorded (by default :data:`larmorx.sdcflows.REGISTRY`).
    sdcflows_version
        ``"2.17.1"`` or ``"2.15"``, for the :class:`FieldmapFile` objects made.
    upstream_defects
        ``"fix"`` (SDC-17: sdcflows 2.15.2's search; SDC-21) or ``"reproduce"`` (sdcflows
        2.17.1's).

    Returns
    -------
    list of FieldmapEstimation
        The estimators that could be built, in sdcflows' order.
    """
    from larmorx.bids import Query

    mode = _defects.check(upstream_defects)
    version = "2.17.1" if mode == "reproduce" else "2.15.2"
    reg = fm._registry(registry)
    logger = logger or create_logger("larmorx.sdcflows.wrangler")

    base_entities: dict[str, Any] = {
        "subject": subject,
        "extension": [".nii", ".nii.gz"],
        "part": ["mag", None],
        "scope": "raw",  # Ensure derivatives are not captured
    }
    no_filter_entities = dict(base_entities)

    if bids_filters:
        filters = dict(bids_filters)
        if "session" in filters:
            if sessions is not None:
                raise ValueError("Filters include session, but session is already defined.")
            sessions = _listify(filters.pop("session"))
        base_entities.update(filters)

    subject_root = Path(layout.root) / f"sub-{subject}"
    sessions = sessions or layout.get_sessions(subject=subject) or [None]
    fmapless = fmapless or {}
    if fmapless is True:
        fmapless = {"bold", "dwi", "asl"}

    estimators: list[FieldmapEstimation] = []

    def _estimation(sources: Any, **kwargs: Any) -> FieldmapEstimation:
        return FieldmapEstimation(
            sources, registry=reg, sdcflows_version=sdcflows_version, **kwargs
        )

    # Step 1. Use B0FieldIdentifier metadata
    b0_ids = _b0_ids(layout, base_entities)
    if b0_ids:
        logger.debug(
            "Dataset includes `B0FieldIdentifier` metadata."
            "Any data missing this metadata will be ignored."
        )
        for b0_id in b0_ids:
            bare_ids = layout.get(**base_entities, B0FieldIdentifier=b0_id)
            listed_ids = _listed_ids(layout, base_entities, b0_id, mode)
            try:
                e = _estimation(
                    [
                        FieldmapFile(
                            fmap.path,
                            metadata=fmap.get_metadata(),
                            sdcflows_version=sdcflows_version,
                        )
                        for fmap in bare_ids + listed_ids
                    ],
                    bids_id=b0_id,
                )
            except (ValueError, TypeError) as err:
                _log_debug_estimator_fail(
                    logger, b0_id, bare_ids + listed_ids, layout.root, str(err)
                )
            else:
                _log_debug_estimation(logger, e, layout.root)
                estimators.append(e)

    # Step 2. If no B0FieldIdentifiers were found, try several heuristics
    if not estimators:
        for fmap in layout.get(
            **{
                **base_entities,
                **{"suffix": ["fieldmap", "phasediff", "phase1"], "session": sessions},
            }
        ):
            try:
                e = _estimation(
                    FieldmapFile(
                        fmap.path,
                        metadata=_filter_metadata(fmap.get_metadata(), subject),
                        sdcflows_version=sdcflows_version,
                    )
                )
            except (ValueError, TypeError) as err:
                _log_debug_estimator_fail(logger, "unnamed fieldmap", [fmap], layout.root, str(err))
            else:
                _log_debug_estimation(logger, e, layout.root)
                estimators.append(e)

        acqs, contrasts = _acq_ce(layout, base_entities, subject, version)
        collections = _pepolar_collections(
            layout, base_entities, sessions, acqs, contrasts, subject, sdcflows_version
        )
        if mode == "fix":
            other = _acq_ce(layout, base_entities, subject, "2.17.1")
            if other != (acqs, contrasts):
                try:
                    alt = _pepolar_collections(
                        layout, base_entities, sessions, *other, subject, sdcflows_version
                    )
                except Exception:
                    alt = None
                if alt is None or [fm._paths(c) for c in alt] != [
                    fm._paths(c) for c in collections
                ]:
                    _warn(
                        "SDC-17",
                        "the acquisition/ceagent filters restrict the EPI field maps (sdcflows "
                        f"2.17.1 searched acquisitions {other[0]} and agents {other[1]} instead of "
                        f"{acqs} and {contrasts})",
                    )
        for collection in collections:
            try:
                e = _estimation(collection)
            except (ValueError, TypeError) as err:
                _log_debug_estimator_fail(
                    logger, "unnamed PEPOLAR", collection, layout.root, str(err)
                )
            else:
                _log_debug_estimation(logger, e, layout.root)
                estimators.append(e)

        # At this point, only single-PE _epi files WITH ``IntendedFor`` can
        # be automatically processed.
        has_intended: Any = ()
        with suppress(ValueError):
            has_intended = layout.get(
                **{
                    **base_entities,
                    **{"suffix": "epi", "IntendedFor": Query.REQUIRED, "session": sessions},
                }
            )
        for epi_fmap in has_intended:
            if epi_fmap.path in reg.estimators.sources:
                logger.debug("Skipping fieldmap %s (already in use)", epi_fmap.relpath)
                continue
            logger.debug("Found single PE fieldmap %s", epi_fmap.relpath)
            epi_base_md = epi_fmap.get_metadata()
            all_targets = []
            for intent in _listify(epi_base_md["IntendedFor"]):
                target = layout.get_file(_resolve_intent(intent, layout, subject))
                if target is None:
                    logger.debug("Single PE target %s not found", intent)
                    continue
                all_targets.append(target)
            sbrefs = [target for target in all_targets if target.entities["suffix"] == "sbref"]
            if sbrefs:
                targets = sbrefs
                intent_map = []
                for sbref in sbrefs:
                    ents = sbref.get_entities(metadata=False)
                    ents["suffix"] = ["bold", "dwi", "asl"]
                    intent_map.append([t for t in layout.get(**ents) if t in all_targets])
            else:
                targets = all_targets
                intent_map = [[t] for t in all_targets]
            for target, intent in zip(targets, intent_map, strict=False):
                logger.debug("Found single PE target %s", target.relpath)
                estimator_md = epi_base_md.copy()
                estimator_md["IntendedFor"] = [_rel(pathlike, subject_root) for pathlike in intent]
                try:
                    e = _estimation(
                        [
                            FieldmapFile(
                                epi_fmap.path,
                                metadata=estimator_md,
                                sdcflows_version=sdcflows_version,
                            ),
                            FieldmapFile(
                                target.path,
                                metadata=target.get_metadata(),
                                sdcflows_version=sdcflows_version,
                            ),
                        ]
                    )
                except (ValueError, TypeError) as err:
                    _log_debug_estimator_fail(
                        logger, "unnamed PEPOLAR", [epi_fmap, target], layout.root, str(err)
                    )
                else:
                    _log_debug_estimation(logger, e, layout.root)
                    estimators.append(e)

    if estimators and not force_fmapless:
        fmapless = False

    # Find fieldmap-less schemes
    anat_entities = base_entities if version == "2.17.1" else no_filter_entities
    anat_file = _anat_query(layout, anat_entities, anat_suffix, sessions, version)
    if mode == "fix" and fmapless:
        try:
            old = _anat_query(layout, base_entities, anat_suffix, sessions, "2.17.1")
        except Exception:
            old = None
        if old is None or [f.path for f in old[:1]] != [f.path for f in anat_file[:1]]:
            _warn(
                "SDC-17",
                "the anatomical reference of the fieldmap-less estimation is "
                f"{[f.relpath for f in anat_file[:1]]} (sdcflows 2.17.1: "
                f"{'an error' if old is None else [f.relpath for f in old[:1]]}, filtered by "
                "bids_filters or without the sessionless fallback)",
            )
        elif anat_file and base_entities != no_filter_entities:
            specs = _anatomical_specs(
                anat_file[0], layout, subject, sessions, anat_entities, fmapless, sdcflows_version
            )
            try:
                old_specs: Any = _anatomical_specs(
                    anat_file[0],
                    layout,
                    subject,
                    sessions,
                    base_entities,
                    fmapless,
                    sdcflows_version,
                )
            except Exception:
                old_specs = None
            if old_specs is None or [fm._paths(s) for s in specs] != [
                fm._paths(s) for s in old_specs
            ]:
                _warn(
                    "SDC-17",
                    "bids_filters no longer restrict the targets of the fieldmap-less estimation "
                    f"({len(specs)} targets; sdcflows 2.17.1: "
                    f"{'an error' if old_specs is None else len(old_specs)})",
                )

    if not fmapless or not anat_file:
        logger.debug("Skipping fmap-less estimation")
        return estimators

    logger.debug("Attempting fmap-less estimation")
    estimator_specs = _anatomical_specs(
        anat_file[0], layout, subject, sessions, anat_entities, fmapless, sdcflows_version
    )
    for spec in estimator_specs:
        try:
            estimator = _estimation(spec)
        except (ValueError, TypeError) as err:
            _log_debug_estimator_fail(logger, "ANAT", spec, layout.root, str(err))
        else:
            _log_debug_estimation(logger, estimator, layout.root)
            estimators.append(estimator)
    return estimators


def _anatomical_specs(
    anat_file: Any,
    layout: Any,
    subject: str,
    sessions: list[Any],
    base_entities: Mapping[str, Any],
    suffixes: Any,
    sdcflows_version: str,
) -> list[list[FieldmapFile]]:
    return find_anatomical_estimators(
        anat_file=anat_file,
        layout=layout,
        subject=subject,
        sessions=sessions,
        base_entities=base_entities,
        suffixes=suffixes,
        sdcflows_version=sdcflows_version,
    )


def find_anatomical_estimators(
    *,
    anat_file: Any,
    layout: Any,
    subject: str,
    sessions: list[Any],
    base_entities: Mapping[str, Any],
    suffixes: Iterable[str],
    sdcflows_version: str = "2.17.1",
) -> list[list[FieldmapFile]]:
    """sdcflows' ``find_anatomical_estimators``: for each EPI run with a phase-encoding direction
    (its single-band reference preferred, the shortest echo first), the anatomical reference and
    the run, the reference ``IntendedFor`` every echo and reference of the run.

    Parameters
    ----------
    anat_file
        The anatomical reference (a :class:`larmorx.bids.BIDSFile`).
    layout
        A :class:`larmorx.bids.BIDSLayout`.
    subject
        The participant label.
    sessions
        Session labels (``[None]`` for all).
    base_entities
        The entities of every query (the filters among them).
    suffixes
        EPI suffixes (``"bold"``, ``"dwi"``, ``"asl"``).
    sdcflows_version
        For the :class:`FieldmapFile` objects made.
    """
    subject_root = Path(layout.root) / f"sub-{subject}"
    hits: set[Path] = set()
    estimators = []
    for ses, suffix in sorted(product(sessions, suffixes)):
        suffixes_ = ["sbref", suffix]  # Order indicates preference; prefer sbref
        datatype = {"bold": "func", "dwi": "dwi", "asl": "perf"}[suffix]
        candidates = layout.get(
            **{**base_entities, **{"suffix": suffixes_, "session": ses, "datatype": datatype}}
        )
        epi_targets = []
        for candidate in candidates:
            meta = candidate.get_metadata()
            if not meta.get("PhaseEncodingDirection"):
                continue
            with suppress(ValueError):
                meta.update({"TotalReadoutTime": _get_trt(meta, candidate.path)})
            epi_targets.append(
                FieldmapFile(candidate, metadata=meta, sdcflows_version=sdcflows_version)
            )

        def sort_key(fmap: FieldmapFile, suffixes: list[str] = suffixes_) -> tuple[int, Any]:
            return suffixes.index(fmap.suffix), fmap.metadata.get("EchoTime", 1)

        for target in sorted(epi_targets, key=sort_key):
            if target.path in hits:
                continue
            query = {**base_entities, **target.entities}
            query.pop("echo", None)
            intent = [Path(epi) for epi in layout.get(suffix=suffixes_, **query)]
            metadata = {"IntendedFor": [_rel(epi, subject_root) for epi in intent]}
            estimators.append(
                [
                    FieldmapFile(anat_file, metadata=metadata, sdcflows_version=sdcflows_version),
                    target,
                ]
            )
            hits.update(intent)
    return estimators


def _log_debug_estimation(
    logger: logging.Logger, estimation: FieldmapEstimation, bids_root: str
) -> None:
    logger.debug(
        "Found %s estimation from %d sources:\n- %s",
        estimation.method.name,
        len(estimation.sources),
        "\n- ".join([str(Path(s.path).relative_to(bids_root)) for s in estimation.sources]),
    )


def _log_debug_estimator_fail(
    logger: logging.Logger, b0_id: str, files: list[Any], bids_root: str, message: str
) -> None:
    logger.debug(
        "Failed to construct %s estimation from %d sources:\n- %s\nError: %s",
        b0_id,
        len(files),
        "\n- ".join([str(Path(s.path).relative_to(bids_root)) for s in files]),
        message,
    )
