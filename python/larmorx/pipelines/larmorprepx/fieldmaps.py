# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's field-map steps of a BOLD run as plain functions (larmorprepx).

Ported from fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; the same on these paths in
25.2.6, the oracle runs'): ``workflows/bold/fit.py`` (``init_bold_fit_wf``'s stage 3, "Register
fieldmap to boldref and reconstruct in BOLD space", with stage 4's field-map nodes;
``init_bold_native_wf``'s field-map nodes; ``_select_ref``), ``workflows/bold/apply.py``
(``init_bold_volumetric_resample_wf``'s field-map nodes, ``_gen_inverses``) and
``workflows/base.py`` (the Jacobian rule). No nipype, no graph: each node is a call of its
larmorx port, in fMRIPrep's order, its files in a folder named as fMRIPrep's node.

- :func:`bold_fit_fieldmap`: the field-map nodes of ``init_bold_fit_wf``: ``fmapref_buffer``
  (:func:`select_ref`), ``fmap_select`` (``KeySelect``), ``fmapreg_wf`` (sdcflows'
  ``init_coeff2epi_wf``) and ``itk_mat2txt`` (``ConcatenateXFMs``) unless the BOLD-to-field-map
  transform is given, ``boldref_fmap`` (``ReconstructFieldmap``, the transform inverted),
  ``distortion_params`` (``DistortionParameters``), ``unwarp_boldref`` (``ResampleSeries``: the
  coregistration reference unwarped) and ``skullstrip_bold_wf`` (its brain mask).
- :func:`resampling_fieldmap`: the field-map nodes of ``init_bold_native_wf`` and of every
  ``init_bold_volumetric_resample_wf`` (``bold_anat_wf``, ``bold_std_wf``, ...): ``fmap_select``,
  ``distortion_params``, ``fmap2target`` and ``inverses`` (:func:`gen_inverses`), and the field
  map reconstructed on the target grid of the resampling (``boldref_fmap`` / ``fmap_recon``).
- :func:`fieldmap_jacobian`: whether fMRIPrep's resamplings apply the Jacobian.

The inputs come from the steps before (the estimators' outputs, the motion-correction
reference, niworkflows' ``init_enhance_and_skullstrip_bold_wf``'s outputs: given by the caller
until larmorx ports those workflows). fMRIPrep writes three of the stage's files as derivatives
(``DerivativesDataSink``) and feeds its later nodes the derivative files: ``sink`` receives each
:class:`DerivativeRequest` and returns the file the later nodes read (without ``sink`` they read
the node files; the data are the same).

Corrected by default (``upstream_defects="reproduce"`` gives fMRIPrep's result bit for bit):
PIPE-20 at the pipeline level: when a BOLD run has a field map but its metadata give no readout
time or no phase-encoding direction, fMRIPrep resamples it without the field map and reports
the correction as made; larmorx stops with fMRIPrep's own message for a missing readout time
(rule B, :class:`FieldmapMetadataError`). The steps' own defects (PIPE-03, PIPE-21, ANTs', FSL's,
sdcflows') follow the same switch in their ports.
"""

from __future__ import annotations

import os
import re
import warnings
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "MISSING_READOUT_TIME",
    "BoldFitFieldmapResult",
    "DerivativeRequest",
    "FieldmapEstimates",
    "FieldmapMetadataError",
    "ResamplingFieldmapResult",
    "bold_fit_fieldmap",
    "fieldmap_jacobian",
    "gen_inverses",
    "resampling_fieldmap",
    "select_ref",
]

PathLike = str | os.PathLike[str]

#: fMRIPrep's message when a readout time is missing (``workflows/base.py``).
MISSING_READOUT_TIME = (
    "Missing readout time information. See documentation for `--fallback-total-readout-time`."
)
#: sdcflows' estimator types (``sdcflows.fieldmaps.EstimatorType``' names).
ESTIMATOR_TYPES = ("PHASEDIFF", "MAPPED", "PEPOLAR", "ANAT")


class FieldmapMetadataError(ValueError):
    """A BOLD run has a field map but its metadata give no readout time or no phase-encoding
    direction (PIPE-20, corrected: fMRIPrep would resample it without the field map)."""


@dataclass(frozen=True)
class FieldmapEstimates:
    """The field-map estimators' outputs as fMRIPrep's field-map buffers give them to every
    BOLD workflow, collated with ``fmap_id`` (one item per estimator): the reference
    (``fmap_ref``), the B-spline coefficient files (``fmap_coeff``, a list per estimator), the
    mask (``fmap_mask``) and the method's description (``sdc_method``). The field maps
    themselves (``fmap``) only feed the reports."""

    fmap_id: Sequence[str]
    fmap_ref: Sequence[PathLike]
    fmap_coeff: Sequence[Sequence[PathLike]]
    fmap_mask: Sequence[PathLike] = ()
    sdc_method: Sequence[str] = ()
    fmap: Sequence[PathLike] = ()


@dataclass(frozen=True)
class DerivativeRequest:
    """One file fMRIPrep writes as a derivative with ``DerivativesDataSink`` before its later
    nodes read it: ``role`` (``"boldref2fmap_xfm"``, ``"coreg_boldref"``, ``"bold_mask"``), the
    node's file (``in_file``), the BOLD file it is named after (``source_file``), the sink's
    entities and settings as fMRIPrep sets them (``entities``: ``desc``, ``suffix``, ``from``,
    ``to``, ``mode``, ``extension``, ``compress``, ``dismiss_entities``; ``echo`` is dismissed as
    with fMRIPrep's default ``--echo-idx``) and the files it was made from (``sources``: the
    ``Sources`` of its sidecar)."""

    role: str
    in_file: str
    source_file: str
    entities: Mapping[str, Any]
    sources: tuple[str, ...] = ()


#: The callable that writes a :class:`DerivativeRequest` and returns the file to read on.
DerivativeSink = Callable[[DerivativeRequest], PathLike]


def select_ref(sbref_files: Any, boldref_files: Any) -> str:
    """fMRIPrep's ``_select_ref``: the first single-band reference, else the first BOLD
    reference (``listify(sbref_files or boldref_files)[0]``)."""
    refs = sbref_files or boldref_files
    return _listify(refs)[0]


def gen_inverses(inlist: Any) -> list[bool]:
    """fMRIPrep's ``_gen_inverses``: ``[True]`` (the BOLD-to-field-map transform inverted)
    followed by one ``False`` per transform from the BOLD reference to the target."""
    if not inlist:
        return [True]
    return [True] + [False] * len(_listify(inlist))


def fieldmap_jacobian(
    method: str | None, force: Sequence[str] = (), ignore: Sequence[str] = ()
) -> bool:
    """fMRIPrep's rule for the Jacobian of its resamplings (``workflows/base.py``): without a
    field map (``method`` None) never; with ``fmap-jacobian`` in ``--force`` always; with it in
    ``--ignore`` never; otherwise only for a PEPOLAR estimator. ``method`` is the estimator
    type's name (``"PHASEDIFF"``, ``"MAPPED"``, ``"PEPOLAR"``, ``"ANAT"``)."""
    if not method:
        return False
    if method not in ESTIMATOR_TYPES:
        raise ValueError(
            f"method must be one of {', '.join(ESTIMATOR_TYPES)} (sdcflows' EstimatorType), "
            f"not {method!r}"
        )
    if "fmap-jacobian" in force:
        return True
    if "fmap-jacobian" in ignore:
        return False
    return method == "PEPOLAR"


def _listify(value: Any) -> Any:
    """niworkflows' ``listify``."""
    if value is None:
        return value
    if isinstance(value, (str, bytes, Path)):
        return [str(value)]
    if isinstance(value, (int, float)):
        return [value]
    return list(value)


def _merge(*inputs: Any) -> list[Any]:
    """nipype's ``Merge`` (``vstack``): the defined inputs in order, lists flattened once."""
    out: list[Any] = []
    for value in inputs:
        if value is None:
            continue
        if isinstance(value, list | tuple):
            out.extend(value)
        else:
            out.append(value)
    return out


def _distortion(
    metadata: Mapping[str, Any],
    in_file: Any,
    fallback: float | str | None,
    bold: str,
    fieldmap_id: str,
    upstream_defects: UpstreamDefectsName,
) -> tuple[float | None, str | None]:
    """``DistortionParameters`` and PIPE-20 at the pipeline level."""
    from larmorx.transforms import distortion_parameters

    with warnings.catch_warnings():
        # the interface's own PIPE-20 warning: the pipeline decides here
        warnings.filterwarnings("ignore", message="PIPE-20", category=UserWarning)
        dp = distortion_parameters(metadata, in_file, fallback, upstream_defects=upstream_defects)
    if dp.readout_time is not None and dp.pe_direction is not None:
        return dp.readout_time, dp.pe_direction
    what = "readout time" if dp.readout_time is None else "phase-encoding direction"
    detail = (
        f"the field map {fieldmap_id!r} of {os.path.basename(bold)} cannot be applied: its "
        f"metadata give no {what}"
    )
    if upstream_defects == "fix":
        message = (
            MISSING_READOUT_TIME
            if dp.readout_time is None
            else "Missing phase-encoding direction (PhaseEncodingDirection) information."
        )
        raise FieldmapMetadataError(f"{message} ({detail}).")
    _defects.warn(
        [
            (
                "PIPE-20",
                "fMRIPrep DistortionParameters",
                upstream_defects,
                detail + "; the run is resampled without its field map, as fMRIPrep does",
            )
        ],
        stacklevel=3,
    )
    return dp.readout_time, dp.pe_direction


def _sanitized(fieldmap_id: str) -> str:
    """fMRIPrep's ``re.sub(r'[^a-zA-Z0-9]', '', fieldmap_id)`` (the ``to`` entity)."""
    return re.sub(r"[^a-zA-Z0-9]", "", fieldmap_id)


# ------------------------------------------------------------------------------------------
# init_bold_fit_wf's field-map nodes


@dataclass(frozen=True)
class BoldFitFieldmapResult:
    """The field-map nodes of fMRIPrep's ``init_bold_fit_wf``.

    ``sdc_boldref`` (``fmapref_buffer``: the reference the field map is reconstructed on and
    the unwarped reference is resampled onto), the selected estimator (``fmap_ref``,
    ``fmap_coeff``, ``fmap_mask``, ``sdc_method``: ``fmap_select``), ``boldref2fmap_xfm`` (the
    transform the later nodes read: the derivative when ``sink`` wrote it), ``registration``
    (``fmapreg_wf``) and ``itk_mat2txt`` (``ConcatenateXFMs``) when it was estimated (``None``
    when given), ``fieldmap`` (``boldref_fmap``'s file, Hz), ``readout_time`` and
    ``pe_direction`` (``distortion_params``), ``unwarped_boldref`` (``unwarp_boldref``'s file),
    ``coreg_boldref`` (the coregistration reference the later nodes read), ``skullstrip``
    (``skullstrip_bold_wf``), ``bold_mask`` (its mask as the later nodes read it) and the
    derivatives fMRIPrep writes (``derivatives``)."""

    sdc_boldref: str
    fmap_ref: str
    fmap_coeff: tuple[str, ...]
    fmap_mask: str
    sdc_method: str | None
    boldref2fmap_xfm: str
    registration: Any
    itk_mat2txt: Any
    fieldmap: str
    readout_time: float | None
    pe_direction: str | None
    unwarped_boldref: str
    coreg_boldref: str
    skullstrip: Any
    bold_mask: str
    derivatives: tuple[DerivativeRequest, ...] = field(default=())


def bold_fit_fieldmap(
    bold_file: PathLike,
    metadata: Mapping[str, Any],
    fieldmap_id: str,
    estimates: FieldmapEstimates,
    hmc_boldref: PathLike,
    coreg_ref: PathLike,
    coreg_mask: PathLike,
    *,
    out_dir: PathLike,
    sbref_files: Sequence[PathLike] | PathLike | None = None,
    boldref2fmap_xfm: PathLike | None = None,
    jacobian: bool = False,
    fallback_total_readout_time: float | str | None = None,
    sloppy: bool = False,
    debug: bool = False,
    ants_random_seed: int | None = None,
    sink: DerivativeSink | None = None,
    fsl_implementation: str = "auto",
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> BoldFitFieldmapResult:
    """The field-map nodes of fMRIPrep's ``init_bold_fit_wf`` for one BOLD run (stage 3 and
    stage 4's field-map branch), in fMRIPrep's order, each in its node folder under
    ``out_dir``.

    Parameters
    ----------
    bold_file
        The BOLD run (the shortest echo of a multi-echo run): ``distortion_params``' ``in_file``
        and the derivatives' source.
    metadata
        Its BIDS metadata (``layout.get_metadata(bold_file)``).
    fieldmap_id
        The estimator assigned to the run (fMRIPrep's ``estimator_map``).
    estimates
        The estimators' outputs (:class:`FieldmapEstimates`).
    hmc_boldref
        The motion-correction reference (stage 1's ``desc-hmc_boldref``).
    coreg_ref, coreg_mask
        niworkflows' ``init_enhance_and_skullstrip_bold_wf`` of the reference:
        ``bias_corrected_file`` and ``mask_file`` (the registration's target and mask, and the
        image unwarped).
    out_dir
        The folder of the BOLD fit's nodes (``fmapreg_wf/``, ``itk_mat2txt/``,
        ``boldref_fmap/``, ``unwarp_boldref/``, ``skullstrip_bold_wf/``).
    sbref_files
        The single-band references fMRIPrep's ``fmapref_buffer`` receives (the raw ones sorted
        by echo time, or the reference ``raw_sbref_wf`` made of a 4D one); the first is the
        stage's reference when given.
    boldref2fmap_xfm
        A transform from the BOLD reference to the field map found among the precomputed
        derivatives: the registration is skipped ("Found fieldmap transform - skipping
        registration").
    jacobian
        ``unwarp_boldref``'s Jacobian (:func:`fieldmap_jacobian`).
    fallback_total_readout_time
        fMRIPrep's ``--fallback-total-readout-time``: ``"estimated"`` or a readout time (s).
    sloppy, debug, ants_random_seed
        ``--sloppy`` (sdcflows' testing registration settings), ``--debug fieldmaps`` (the
        registration's warped images) and the seed ANTs reads (fMRIPrep's
        ``ANTS_RANDOM_SEED``: unseeded, ANTs seeds its sampling from the clock).
    sink
        Writes a derivative and returns the file the later nodes read (see the module).
    fsl_implementation
        BET's and fslmaths' implementation in ``skullstrip_bold_wf`` (``"auto"``,
        ``"replica"``, ``"clean-room"``).
    n_threads
        Worker threads (fMRIPrep's ``omp_nthreads``); the results do not depend on it.
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"`` (fMRIPrep's result bit for bit; see the module).

    Raises
    ------
    FieldmapMetadataError
        No readout time or phase-encoding direction (PIPE-20, ``"fix"``).
    """
    from larmorx._nipype import fname_presuffix
    from larmorx.io.nibabel_writer import save_nibabel
    from larmorx.niworkflows import concatenate_xfms, key_select, skullstrip_bold_workflow
    from larmorx.sdcflows import coeff2epi_workflow
    from larmorx.transforms import reconstruct_fieldmap_node, resample_series

    _defects.check(upstream_defects)
    bold = os.path.abspath(os.fspath(bold_file))
    root = os.path.abspath(os.fspath(out_dir))
    os.makedirs(root, exist_ok=True)

    def node(*names: str) -> str:
        p = os.path.join(root, *names)
        os.makedirs(p, exist_ok=True)
        return p

    def write(request: DerivativeRequest) -> str:
        return request.in_file if sink is None else os.fspath(sink(request))

    sbrefs = [os.fspath(s) for s in _listify(sbref_files)] if sbref_files else []
    sdc_boldref = select_ref(sbrefs, os.fspath(hmc_boldref))
    fields: dict[str, Sequence[Any]] = {
        "fmap_ref": [os.fspath(f) for f in estimates.fmap_ref],
        "fmap_coeff": [[os.fspath(c) for c in _listify(cs)] for cs in estimates.fmap_coeff],
        "fmap_mask": [os.fspath(f) for f in estimates.fmap_mask],
        "sdc_method": list(estimates.sdc_method),
    }
    selected = key_select(list(estimates.fmap_id), fieldmap_id, **fields)
    fmap_ref, fmap_coeff = selected["fmap_ref"], list(selected["fmap_coeff"])
    derivatives: list[DerivativeRequest] = []

    registration = itk = None
    if boldref2fmap_xfm is None:
        registration = coeff2epi_workflow(
            os.fspath(coreg_ref),
            os.fspath(coreg_mask),
            fmap_ref,
            selected["fmap_mask"],
            sloppy=sloppy,
            debug=debug,
            out_dir=node("fmapreg_wf"),
            ants_random_seed=ants_random_seed,
            upstream_defects=upstream_defects,
            n_threads=n_threads,
        )
        itk = concatenate_xfms(
            list(registration.target2fmap_xfm), out_fmt="itk", out_dir=node("itk_mat2txt")
        )
        request = DerivativeRequest(
            "boldref2fmap_xfm",
            os.fspath(itk.out_xfm),
            bold,
            {
                "mode": "image",
                "desc": "fmap",
                "suffix": "xfm",
                "extension": ".txt",
                "dismiss_entities": ("part", "echo"),
                "from": "boldref",
                "to": _sanitized(fieldmap_id),
            },
            (sdc_boldref, fmap_ref),
        )
        derivatives.append(request)
        xfm = write(request)
    else:
        xfm = os.path.abspath(os.fspath(boldref2fmap_xfm))

    recon = reconstruct_fieldmap_node(
        fmap_coeff,
        sdc_boldref,
        fmap_ref,
        [xfm],
        [True],
        out_dir=node("boldref_fmap"),
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    assert recon.out_file is not None
    ro_time, pe_dir = _distortion(
        metadata,
        bold,
        fallback_total_readout_time,
        bold,
        fieldmap_id,
        upstream_defects,
    )
    unwarped = resample_series(
        os.fspath(coreg_ref),
        sdc_boldref,
        None,
        inverse=[False],
        fieldmap=os.fspath(recon.out_file),
        pe_dir=pe_dir,
        ro_time=ro_time,
        jacobian=jacobian,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    unwarped_file = fname_presuffix(
        os.fspath(coreg_ref), suffix="resampled", newpath=node("unwarp_boldref")
    )
    save_nibabel(unwarped, unwarped_file, n_threads=n_threads)
    request = DerivativeRequest(
        "coreg_boldref",
        unwarped_file,
        bold,
        {
            "desc": "coreg",
            "suffix": "boldref",
            "compress": True,
            "dismiss_entities": ("echo",),
        },
        (sdc_boldref, xfm, *fmap_coeff),
    )
    derivatives.append(request)
    coreg_boldref = write(request)
    skull = skullstrip_bold_workflow(
        coreg_boldref,
        out_dir=node("skullstrip_bold_wf"),
        bet_implementation=fsl_implementation,
        maths_implementation=fsl_implementation,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )
    assert skull.mask_file is not None
    request = DerivativeRequest(
        "bold_mask",
        skull.mask_file,
        bold,
        {"desc": "brain", "suffix": "mask", "compress": True, "dismiss_entities": ("echo",)},
        (coreg_boldref,),
    )
    derivatives.append(request)
    bold_mask = write(request)
    return BoldFitFieldmapResult(
        sdc_boldref=sdc_boldref,
        fmap_ref=fmap_ref,
        fmap_coeff=tuple(fmap_coeff),
        fmap_mask=selected["fmap_mask"],
        sdc_method=selected["sdc_method"],
        boldref2fmap_xfm=xfm,
        registration=registration,
        itk_mat2txt=itk,
        fieldmap=os.fspath(recon.out_file),
        readout_time=ro_time,
        pe_direction=pe_dir,
        unwarped_boldref=unwarped_file,
        coreg_boldref=coreg_boldref,
        skullstrip=skull,
        bold_mask=bold_mask,
        derivatives=tuple(derivatives),
    )


# ------------------------------------------------------------------------------------------
# init_bold_native_wf's and init_bold_volumetric_resample_wf's field-map nodes


@dataclass(frozen=True)
class ResamplingFieldmapResult:
    """The field-map nodes of a resampling workflow: the selected estimator (``fmap_ref``,
    ``fmap_coeff``), the transforms from the target to the field map with their inversion flags
    (``transforms``, ``inverse``: ``fmap2target``, ``inverses``), the field map on the target
    grid (``fieldmap``, an image, and its file ``out_file`` with ``out_dir``) and the
    distortion parameters (``readout_time``, ``pe_direction``) the resampling takes."""

    fmap_ref: str
    fmap_coeff: tuple[str, ...]
    transforms: tuple[str, ...]
    inverse: tuple[bool, ...]
    fieldmap: Any
    out_file: str | None
    readout_time: float | None
    pe_direction: str | None


def resampling_fieldmap(
    fieldmap_id: str,
    estimates: FieldmapEstimates,
    target_ref_file: PathLike,
    boldref2fmap_xfm: PathLike,
    metadata: Mapping[str, Any],
    in_file: PathLike | None = None,
    boldref2target: Sequence[PathLike] | PathLike | None = None,
    *,
    fallback_total_readout_time: float | str | None = None,
    out_dir: PathLike | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ResamplingFieldmapResult:
    """The field-map nodes of fMRIPrep's ``init_bold_native_wf`` (``boldref2target`` None: the
    field map on the coregistration reference, ``target_ref_file``) and of an
    ``init_bold_volumetric_resample_wf`` (``boldref2target``: the transforms from the BOLD
    reference to the target, e.g. ``[boldref2anat]`` or ``[boldref2anat, anat2std]``;
    ``target_ref_file``: ``gen_ref``'s grid): ``fmap_select``, ``distortion_params``
    (``in_file``: the raw BOLD in ``bold_native_wf``, the minimally processed one in the
    resampling workflows), the transforms ``[boldref2fmap_xfm, *boldref2target]`` with
    :func:`gen_inverses`, and ``ReconstructFieldmap`` written to ``out_dir`` when given.
    fMRIPrep gives the resampling workflows no field map for a multi-echo run (the caller's
    rule). ``FieldmapMetadataError`` as :func:`bold_fit_fieldmap`."""
    from larmorx.niworkflows import key_select
    from larmorx.transforms import reconstruct_fieldmap_node

    _defects.check(upstream_defects)
    selected = key_select(
        list(estimates.fmap_id),
        fieldmap_id,
        fmap_ref=[os.fspath(f) for f in estimates.fmap_ref],
        fmap_coeff=[[os.fspath(c) for c in _listify(cs)] for cs in estimates.fmap_coeff],
    )
    to_target = [os.fspath(x) for x in (_listify(boldref2target) or [])]
    transforms = _merge(os.fspath(boldref2fmap_xfm), to_target)
    inverse = gen_inverses(to_target)
    ro_time, pe_dir = _distortion(
        metadata,
        None if in_file is None else os.fspath(in_file),
        fallback_total_readout_time,
        os.fspath(in_file) if in_file is not None else "the BOLD run",
        fieldmap_id,
        upstream_defects,
    )
    recon = reconstruct_fieldmap_node(
        list(selected["fmap_coeff"]),
        target_ref_file,
        selected["fmap_ref"],
        transforms,
        inverse,
        out_dir=out_dir,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    return ResamplingFieldmapResult(
        fmap_ref=selected["fmap_ref"],
        fmap_coeff=tuple(selected["fmap_coeff"]),
        transforms=tuple(transforms),
        inverse=tuple(inverse),
        fieldmap=recon.fieldmap,
        out_file=None if recon.out_file is None else os.fspath(recon.out_file),
        readout_time=ro_time,
        pe_direction=pe_dir,
    )
