# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's choice of field-map estimators for a subject's BOLD runs (larmorprepx).

Ported from fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; the same on these paths in
25.2.6, the oracle runs'): ``config.py`` (the BIDS index of ``execution.init``: its ignore
patterns), ``workflows/base.py`` (``map_fieldmap_estimation``, ``get_estimator``, and in
``init_single_subject_wf`` the precomputed field maps, the "Setting up fieldmap" checks and the
PEPOLAR schemes it accepts) and ``utils/bids.py`` (``collect_fieldmaps`` with
``data/fmap_spec.json``). The estimators are sdcflows' (:func:`larmorx.sdcflows.find_estimators`)
on a :class:`larmorx.bids.BIDSLayout`.

- :func:`fmriprep_layout`: the dataset indexed as fMRIPrep indexes it.
- :func:`map_fieldmap_estimation`: the estimators fMRIPrep uses and the one each BOLD run gets.
- :func:`get_estimator`: the estimators of one BOLD file (``B0FieldSource``, else
  ``IntendedFor``).
- :func:`collect_fieldmaps`: the precomputed field maps of a derivatives dataset.
- :func:`split_precomputed`: the estimators whose field maps are precomputed, and the others.
- :func:`check_pepolar_scheme`: fMRIPrep's refusal of "sophisticated" PEPOLAR schemes.
- :func:`setting_up_message`: fMRIPrep's log line for each estimator it sets up.

fMRIPrep logs through ``nipype.workflow``; these functions log the same messages through the
``logger`` given (by default ``larmorx.larmorprepx``).
"""

from __future__ import annotations

import logging
import re
import warnings
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "FMAP_SPEC",
    "FMRIPREP_IGNORE",
    "check_pepolar_scheme",
    "collect_fieldmaps",
    "fmriprep_layout",
    "get_estimator",
    "map_fieldmap_estimation",
    "setting_up_message",
    "split_precomputed",
]

#: fMRIPrep's ``data/fmap_spec.json`` queries (``collect_fieldmaps``).
FMAP_SPEC: dict[str, dict[str, Any]] = {
    "fieldmaps": {
        "fieldmap": {
            "datatype": "fmap",
            "desc": "preproc",
            "suffix": "fieldmap",
            "extension": [".nii.gz", ".nii"],
        },
        "coeffs": {
            "datatype": "fmap",
            "desc": ["coeff", "coeff0", "coeff1"],
            "suffix": "fieldmap",
            "extension": [".nii.gz", ".nii"],
        },
        "magnitude": {
            "datatype": "fmap",
            "desc": ["magnitude", "epi"],
            "suffix": "fieldmap",
            "extension": [".nii.gz", ".nii"],
        },
    }
}

#: The ignore patterns of fMRIPrep's BIDS index ("Recommended after PyBIDS 12.1"); the
#: participant pattern is added by :func:`fmriprep_layout`.
FMRIPREP_IGNORE: tuple[Any, ...] = (
    "code",
    "stimuli",
    "sourcedata",
    "models",
    re.compile(r"^\."),
    re.compile(r"sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|dwi|eeg|ieeg|meg|perf)"),
)

_LOGGER = logging.getLogger("larmorx.larmorprepx")


def fmriprep_layout(
    bids_dir: Any,
    participant_label: Sequence[str] | None = None,
    *,
    bids_database_dir: Any = None,
    pybids_version: str = "0.22.0",
) -> Any:
    """The dataset indexed as fMRIPrep's ``config.execution.init`` indexes it: ``code``,
    ``stimuli``, ``sourcedata``, ``models`` and the subjects' ``beh``, ``dwi``, ``eeg``,
    ``ieeg``, ``meg`` and ``perf`` folders ignored, and, with ``participant_label`` and no
    ``bids_database_dir`` (fMRIPrep's default), every other subject. The root must hold a valid
    ``dataset_description.json`` (pybids' ``validate=True`` root checks)."""
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    ignore = list(FMRIPREP_IGNORE)
    if participant_label and bids_database_dir is None:
        ignore.append(re.compile(r"sub-(?!(" + "|".join(participant_label) + r")(\b|_))"))
    indexer = BIDSLayoutIndexer(validate=False, ignore=ignore)
    return BIDSLayout(str(bids_dir), indexer=indexer, pybids_version=pybids_version)


def get_estimator(layout: Any, fname: str, *, registry: Any = None) -> Any:
    """fMRIPrep's ``get_estimator``: the estimators of a BOLD file, from its ``B0FieldSource``
    metadata, else from the estimators ``IntendedFor`` it (the path below the subject's
    folder)."""
    from larmorx.sdcflows.estimators import get_identifier

    field_source = layout.get_metadata(fname).get("B0FieldSource")
    if isinstance(field_source, str):
        field_source = (field_source,)
    if field_source is None:
        # Fallback to IntendedFor (fMRIPrep: str(...relative_to(root)); "/" on every platform)
        intended_rel = re.sub(
            r"^sub-[a-zA-Z0-9]*/", "", Path(fname).relative_to(layout.root).as_posix()
        )
        field_source = get_identifier(intended_rel, registry=registry)
    return field_source


def map_fieldmap_estimation(
    layout: Any,
    subject_id: str,
    session_id: str | list[str] | None,
    bold_data: Sequence[Sequence[str]],
    ignore_fieldmaps: bool,
    use_syn: bool | str,
    force_syn: bool,
    filters: Mapping[str, Any] | None,
    *,
    logger: logging.Logger | None = None,
    registry: Any = None,
    sdcflows_version: str = "2.17.1",
    upstream_defects: UpstreamDefectsName = "fix",
) -> tuple[list[Any], dict[str, str]]:
    """fMRIPrep's ``map_fieldmap_estimation``: sdcflows' estimators of the subject (with
    ``--use-syn-sdc``, ``--force syn-sdc`` and ``--ignore fieldmaps`` as fMRIPrep maps them),
    kept when a BOLD run uses them; each run's estimator (the first when several are intended
    for it, with fMRIPrep's warning).

    Parameters
    ----------
    layout
        The dataset (:func:`fmriprep_layout`).
    subject_id, session_id
        The participant and its session(s) (``None``: every session).
    bold_data
        The BOLD runs, each a list of its echoes (the first is the run's file).
    ignore_fieldmaps, use_syn, force_syn
        ``--ignore fieldmaps``, ``--use-syn-sdc`` (``True``, ``"warn"`` or ``"error"``) and
        ``--force syn-sdc``.
    filters
        The ``fmap`` entry of ``--bids-filter-file``.
    logger
        Where fMRIPrep's messages go.
    registry, sdcflows_version
        For :func:`larmorx.sdcflows.find_estimators`.
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: sdcflows' SDC-17 and SDC-21
        (:func:`larmorx.sdcflows.find_estimators`) and PIPE-25: when the sources of an
        estimator list several ``B0FieldIdentifier`` values, sdcflows names it after one of
        them (SDC-19), and fMRIPrep leaves a BOLD run whose ``B0FieldSource`` is another of
        them without a field map; corrected, the run gets that estimator.

    Returns
    -------
    tuple
        The estimators used, and each corrected BOLD file's estimator identifier.
    """
    from larmorx.sdcflows.estimators import EstimatorType
    from larmorx.sdcflows.wrangler import find_estimators

    log = logger or _LOGGER
    if not any((not ignore_fieldmaps, use_syn, force_syn)):
        return [], {}

    fmap_estimators = find_estimators(
        layout=layout,
        subject=subject_id,
        sessions=[session_id] if isinstance(session_id, str) else session_id,
        fmapless=bool(use_syn) or (ignore_fieldmaps and force_syn),
        force_fmapless=force_syn or (ignore_fieldmaps and bool(use_syn)),
        bids_filters=filters,
        registry=registry,
        sdcflows_version=sdcflows_version,
        upstream_defects=upstream_defects,
    )

    if not fmap_estimators:
        if use_syn:
            message = (
                "Fieldmap-less (SyN) estimation was requested, but PhaseEncodingDirection "
                "information appears to be absent."
            )
            log.error(message)
            if use_syn == "error":
                raise ValueError(message)
        return [], {}

    if ignore_fieldmaps and any(f.method == EstimatorType.ANAT for f in fmap_estimators):
        log.info(
            'Option "--ignore fieldmaps" was set, but either "--use-syn-sdc" '
            'or "--force syn-sdc" were given, so fieldmap-less estimation will be executed.'
        )
        fmap_estimators = [f for f in fmap_estimators if f.method == EstimatorType.ANAT]

    all_ids = {fmap.bids_id for fmap in fmap_estimators}
    bold_files = (bold_series[0] for bold_series in bold_data)
    aliases = _aliases(fmap_estimators) if upstream_defects == "fix" else {}
    all_estimators = {}
    for bold_file in bold_files:
        found = list(get_estimator(layout, bold_file, registry=registry))
        keys = [fmap_id for fmap_id in found if fmap_id in all_ids]
        if aliases:
            fixed = list(
                dict.fromkeys(aliases.get(i, i) for i in found if aliases.get(i, i) in all_ids)
            )
            if fixed != keys:
                warnings.warn(
                    _defects.UpstreamDefectWarning(
                        "PIPE-25",
                        "fMRIPrep",
                        "fix",
                        f"{bold_file}'s B0FieldSource {found} names the estimator(s) "
                        f"{[aliases[i] for i in found if i in aliases]} by another of their "
                        "sources' B0FieldIdentifier values (fMRIPrep: "
                        f"{keys or 'no field map'})",
                    ),
                    stacklevel=2,
                )
                keys = fixed
        all_estimators[bold_file] = keys
    for bold_file, estimator_key in all_estimators.items():
        if len(estimator_key) > 1:
            log.warning(
                f"Several fieldmaps <{', '.join(estimator_key)}> are "
                f"'IntendedFor' <{bold_file}>, using {estimator_key[0]}"
            )
            estimator_key[1:] = []

    estimator_map = {
        bold_file: estimator_key[0]
        for bold_file, estimator_key in all_estimators.items()
        if estimator_key
    }
    fmap_estimators = [f for f in fmap_estimators if f.bids_id in estimator_map.values()]
    return fmap_estimators, estimator_map


def _aliases(estimators: Sequence[Any]) -> dict[str, str]:
    """PIPE-25: every other ``B0FieldIdentifier`` value of an estimator's sources, mapped to
    the identifier sdcflows gave the estimator."""
    out: dict[str, str] = {}
    for est in estimators:
        for src in est.sources:
            ids = src.metadata.get("B0FieldIdentifier")
            for i in ids if isinstance(ids, (list, tuple)) else [ids]:
                if i and i != est.bids_id:
                    out.setdefault(i, est.bids_id)
    return out


def collect_fieldmaps(
    derivatives_dir: Any,
    entities: Mapping[str, Any],
    spec: Mapping[str, Any] | None = None,
) -> defaultdict[str, dict[str, Any]]:
    """fMRIPrep's ``collect_fieldmaps``: the precomputed field maps of a derivatives dataset
    (fMRIPrep's or sdcflows' ``fmap/`` files), by field-map identifier (``fmapid``): the
    field map, its coefficients and its reference (a list when several match)."""
    from larmorx.bids import NIPREPS_CONFIG, BIDSLayout

    if spec is None:
        spec = FMAP_SPEC
    fmap_cache: defaultdict[str, dict[str, Any]] = defaultdict(dict, {})
    layout = BIDSLayout(derivatives_dir, config=[NIPREPS_CONFIG], validate=False)
    for fmapid in layout.get_fmapids(**entities):
        for k, q in spec["fieldmaps"].items():
            query = {**entities, **q}
            item = layout.get(return_type="filename", fmapid=fmapid, **query)
            if not item:
                continue
            fmap_cache[fmapid][k] = item[0] if len(item) == 1 else item
    return fmap_cache


def split_precomputed(
    all_estimators: Sequence[Any],
    fmap_cache: Mapping[str, Mapping[str, Any]],
    *,
    logger: logging.Logger | None = None,
) -> tuple[list[Any], dict[str, Mapping[str, Any]]]:
    """``init_single_subject_wf``'s split: the estimators whose field maps the derivatives hold
    (by identifier without its non-alphanumeric characters) and the estimators left to run,
    with fMRIPrep's messages."""
    log = logger or _LOGGER
    fmap_estimators = []
    pared_cache: dict[str, Mapping[str, Any]] = {}
    for est in all_estimators:
        found = fmap_cache.get(re.sub(r"[^a-zA-Z0-9]", "", est.bids_id))
        if found:
            pared_cache[est.bids_id] = found
        else:
            fmap_estimators.append(est)
    if pared_cache:
        log.info(
            "Precomputed B0 field inhomogeneity maps found for the following "
            f"{len(pared_cache)} estimator(s): {list(pared_cache)}."
        )
        log.debug(f"Reusing fieldmaps: {[fmap['fieldmap'] for fmap in pared_cache.values()]}")
        log.debug(f"Reusing references: {[fmap['magnitude'] for fmap in pared_cache.values()]}")
        log.debug(f"Reusing coefficients: {[fmap['coeffs'] for fmap in pared_cache.values()]}")
    if fmap_estimators:
        log.info(
            "B0 field inhomogeneity map will be estimated with the following "
            f"{len(fmap_estimators)} estimator(s): {[e.method for e in fmap_estimators]}."
        )
    return fmap_estimators, pared_cache


def setting_up_message(estimator: Any) -> str:
    """fMRIPrep's log line for an estimator it sets up."""
    return (
        f'Setting up fieldmap "{estimator.bids_id}" ({estimator.method}) with '
        f"<{', '.join(s.path.name for s in estimator.sources)}>"
    )


def check_pepolar_scheme(estimator: Any) -> None:
    """fMRIPrep's PEPOLAR check: every source of one modality, or two EPI-like sources
    (``epi``, ``bold``, ``sbref``); otherwise ``NotImplementedError('Sophisticated PEPOLAR
    schemes are unsupported.')`` (they are run with sdcflows itself)."""
    from larmorx.sdcflows.estimators import EstimatorType

    if estimator.method != EstimatorType.PEPOLAR:
        return
    suffices = [s.suffix for s in estimator.sources]
    if len(set(suffices)) == 1 or (
        len(suffices) == 2 and all(suf in ("epi", "bold", "sbref") for suf in suffices)
    ):
        return
    raise NotImplementedError("Sophisticated PEPOLAR schemes are unsupported.")
