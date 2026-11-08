# SPDX-License-Identifier: Apache-2.0
"""sdcflows' brain-extraction, magnitude and field-map workflows as plain functions.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1, fMRIPrep
25.2.6's): ``sdcflows/workflows/ancillary.py`` (``init_brainextraction_wf``) and
``sdcflows/workflows/fit/fieldmap.py`` (``init_magnitude_wf``, ``init_fmap_wf`` in both
modes, with part A's ``init_phdiff_wf``: :func:`larmorx.sdcflows.phdiff_workflow`). Each node
runs as nipype runs it, in the order the graph connects them, through larmorx's ports:
niworkflows' ``IntensityClip`` and ``IntraModalMerge`` (:mod:`larmorx.niworkflows`), ANTs'
``N4BiasFieldCorrection`` with nipype's command line and its ``copy_header``
(:func:`larmorx.ants.n4_bias_field_correction`), sdcflows' ``BrainExtraction``,
``CheckRegister``, ``CheckB0Units`` and ``BSplineApprox``.

Every node writes its files where nipype's node directory would hold them, under ``out_dir``
(``clipper_pre/clipped.nii.gz``, ``n4/clipped_corrected.nii.gz``, ...), and the next node reads
them back, so each step sees the values nipype's next node reads (nibabel's scaled writer
re-quantises N4's ``float32`` output into the clipped image's ``int16``, for example). Without
``out_dir`` the files go to a temporary directory and only the images are returned (the paths
are ``None``).

The library releases that change results are switches, as in the steps: ``numpy_version`` (the
percentiles of ``IntensityClip`` and ``brain_masker``; numpy 2.2's buffered whole-array sums for
fMRIPrep 25.2.6's image: ``"2.2.6"``) and ``scipy_version`` (the random walker's sparse row
order). The defaults are the newest releases. ``upstream_defects`` is passed to every step:
``"fix"`` (the default) corrects the defects of ANTs' N4 (ANTS-38 to ANTS-40: the zero
background of the clipped image is left out of its fit), of ``brain_masker``, of the phase
steps and of ``BSplineApprox``; ``"reproduce"`` gives sdcflows' results. ``n_threads`` never
changes a result.
"""

from __future__ import annotations

import dataclasses
import os
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import load_nibabel, nibabel_affine
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.niworkflows import IntensityClipResult, IntraModalMergeResult, intensity_clip
from larmorx.niworkflows import intra_modal_merge as _intra_modal_merge
from larmorx.sdcflows.approx import BSplineApproxResult, bspline_approx
from larmorx.sdcflows.bspline import DEFAULT_HF_ZOOMS_MM
from larmorx.sdcflows.masks import NUMPY_NEWEST, SCIPY_NEWEST, BrainMaskerResult, brain_extraction
from larmorx.sdcflows.phase import (
    CheckRegisterResult,
    PhaseResult,
    PhdiffWorkflowResult,
    check_b0_units,
    check_register,
    phdiff_workflow,
)

__all__ = [
    "BrainExtractionWorkflowResult",
    "FmapWorkflowResult",
    "MagnitudeWorkflowResult",
    "brainextraction_workflow",
    "fmap_workflow",
    "magnitude_workflow",
]

#: ``init_fmap_wf``'s ``method`` output, per mode.
METHODS = {
    "phasediff": "FMB (fieldmap-based) - phase-difference map",
    "mapped": "FMB (fieldmap-based) - directly measured B0 map",
}


def _strip(obj: Any, root: str) -> Any:
    """``obj`` (a result dataclass, nested) with every path under ``root`` replaced: ``None``
    for one path, ``()`` for a tuple of them (the temporary directory is gone)."""

    def inside(v: Any) -> bool:
        return isinstance(v, (str, os.PathLike)) and os.fspath(v).startswith(root)

    if not dataclasses.is_dataclass(obj) or isinstance(obj, type):
        return obj
    changes = {}
    for f in dataclasses.fields(obj):
        v = getattr(obj, f.name)
        if inside(v):
            changes[f.name] = None
        elif isinstance(v, tuple) and v and all(inside(x) for x in v):
            changes[f.name] = ()
        elif isinstance(v, tuple) and any(dataclasses.is_dataclass(x) for x in v):
            changes[f.name] = tuple(_strip(x, root) for x in v)
        elif dataclasses.is_dataclass(v):
            changes[f.name] = _strip(v, root)
    return dataclasses.replace(obj, **changes) if changes else obj


class _Workdir:
    """The workflow's directory (``out_dir``, made) or a temporary one."""

    def __init__(self, out_dir: str | os.PathLike[str] | None) -> None:
        self.keep = out_dir is not None
        self._tmp = None if self.keep else tempfile.TemporaryDirectory(prefix="larmorx-wf-")
        self.path = os.path.abspath(os.fspath(out_dir)) if self.keep else self._tmp.name  # type: ignore[union-attr]
        os.makedirs(self.path, exist_ok=True)

    def node(self, *names: str) -> str:
        p = os.path.join(self.path, *names)
        os.makedirs(p, exist_ok=True)
        return p

    def finish(self, result: Any) -> Any:
        if self.keep:
            return result
        out = _strip(result, self.path)
        self._tmp.cleanup()  # type: ignore[union-attr]
        return out

    def fail(self) -> None:
        if self._tmp is not None:
            self._tmp.cleanup()


@dataclass(frozen=True)
class BrainExtractionWorkflowResult:
    """``init_brainextraction_wf``'s nodes: the clipped input (``clipper_pre``), N4's corrected
    image as nipype's ``copy_header`` writes it (``n4``, ``n4_file``) and its iterations, the
    clipped corrected image (``clipper_post``: the workflow's ``out_file``) and the brain mask
    (``masker``: ``out_brain``, ``out_mask``, ``out_probseg``)."""

    clipper_pre: IntensityClipResult
    n4: Image
    n4_file: str | None
    n4_iterations: tuple[Any, ...]
    clipper_post: IntensityClipResult
    masker: BrainMaskerResult

    @property
    def out_file(self) -> str | None:
        return self.clipper_post.out_file

    @property
    def out_brain(self) -> str | None:
        return self.masker.out_brain

    @property
    def out_mask(self) -> str | None:
        return self.masker.out_mask

    @property
    def out_probseg(self) -> str | None:
        return self.masker.out_probseg


def _n4_copy_header(
    in_file: str,
    out_dir: str,
    bspline_dist: float | None,
    upstream_defects: str,
    n_threads: int,
) -> tuple[Image, str, tuple[Any, ...]]:
    """sdcflows' ``n4`` node: nipype's ``N4BiasFieldCorrection(dimension=3, copy_header=True,
    n_iterations=[50] * 5, convergence_threshold=1e-7, shrink_factor=4)``, i.e. ``-d 3
    --input-image <in> --convergence [ 50x50x50x50x50, 1e-07 ] --output <in>_corrected
    --shrink-factor 4`` (``--bspline-fitting [ %g ]`` with ``bspline_dist``), then nipype's
    ``copy_header(keep_dtype=False)``: ANTs' ``float32`` values written with the input's header
    in the input's stored type (nibabel's scaled writer)."""
    from larmorx import ants
    from larmorx.sdcflows.approx import _fname_presuffix

    distance = None if bspline_dist is None else float(f"{float(bspline_dist):g}")
    res = ants.n4_bias_field_correction(
        in_file,
        shrink_factor=4,
        convergence=((50, 50, 50, 50, 50), float("1e-07")),
        bspline_fitting=distance,
        dimension=3,
        n_threads=n_threads,
        upstream_defects=upstream_defects,  # type: ignore[arg-type]
    )
    src = load_nibabel(in_file)
    hdr = src.header
    assert hdr is not None
    data = np.asarray(res.corrected.data, dtype=np.float32)
    out = Image(data, nibabel_affine(hdr), hdr)
    path = _fname_presuffix(in_file, "_corrected", out_dir)
    save_nibabel(out, path, dtype=hdr.data_dtype, n_threads=n_threads)
    return out, path, tuple(res.iterations)


def _brainextraction(
    wd: _Workdir,
    prefix: tuple[str, ...],
    in_file: Any,
    bspline_dist: float | None,
    numpy_version: str,
    scipy_version: str,
    upstream_defects: str,
    n_threads: int,
) -> BrainExtractionWorkflowResult:
    pre = intensity_clip(
        in_file,
        out_dir=wd.node(*prefix, "clipper_pre"),
        numpy_version=numpy_version,
        n_threads=n_threads,
    )
    assert pre.out_file is not None
    n4_img, n4_file, iters = _n4_copy_header(
        pre.out_file, wd.node(*prefix, "n4"), bspline_dist, upstream_defects, n_threads
    )
    post = intensity_clip(
        n4_file,
        p_min=0.01,
        p_max=99.9,
        out_dir=wd.node(*prefix, "clipper_post"),
        numpy_version=numpy_version,
        n_threads=n_threads,
    )
    assert post.out_file is not None
    masker = brain_extraction(
        post.out_file,
        out_dir=wd.node(*prefix, "masker"),
        numpy_version=numpy_version,
        scipy_version=scipy_version,
        upstream_defects=upstream_defects,  # type: ignore[arg-type]
        n_threads=n_threads,
    )
    return BrainExtractionWorkflowResult(pre, n4_img, n4_file, iters, post, masker)


def brainextraction_workflow(
    in_file: str | os.PathLike[str] | Any,
    bspline_dist: float | None = None,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> BrainExtractionWorkflowResult:
    """sdcflows' ``init_brainextraction_wf``: ``IntensityClip()`` (35th to 99.98th percentile)
    of ``in_file`` (a GRE magnitude or an EPI reference), ANTs' N4 bias-field correction
    (``-d 3``, five levels of 50 iterations, threshold 1e-7, shrink factor 4, ``bspline_dist``
    as ``--bspline-fitting``; nipype's ``copy_header``), ``IntensityClip(p_min=0.01,
    p_max=99.9)`` and ``BrainExtraction`` (:func:`larmorx.sdcflows.brain_extraction`).

    The outputs are the result's ``out_file`` (the clipped corrected image), ``out_brain``,
    ``out_mask`` and ``out_probseg``; the nodes' files are in ``out_dir``'s ``clipper_pre``,
    ``n4``, ``clipper_post`` and ``masker`` folders."""
    _defects.check(upstream_defects)
    wd = _Workdir(out_dir)
    try:
        res = _brainextraction(
            wd, (), in_file, bspline_dist, numpy_version, scipy_version, upstream_defects, n_threads
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(res)


@dataclass(frozen=True)
class MagnitudeWorkflowResult:
    """``init_magnitude_wf``'s nodes: the magnitudes' merge and average (``magmrg``) and the
    brain extraction of the average (``brainextraction``); its outputs ``fmap_ref`` (the
    clipped N4-corrected average), ``fmap_mask`` and ``fmap_probseg``."""

    magmrg: IntraModalMergeResult
    brainextraction: BrainExtractionWorkflowResult

    @property
    def fmap_ref(self) -> str | None:
        return self.brainextraction.out_file

    @property
    def fmap_mask(self) -> str | None:
        return self.brainextraction.out_mask

    @property
    def fmap_probseg(self) -> str | None:
        return self.brainextraction.out_probseg


def _magnitude(
    wd: _Workdir,
    prefix: tuple[str, ...],
    magnitude: Any,
    numpy_version: str,
    scipy_version: str,
    upstream_defects: str,
    n_threads: int,
) -> MagnitudeWorkflowResult:
    mrg = _intra_modal_merge(
        magnitude,
        hmc=False,
        to_ras=False,
        out_dir=wd.node(*prefix, "magmrg"),
        upstream_defects=upstream_defects,  # type: ignore[arg-type]
        n_threads=n_threads,
    )
    assert mrg.out_avg is not None
    bx = _brainextraction(
        wd,
        (*prefix, "brainextraction_wf"),
        mrg.out_avg,
        None,
        numpy_version,
        scipy_version,
        upstream_defects,
        n_threads,
    )
    return MagnitudeWorkflowResult(mrg, bx)


def magnitude_workflow(
    magnitude: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    *,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> MagnitudeWorkflowResult:
    """sdcflows' ``init_magnitude_wf``: the magnitude image(s) merged and averaged by
    ``IntraModalMerge(hmc=False, to_ras=False)`` (not reoriented, so the phase-encoding
    direction stays valid) and the average brain-extracted (:func:`brainextraction_workflow`,
    in ``out_dir``'s ``brainextraction_wf`` folder)."""
    _defects.check(upstream_defects)
    wd = _Workdir(out_dir)
    try:
        res = _magnitude(
            wd, (), magnitude, numpy_version, scipy_version, upstream_defects, n_threads
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(res)


@dataclass(frozen=True)
class FmapWorkflowResult:
    """``init_fmap_wf``'s nodes and outputs: ``check_register``, the magnitude workflow
    (``magnitude``), the phase-difference workflow (``phdiff``, mode ``phasediff``) or the
    field maps' merge and units (``fmapmrg``, ``units``, mode ``mapped``), and the B-spline fit
    (``bs_filter``). The outputs: ``fmap`` (the fit's extrapolated field, its field in
    ``debug``), ``fmap_ref``, ``fmap_mask``, ``fmap_coeff`` and ``method``."""

    check_register: CheckRegisterResult
    magnitude: MagnitudeWorkflowResult
    bs_filter: BSplineApproxResult
    method: str
    debug: bool
    phdiff: PhdiffWorkflowResult | None = None
    fmapmrg: IntraModalMergeResult | None = None
    units: PhaseResult | None = None

    @property
    def fmap(self) -> Path | None:
        return self.bs_filter.out_field if self.debug else self.bs_filter.out_extrapolated

    @property
    def fmap_image(self) -> Image | None:
        return self.bs_filter.field if self.debug else self.bs_filter.extrapolated

    @property
    def fmap_ref(self) -> str | None:
        return self.magnitude.fmap_ref

    @property
    def fmap_mask(self) -> str | None:
        return self.magnitude.fmap_mask

    @property
    def fmap_coeff(self) -> tuple[Path, ...]:
        return self.bs_filter.out_coeff


def _fieldmap_spec(fieldmap: Any) -> tuple[list[str], list[dict[str, Any]]]:
    """``inputnode.fieldmap`` (a ``(path, metadata)`` pair or a list of them) unzipped as
    sdcflows' ``unzip`` map node does: the paths and the metadata."""
    items = [fieldmap] if isinstance(fieldmap, tuple) else list(fieldmap)
    files, metas = [], []
    for item in items:
        if not isinstance(item, (tuple, list)) or len(item) != 2:
            raise ValueError("fieldmap holds (path, metadata) pairs")
        files.append(os.fspath(item[0]))
        metas.append(dict(item[1]))
    return files, metas


def fmap_workflow(
    magnitude: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    fieldmap: Any,
    mode: Literal["phasediff", "mapped"] = "phasediff",
    *,
    sloppy: bool = False,
    debug: bool = False,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    prelude_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> FmapWorkflowResult:
    """sdcflows' ``init_fmap_wf``: a field map from a GRE field-mapping acquisition.

    ``magnitude`` is the magnitude image(s); ``fieldmap`` the ``(path, metadata)`` pair(s):
    one phase difference (``EchoTime1``, ``EchoTime2``) or two phases (``EchoTime``) in mode
    ``"phasediff"``, the field map(s) with their ``Units`` (``Hz`` or ``rad/s``) in mode
    ``"mapped"`` (any other mode is sdcflows' direct branch too).

    1. ``CheckRegister`` of the magnitudes and the field-map files
       (:func:`larmorx.sdcflows.check_register`);
    2. :func:`magnitude_workflow` on the checked magnitudes: ``fmap_ref`` and ``fmap_mask``;
    3. ``"phasediff"``: :func:`larmorx.sdcflows.phdiff_workflow` with ``fmap_ref`` and
       ``fmap_mask`` (PRELUDE: ``prelude_implementation``); ``"mapped"``: the field maps
       merged by ``IntraModalMerge(zero_based_avg=False, hmc=False, to_ras=False)`` and
       converted to Hz (``CheckB0Units`` with the first field map's ``Units``);
    4. ``BSplineApprox`` (:func:`larmorx.sdcflows.bspline_approx`) of the field in
       ``fmap_mask`` on the high-frequency grid (16, 16, 10) mm, at 4 mm or coarser with
       ``sloppy``, extrapolated unless ``debug`` (then the output is the fit inside the
       mask).

    The nodes' files are in ``out_dir``'s folders as nipype's: ``check_register``,
    ``magnitude_wf/magmrg``, ``magnitude_wf/brainextraction_wf/...``, ``phdiff_wf`` (part A's
    files in one folder) or ``fmapmrg`` and ``units``, and ``bs_filter``."""
    _defects.check(upstream_defects)
    files, metas = _fieldmap_spec(fieldmap)
    mags = [
        os.fspath(m) for m in (magnitude if isinstance(magnitude, (list, tuple)) else [magnitude])
    ]
    wd = _Workdir(out_dir)
    try:
        chk = check_register(mags, files, out_dir=wd.node("check_register"))
        mag = _magnitude(
            wd,
            ("magnitude_wf",),
            [os.fspath(p) for p in chk.mag_files],
            numpy_version,
            scipy_version,
            upstream_defects,
            n_threads,
        )
        assert mag.fmap_ref is not None and mag.fmap_mask is not None
        phdiff = fmapmrg = units = None
        if mode == "phasediff":
            phdiff = phdiff_workflow(
                mag.fmap_ref,
                [os.fspath(p) for p in chk.fmap_files],
                metas,
                mag.fmap_mask,
                out_dir=wd.node("phdiff_wf"),
                prelude_implementation=prelude_implementation,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
            )
            in_data: Any = phdiff.fieldmap.out_file
        else:
            if "Units" not in metas[0]:
                raise KeyError("Units")
            fmapmrg = _intra_modal_merge(
                [os.fspath(p) for p in chk.fmap_files],
                zero_based_avg=False,
                hmc=False,
                to_ras=False,
                out_dir=wd.node("fmapmrg"),
                upstream_defects=upstream_defects,
                n_threads=n_threads,
            )
            units = check_b0_units(fmapmrg.out_avg, metas[0]["Units"], out_dir=wd.node("units"))
            in_data = units.out_file
        bs = bspline_approx(
            in_data,
            mag.fmap_mask,
            bs_spacing=[DEFAULT_HF_ZOOMS_MM],
            extrapolate=not debug,
            zooms_min=4.0 if sloppy else 1.0,
            out_dir=wd.node("bs_filter"),
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )
        res = FmapWorkflowResult(
            chk,
            mag,
            bs,
            METHODS["phasediff" if mode == "phasediff" else "mapped"],
            debug,
            phdiff,
            fmapmrg,
            units,
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(res)
