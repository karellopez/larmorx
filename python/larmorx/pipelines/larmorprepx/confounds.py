# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's confound estimation of a BOLD run as a plain function (larmorprepx).

Ported from fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; the same in 25.2.6, the oracle
runs', but ``FSLMotionParams``: ``fmriprep_version``): ``workflows/bold/confounds.py``
(``init_bold_confs_wf``, ``_binary_union``, ``_get_zooms``, its ``_select_cols``) and
``workflows/bold/base.py`` (``bold_confounds_wf``'s inputs and the ``ds_confounds`` sink). No
nipype, no graph: each node is a call of its larmorx port, in fMRIPrep's order, its files in a
folder named as fMRIPrep's node (a MapNode's items in ``<node>/mapflow/_<node><i>``):

- ``t1w_mask_tfm`` (niworkflows' ``FixHeaderApplyTransforms``: ``antsApplyTransforms
  -n MultiLabel`` of the T1w mask onto the BOLD mask through the inverted coregistration, the
  reference's x-forms copied), ``union_mask`` (:func:`larmorx.confounds.binary_union`),
  ``dilated_mask`` and ``subtract_mask`` (niworkflows' ``BinaryDilation`` radius 2 and
  ``BinarySubtraction``: the crown);
- ``dvars`` (nipype's ``ComputeDVARS``), ``motion_params`` (``FSLMotionParams``), ``fdisp``
  (fMRIPrep's ``FramewiseDisplacement``), ``rmsd`` (``FSLRMSDeviation``);
- ``acc_masks`` (``aCompCorMasks``), ``acc_msk_tfm`` (``-n Gaussian``), ``acc_msk_brain``
  (``ApplyMask``), ``acc_msk_bin`` (``Binarize`` above 0.99), ``acompcor``, ``crowncompcor``
  and ``tcompcor`` (niworkflows' ``RobustACompCor``/``RobustTCompCor``), ``rename_acompcor``;
- ``merge_rois`` and ``signals`` (``SignalExtraction`` of the brain, CSF, WM, CSF + WM and
  tCompCor masks), ``add_dvars_header``, ``add_std_dvars_header``, ``concat``
  (``GatherConfounds``);
- the metadata: ``tcc_metadata_filter``/``acc_metadata_filter`` (``FilterDropped``),
  ``tcc_metadata_fmt``, ``acc_metadata_fmt``, ``crowncc_metadata_fmt`` (``TSV2JSON``),
  ``merge_confound_metadata`` and ``merge_confound_metadata2`` (``DictMerge``);
- ``model_expansion`` (``ExpandModel``, ``(dd1(rps + wm + csf + gsr))^^2 + others``) and
  ``spike_regressors`` (``SpikeRegressors``): the confounds file;
- ``ds_confounds`` (``base.py``'s ``DerivativesDataSink``: ``desc-confounds_timeseries.tsv`` and
  its JSON) when ``fmriprep_dir`` is given.

The reportlets (``rois_plot`` with ``mrg_compcor``, ``compcor_plot`` with
``merge_compcor_metadata``, ``conf_corr_plot``, their ``ds_report_*`` sinks) and
``init_carpetplot_wf`` go with larmorprepx's reports: their inputs are returned
(:class:`ConfoundsReportInputs`), not drawn. nipype deletes the files of outputs nothing reads
(the aCompCor and crown pre-filter files); larmorx keeps them.

Corrected by default (``upstream_defects="reproduce"`` gives fMRIPrep's result bit for bit):

- PIPE-41 (rule A): fMRIPrep sets ``merge_confound_metadata``'s third input to the tissue
  signals' metadata (``{"Method": "Mean"}`` for ``global_signal``, ``csf``, ``white_matter``,
  ``csf_wm`` and ``tcompcor``) and then connects the crown's metadata to the same input, which
  replaces them: the confounds' JSON never describes those five columns. larmorx keeps both.

Kept as fMRIPrep does it (documented, no switch):

- PIPE-49 (rule C): the crown's metadata are not filtered (``FilterDropped`` runs on the
  tCompCor and aCompCor metadata only), so the JSON lists the crown's components not retained
  as ``dropped_<k>``; fMRIPrep's documentation ("Entries that are not saved in the data file for
  denoising are still stored in metadata with the ``dropped`` prefix") describes that.
- PIPE-79 (rule D, open: measured when fMRIPrep runs with FreeSurfer exist): ``acc_masks`` takes
  ``is_aseg=freesurfer``, although the tissue maps are always FAST's partial-volume maps, so with
  FreeSurfer the aseg path smooths them again and keeps as grey matter only the voxels of
  fraction exactly 1. larmorx keeps fMRIPrep's wiring in both modes until it is measured.

The steps' own defects (PIPE-43 to PIPE-78, PD-01 to PD-03, NP-10, GLIBC-07, ...) follow the
same switch in their ports; the libraries' versions (``fmriprep_version``,
``niworkflows_version``, ``numpy_version``, ``scipy_version``, ``pandas_version``) select the
arithmetic and the texts of the releases fMRIPrep ran with (the newest by default; fMRIPrep
25.2.6's image: ``"25.2.6"``, ``"1.15.2"``, ``"2.2.6"``, ``"1.15.2"``, ``"2.2.3"``).
"""

from __future__ import annotations

import contextvars
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "MODEL_FORMULA",
    "MOTION_METADATA",
    "SIGNALS_CLASS_LABELS",
    "BoldConfoundsResult",
    "ConfoundsReportInputs",
    "bold_confounds",
    "bold_confounds_description",
    "ds_confounds",
    "select_cols",
]

PathLike = str | os.PathLike[str]

#: The newest releases larmorx follows by default.
FMRIPREP_NEWEST = "25.2.7"
NIWORKFLOWS_NEWEST = "1.15.4"
NUMPY_NEWEST = "2.5.4"
SCIPY_NEWEST = "1.18.1"
PANDAS_NEWEST = "3.0.6"

#: ``signals``' class labels (``SignalExtraction``), in ``merge_rois``' order.
SIGNALS_CLASS_LABELS = ("global_signal", "csf", "white_matter", "csf_wm", "tcompcor")

#: ``model_expansion``'s formula.
MODEL_FORMULA = "(dd1(rps + wm + csf + gsr))^^2 + others"

#: ``merge_confound_metadata``'s fourth input: the motion parameters' and FD's metadata.
MOTION_METADATA: Mapping[str, Mapping[str, str]] = {
    "trans_x": {"Description": "Translation along left-right axis.", "Units": "mm"},
    "trans_y": {"Description": "Translation along anterior-posterior axis.", "Units": "mm"},
    "trans_z": {"Description": "Translation along superior-inferior axis.", "Units": "mm"},
    "rot_x": {
        "Description": 'Rotation about left-right axis. Also known as "pitch".',
        "Units": "rad",
    },
    "rot_y": {
        "Description": 'Rotation about anterior-posterior axis. Also known as "roll".',
        "Units": "rad",
    },
    "rot_z": {
        "Description": 'Rotation about superior-inferior axis. Also known as "yaw".',
        "Units": "rad",
    },
    "framewise_displacement": {"Units": "mm"},
}


def bold_confounds_description(
    regressors_fd_th: float, regressors_dvars_th: float, freesurfer: bool = False
) -> str:
    """fMRIPrep's description of the workflow (``workflow.__desc__``, the boilerplate's
    paragraph), with the thresholds as Python writes them and the grey-matter mask's origin."""
    gm_desc = (
        "dilating a GM mask extracted from the FreeSurfer's *aseg* segmentation"
        if freesurfer
        else "thresholding the corresponding partial volume map at 0.05"
    )
    return f"""\
Several confounding time-series were calculated based on the
*preprocessed BOLD*: framewise displacement (FD), DVARS and
three region-wise global signals.
FD was computed using two formulations following Power (absolute sum of
relative motions, @power_fd_dvars) and Jenkinson (relative root mean square
displacement between affines, @mcflirt).
FD and DVARS are calculated for each functional run, both using their
implementations in *Nipype* [following the definitions by @power_fd_dvars].
The three global signals are extracted within the CSF, the WM, and
the whole-brain masks.
Additionally, a set of physiological regressors were extracted to
allow for component-based noise correction [*CompCor*, @compcor].
Principal components are estimated after high-pass filtering the
*preprocessed BOLD* time-series (using a discrete cosine filter with
128s cut-off) for the two *CompCor* variants: temporal (tCompCor)
and anatomical (aCompCor).
tCompCor components are then calculated from the top 2% variable
voxels within the brain mask.
For aCompCor, three probabilistic masks (CSF, WM and combined CSF+WM)
are generated in anatomical space.
The implementation differs from that of Behzadi et al. in that instead
of eroding the masks by 2 pixels on BOLD space, a mask of pixels that
likely contain a volume fraction of GM is subtracted from the aCompCor masks.
This mask is obtained by {gm_desc}, and it ensures components are not extracted
from voxels containing a minimal fraction of GM.
Finally, these masks are resampled into BOLD space and binarized by
thresholding at 0.99 (as in the original implementation).
Components are also calculated separately within the WM and CSF masks.
For each CompCor decomposition, the *k* components with the largest singular
values are retained, such that the retained components' time series are
sufficient to explain 50 percent of variance across the nuisance mask (CSF,
WM, combined, or temporal). The remaining components are dropped from
consideration.
The head-motion estimates calculated in the correction step were also
placed within the corresponding confounds file.
The confound time series derived from head motion estimates and global
signals were expanded with the inclusion of temporal derivatives and
quadratic terms for each [@confounds_satterthwaite_2013].
Frames that exceeded a threshold of {regressors_fd_th} mm FD or
{regressors_dvars_th} standardized DVARS were annotated as motion outliers.
Additional nuisance timeseries are calculated by means of principal components
analysis of the signal found within a thin band (*crown*) of voxels around
the edge of the brain, as proposed by [@patriat_improved_2017].
"""


def select_cols(table: PathLike) -> list[str]:
    """fMRIPrep's ``_select_cols`` (``conf_corr_plot``'s columns): the names of the table's
    columns as pandas' ``read_table(table, nrows=2)`` gives them (repeated names renamed), but
    those starting with ``a_comp_cor_``, ``t_comp_cor_`` or ``std_dvars``."""
    from larmorx.confounds import _tables

    records = _tables._records(_tables._open_text(os.fspath(table)))
    if not records:
        raise _tables.EmptyDataError("No columns to parse from file")
    names = _tables._header(records[0][1])
    return [c for c in names if not c.startswith(("a_comp_cor_", "t_comp_cor_", "std_dvars"))]


@dataclass(frozen=True)
class ConfoundsReportInputs:
    """What fMRIPrep's reportlets of the workflow receive (drawn by larmorprepx's reports, not
    here). ``rois``: ``rois_plot``'s ``in_file`` (the BOLD), ``in_mask`` (the BOLD mask) and
    ``in_rois`` (``mrg_compcor``: the tCompCor mask, the last aCompCor mask (CSF + WM) and the
    crown), its colours ``b``, ``magenta``, ``g``; ``compcor_metadata``: the tCompCor, aCompCor
    and crown metadata files (``merge_compcor_metadata``, a ``Merge(2)`` with the crown wired to
    a third input, passes the first two only: PIPE-42, decided with the reports), for
    ``CompCorVariancePlot`` with ``variance_thresholds`` (0.5, 0.7, 0.9) and
    ``metadata_sources`` (``tCompCor``, ``aCompCor``, ``crownCompCor``); ``confounds_file``
    and ``columns`` (``concat``'s table and :func:`select_cols` of it), ``skip_vols``, for
    ``ConfoundsCorrelationPlot`` (``reference_column="global_signal"``, ``max_dim=20``); the
    carpet plot's inputs (``carpet_confounds_file``: the confounds file; ``crown_mask``;
    ``acompcor_mask``: the last aCompCor mask)."""

    bold: str
    bold_mask: str
    rois: tuple[str, ...]
    compcor_metadata: tuple[str, ...]
    confounds_file: str
    columns: tuple[str, ...]
    skip_vols: int
    carpet_confounds_file: str
    crown_mask: str
    acompcor_mask: str
    rois_colors: tuple[str, ...] = ("b", "magenta", "g")
    variance_thresholds: tuple[float, ...] = (0.5, 0.7, 0.9)
    metadata_sources: tuple[str, ...] = ("tCompCor", "aCompCor", "crownCompCor")


@dataclass(frozen=True)
class BoldConfoundsResult:
    """fMRIPrep's ``init_bold_confs_wf``'s outputs: ``confounds_file`` (``spike_regressors``'
    table, what ``ds_confounds`` writes as ``desc-confounds_timeseries.tsv``),
    ``confounds_metadata`` (``merge_confound_metadata2``'s dictionary, its JSON),
    ``acompcor_masks`` (the CSF, WM and CSF + WM masks in BOLD space, ``acc_msk_bin``'s
    files), ``tcompcor_mask`` and ``crown_mask``; ``nodes``, every node's outputs by fMRIPrep's
    node name (paths, or the ports' result objects); ``report_inputs``; ``description``
    (:func:`bold_confounds_description`); ``derivatives``, ``ds_confounds``' result when
    ``fmriprep_dir`` was given."""

    confounds_file: str
    confounds_metadata: dict[str, Any]
    acompcor_masks: tuple[str, ...]
    tcompcor_mask: str
    crown_mask: str
    nodes: Mapping[str, Any]
    report_inputs: ConfoundsReportInputs
    description: str
    derivatives: Any = field(default=None)


def _numpy_lapack(numpy_version: str) -> str:
    """The LAPACK release of numpy ``numpy_version``'s OpenBLAS (nibabel's ``set_qform``)."""
    major, minor = (int(v) for v in str(numpy_version).split(".")[:2])
    return "3.12.1" if (major, minor) >= (2, 4) else "3.12.0"


def _ravel(*inputs: Any) -> list[Any]:
    """nipype's ``Merge(ravel_inputs=True)``: the inputs in order, lists flattened."""
    out: list[Any] = []
    for value in inputs:
        if isinstance(value, list | tuple):
            out.extend(value)
        else:
            out.append(value)
    return out


def _apply_transforms(
    cwd: str,
    input_image: str,
    reference_image: str,
    transform: str,
    interpolation: str,
    niworkflows_version: str,
    upstream_defects: str,
    n_threads: int,
) -> str:
    """niworkflows' ``FixHeaderApplyTransforms(interpolation=..., invert_transform_flags=[True])``:
    nipype's ``antsApplyTransforms`` line (``--default-value 0 --float 0``) run by larmorx's
    port, then the reference's x-forms copied onto the output (``_copyxform``)."""
    from larmorx import _nipype
    from larmorx.niworkflows.header import copy_xform

    cmd = _nipype.apply_transforms(
        cwd=cwd,
        input_image=input_image,
        reference_image=reference_image,
        transforms=[transform],
        invert_transform_flags=[True],
        interpolation=interpolation,
    )
    _nipype.run(cmd, n_threads=n_threads, upstream_defects=upstream_defects)
    out = cmd.outputs["output_image"]
    copy_xform(
        reference_image,
        out,
        message=f"FixHeaderApplyTransforms (niworkflows v{niworkflows_version})",
        n_threads=n_threads,
    )
    return out


def ds_confounds(
    confounds_file: PathLike,
    confounds_metadata: Mapping[str, Any],
    source_file: PathLike,
    fmriprep_dir: PathLike,
    *,
    echo_idx: int | Sequence[int] | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """``base.py``'s ``ds_confounds``: niworkflows' ``DerivativesDataSink`` as fMRIPrep
    subclasses it (``out_path_base=''``) with ``base_directory=fmriprep_dir``,
    ``desc='confounds'``, ``suffix='timeseries'``, ``dismiss_entities=dismiss_echo()`` (``echo``
    unless ``--echo-idx`` names one or two echoes: ``echo_idx``) and the metadata as
    ``meta_dict``: ``desc-confounds_timeseries.tsv`` and its JSON (keys sorted, two-space
    indents), named after ``source_file`` (the run's first echo). Its result
    (:class:`larmorx.niworkflows.bids.DerivativesDataSinkResult`)."""
    from larmorx.niworkflows.bids import derivatives_data_sink

    dismiss: list[str] = []
    if echo_idx is None or len(_listify(echo_idx)) > 2:
        dismiss.append("echo")
    return derivatives_data_sink(
        os.fspath(confounds_file),
        os.fspath(source_file),
        base_directory=os.fspath(fmriprep_dir),
        out_path_base="",
        dismiss_entities=dismiss,
        meta_dict=dict(confounds_metadata),
        entities={"desc": "confounds", "suffix": "timeseries"},
        upstream_defects=upstream_defects,
    )


def _listify(value: Any) -> list[Any]:
    """niworkflows' ``listify`` of a number or a sequence."""
    if isinstance(value, list | tuple):
        return list(value)
    return [value]


def bold_confounds(
    bold: PathLike,
    bold_mask: PathLike,
    hmc_boldref: PathLike,
    motion_xfm: PathLike,
    skip_vols: int,
    t1w_mask: PathLike,
    t1w_tpms: Sequence[PathLike],
    boldref2anat_xfm: PathLike,
    *,
    metadata: Mapping[str, Any],
    out_dir: PathLike,
    regressors_all_comps: bool = False,
    regressors_dvars_th: float = 1.5,
    regressors_fd_th: float = 0.5,
    freesurfer: bool = False,
    fmriprep_dir: PathLike | None = None,
    source_file: PathLike | None = None,
    echo_idx: int | Sequence[int] | None = None,
    fmriprep_version: str = FMRIPREP_NEWEST,
    niworkflows_version: str = NIWORKFLOWS_NEWEST,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    pandas_version: str = PANDAS_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> BoldConfoundsResult:
    """fMRIPrep's ``init_bold_confs_wf`` for one BOLD run (``bold_confounds_wf`` in
    ``init_bold_wf``), its nodes in fMRIPrep's order, each in its folder under ``out_dir``.

    Parameters
    ----------
    bold
        The BOLD series after slice timing, head-motion and distortion correction, in its
        reference space (``bold_native_wf``'s ``bold_native``; the optimally combined series of
        a multi-echo run).
    bold_mask, hmc_boldref, motion_xfm, skip_vols, boldref2anat_xfm
        ``bold_fit_wf``'s outputs: the BOLD mask, the head-motion reference, the head-motion
        transforms (ITK text, one per volume), the number of non-steady-state volumes
        (``dummy_scans``) and the BOLD-reference-to-T1w transform.
    t1w_mask, t1w_tpms
        The T1w brain mask and the GM, WM and CSF probability maps (in that order).
    metadata
        The run's BIDS metadata: its ``RepetitionTime`` goes to the three CompCor nodes (without
        it they read the series' header).
    out_dir
        The workflow's folder (fMRIPrep's ``bold_confounds_wf``), which holds the node folders.
    regressors_all_comps, regressors_dvars_th, regressors_fd_th, freesurfer
        fMRIPrep's ``--return-all-components``, ``--dvars-spike-threshold``,
        ``--fd-spike-threshold`` and whether FreeSurfer ran (``acc_masks``' ``is_aseg``:
        PIPE-79).
    fmriprep_dir, source_file, echo_idx
        ``ds_confounds`` (:func:`ds_confounds`) when ``fmriprep_dir`` is given: the derivatives
        folder, the raw BOLD the names come from (the first echo; required then) and
        ``--echo-idx``.
    fmriprep_version, niworkflows_version, numpy_version, scipy_version, pandas_version
        The releases whose arithmetic and texts are reproduced (see the module).
    n_threads
        Worker threads (fMRIPrep's ``omp_nthreads``); the results do not depend on it.
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"`` (fMRIPrep's result bit for bit; see the module).
    """
    _defects.check(upstream_defects)
    if fmriprep_dir is not None and source_file is None:
        raise ValueError("source_file is required with fmriprep_dir (ds_confounds' names)")
    tpms = [os.fspath(p) for p in t1w_tpms]
    run = contextvars.copy_context().run
    return run(
        _bold_confounds,
        os.path.abspath(os.fspath(bold)),
        os.path.abspath(os.fspath(bold_mask)),
        os.path.abspath(os.fspath(hmc_boldref)),
        os.path.abspath(os.fspath(motion_xfm)),
        int(skip_vols),
        os.path.abspath(os.fspath(t1w_mask)),
        [os.path.abspath(p) for p in tpms],
        os.path.abspath(os.fspath(boldref2anat_xfm)),
        dict(metadata),
        os.path.abspath(os.fspath(out_dir)),
        bool(regressors_all_comps),
        regressors_dvars_th,
        regressors_fd_th,
        bool(freesurfer),
        fmriprep_dir,
        source_file,
        echo_idx,
        fmriprep_version,
        niworkflows_version,
        numpy_version,
        scipy_version,
        pandas_version,
        max(int(n_threads), 1),
        upstream_defects,
    )


def _bold_confounds(
    bold: str,
    bold_mask: str,
    hmc_boldref: str,
    motion_xfm: str,
    skip_vols: int,
    t1w_mask: str,
    t1w_tpms: list[str],
    boldref2anat_xfm: str,
    metadata: dict[str, Any],
    root: str,
    regressors_all_comps: bool,
    regressors_dvars_th: float,
    regressors_fd_th: float,
    freesurfer: bool,
    fmriprep_dir: PathLike | None,
    source_file: PathLike | None,
    echo_idx: int | Sequence[int] | None,
    fmriprep_version: str,
    niworkflows_version: str,
    numpy_version: str,
    scipy_version: str,
    pandas_version: str,
    n_threads: int,
    upstream_defects: UpstreamDefectsName,
) -> BoldConfoundsResult:
    from importlib import import_module

    from larmorx.io.nibabel_header import use_lapack

    # The step modules (by module, so that the package's re-exported names, ``compcor`` among
    # them, do not hide them).
    lcc = import_module("larmorx.confounds.compcor")
    ldv = import_module("larmorx.confounds.dvars")
    lfp = import_module("larmorx.confounds.fmriprep")
    lms = import_module("larmorx.confounds.masks")
    lmo = import_module("larmorx.confounds.motion")
    nwc = import_module("larmorx.niworkflows.confounds")
    nwh = import_module("larmorx.niworkflows.header")
    nwi = import_module("larmorx.niworkflows.images")
    nwm = import_module("larmorx.niworkflows.morphology")
    nwn = import_module("larmorx.niworkflows.nibabel")
    nwu = import_module("larmorx.niworkflows.utility")

    # nibabel's set_qform (the x-forms the nodes write) on numpy_version's LAPACK; this runs in
    # a copy of the caller's context, so the setting ends with the call.
    use_lapack(_numpy_lapack(numpy_version))
    mode = upstream_defects
    nodes: dict[str, Any] = {}

    def node(*names: str) -> str:
        p = os.path.join(root, *names)
        os.makedirs(p, exist_ok=True)
        return p

    def mapnode(name: str, i: int) -> str:
        return node(name, "mapflow", f"_{name}{i}")

    numerics = {"numpy_version": numpy_version, "scipy_version": scipy_version}
    tables = {"pandas_version": pandas_version, "upstream_defects": mode}

    # Brain mask: the T1w mask in BOLD space, merged with the BOLD mask; the crown.
    t1w_mask_bold = _apply_transforms(
        node("t1w_mask_tfm"), t1w_mask, bold_mask, boldref2anat_xfm, "MultiLabel",
        niworkflows_version, mode, n_threads,
    )  # fmt: skip
    nodes["t1w_mask_tfm"] = t1w_mask_bold
    union = lms.binary_union(
        bold_mask, t1w_mask_bold, out_dir=node("union_mask"), n_threads=n_threads,
        upstream_defects=mode,
    )  # fmt: skip
    nodes["union_mask"] = union
    dilated = nwm.binary_dilation(
        union, radius=2, out_dir=node("dilated_mask"), n_threads=n_threads, upstream_defects=mode
    )
    nodes["dilated_mask"] = dilated
    crown = nwm.binary_subtraction(
        dilated, union, out_dir=node("subtract_mask"), n_threads=n_threads
    )
    nodes["subtract_mask"] = crown

    # DVARS, motion parameters, framewise displacement, RMS deviation.
    dvars = ldv.compute_dvars_node(
        bold, bold_mask, save_nstd=True, save_std=True, remove_zerovariance=True,
        out_dir=node("dvars"), upstream_defects=mode, n_threads=n_threads, **numerics,
    )  # fmt: skip
    nodes["dvars"] = dvars
    motion = lmo.fsl_motion_params(
        motion_xfm, hmc_boldref, out_dir=node("motion_params"),
        fmriprep_version=fmriprep_version, numpy_version=numpy_version,
        upstream_defects=mode, n_threads=n_threads,
    )  # fmt: skip
    nodes["motion_params"] = motion
    fd = lfp.fmriprep_framewise_displacement(motion.out_file, out_dir=node("fdisp"), **tables)
    nodes["fdisp"] = fd
    rmsd = lmo.fsl_rms_deviation(
        motion_xfm, hmc_boldref, out_dir=node("rmsd"), upstream_defects=mode
    )
    nodes["rmsd"] = rmsd

    # aCompCor's masks: made in T1w space, resampled into BOLD space, masked, binarised.
    acc = lms.acompcor_masks_node(
        t1w_tpms, lms.get_zooms(bold), is_aseg=freesurfer, out_dir=node("acc_masks"),
        numpy_version=numpy_version, n_threads=n_threads, upstream_defects=mode,
    )  # fmt: skip
    nodes["acc_masks"] = acc
    acc_tfm = [
        _apply_transforms(
            mapnode("acc_msk_tfm", i), m, bold_mask, boldref2anat_xfm, "Gaussian",
            niworkflows_version, mode, n_threads,
        )  # fmt: skip
        for i, m in enumerate(acc.out_masks)
    ]
    nodes["acc_msk_tfm"] = acc_tfm
    acc_brain = [
        nwh.apply_mask(m, bold_mask, out_dir=mapnode("acc_msk_brain", i), n_threads=n_threads)
        for i, m in enumerate(acc_tfm)
    ]
    nodes["acc_msk_brain"] = acc_brain
    acc_bin = [
        nwn.binarize(m, 0.99, out_dir=mapnode("acc_msk_bin", i), n_threads=n_threads)
        for i, m in enumerate(acc_brain)
    ]
    nodes["acc_msk_bin"] = acc_bin
    acc_masks = [b.out_file for b in acc_bin]

    # The three CompCor decompositions (niworkflows' Robust* run nipype's interfaces).
    compcor_options: dict[str, Any] = {
        "pre_filter": "cosine",
        "save_pre_filter": True,
        "save_metadata": True,
        "failure_mode": "NaN",
        "ignore_initial_volumes": skip_vols,
        "upstream_defects": mode,
        "n_threads": n_threads,
        **numerics,
    }
    if "RepetitionTime" in metadata:
        compcor_options["repetition_time"] = metadata["RepetitionTime"]
    selection: dict[str, Any] = (
        {"num_components": "all"} if regressors_all_comps else {"variance_threshold": 0.5}
    )
    acompcor = lcc.acompcor(
        bold, acc_masks, components_file="acompcor.tsv", header_prefix="a_comp_cor_",
        mask_names=["CSF", "WM", "combined"], merge_method="none", out_dir=node("acompcor"),
        **selection, **compcor_options,
    )  # fmt: skip
    nodes["acompcor"] = acompcor
    crowncompcor = lcc.acompcor(
        bold, crown, components_file="crown_compcor.tsv", header_prefix="edge_comp_",
        mask_names=["Edge"], merge_method="none", num_components=24,
        out_dir=node("crowncompcor"), **compcor_options,
    )  # fmt: skip
    nodes["crowncompcor"] = crowncompcor
    tcompcor = lcc.tcompcor(
        bold, bold_mask, components_file="tcompcor.tsv", header_prefix="t_comp_cor_",
        percentile_threshold=0.02, out_dir=node("tcompcor"), **selection, **compcor_options,
    )  # fmt: skip
    nodes["tcompcor"] = tcompcor
    assert acompcor.metadata_file is not None and crowncompcor.metadata_file is not None
    assert tcompcor.metadata_file is not None and tcompcor.pre_filter_file is not None
    (tcompcor_mask,) = tcompcor.high_variance_masks
    renamed = lfp.rename_acompcor(
        acompcor.components_file, acompcor.metadata_file, out_dir=node("rename_acompcor"),
        **tables,
    )  # fmt: skip
    nodes["rename_acompcor"] = renamed

    # The tissue signals.
    label_files = _ravel(bold_mask, acc_masks, tcompcor_mask)
    nodes["merge_rois"] = label_files
    signals = nwi.signal_extraction(
        bold, label_files, list(SIGNALS_CLASS_LABELS), out_dir=node("signals"),
        numpy_version=numpy_version, n_threads=n_threads, upstream_defects=mode,
    )  # fmt: skip
    nodes["signals"] = signals

    # The confounds table.
    assert dvars.out_nstd is not None and dvars.out_std is not None
    dvars_file = nwu.add_tsv_header(
        dvars.out_nstd, ["dvars"], out_dir=node("add_dvars_header"), upstream_defects=mode
    )
    nodes["add_dvars_header"] = dvars_file
    std_dvars_file = nwu.add_tsv_header(
        dvars.out_std, ["std_dvars"], out_dir=node("add_std_dvars_header"), upstream_defects=mode
    )
    nodes["add_std_dvars_header"] = std_dvars_file
    concat = lfp.gather_confounds(
        signals=signals, dvars=dvars_file, std_dvars=std_dvars_file, fd=fd, rmsd=rmsd.out_file,
        tcompcor=tcompcor.components_file, acompcor=renamed.components_file,
        crowncompcor=crowncompcor.components_file, cos_basis=tcompcor.pre_filter_file,
        motion=motion.out_file, out_dir=node("concat"), **tables,
    )  # fmt: skip
    nodes["concat"] = concat

    # The metadata.
    tcc_filtered = lfp.filter_dropped(
        tcompcor.metadata_file, out_dir=node("tcc_metadata_filter"), **tables
    )
    nodes["tcc_metadata_filter"] = tcc_filtered
    acc_filtered = lfp.filter_dropped(
        renamed.metadata_file, out_dir=node("acc_metadata_filter"), **tables
    )
    nodes["acc_metadata_filter"] = acc_filtered
    tcc_meta = nwu.tsv2json(
        tcc_filtered, "component", output=None, drop_columns=["mask"],
        additional_metadata={"Method": "tCompCor"}, enforce_case=True,
        out_dir=node("tcc_metadata_fmt"), **tables,
    )  # fmt: skip
    nodes["tcc_metadata_fmt"] = tcc_meta
    acc_meta = nwu.tsv2json(
        acc_filtered, "component", output=None, additional_metadata={"Method": "aCompCor"},
        enforce_case=True, out_dir=node("acc_metadata_fmt"), **tables,
    )  # fmt: skip
    nodes["acc_metadata_fmt"] = acc_meta
    crown_meta = nwu.tsv2json(
        crowncompcor.metadata_file, "component", output=None,
        additional_metadata={"Method": "EdgeRegressor"}, enforce_case=True,
        out_dir=node("crowncc_metadata_fmt"), **tables,
    )  # fmt: skip
    nodes["crowncc_metadata_fmt"] = crown_meta
    assert isinstance(tcc_meta, dict) and isinstance(acc_meta, dict)
    assert isinstance(crown_meta, dict)
    signals_meta = {label: {"Method": "Mean"} for label in SIGNALS_CLASS_LABELS}
    motion_meta = {k: dict(v) for k, v in MOTION_METADATA.items()}
    detail = (
        "the tissue signals' metadata (Method: Mean for "
        + ", ".join(SIGNALS_CLASS_LABELS)
        + ") are "
    )
    if mode == "fix":
        merged = [tcc_meta, acc_meta, signals_meta, crown_meta, motion_meta]
        detail += "kept in the confounds' metadata beside the crown's"
    else:
        # fMRIPrep's Merge(4): in3, set to the signals' metadata, is replaced by the crown's.
        merged = [tcc_meta, acc_meta, crown_meta, motion_meta]
        detail += "lost, as fMRIPrep's merge_confound_metadata replaces them with the crown's"
    _defects.warn([("PIPE-41", "fMRIPrep init_bold_confs_wf", mode, detail)], stacklevel=3)
    nodes["merge_confound_metadata"] = merged
    metadata_out = nwu.dict_merge(merged)
    nodes["merge_confound_metadata2"] = metadata_out

    # The model expansion and the spikes: the confounds file.
    expansion = nwc.expand_model(
        concat.confounds_file, MODEL_FORMULA, out_dir=node("model_expansion"), **tables
    )
    nodes["model_expansion"] = expansion
    confounds_file = nwc.spike_regressors_node(
        expansion, fd_thresh=regressors_fd_th, dvars_thresh=regressors_dvars_th,
        out_dir=node("spike_regressors"), **tables,
    )  # fmt: skip
    nodes["spike_regressors"] = confounds_file

    report = ConfoundsReportInputs(
        bold=bold,
        bold_mask=bold_mask,
        rois=(tcompcor_mask, acc_masks[-1], crown),
        compcor_metadata=(
            tcompcor.metadata_file,
            acompcor.metadata_file,
            crowncompcor.metadata_file,
        ),
        confounds_file=concat.confounds_file,
        columns=tuple(select_cols(concat.confounds_file)),
        skip_vols=skip_vols,
        carpet_confounds_file=confounds_file,
        crown_mask=crown,
        acompcor_mask=acc_masks[-1],
    )
    derivatives = None
    if fmriprep_dir is not None:
        assert source_file is not None
        derivatives = ds_confounds(
            confounds_file, metadata_out, source_file, fmriprep_dir, echo_idx=echo_idx,
            upstream_defects=mode,
        )  # fmt: skip
        nodes["ds_confounds"] = derivatives
    return BoldConfoundsResult(
        confounds_file=confounds_file,
        confounds_metadata=metadata_out,
        acompcor_masks=tuple(acc_masks),
        tcompcor_mask=tcompcor_mask,
        crown_mask=crown,
        nodes=nodes,
        report_inputs=report,
        description=bold_confounds_description(regressors_fd_th, regressors_dvars_th, freesurfer),
        derivatives=derivatives,
    )
