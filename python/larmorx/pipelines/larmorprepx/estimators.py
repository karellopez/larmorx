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
- :func:`precomputed_fieldmaps`: the precomputed field maps of every derivatives folder
  (``--derivatives``).
- :func:`plan_fieldmap_estimation`, :class:`FieldmapEstimationPlan`,
  :func:`estimate_fieldmaps`: ``init_single_subject_wf``'s field-map estimation stage (the
  caller of sdcflows' ``init_fmap_preproc_wf``): what fMRIPrep sets up when it builds its graph
  (the plan: its checks and messages, the precomputed field maps, the estimators' inputs, the
  fieldmap-less estimators' ``init_syn_preprocessing_wf``), then run with the anatomical
  workflow's outputs into the field-map buffers every BOLD workflow reads
  (:class:`larmorx.pipelines.larmorprepx.fieldmaps.FieldmapEstimates`).

fMRIPrep logs through ``nipype.workflow``; these functions log the same messages through the
``logger`` given (by default ``larmorx.larmorprepx``).

Corrected by default (``upstream_defects="reproduce"`` gives fMRIPrep's behaviour): PIPE-31, a
precomputed field map whose derivatives lack its field map, reference or coefficients file
stops fMRIPrep with a bare ``KeyError`` ('fieldmap', 'magnitude' or 'coeffs'); here a
``ValueError`` names the identifier, the missing file and the derivatives (rule B). PIPE-32,
participants of one run that share a ``B0FieldIdentifier`` stop fMRIPrep with sdcflows'
``KeyError`` (one registry for every participant); here each participant's search has a
registry of its own (rule A, :func:`map_fieldmap_estimation`).
"""

from __future__ import annotations

import logging
import os
import re
import warnings
from collections import defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "BUFFER_FIELDS",
    "EX_DATAERR",
    "FMAP_PREPROC_DESC",
    "FMAP_SPEC",
    "FMRIPREP_IGNORE",
    "SYN_PRIOR_TEMPLATE",
    "FieldmapEstimationPlan",
    "FieldmapEstimationResult",
    "SynPreprocessingCall",
    "check_pepolar_scheme",
    "collect_fieldmaps",
    "estimate_fieldmaps",
    "fmriprep_layout",
    "get_estimator",
    "map_fieldmap_estimation",
    "merge_buffers",
    "plan_fieldmap_estimation",
    "precomputed_fieldmaps",
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
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """The dataset indexed as fMRIPrep's ``config.execution.init`` indexes it: ``code``,
    ``stimuli``, ``sourcedata``, ``models`` and the subjects' ``beh``, ``dwi``, ``eeg``,
    ``ieeg``, ``meg`` and ``perf`` folders ignored, and, with ``participant_label`` and no
    ``bids_database_dir`` (fMRIPrep's default), every other subject. The root must hold a valid
    ``dataset_description.json`` (pybids' ``validate=True`` root checks). ``upstream_defects``:
    PIPE-37, the files' entities from their paths below the root (``"fix"``) or, as pybids,
    from their absolute paths (``"reproduce"``)."""
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    ignore = list(FMRIPREP_IGNORE)
    if participant_label and bids_database_dir is None:
        ignore.append(re.compile(r"sub-(?!(" + "|".join(participant_label) + r")(\b|_))"))
    indexer = BIDSLayoutIndexer(validate=False, ignore=ignore)
    return BIDSLayout(
        str(bids_dir),
        indexer=indexer,
        pybids_version=pybids_version,
        upstream_defects=upstream_defects,
    )


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
        (:func:`larmorx.sdcflows.find_estimators`); PIPE-25: when the sources of an
        estimator list several ``B0FieldIdentifier`` values, sdcflows names it after one of
        them (SDC-19), and fMRIPrep leaves a BOLD run whose ``B0FieldSource`` is another of
        them without a field map; corrected, the run gets that estimator; PIPE-32: fMRIPrep
        records every participant's estimators in sdcflows' one registry, so a second
        participant with a ``B0FieldIdentifier`` an earlier one used (BIDS asks it to be
        unique only within a participant) stops the run with sdcflows' ``KeyError``;
        corrected, each call searches in a registry of its own (holding the earlier
        participants' automatic names, so ``auto_NNNNN`` stay fMRIPrep's), then adds its
        entries to ``registry`` where their names are free, with a warning for a shared one
        (``registry`` keeps the first participant's sources under it; the estimators returned
        hold their own).

    Returns
    -------
    tuple
        The estimators used, and each corrected BOLD file's estimator identifier.
    """
    from larmorx.sdcflows import estimators as fm
    from larmorx.sdcflows.estimators import EstimatorType
    from larmorx.sdcflows.wrangler import find_estimators

    log = logger or _LOGGER
    if not any((not ignore_fieldmaps, use_syn, force_syn)):
        return [], {}

    shared = fm._registry(registry)
    if upstream_defects == "reproduce":
        reg = shared
    else:  # PIPE-32: a registry of the participant's own, with the earlier automatic names
        reg = fm.FieldmapRegistry()
        for key, value in shared.estimators.items():
            if key.startswith("auto_"):
                reg.estimators[key] = value
    fmap_estimators = find_estimators(
        layout=layout,
        subject=subject_id,
        sessions=[session_id] if isinstance(session_id, str) else session_id,
        fmapless=bool(use_syn) or (ignore_fieldmaps and force_syn),
        force_fmapless=force_syn or (ignore_fieldmaps and bool(use_syn)),
        bids_filters=filters,
        registry=reg,
        sdcflows_version=sdcflows_version,
        upstream_defects=upstream_defects,
    )
    if reg is not shared:
        _merge_registry(reg, shared, subject_id)

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
        found = list(get_estimator(layout, bold_file, registry=reg))
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


def _merge_registry(own: Any, shared: Any, subject_id: str) -> None:
    """PIPE-32: the participant's registry ``own`` added to ``shared`` where the names are
    free (the automatic ones it already holds are the same entries); a name another
    participant's sources hold stays theirs, with a warning."""
    for key, value in own.estimators.items():
        if key not in shared.estimators:
            shared.estimators[key] = value
        elif shared.estimators[key] != value:
            warnings.warn(
                _defects.UpstreamDefectWarning(
                    "PIPE-32",
                    "fMRIPrep",
                    "fix",
                    f"sub-{subject_id}'s B0FieldIdentifier {key!r} is already an earlier "
                    "participant's: fMRIPrep stops with sdcflows' KeyError (one registry for "
                    "every participant); here each participant's search has its own",
                ),
                stacklevel=3,
            )
    for path, ids in own.intents.items():
        shared.intents[path] |= ids


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
    upstream_defects: UpstreamDefectsName = "fix",
) -> tuple[list[Any], dict[str, Mapping[str, Any]]]:
    """``init_single_subject_wf``'s split: the estimators whose field maps the derivatives hold
    (by identifier without its non-alphanumeric characters) and the estimators left to run,
    with fMRIPrep's messages. A precomputed field map without its field map, reference or
    coefficients file: PIPE-31 (module docstring)."""
    _defects.check(upstream_defects)
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
        if upstream_defects != "reproduce":
            for fmapid, found in pared_cache.items():
                lacking = [key for _, key in _PRECOMPUTED_KEYS if key not in found]
                if lacking:
                    raise ValueError(
                        f"the precomputed field map {fmapid!r} lacks its "
                        f"{', '.join(lacking)} file(s) in the derivatives (it has "
                        f"{sorted(found)}; PIPE-31: fMRIPrep stops with a KeyError; "
                        "upstream_defects='reproduce' does too)"
                    )
        fieldmaps = [fmap["fieldmap"] for fmap in pared_cache.values()]
        refs = [fmap["magnitude"] for fmap in pared_cache.values()]
        coeffs = [fmap["coeffs"] for fmap in pared_cache.values()]
        log.debug(f"Reusing fieldmaps: {fieldmaps}")
        log.debug(f"Reusing references: {refs}")
        log.debug(f"Reusing coefficients: {coeffs}")
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


# ------------------------------------------------------------------------------------------
# init_single_subject_wf's field-map estimation stage

#: fMRIPrep's exit status for a missing readout time (``os.EX_DATAERR``, which Windows' ``os``
#: lacks: the same number everywhere).
EX_DATAERR = 65
#: The template whose transform to the T1w the fieldmap-less estimators' prior is brought
#: through (fMRIPrep's ``fmap_select_std``; always among fMRIPrep's spaces).
SYN_PRIOR_TEMPLATE = "MNI152NLin2009cAsym"
#: The boilerplate fMRIPrep gives the field-map preprocessing (``fmap_wf.__desc__``).
FMAP_PREPROC_DESC = """

Preprocessing of B<sub>0</sub> inhomogeneity mappings

: A total of {n} fieldmaps were found available within the input
BIDS structure for this particular subject.
"""
#: The field-map buffers (``fmap_buffers``), in fMRIPrep's order.
BUFFER_FIELDS = ("fmap", "fmap_ref", "fmap_coeff", "fmap_mask", "fmap_id", "sdc_method")
# collect_fieldmaps' keys each buffer reads, by buffer
_PRECOMPUTED_KEYS = (("fmap", "fieldmap"), ("fmap_ref", "magnitude"), ("fmap_coeff", "coeffs"))


def precomputed_fieldmaps(
    derivatives: Any,
    subject: str,
    *,
    logger: logging.Logger | None = None,
) -> dict[str, dict[str, Any]]:
    """fMRIPrep's ``fmap_cache``: :func:`collect_fieldmaps` of every derivatives folder
    (``--derivatives``: a mapping of names to folders, a sequence of folders or one folder) for
    ``subject``, in order (an identifier a later folder holds replaces an earlier one's), with
    fMRIPrep's debug line "Detected precomputed fieldmaps in <folder> for fieldmap IDs: [...]"."""
    log = logger or _LOGGER
    if isinstance(derivatives, (str, os.PathLike)):
        folders: list[Any] = [derivatives]
    elif isinstance(derivatives, Mapping):
        folders = list(derivatives.values())
    else:
        folders = list(derivatives)
    cache: dict[str, dict[str, Any]] = {}
    for deriv_dir in folders:
        fmaps = collect_fieldmaps(derivatives_dir=deriv_dir, entities={"subject": subject})
        log.debug(f"Detected precomputed fieldmaps in {deriv_dir} for fieldmap IDs: {list(fmaps)}")
        cache.update(fmaps)
    return cache


@dataclass(frozen=True)
class SynPreprocessingCall:
    """A fieldmap-less estimator's ``init_syn_preprocessing_wf`` as fMRIPrep sets it up: the
    workflow's ``name`` (``syn_preprocessing_<sanitized_id>``), its EPI files (the estimator's
    ``bold`` and ``sbref`` sources, ``in_epis``) and their metadata (``in_meta``), and the
    options of :func:`larmorx.sdcflows.syn_preprocessing_workflow` (``omp_nthreads``, ``debug``
    = ``--sloppy``, ``auto_bold_nss=True``, ``t1w_inversion=False``, and the larmorx options
    given to the plan). Its anatomical inputs come at the run (``t1w_preproc``, ``t1w_mask``,
    the MNI152NLin2009cAsym-to-T1w transform)."""

    sanitized_id: str
    name: str
    in_epis: tuple[str, ...]
    in_meta: tuple[Mapping[str, Any], ...]
    options: Mapping[str, Any]


@dataclass(frozen=True)
class FieldmapEstimationResult:
    """The field-map estimation stage run: the field-map buffers every BOLD workflow reads
    (``estimates``: the precomputed field maps first, then the estimated ones, as fMRIPrep's
    ``Merge`` nodes give them), the estimation method of every estimator by identifier
    (``estimator_types``: fMRIPrep's Jacobian rule,
    :func:`larmorx.pipelines.larmorprepx.fieldmaps.fieldmap_jacobian`), the transform
    ``fmap_select_std`` selected (``std2anat_xfm``, ``None`` without fieldmap-less estimators),
    the fieldmap-less estimators' preprocessing (``syn_preprocessing``, by sanitized
    identifier) and sdcflows' ``init_fmap_preproc_wf`` result (``preproc``, ``None`` when every
    field map was precomputed or there is no estimator)."""

    estimates: Any
    estimator_types: Mapping[str, Any]
    std2anat_xfm: str | None
    syn_preprocessing: Mapping[str, Any]
    preproc: Any


@dataclass(frozen=True)
class FieldmapEstimationPlan:
    """What fMRIPrep's ``init_single_subject_wf`` sets up for the field maps when it builds its
    graph (:func:`plan_fieldmap_estimation`), before anything runs.

    - ``all_estimators``: the subject's estimators (:func:`map_fieldmap_estimation`), and their
      methods by identifier (``estimator_types``);
    - ``precomputed``: the estimators whose field maps the derivatives hold, by identifier
      (their files: ``fieldmap``, ``magnitude``, ``coeffs``), and ``precomputed_buffers``: the
      first input of each field-map buffer (``in1``: the files, ``'MISSING'`` masks, the
      identifiers, ``'precomputed'``), empty without precomputed field maps;
    - ``estimators``: those left to estimate, in order, with their workflows built
      (:func:`larmorx.sdcflows.build_workflows`) and the options of
      :func:`larmorx.sdcflows.fmap_preproc_workflow` (``preproc_options``: fMRIPrep's
      ``use_metadata_estimates``, ``fallback_total_readout_time``, ``debug``, ``omp_nthreads``,
      ``output_dir``, ``subject``, ``out_path_base=''`` and the larmorx options);
    - ``inputs``: the ``in_<sanitized_id>`` inputs fMRIPrep sets when it builds the graph (a
      PEPOLAR estimator's ``in_data`` and ``metadata``);
    - ``syn_preprocessing``: each fieldmap-less estimator's :class:`SynPreprocessingCall`;
    - ``desc``: the boilerplate of the field-map preprocessing (``FMAP_PREPROC_DESC``; empty
      when nothing is estimated).

    :meth:`run` runs it with the anatomical workflow's outputs."""

    all_estimators: tuple[Any, ...]
    estimator_types: Mapping[str, Any]
    precomputed: Mapping[str, Mapping[str, Any]]
    precomputed_buffers: Mapping[str, list[Any]]
    estimators: tuple[Any, ...]
    preproc_options: Mapping[str, Any]
    inputs: Mapping[str, Mapping[str, Any]]
    syn_preprocessing: tuple[SynPreprocessingCall, ...] = ()
    desc: str = ""
    workflows: tuple[Any, ...] = field(default=(), repr=False)

    @property
    def needs_anat(self) -> bool:
        """Whether a fieldmap-less estimator needs the anatomical inputs (fMRIPrep connects
        ``fmap_select_std`` only then)."""
        return bool(self.syn_preprocessing)

    def run(
        self,
        *,
        work_dir: str | os.PathLike[str],
        t1w_preproc: str | os.PathLike[str] | None = None,
        t1w_mask: str | os.PathLike[str] | None = None,
        std2anat_xfm: Any = None,
        templates: Sequence[str] | None = None,
    ) -> FieldmapEstimationResult:
        """Run the plan: each fieldmap-less estimator's ``init_syn_preprocessing_wf``
        (:func:`larmorx.sdcflows.syn_preprocessing_workflow` in ``work_dir/<name>``, with
        ``t1w_preproc``, ``t1w_mask`` and the MNI152NLin2009cAsym-to-T1w transform), then
        sdcflows' ``init_fmap_preproc_wf`` (:func:`larmorx.sdcflows.fmap_preproc_workflow` in
        ``work_dir/fmap_preproc_wf``, the PEPOLAR inputs and the preprocessing's outputs as its
        ``in_<sanitized_id>`` inputs), and the field-map buffers merged.

        ``work_dir`` is the subject's workflow folder (fMRIPrep's ``sub_<label>_wf``).
        ``std2anat_xfm`` and ``templates`` are the anatomical workflow's ``std2anat_xfm`` and
        ``template`` lists, from which ``fmap_select_std`` (niworkflows' ``KeySelect``) takes
        MNI152NLin2009cAsym's transform; without ``templates``, ``std2anat_xfm`` is that
        transform itself. The anatomical inputs are needed only by fieldmap-less estimators
        (``ValueError`` without them)."""
        from larmorx.niworkflows import key_select
        from larmorx.sdcflows import base as sdc_base
        from larmorx.sdcflows import syn as sdc_syn

        xfm = None
        syn_results: dict[str, Any] = {}
        inputs = {k: dict(v) for k, v in self.inputs.items()}
        if self.needs_anat:
            missing = [
                n
                for n, v in (
                    ("t1w_preproc", t1w_preproc),
                    ("t1w_mask", t1w_mask),
                    ("std2anat_xfm", std2anat_xfm),
                )
                if v is None
            ]
            if missing:
                raise ValueError(
                    f"the fieldmap-less estimators need the anatomical inputs {missing} "
                    "(fMRIPrep's anat_fit_wf outputs)"
                )
            if templates is not None:
                selected = key_select(
                    list(templates), SYN_PRIOR_TEMPLATE, std2anat_xfm=list(std2anat_xfm)
                )
                xfm = os.fspath(selected["std2anat_xfm"])
            else:
                xfm = os.fspath(std2anat_xfm)
            for call in self.syn_preprocessing:
                res = sdc_syn.syn_preprocessing_workflow(
                    list(call.in_epis),
                    list(call.in_meta),
                    t1w_preproc,  # type: ignore[arg-type]
                    t1w_mask,  # type: ignore[arg-type]
                    xfm,
                    out_dir=work_dir,
                    name=call.name,
                    **call.options,
                )
                syn_results[call.sanitized_id] = res
                inputs[call.sanitized_id] = {f: getattr(res, f) for f in sdc_syn.INPUT_FIELDS}
        preproc = None
        if self.estimators:
            preproc = sdc_base.fmap_preproc_workflow(
                list(self.estimators),
                inputs=inputs,
                work_dir=work_dir,
                **self.preproc_options,
            )
        estimates = merge_buffers(self.precomputed_buffers, preproc)
        return FieldmapEstimationResult(
            estimates, dict(self.estimator_types), xfm, syn_results, preproc
        )


def merge_buffers(precomputed_buffers: Mapping[str, Sequence[Any]], preproc: Any) -> Any:
    """fMRIPrep's field-map buffers (``fmap_buffers``, nipype's ``Merge(2)``): the precomputed
    field maps' inputs (``in1``, :attr:`FieldmapEstimationPlan.precomputed_buffers`) followed by
    sdcflows' ``init_fmap_preproc_wf`` outputs (``in2``: ``preproc``'s ``fmap``, ``fmap_ref``,
    ``fmap_coeff``, ``fmap_mask``, ``fmap_id`` and ``method``, or nothing for ``None``), each
    input's items appended (a coefficient item stays a list, or a file when the derivatives held
    one). The :class:`larmorx.pipelines.larmorprepx.fieldmaps.FieldmapEstimates` every BOLD
    workflow reads."""
    from larmorx.pipelines.larmorprepx.fieldmaps import FieldmapEstimates, _merge

    in2: dict[str, list[Any]] = {}
    if preproc is not None:
        in2 = {
            "fmap": list(preproc.fmap),
            "fmap_ref": list(preproc.fmap_ref),
            "fmap_coeff": [list(c) for c in preproc.fmap_coeff],
            "fmap_mask": list(preproc.fmap_mask),
            "fmap_id": list(preproc.fmap_id),
            "sdc_method": list(preproc.method),
        }
    merged = {f: _merge(precomputed_buffers.get(f), in2.get(f)) for f in BUFFER_FIELDS}
    return FieldmapEstimates(
        fmap_id=merged["fmap_id"],
        fmap_ref=merged["fmap_ref"],
        fmap_coeff=merged["fmap_coeff"],
        fmap_mask=merged["fmap_mask"],
        sdc_method=merged["sdc_method"],
        fmap=merged["fmap"],
    )


def _precomputed_buffers(pared_cache: Mapping[str, Mapping[str, Any]]) -> dict[str, list[Any]]:
    """The precomputed field maps' inputs of the buffers (``in1``)."""
    if not pared_cache:
        return {}
    out = {buf: [fmap[key] for fmap in pared_cache.values()] for buf, key in _PRECOMPUTED_KEYS}
    out["fmap_mask"] = [pared_cache[fmapid].get("mask", "MISSING") for fmapid in pared_cache]
    out["fmap_id"] = list(pared_cache)
    out["sdc_method"] = ["precomputed"] * len(pared_cache)
    return out


def plan_fieldmap_estimation(
    all_estimators: Sequence[Any],
    fmap_cache: Mapping[str, Mapping[str, Any]] | None = None,
    *,
    output_dir: str | os.PathLike[str],
    subject: str,
    omp_nthreads: int = 1,
    fallback_total_readout_time: float | str | None = None,
    debug: bool = False,
    sloppy: bool = False,
    exit_on_missing_readout: bool = True,
    syn_options: Mapping[str, Any] | None = None,
    logger: logging.Logger | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
    **workflow_options: Any,
) -> FieldmapEstimationPlan:
    """fMRIPrep's field-map estimation stage as its ``init_single_subject_wf`` builds it:

    1. :func:`split_precomputed` (``fmap_cache``: :func:`precomputed_fieldmaps`): the
       estimators whose field maps the derivatives hold feed the buffers directly, the others
       are estimated (with fMRIPrep's messages);
    2. sdcflows' ``init_fmap_preproc_wf`` is built (:func:`larmorx.sdcflows.build_workflows`):
       ``use_metadata_estimates`` when ``fallback_total_readout_time`` is ``'estimated'``, the
       fallback when it is a number, ``debug`` (fMRIPrep's ``--debug fieldmaps``), the
       derivatives into ``output_dir`` with ``out_path_base=''``; a missing readout time logs
       fMRIPrep's critical "Missing readout time information. See documentation for
       `--fallback-total-readout-time`." with the traceback and raises ``SystemExit(65)``
       (``exit_on_missing_readout=False``: sdcflows' ``RuntimeError`` instead);
    3. for each estimator, the "Setting up fieldmap ..." line; a PEPOLAR estimator's scheme
       checked (:func:`check_pepolar_scheme`) and its sources and their metadata set as its
       ``in_data`` and ``metadata``; a fieldmap-less one gets its ``init_syn_preprocessing_wf``
       (:class:`SynPreprocessingCall`: ``debug`` is ``sloppy``, fMRIPrep's ``--sloppy``).

    ``workflow_options`` go to the estimators' workflows (:func:`larmorx.sdcflows.
    fmap_preproc_workflow`'s keyword arguments: the libraries' versions, the implementations,
    sdcflows 2.15's PEPOLAR ``wiring``, ``ants_random_seed``, ...), ``syn_options`` to
    :func:`larmorx.sdcflows.syn_preprocessing_workflow`; ``upstream_defects`` and
    ``n_threads`` to both. Nothing runs before :meth:`FieldmapEstimationPlan.run`."""
    from larmorx.sdcflows import base as sdc_base
    from larmorx.sdcflows.estimators import EstimatorType

    _defects.check(upstream_defects)
    log = logger or _LOGGER
    if isinstance(fallback_total_readout_time, str) and fallback_total_readout_time != "estimated":
        raise ValueError(
            "fallback_total_readout_time must be a number, 'estimated' or None "
            f"(fMRIPrep's --fallback-total-readout-time), not {fallback_total_readout_time!r}"
        )
    fallback_number = (
        float(fallback_total_readout_time)
        if isinstance(fallback_total_readout_time, (int, float))
        and not isinstance(fallback_total_readout_time, bool)
        else None
    )
    estimator_types = {est.bids_id: est.method for est in all_estimators}
    fmap_estimators: list[Any] = []
    pared_cache: dict[str, Mapping[str, Any]] = {}
    if all_estimators:
        fmap_estimators, pared_cache = split_precomputed(
            all_estimators, fmap_cache or {}, logger=log, upstream_defects=upstream_defects
        )
    buffers = _precomputed_buffers(pared_cache)
    if not fmap_estimators:
        return FieldmapEstimationPlan(
            tuple(all_estimators), estimator_types, pared_cache, buffers, (), {}, {}
        )
    preproc_options: dict[str, Any] = {
        "use_metadata_estimates": fallback_total_readout_time == "estimated",
        "fallback_total_readout_time": fallback_number,
        "debug": debug,
        "omp_nthreads": omp_nthreads,
        "output_dir": output_dir,
        "subject": subject,
        "out_path_base": "",
        "upstream_defects": upstream_defects,
        "n_threads": n_threads,
        **workflow_options,
    }
    build = {
        k: v
        for k, v in preproc_options.items()
        if k not in ("output_dir", "subject", "out_path_base")
    }
    try:
        workflows = sdc_base.build_workflows(fmap_estimators, **build)
    except RuntimeError:
        if not exit_on_missing_readout:
            raise
        message = (
            "Missing readout time information. "
            "See documentation for `--fallback-total-readout-time`."
        )
        log.critical(message, exc_info=True)
        raise SystemExit(EX_DATAERR) from None
    inputs: dict[str, dict[str, Any]] = {}
    syn_calls: list[SynPreprocessingCall] = []
    for estimator in fmap_estimators:
        log.info(setting_up_message(estimator))
        if estimator.method in (EstimatorType.MAPPED, EstimatorType.PHASEDIFF):
            continue
        if estimator.method == EstimatorType.PEPOLAR:
            check_pepolar_scheme(estimator)
            inputs[estimator.sanitized_id] = {
                "in_data": [str(s.path) for s in estimator.sources],
                "metadata": [s.metadata for s in estimator.sources],
            }
        elif estimator.method == EstimatorType.ANAT:
            epis = [s for s in estimator.sources if s.suffix in ("bold", "sbref")]
            syn_calls.append(
                SynPreprocessingCall(
                    estimator.sanitized_id,
                    f"syn_preprocessing_{estimator.sanitized_id}",
                    tuple(str(s.path) for s in epis),
                    tuple(s.metadata for s in epis),
                    {
                        "omp_nthreads": omp_nthreads,
                        "debug": sloppy,
                        "auto_bold_nss": True,
                        "t1w_inversion": False,
                        "upstream_defects": upstream_defects,
                        "n_threads": n_threads,
                        **(syn_options or {}),
                    },
                )
            )
    return FieldmapEstimationPlan(
        tuple(all_estimators),
        estimator_types,
        pared_cache,
        buffers,
        tuple(fmap_estimators),
        preproc_options,
        inputs,
        tuple(syn_calls),
        FMAP_PREPROC_DESC.format(n=len(fmap_estimators)),
        tuple(workflows),
    )


def estimate_fieldmaps(
    all_estimators: Sequence[Any],
    fmap_cache: Mapping[str, Mapping[str, Any]] | None = None,
    *,
    output_dir: str | os.PathLike[str],
    subject: str,
    work_dir: str | os.PathLike[str],
    t1w_preproc: str | os.PathLike[str] | None = None,
    t1w_mask: str | os.PathLike[str] | None = None,
    std2anat_xfm: Any = None,
    templates: Sequence[str] | None = None,
    **options: Any,
) -> FieldmapEstimationResult:
    """fMRIPrep's field-map estimation stage, planned (:func:`plan_fieldmap_estimation`, with
    ``options``) and run (:meth:`FieldmapEstimationPlan.run`)."""
    plan = plan_fieldmap_estimation(
        all_estimators, fmap_cache, output_dir=output_dir, subject=subject, **options
    )
    return plan.run(
        work_dir=work_dir,
        t1w_preproc=t1w_preproc,
        t1w_mask=t1w_mask,
        std2anat_xfm=std2anat_xfm,
        templates=templates,
    )
