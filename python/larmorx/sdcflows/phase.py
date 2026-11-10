# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' phase-difference preparation: phase maps to radians, their difference, the field
map in Hz, the field map's units, and the magnitude/field-map geometry check.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers):
``sdcflows/utils/phasemanip.py`` (``au2rads``, ``subtract_phases``, ``phdiff2fmap``,
``delta_te``), ``sdcflows/interfaces/fmap.py`` (``PhaseMap2rads``, ``SubtractPhases``,
``Phasediff2Fieldmap``, ``CheckB0Units``, ``CheckRegister`` with ``_check_gross_geometry`` and
``_conform_img``) and ``sdcflows/workflows/fit/fieldmap.py`` (``init_phdiff_wf``: PRELUDE
through :func:`larmorx.fsl.prelude`), with nibabel's (5.4.2, MIT) loading, headers and
writer (:func:`larmorx.io.save_nibabel`). The arithmetic is numpy's own, as sdcflows
calls it.

Each function returns its images and, with ``out_dir``, writes sdcflows' files there
(``<in>_rads``, ``<phase1>_phdiff``, ``<in>_fmap``, ``<in>_Hz``; nipype's
``fname_presuffix``), where the interface writes them (``runtime.cwd``).

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows' behaviour): a constant
phase map becomes NaN everywhere in ``au2rads`` (SDC-04: a ``ValueError``), and two phase maps
without ``EchoTime`` fail with Python's ``TypeError`` of a comparison with ``None`` in
``subtract_phases`` (SDC-09: a ``ValueError`` that names the missing metadata).
"""

from __future__ import annotations

import logging
import os
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import header_for_image as _nb_header_for_image
from larmorx.io.nibabel_header import io_orientation as _io_orientation
from larmorx.io.nibabel_header import load_nibabel, nibabel_affine
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.approx import _fname_presuffix, _split_filename
from larmorx.sdcflows.tools import _as_loaded, _header
from larmorx.transforms.resample import _ornt2axcodes

__all__ = [
    "CheckRegisterResult",
    "PhaseResult",
    "PhdiffWorkflowResult",
    "au2rads",
    "check_b0_units",
    "check_register",
    "delta_te",
    "phdiff2fmap",
    "phdiff_workflow",
    "subtract_phases",
]

LOGGER = logging.getLogger("larmorx.sdcflows")
_XYZT_MM = 2


@dataclass(frozen=True)
class PhaseResult:
    """An image a phase step computes (``image``), the metadata it hands on (``metadata``,
    ``SubtractPhases``), and the file it wrote (``out_file``, with ``out_dir``; the input's
    path where the step passes its input through)."""

    image: Image
    out_file: Path | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


def _load64(obj: Any) -> Image:
    """``nb.load(path).get_fdata()`` (float64, scaled) with the file's header."""
    if isinstance(obj, (str, os.PathLike)):
        return load_nibabel(obj, dtype=np.float64)
    img = _as_loaded(obj)
    return Image(np.asarray(img.data, dtype=np.float64), img.affine, img.header)


def _load32(obj: Any) -> Image:
    """``get_fdata(dtype='float32')`` with the file's header."""
    if isinstance(obj, (str, os.PathLike)):
        return load_nibabel(obj, dtype=np.float32)
    img = _as_loaded(obj)
    return Image(np.asarray(img.data, dtype=np.float32), img.affine, img.header)


def _name(obj: Any, default: str) -> str:
    return os.fspath(obj) if isinstance(obj, (str, os.PathLike)) else default


def _header_like(hdr: NiftiHeader, data: np.ndarray, affine: Any, **fields: Any) -> NiftiHeader:
    """``hdr.copy()`` with ``fields`` set, written as nibabel writes ``Nifti1Image(data, affine,
    hdr)`` (``update_header``: the xforms kept while ``affine`` matches them)."""
    d = hdr.to_dict()
    d.update(fields)
    out = _nb_header_for_image(
        list(data.shape), data.dtype.name, np.ascontiguousarray(affine, dtype=np.float64), d, 1
    )
    return NiftiHeader.from_dict(out)


def _write(img: Image, name: str, suffix: str, out_dir: Any) -> Path | None:
    if out_dir is None:
        return None
    os.makedirs(out_dir, exist_ok=True)
    out = Path(_fname_presuffix(name, suffix, out_dir))
    save_nibabel(img, out)
    return out


def au2rads(
    in_file: Any,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
) -> PhaseResult:
    """sdcflows' ``au2rads`` (``PhaseMap2rads``): a phase map in arbitrary units rescaled to
    radians, ``(data - min) * 2 pi / (max - min)`` (float64), rounded to float32 and clipped
    to ``[0, 2 pi]`` (numpy's float32 bounds), with the input's header (float32, spatial units
    mm, time units unset) and xforms. With ``out_dir``: ``<in>_rads``.

    A constant phase map gives NaN everywhere in sdcflows (0 / 0): larmorx raises a
    ``ValueError`` (SDC-04, ``"fix"``) or returns the NaN map (``"reproduce"``)."""
    _defects.check(upstream_defects)
    im = _load64(in_file)
    data = np.asarray(im.data)
    lo, hi = data.min(), data.max()
    if not hi > lo and not np.isnan(hi - lo):
        if upstream_defects == "fix":
            raise ValueError(
                "the phase map is constant: it cannot be rescaled to radians (sdcflows "
                "returns NaN everywhere, SDC-04; upstream_defects='reproduce' gives that)"
            )
        _defects.warn(
            [("SDC-04", "sdcflows", "reproduce", "a constant phase map is rescaled to NaN")]
        )
    with np.errstate(divide="ignore", invalid="ignore"):
        data = (data - data.min()) * (2 * np.pi / (data.max() - data.min()))
    data = np.clip(np.float32(data), 0.0, 2 * np.pi)
    hdr = _header(im)
    affine = nibabel_affine(hdr)  # Nifti1Image(data, None, hdr): get_best_affine
    out_hdr = _header_like(hdr, data, affine, xyzt_units=_XYZT_MM)
    img = Image(data, affine, out_hdr)
    return PhaseResult(img, _write(img, _name(in_file, "phase.nii.gz"), "_rads", out_dir))


def subtract_phases(
    in_phases: Sequence[Any],
    in_meta: Sequence[dict[str, Any]],
    *,
    out_dir: str | os.PathLike[str] | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
) -> PhaseResult:
    """sdcflows' ``SubtractPhases`` (``subtract_phases``): the phase difference of two phase
    maps in radians (each with its metadata), ordered by ``EchoTime``: ``phase2 - phase1``
    (float32), negative values plus 2 pi, clipped to ``[0, 2 pi]``, with the shorter echo's
    header (float32, units mm) and affine; the metadata are the second's updated by the first's,
    without ``EchoTime``, with ``EchoTime1`` and ``EchoTime2``. One map is passed through (its
    file and metadata). With ``out_dir``: ``<phase1>_phdiff``.

    ``ValueError`` when the lists' lengths differ (sdcflows' message). Without ``EchoTime``,
    sdcflows fails with a ``TypeError`` (``None > x``): larmorx raises a ``ValueError`` that
    names it (SDC-09, ``"fix"``), or the same ``TypeError`` (``"reproduce"``)."""
    _defects.check(upstream_defects)
    phases = list(in_phases)
    metas = [dict(m) for m in in_meta]
    if len(phases) != len(metas):
        raise ValueError("Length of input phase-difference maps and metadata files should match.")
    if not 1 <= len(phases) <= 2:
        raise ValueError(f"in_phases holds one or two phase maps, not {len(phases)}")
    if len(phases) == 1:
        img = _as_loaded(phases[0])
        path = Path(phases[0]) if isinstance(phases[0], (str, os.PathLike)) else None
        return PhaseResult(img, path, dict(in_meta[0]))
    echo_times = tuple(m.pop("EchoTime", None) for m in metas)
    if None in echo_times and upstream_defects == "fix":
        raise ValueError(
            "SubtractPhases needs the EchoTime of both phase maps (sdcflows raises a "
            "TypeError, SDC-09; upstream_defects='reproduce' gives that)"
        )
    if echo_times[0] > echo_times[1]:
        phases = [phases[1], phases[0]]
        metas = [metas[1], metas[0]]
        echo_times = (echo_times[1], echo_times[0])
    ph = [_load32(p) for p in phases]
    sub_data = np.asarray(ph[1].data) - np.asarray(ph[0].data)
    sub_data[sub_data < 0] += 2 * np.pi
    sub_data = np.clip(sub_data, 0.0, 2 * np.pi)
    new_meta = metas[1].copy()
    new_meta.update(metas[0])
    new_meta["EchoTime1"] = echo_times[0]
    new_meta["EchoTime2"] = echo_times[1]
    hdr = _header(ph[0])
    out_hdr = _header_like(hdr, sub_data, ph[0].affine, xyzt_units=_XYZT_MM)
    img = Image(sub_data, ph[0].affine, out_hdr)
    out = _write(img, _name(phases[0], "phase1.nii.gz"), "_phdiff", out_dir)
    return PhaseResult(img, out, new_meta)


def delta_te(in_values: dict[str, Any]) -> float:
    """sdcflows' ``delta_te``: the echo-time difference (s) of phase-difference metadata,
    ``|EchoTime2 - EchoTime1|``, else ``|EchoTimeDifference|``, with sdcflows' messages."""
    te2 = in_values.get("EchoTime2")
    te1 = in_values.get("EchoTime1")
    if te2 is None and te1 is None:
        try:
            te2 = float(in_values.get("EchoTimeDifference"))  # type: ignore[arg-type]
            return abs(te2)
        except TypeError:
            raise ValueError(
                "Phase/phase-difference fieldmaps: no echo-times information."
            ) from None
        except ValueError:
            raise ValueError(f"Could not interpret metadata <EchoTimeDifference={te2}>.") from None
    try:
        te2 = float(te2 or "unknown")
        te1 = float(te1 or "unknown")
    except ValueError:
        raise ValueError(f"Could not interpret metadata <EchoTime(1,2)={(te1, te2)}>.") from None
    return abs(te2 - te1)


_delta_te = delta_te


def phdiff2fmap(
    in_file: Any,
    delta_te: float | None = None,
    *,
    metadata: dict[str, Any] | None = None,
    out_dir: str | os.PathLike[str] | None = None,
) -> PhaseResult:
    """sdcflows' ``phdiff2fmap`` (``Phasediff2Fieldmap``, ``delta_te`` from ``metadata``
    when it is not given): an unwrapped phase difference (radians) as a field map in Hz,
    ``phase / (2 pi delta_te)`` in float32, with the input's header (float32). With
    ``out_dir``: ``<in>_fmap``."""
    if delta_te is None:
        if metadata is None:
            raise ValueError("phdiff2fmap needs delta_te or the metadata to compute it")
        delta_te = _delta_te(metadata)
    image = _load32(in_file)
    data = np.asarray(image.data) / (2.0 * np.pi * delta_te)
    hdr = _header(image)
    out_hdr = _header_like(hdr, data.astype(np.float32, copy=False), image.affine)
    img = Image(data, image.affine, out_hdr)
    return PhaseResult(img, _write(img, _name(in_file, "phasediff.nii.gz"), "_fmap", out_dir))


def check_b0_units(
    in_file: Any, units: str, *, out_dir: str | os.PathLike[str] | None = None
) -> PhaseResult:
    """sdcflows' ``CheckB0Units``: a field map given in ``Hz`` is passed through; one in
    ``rad/s`` is divided by 2 pi (its values as nibabel reads them, scaled), keeping the input's
    header, so nibabel stores it in the input's type (an integer type re-scaled: sdcflows'
    behaviour, kept). With ``out_dir``: ``<in>_Hz``."""
    if units not in ("Hz", "rad/s"):
        raise ValueError(f"units must be 'Hz' or 'rad/s', not {units!r}")
    img = _as_loaded(in_file)
    if units == "Hz":
        path = Path(in_file) if isinstance(in_file, (str, os.PathLike)) else None
        return PhaseResult(img, path)
    data = np.asanyarray(img.data) / (2.0 * np.pi)
    hdr = _header(img)
    disk = hdr.data_dtype if hdr.data_dtype is not None else data.dtype
    out_hdr = NiftiHeader.from_dict(
        _nb_header_for_image(
            list(data.shape),
            np.dtype(disk).name,
            np.ascontiguousarray(img.affine, dtype=np.float64),
            hdr.to_dict(),
            None,
        )
    )
    out = Image(data, img.affine, out_hdr)
    return PhaseResult(out, _write(out, _name(in_file, "fmap.nii.gz"), "_Hz", out_dir))


@dataclass(frozen=True)
class CheckRegisterResult:
    """``CheckRegister``'s outputs: the magnitude and field-map files (``mag_files``,
    ``fmap_files``), each its input's path, or the copy written in ``out_dir`` with the first
    magnitude's affine where its affine differed."""

    mag_files: tuple[Path, ...]
    fmap_files: tuple[Path, ...]


def _gross_geometry(p1: str, i1: Image, p2: str, i2: Image) -> str | None:
    """sdcflows' ``_check_gross_geometry``: a shape or orientation mismatch message."""
    if tuple(i1.shape[:3]) != tuple(i2.shape[:3]):
        return f"Images have shape mismatch: {p1} {tuple(i1.shape)}, {p2} {tuple(i2.shape)}"
    c1 = _ornt2axcodes(_io_orientation(np.asarray(i1.affine)))
    c2 = _ornt2axcodes(_io_orientation(np.asarray(i2.affine)))
    if c1 != c2:
        return (
            "Images have orientation mismatch: "
            f"{p1} {''.join(str(c) for c in c1)}, {p2} {''.join(str(c) for c in c2)}"
        )
    return None


def check_register(
    mag_files: Any,
    fmap_files: Any,
    rot_thresh: float = 0.02,
    trans_thresh: float = 1.0,
    *,
    out_dir: str | os.PathLike[str] | None = None,
) -> CheckRegisterResult:
    """sdcflows' ``CheckRegister``: the magnitude image(s) and the phase or field-map image(s)
    (one or two each, paths) checked for the same shape and orientation (a ``ValueError``
    with sdcflows' message, logged as critical), and each image whose affine is not close to
    the first magnitude's rewritten into ``out_dir`` (default: the current directory, as
    nipype's) with that affine ("Copying affine to <name>").

    Two magnitudes with different affines are registered by sdcflows with FreeSurfer's
    ``mri_robust_register`` before this; that branch waits for larmorx's FreeSurfer port and
    raises ``NotImplementedError`` (no fMRIPrep oracle run reaches it)."""
    # the paths as given (nibabel's get_filename() in the messages)
    mags = [
        os.fspath(p) for p in (mag_files if isinstance(mag_files, (list, tuple)) else [mag_files])
    ]
    fmaps = [
        os.fspath(p)
        for p in (fmap_files if isinstance(fmap_files, (list, tuple)) else [fmap_files])
    ]
    for lst, what in ((mags, "mag_files"), (fmaps, "fmap_files")):
        if not 1 <= len(lst) <= 2:
            raise ValueError(f"{what} holds one or two images, not {len(lst)}")
    mag_imgs = [load_nibabel(p) for p in mags]
    fmap_imgs = [load_nibabel(p) for p in fmaps]

    def fail(msg: str | None) -> None:
        if msg is not None:
            LOGGER.critical(msg)
            raise ValueError(msg)

    for pm, im, pf, imf in zip(mags, mag_imgs, fmaps, fmap_imgs, strict=False):
        fail(_gross_geometry(pm, im, pf, imf))
    if len(mags) == 2:
        fail(_gross_geometry(mags[0], mag_imgs[0], mags[1], mag_imgs[1]))
        if not np.allclose(mag_imgs[0].affine, mag_imgs[1].affine):
            raise NotImplementedError(
                "the two magnitude images have different affines: sdcflows registers them "
                "with FreeSurfer's mri_robust_register, which larmorx does not port yet"
            )
    if len(fmaps) == 2:
        fail(_gross_geometry(fmaps[0], fmap_imgs[0], fmaps[1], fmap_imgs[1]))
    cwd = Path(out_dir) if out_dir is not None else Path.cwd()
    target = np.asarray(mag_imgs[0].affine)

    def conform(p: str, img: Image) -> Path:
        if np.allclose(img.affine, target):
            return Path(p)
        basename = os.path.basename(p)
        out_file = cwd / basename
        LOGGER.info(f"Copying affine to {basename}")
        hdr = _header(img)
        disk = hdr.data_dtype if hdr.data_dtype is not None else np.asarray(img.data).dtype
        new_hdr = NiftiHeader.from_dict(
            _nb_header_for_image(list(img.shape), np.dtype(disk).name, target, hdr.to_dict(), None)
        )
        os.makedirs(cwd, exist_ok=True)
        save_nibabel(Image(np.asanyarray(img.data), target, new_hdr), out_file)
        return out_file

    return CheckRegisterResult(
        tuple(conform(p, i) for p, i in zip(mags, mag_imgs, strict=True)),
        tuple(conform(p, i) for p, i in zip(fmaps, fmap_imgs, strict=True)),
    )


@dataclass(frozen=True)
class PhdiffWorkflowResult:
    """``init_phdiff_wf``'s steps: the phase maps in radians (``rads``), their difference and
    metadata (``phase_diff``), PRELUDE's unwrapped difference (``unwrapped``) and the field map
    in Hz (``fieldmap``, the workflow's output)."""

    rads: tuple[PhaseResult, ...]
    phase_diff: PhaseResult
    unwrapped: Image
    fieldmap: PhaseResult


def phdiff_workflow(
    magnitude: Any,
    phase: Any,
    phase_meta: Any,
    mask: Any,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    prelude_implementation: str = "auto",
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> PhdiffWorkflowResult:
    """sdcflows' ``init_phdiff_wf``: each phase map (one phase difference, or the two phases
    of a phase1/phase2 acquisition, with their metadata) to radians (:func:`au2rads`), their
    difference (:func:`subtract_phases`), unwrapped by FSL's PRELUDE inside ``mask`` with
    ``magnitude`` (:func:`larmorx.fsl.prelude`: ``-a``, ``-p``, ``-m``, as nipype's ``PRELUDE``
    calls it; ``prelude_implementation`` picks larmorx's replica or clean-room original), and
    converted to Hz with the metadata's echo times (:func:`phdiff2fmap`). With ``out_dir`` each
    step writes sdcflows' file there (PRELUDE's as ``<phase>_unwrapped``)."""
    from larmorx import fsl

    phases = list(phase) if isinstance(phase, (list, tuple)) else [phase]
    metas = list(phase_meta) if isinstance(phase_meta, (list, tuple)) else [phase_meta]
    rads = tuple(au2rads(p, out_dir=out_dir, upstream_defects=upstream_defects) for p in phases)
    inputs = [r.out_file if r.out_file is not None else r.image for r in rads]
    diff = subtract_phases(inputs, metas, out_dir=out_dir, upstream_defects=upstream_defects)
    src = diff.out_file if diff.out_file is not None else diff.image
    unwrapped = fsl.prelude(
        src,
        magnitude,
        mask=mask,
        implementation=prelude_implementation,  # type: ignore[arg-type]
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    ).unwrapped
    name = _name(src, "phasediff.nii.gz")
    unwrapped_path = None
    if out_dir is not None:
        # nipype's PRELUDE: <phase>_unwrapped, the file FSL's prelude writes, with the extension
        # of FSL's output type (NIFTI_GZ, as fMRIPrep's and sdcflows' environments set it), also
        # for an uncompressed phase map
        _, stem, _ext = _split_filename(name)
        unwrapped_path = Path(os.path.abspath(os.fspath(out_dir))) / f"{stem}_unwrapped.nii.gz"
        unwrapped.save(unwrapped_path)
    fmap = phdiff2fmap(
        unwrapped_path if unwrapped_path is not None else unwrapped,
        metadata=diff.metadata,
        out_dir=out_dir,
    )
    if unwrapped_path is None:
        # sdcflows names the field map after PRELUDE's file
        fmap = PhaseResult(fmap.image, None, {})
    return PhdiffWorkflowResult(rads, diff, unwrapped, fmap)
