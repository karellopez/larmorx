# SPDX-License-Identifier: Apache-2.0
"""sdcflows' fieldmap-less ("SyN") estimation as plain functions: the preprocessing (the EPI
reference, the anatomical reference and its masks in the EPI's space, the prior of strong
distortions) and the estimation itself (the EPI registered to the anatomy with the deformation
restricted to the phase-encoding axis, its field map smoothed by sdcflows' B-spline fit).

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1, fMRIPrep
25.2.6's): ``workflows/fit/syn.py`` (``init_syn_preprocessing_wf``, ``init_syn_sdc_wf``,
``_set_dtype``, ``_merge_meta``, ``_warp_dir``, ``_mm2vox``, ``_adjust_zooms``, ``_norm_lap``)
and ``interfaces/utils.py`` (``Deoblique``, ``Reoblique``: ``_deoblique``,
``_reoblique``, ``OBLIQUE_THRESHOLD_DEG``). The nodes run as nipype runs them, in the graph's
order, each in its folder under ``out_dir`` (``syn_preprocessing_wf/...``), through larmorx's
ports: niworkflows' EPI reference (:func:`larmorx.niworkflows.epi_reference_workflow`),
``ApplyMask``, ``IntensityClip``, ``GenerateSamplingReference`` and ``Binarize``; ANTs'
``DenoiseImage``, ``antsRegistration`` (``affine.json``) and ``antsApplyTransforms`` with
nipype's command lines (:mod:`larmorx._nipype`) and niworkflows' header fixes; sdcflows'
``BrainExtraction`` and ``BinaryDilation``.

The prior (``fmap_atlas.nii.gz``, sdcflows' data, Apache-2.0, 5.8 MB) is not shipped: it is
fetched once from sdcflows' release into larmorx's data cache and checked against its SHA-256
(:func:`fmap_atlas`); its small affine to MNI152NLin2009cAsym ships.

Kept as sdcflows (SDC-13, rule C after rule D's measurement): sdcflows compares nibabel's
``obliquity`` (radians) with ``OBLIQUE_THRESHOLD_DEG = 0.5`` (meant as degrees), so ``Deoblique``
and ``Reoblique`` act only beyond 28.6 degrees; and ``_deoblique``'s plumb grid centres the image
in voxels, not millimetres, and compares the axis codes with ``RAS`` by position. Making the EPI
plumb as meant changes fMRIPrep's SyN field maps well beyond the estimation's own spread without
bringing the unwarped reference closer to the anatomy (the findings,
``docs/findings/sdcflows-estimation.md``), so both modes give sdcflows' behaviour.

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows' behaviour): SDC-14
(``_norm_lap`` of an image without positive or negative values: a ``ValueError`` instead of
numpy's ``IndexError``), SDC-16 (``_warp_dir`` restricts the deformation along the world axis
of the phase encoding: ANTs weighs physical axes, sdcflows indexes the fixed image's voxel
axes). SDC-15: ``init_syn_sdc_wf``'s ``zooms_epi`` node, whose output nothing reads, is not
computed.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import tempfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _defects, _nipype
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import load_nibabel, nibabel_affine, set_xform
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = [
    "FMAP_ATLAS_SHA256",
    "INPUT_FIELDS",
    "MAX_LAPLACIAN_WEIGHT",
    "OBLIQUE_THRESHOLD_DEG",
    "SYN_METHOD",
    "SynPreprocessingResult",
    "SynSdcResult",
    "adjust_zooms",
    "deoblique",
    "fmap_atlas",
    "fmap_atlas_affine",
    "merge_meta",
    "mm2vox",
    "norm_lap",
    "obliquity",
    "reoblique",
    "set_dtype",
    "syn_preprocessing_workflow",
    "syn_sdc_workflow",
    "warp_dir",
]

_DATA = Path(__file__).resolve().parent / "data"

#: sdcflows' threshold (``interfaces/utils.py``), compared with nibabel's obliquity in radians.
OBLIQUE_THRESHOLD_DEG = 0.5
#: sdcflows 2.17.1's ``data/fmap_atlas.nii.gz`` (the same in 2.15.1).
FMAP_ATLAS_SHA256 = "72b8d2cc7ff3324f3bfaf52712bc191256e795bcff5ee168563b89e584aa40bb"
FMAP_ATLAS_URL = (
    "https://raw.githubusercontent.com/nipreps/sdcflows/2.17.1/sdcflows/data/fmap_atlas.nii.gz"
)
#: The environment variable naming a local copy of the atlas (offline machines, tests).
FMAP_ATLAS_ENV = "LARMORX_SDCFLOWS_ATLAS"
NIWORKFLOWS_VERSION = "1.15.4"


def _cache_dir() -> Path | None:
    """larmorx's data cache (``LARMORX_DATA_DIR``; else the platform's user cache folder)."""
    env = os.environ
    if env.get("LARMORX_DATA_DIR"):
        return Path(env["LARMORX_DATA_DIR"])
    if os.name == "nt":
        return Path(env["LOCALAPPDATA"]) / "larmorx" / "cache" if env.get("LOCALAPPDATA") else None
    import sys

    if sys.platform == "darwin":
        return Path(env["HOME"]) / "Library" / "Caches" / "larmorx" if env.get("HOME") else None
    if env.get("XDG_CACHE_HOME"):
        return Path(env["XDG_CACHE_HOME"]) / "larmorx"
    return Path(env["HOME"]) / ".cache" / "larmorx" if env.get("HOME") else None


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fmap_atlas() -> str:
    """The path of sdcflows' prior of strong susceptibility distortions
    (``fmap_atlas.nii.gz``): ``LARMORX_SDCFLOWS_ATLAS`` (a local copy), else larmorx's data
    cache (``sdcflows-2.17.1/fmap_atlas.nii.gz``), fetched there on first use from sdcflows'
    release. Every copy is checked against :data:`FMAP_ATLAS_SHA256` (``ValueError``)."""
    local = os.environ.get(FMAP_ATLAS_ENV)
    if local:
        p = Path(local)
        if _sha256(p) != FMAP_ATLAS_SHA256:
            raise ValueError(f"{p} is not sdcflows' fmap_atlas.nii.gz (SHA-256 differs)")
        return str(p)
    cache = _cache_dir()
    if cache is None:
        raise RuntimeError(
            "no data cache for sdcflows' fmap_atlas.nii.gz: set LARMORX_DATA_DIR, or "
            f"{FMAP_ATLAS_ENV} to a copy of the file"
        )
    target = cache / "sdcflows-2.17.1" / "fmap_atlas.nii.gz"
    if target.is_file() and _sha256(target) == FMAP_ATLAS_SHA256:
        return str(target)
    import urllib.request

    target.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=target.parent, suffix=".part")
    try:
        with os.fdopen(fd, "wb") as out, urllib.request.urlopen(FMAP_ATLAS_URL, timeout=120) as r:
            shutil.copyfileobj(r, out)
        if _sha256(Path(tmp)) != FMAP_ATLAS_SHA256:
            raise ValueError(f"the file fetched from {FMAP_ATLAS_URL} is not the pinned atlas")
        os.replace(tmp, target)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)
    return str(target)


def fmap_atlas_affine() -> str:
    """The path of sdcflows' ``fmap_atlas_2_MNI152NLin2009cAsym_affine.mat`` (shipped)."""
    return str(_DATA / "fmap_atlas_2_MNI152NLin2009cAsym_affine.mat")


def obliquity(affine: Any) -> np.ndarray:
    """nibabel's ``affines.obliquity``: per axis, the angle (radians) between the voxel axis
    and the closest world axis (``arccos`` of the largest cosine; numpy's ``arccos``)."""
    affine = np.asarray(affine, dtype=np.float64)
    vs = np.sqrt(np.sum(affine[:-1, :-1] ** 2, axis=0))
    best_cosines = np.abs(affine[:-1, :-1] / vs).max(axis=1)
    return np.arccos(best_cosines)


def _is_plumb(affine: Any) -> bool:
    return bool(np.all(obliquity(affine) < OBLIQUE_THRESHOLD_DEG))


def deoblique(
    in_file: str | os.PathLike[str],
    in_affine: str | os.PathLike[str] | None = None,
    *,
    out_dir: str | os.PathLike[str] = ".",
    n_threads: int = 1,
) -> str:
    """sdcflows' ``_deoblique``: ``in_file`` itself when every axis's obliquity is below
    :data:`OBLIQUE_THRESHOLD_DEG` (compared in radians: SDC-13); otherwise written as
    ``<in>_plumb`` in ``out_dir`` with a plumb affine (the voxel sizes on the diagonal, signed
    by whether each axis code equals ``R``, ``A``, ``S`` in that position, the origin at minus
    half the voxel counts) or ``in_affine``'s, set as qform and sform with code 1."""
    from larmorx.niworkflows import aff2axcodes

    in_file = os.fspath(in_file)
    nii = load_nibabel(in_file, n_threads=n_threads)
    if _is_plumb(nii.affine):
        return in_file
    hdr = nii.header
    assert hdr is not None
    if in_affine is None:
        orientation = aff2axcodes(nii.affine)
        directions = (
            np.array(
                [int(l1 == l2) for l1, l2 in zip(orientation, "RAS", strict=False)],
                dtype="float32",
            )
            * 2
            - 1
        )
        newaff = np.eye(4)
        zooms = np.asarray(hdr.pixdim, dtype=np.float32)[1:4]
        newaff[:3, :3] = np.diag(directions * zooms)
        newaff[:3, 3] -= (np.array(nii.shape[:3]) - 1) * 0.5
    else:
        newaff = np.array(load_nibabel(in_affine).affine, dtype=np.float64, copy=True)
    fields = set_xform(hdr.to_dict(), "qform", newaff, 1)
    fields = set_xform(fields, "sform", newaff, 1)
    os.makedirs(out_dir, exist_ok=True)
    out_file = _nipype.fname_presuffix(in_file, suffix="_plumb", newpath=out_dir)
    save_nibabel(
        Image(nii.data, newaff, NiftiHeader.from_dict(fields)), out_file, n_threads=n_threads
    )
    return out_file


def _with_xforms_of(fields: dict, epi: NiftiHeader) -> dict:
    """``hdr.set_qform(*epi.get_qform(coded=True)); hdr.set_sform(*epi.get_sform(coded=True))``."""
    from larmorx.niworkflows.header import _qform_sform

    qform, qcode, sform, scode = _qform_sform(epi)
    for which, matrix, code in (("qform", qform, qcode), ("sform", sform, scode)):
        if matrix is None:
            fields[f"{which}_code"] = int(code)
        else:
            fields = set_xform(fields, which, matrix, code)
    return fields


def reoblique(
    in_epi: str | os.PathLike[str],
    in_plumb: str | os.PathLike[str],
    in_field: str | os.PathLike[str],
    in_mask: str | os.PathLike[str] | None = None,
    *,
    out_dir: str | os.PathLike[str] = ".",
    n_threads: int = 1,
) -> tuple[str, str, str | None]:
    """sdcflows' ``_reoblique``: the plumb EPI, field and mask unchanged when ``in_epi`` is
    plumb (SDC-13); otherwise each written as ``<name>_oriented`` in ``out_dir`` with the
    original EPI's affine: the plumb EPI with the EPI's whole header (its stored type
    included), the field and mask with their own headers and the EPI's qform and sform (with
    their codes)."""
    in_epi, in_plumb, in_field = (os.fspath(p) for p in (in_epi, in_plumb, in_field))
    epinii = load_nibabel(in_epi, n_threads=n_threads)
    if _is_plumb(epinii.affine):
        return in_plumb, in_field, None if in_mask is None else os.fspath(in_mask)
    assert epinii.header is not None
    os.makedirs(out_dir, exist_ok=True)
    out_plumb = _nipype.fname_presuffix(in_plumb, suffix="_oriented", newpath=out_dir)
    out_field = _nipype.fname_presuffix(in_field, suffix="_oriented", newpath=out_dir)
    plumb = load_nibabel(in_plumb, n_threads=n_threads)
    save_nibabel(Image(plumb.data, epinii.affine, epinii.header), out_plumb, n_threads=n_threads)
    out_mask = None
    pairs = [(in_field, out_field)]
    if in_mask:
        out_mask = _nipype.fname_presuffix(in_mask, suffix="_oriented", newpath=out_dir)
        pairs.append((os.fspath(in_mask), out_mask))
    for src, dst in pairs:
        img = load_nibabel(src, n_threads=n_threads)
        assert img.header is not None
        fields = _with_xforms_of(img.header.to_dict(), epinii.header)
        save_nibabel(
            Image(img.data, epinii.affine, NiftiHeader.from_dict(fields)), dst, n_threads=n_threads
        )
    return out_plumb, out_field, out_mask


def set_dtype(in_file: str | os.PathLike[str], dtype: str = "int16", *, n_threads: int = 1) -> str:
    """sdcflows' ``_set_dtype``: ``in_file`` itself when its stored type is ``dtype``; otherwise
    its data written as ``dtype`` (nibabel's writer) with its header, as ``<in>_<dtype>`` beside
    it."""
    in_file = os.fspath(in_file)
    img = load_nibabel(in_file, n_threads=n_threads)
    assert img.header is not None
    if np.dtype(img.header.data_dtype) == np.dtype(dtype):
        return in_file
    out_file = _nipype.fname_presuffix(in_file, suffix=f"_{dtype}")
    save_nibabel(img, out_file, dtype=np.dtype(dtype), n_threads=n_threads)
    return out_file


def merge_meta(epi_ref: str, meta_list: Sequence[Mapping[str, Any]]) -> tuple[str, Any]:
    """sdcflows' ``_merge_meta``: the EPI reference and the first input's metadata."""
    return (epi_ref, meta_list[0])


@dataclass(frozen=True)
class SynPreprocessingResult:
    """``init_syn_preprocessing_wf``'s outputs: ``epi_ref`` (the EPI reference and its
    metadata), ``epi_mask`` (its brain mask), ``anat_ref`` (the denoised, clipped, masked
    anatomical reference resampled onto the EPI's sampling grid), ``anat_mask`` (the
    anatomical mask there, ``uint8``), ``sd_prior`` (the mask of the prior's strong distortions
    there, or ``anat_mask`` without the prior); and the files of the other nodes and the command
    lines nipype would write."""

    epi_ref: tuple[str, Any]
    epi_mask: str | None
    anat_ref: str
    anat_mask: str
    sd_prior: str
    epi_reference: Any
    deob_epi: str
    sampling_ref: str
    epi2anat_xfm: tuple[str, ...]
    files: Mapping[str, str]
    commands: Mapping[str, str]


def _denoise_copy_header(
    in_file: str, cwd: str, upstream_defects: str, n_threads: int
) -> tuple[str, str]:
    """sdcflows' ``DenoiseImage(copy_header=True)``: ``DenoiseImage -i <in> -n Gaussian -o
    <in>_noise_corrected -s 1`` (nipype's line), then nipype's ``copy_header`` (``keep_dtype``):
    the output's values with the input's header in the output's stored type."""
    cmd = _nipype.denoise_image(cwd=cwd, input_image=in_file)
    _nipype.run(cmd, n_threads=n_threads, upstream_defects=upstream_defects)
    out = cmd.outputs["output_image"]
    _copy_header_keep_dtype(in_file, out, n_threads)
    return out, cmd.cmdline


def _copy_header_keep_dtype(header_file: str, out: str, n_threads: int) -> None:
    """nipype's ``copy_header(header_file, out, keep_dtype=True)`` (its ``CopyHeaderInterface``
    after ANTs' ``DenoiseImage`` and ``ImageMath``): ``out``'s values with ``header_file``'s
    header in ``out``'s stored type, written over ``out``."""
    hdr_img = load_nibabel(header_file, n_threads=n_threads)
    out_img = load_nibabel(out, n_threads=n_threads)
    assert hdr_img.header is not None and out_img.header is not None
    dtype = np.dtype(out_img.header.data_dtype)
    header = NiftiHeader.from_dict(hdr_img.header.to_dict())
    save_nibabel(
        Image(out_img.data, nibabel_affine(header), header), out, dtype=dtype, n_threads=n_threads
    )


def _apply_transforms(
    cwd: str,
    input_image: str,
    reference_image: str,
    transforms: Sequence[str],
    invert: Sequence[bool] | None,
    interpolation: str,
    message: str,
    upstream_defects: str,
    n_threads: int,
    args: str | None = None,
) -> tuple[str, str]:
    """niworkflows' ``FixHeaderApplyTransforms``: nipype's ``antsApplyTransforms`` line run by
    larmorx's port, then the reference's x-forms copied onto the output (``_copyxform``).
    ``invert`` ``None``: no flags (nipype's line for ``transforms='identity'``)."""
    from larmorx.niworkflows.header import copy_xform

    inputs: dict[str, Any] = {
        "input_image": input_image,
        "reference_image": reference_image,
        "transforms": list(transforms),
        "interpolation": interpolation,
        "num_threads": max(int(n_threads), 1),
    }
    if invert is not None:
        inputs["invert_transform_flags"] = list(invert)
    if args is not None:
        inputs["args"] = args
    cmd = _nipype.apply_transforms(cwd=cwd, **inputs)
    _nipype.run(cmd, n_threads=n_threads, upstream_defects=upstream_defects)
    out = cmd.outputs["output_image"]
    copy_xform(reference_image, out, message=message, n_threads=n_threads)
    return out, cmd.cmdline


def syn_preprocessing_workflow(
    in_epis: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    in_meta: Sequence[Mapping[str, Any]],
    in_anat: str | os.PathLike[str],
    mask_anat: str | os.PathLike[str],
    std2anat_xfm: str | os.PathLike[str] | None = None,
    t_masks: Sequence[Sequence[bool] | None] | None = None,
    *,
    out_dir: str | os.PathLike[str],
    atlas_threshold: float = 3,
    debug: bool = False,
    omp_nthreads: int = 1,
    auto_bold_nss: bool = False,
    t1w_inversion: bool = False,
    sd_prior: bool = True,
    prior_file: str | os.PathLike[str] | None = None,
    ants_random_seed: int | None = None,
    niworkflows_version: str = NIWORKFLOWS_VERSION,
    numpy_version: str = "2.5.4",
    scipy_version: str = "1.18.1",
    nilearn_version: str = "0.14.1",
    volreg_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> SynPreprocessingResult:
    """sdcflows' ``init_syn_preprocessing_wf``: the EPI reference of ``in_epis``
    (:func:`larmorx.niworkflows.epi_reference_workflow`, non-steady states detected with
    ``auto_bold_nss``, else ``t_masks``), made plumb (:func:`deoblique`) and brain-extracted;
    the anatomical image masked, clipped (0 to 99.8th percentile) and denoised (ANTs'
    ``DenoiseImage``); the EPI reference registered to it (``antsRegistration`` with sdcflows'
    ``affine.json``, both masks dilated by 3 voxels, the EPI's only in the second stage); a
    sampling grid of the EPI's field of view at the anatomical resolution
    (``GenerateSamplingReference``); the anatomical reference and mask resampled onto it through
    the inverse registration (``Linear``, ``MultiLabel``; the mask stored as ``uint8``); and,
    with ``sd_prior``, sdcflows' prior (:func:`fmap_atlas`) brought there through
    ``[epi2anat (inverted), std2anat_xfm, the atlas's affine to MNI]`` and thresholded above
    ``atlas_threshold`` (``Binarize``). The nodes write into ``out_dir`` as nipype's do.

    fMRIPrep 25.2.6 calls it with ``auto_bold_nss=True``, its BOLD and SBRef files and their
    metadata, the T1w and its mask, and the MNI-to-T1w transform. ``prior_file`` replaces
    sdcflows' atlas (a larmorx addition, for tests and other priors; its affine to MNI is still
    sdcflows'). ``ants_random_seed`` is what
    ANTs reads from ``ANTS_RANDOM_SEED`` (fMRIPrep sets it). ``t1w_inversion`` is accepted and
    unused, as in sdcflows. ``debug`` (sdcflows' fast settings: the anatomical image regridded
    to 2 mm with niworkflows' ``RegridToZooms``) waits for that port (``NotImplementedError``).
    ``numpy_version``, ``scipy_version`` and ``nilearn_version`` select the libraries'
    arithmetic (the newest by default; fMRIPrep 25.2.6's image: numpy 2.2.6, SciPy 1.15.2,
    nilearn 0.11.1); ``niworkflows_version`` the version written into ``descrip``."""
    from larmorx.niworkflows import apply_mask, binarize, epi_reference_workflow, intensity_clip
    from larmorx.niworkflows.header import copy_xform
    from larmorx.niworkflows.sampling import generate_sampling_reference
    from larmorx.sdcflows.masks import binary_dilation, brain_extraction
    from larmorx.sdcflows.registration import registration_settings

    _defects.check(upstream_defects)
    del t1w_inversion
    if debug:
        raise NotImplementedError(
            "init_syn_preprocessing_wf(debug=True) regrids the anatomical image with "
            "niworkflows' RegridToZooms, which larmorx has not ported yet"
        )
    if sd_prior and std2anat_xfm is None:
        raise ValueError("std2anat_xfm is needed for the prior (sd_prior=True)")
    wf = os.path.join(os.path.abspath(os.fspath(out_dir)), "syn_preprocessing_wf")

    def node(name: str) -> str:
        p = os.path.join(wf, name)
        os.makedirs(p, exist_ok=True)
        return p

    if isinstance(in_epis, (str, os.PathLike)):
        in_epis = [in_epis]
    in_anat = os.path.abspath(os.fspath(in_anat))
    mask_anat = os.path.abspath(os.fspath(mask_anat))
    files: dict[str, str] = {}
    commands: dict[str, str] = {}
    # epi_reference_wf, deob_epi, merge_output
    ref = epi_reference_workflow(
        [os.fspath(f) for f in in_epis],
        t_masks=t_masks,
        auto_bold_nss=auto_bold_nss,
        out_dir=wf,
        omp_nthreads=omp_nthreads,
        numpy_version=numpy_version,
        volreg_implementation=volreg_implementation,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )
    deob = deoblique(ref.epi_ref_file, out_dir=node("deob_epi"), n_threads=n_threads)
    epi_ref = merge_meta(deob, in_meta)
    # the anatomical side: anat_dilmsk, mask_anat, clip_anat, ref_anat
    _, anat_dil = binary_dilation(mask_anat, out_dir=node("anat_dilmsk"), n_threads=n_threads)
    assert anat_dil is not None
    masked = apply_mask(in_anat, mask_anat, out_dir=node("mask_anat"), n_threads=n_threads)
    clipped = intensity_clip(
        masked,
        p_min=0.0,
        p_max=99.8,
        out_dir=node("clip_anat"),
        numpy_version=numpy_version,
        n_threads=n_threads,
    )
    assert clipped.out_file is not None
    anat_ref, commands["ref_anat"] = _denoise_copy_header(
        clipped.out_file, node("ref_anat"), upstream_defects, max(int(omp_nthreads), 1)
    )
    # epi_brain, epi_dilmsk
    brain = brain_extraction(
        deob,
        out_dir=node("epi_brain"),
        numpy_version=numpy_version,
        scipy_version=scipy_version,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )
    assert brain.out_mask is not None
    _, epi_dil = binary_dilation(brain.out_mask, out_dir=node("epi_dilmsk"), n_threads=n_threads)
    assert epi_dil is not None
    # epi2anat: antsRegistration with affine.json
    reg = _nipype.registration(
        cwd=node("epi2anat"),
        from_file=registration_settings("affine.json"),
        output_warped_image=debug,
        output_inverse_warped_image=debug,
        fixed_image=[anat_ref],
        moving_image=[deob],
        fixed_image_masks=[anat_dil],
        moving_image_masks=["NULL", epi_dil],
        num_threads=max(int(omp_nthreads), 1),
    )
    _nipype.run(
        reg,
        n_threads=max(int(omp_nthreads), 1),
        upstream_defects=upstream_defects,
        ants_random_seed=ants_random_seed,
    )
    commands["epi2anat"] = reg.cmdline
    message = f"FixHeaderRegistration (niworkflows v{niworkflows_version})"
    for key, ref_image in (("warped_image", anat_ref), ("inverse_warped_image", deob)):
        produced = reg.outputs.get(key)
        if produced:
            copy_xform(ref_image, produced, message=message, n_threads=n_threads)
    forward = list(reg.outputs["forward_transforms"])
    # sampling_ref
    sampling = generate_sampling_reference(
        deob,
        in_anat,
        out_dir=node("sampling_ref"),
        niworkflows_version=niworkflows_version,
        nilearn_version=nilearn_version,
        scipy_version=scipy_version,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    # anat2epi, mask2epi, mask_dtype
    at_message = f"FixHeaderApplyTransforms (niworkflows v{niworkflows_version})"
    anat2epi, commands["anat2epi"] = _apply_transforms(
        node("anat2epi"),
        anat_ref,
        sampling,
        forward,
        [True],
        "Linear",
        at_message,
        upstream_defects,
        omp_nthreads,
    )
    mask2epi, commands["mask2epi"] = _apply_transforms(
        node("mask2epi"),
        mask_anat,
        sampling,
        forward,
        [True],
        "MultiLabel",
        at_message,
        upstream_defects,
        omp_nthreads,
    )
    node("mask_dtype")
    anat_mask = set_dtype(mask2epi, "uint8", n_threads=n_threads)
    prior = anat_mask
    if sd_prior:
        node("transform_list")
        assert std2anat_xfm is not None
        transforms = [*forward, os.fspath(std2anat_xfm), fmap_atlas_affine()]
        prior2epi, commands["prior2epi"] = _apply_transforms(
            node("prior2epi"),
            os.fspath(prior_file) if prior_file is not None else fmap_atlas(),
            sampling,
            transforms,
            [True, False, False],
            "Linear",
            at_message,
            upstream_defects,
            omp_nthreads,
        )
        msk = binarize(prior2epi, atlas_threshold, out_dir=node("prior_msk"), n_threads=n_threads)
        prior = msk.out_mask
        files["prior2epi"] = prior2epi
        files["prior_msk"] = msk.out_mask
    files.update(
        {
            "anat_dilmsk": anat_dil,
            "mask_anat": masked,
            "clip_anat": clipped.out_file,
            "ref_anat": anat_ref,
            "epi_brain": brain.out_mask,
            "epi_dilmsk": epi_dil,
            "anat2epi": anat2epi,
            "mask2epi": mask2epi,
            "mask_dtype": anat_mask,
        }
    )
    return SynPreprocessingResult(
        epi_ref=epi_ref,
        epi_mask=brain.out_mask,
        anat_ref=anat2epi,
        anat_mask=anat_mask,
        sd_prior=prior,
        epi_reference=ref,
        deob_epi=deob,
        sampling_ref=sampling,
        epi2anat_xfm=tuple(forward),
        files=files,
        commands=commands,
    )


#: ``init_syn_sdc_wf``'s input fields (sdcflows' ``INPUT_FIELDS``).
INPUT_FIELDS = ("epi_ref", "epi_mask", "anat_ref", "anat_mask", "sd_prior")
#: The largest weight of the Laplacian term sdcflows accepts (``MAX_LAPLACIAN_WEIGHT``).
MAX_LAPLACIAN_WEIGHT = 0.5
#: ``outputnode.method`` of the fieldmap-less estimation.
SYN_METHOD = 'FLB ("fieldmap-less", SyN-based)'
#: sdcflows' threshold of ``_warp_dir``'s warning (nibabel's obliquity, radians).
_WARP_DIR_OBLIQUITY = 0.05


def _front(x: Any) -> Any:
    """sdcflows' ``utils.misc.front``: the first element of a list or tuple, else ``x``."""
    return x[0] if isinstance(x, (list, tuple)) else x


def _last(x: Any) -> Any:
    """sdcflows' ``utils.misc.last``: the last element of a list or tuple, else ``x``."""
    return x[-1] if isinstance(x, (list, tuple)) else x


def _header_of(path: Any) -> NiftiHeader:
    from larmorx.sdcflows.bspline import _read_header

    return _read_header(path)


def _unsigned_axcodes(affine: Any) -> tuple[str | None, ...]:
    """``nb.aff2axcodes(affine, ['RR', 'AA', 'SS'])``: each voxel axis's closest world axis,
    without its sign."""
    from larmorx.niworkflows import aff2axcodes

    unsign = {"L": "R", "R": "R", "P": "A", "A": "A", "I": "S", "S": "S"}
    return tuple(None if c is None else unsign[c] for c in aff2axcodes(affine))


def warp_dir(
    moving_image: str | os.PathLike[str],
    fixed_image: str | os.PathLike[str],
    pe_dir: str,
    nlevels: int = 2,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> list[list[float]]:
    """sdcflows' ``_warp_dir``: ``antsRegistration``'s ``restrict_deformation`` for each of
    ``nlevels`` levels: 0.1 on two axes and 1.0 on the world axis closest to the moving
    image's phase-encoding axis (``pe_dir``: ``i``, ``j``, ``k``, signed or not). Logs
    sdcflows' warning (``larmorx.sdcflows``) when the fixed image is oblique (nibabel's
    obliquity above 0.05 radians on an axis). A ``pe_dir`` or an axis it cannot match raises
    the ``ValueError`` sdcflows' ``str.index``/``tuple.index`` raise.

    ANTs weighs the deformation's physical components (x, y, z), while sdcflows puts the 1.0
    at the position of that axis among the fixed image's voxel axes: the same position for a
    fixed grid in R, A, S order (fMRIPrep's axial EPIs), another world axis for a sagittal or
    coronal grid, where the deformation along the phase encoding is then held at 0.1 (SDC-16,
    rule A). larmorx puts the 1.0 on that world axis; ``"reproduce"`` gives sdcflows'
    position; an ``lx.UpstreamDefectWarning`` says when the two differ."""
    import logging

    _defects.check(upstream_defects)
    moving = nibabel_affine(_header_of(moving_image))
    fixed = nibabel_affine(_header_of(fixed_image))
    if np.any(obliquity(fixed) > _WARP_DIR_OBLIQUITY):
        logging.getLogger("larmorx.sdcflows").warning(
            "Running fieldmap-less registration on an oblique dataset"
        )
    moving_pe_axis = _unsigned_axcodes(moving)["ijk".index(pe_dir[0])]
    fixed_axcodes = _unsigned_axcodes(fixed)
    position = fixed_axcodes.index(moving_pe_axis)
    world = "RAS".index(moving_pe_axis)
    if position != world:
        _defects.warn(
            [
                (
                    "SDC-16",
                    "sdcflows",
                    upstream_defects,
                    f"the deformation is restricted to world axis {'xyz'[world]} (the phase "
                    f"encoding's, {moving_pe_axis}); sdcflows frees world axis "
                    f"{'xyz'[position]} instead (the position of {moving_pe_axis} among the "
                    f"fixed image's voxel axes {''.join(str(c) for c in fixed_axcodes)}), and "
                    "ANTs weighs the physical axes",
                )
            ]
        )
    deformation = [0.1, 0.1, 0.1]
    deformation[position if upstream_defects == "reproduce" else world] = 1.0
    return nlevels * [deformation]


def mm2vox(
    moving_image: str | os.PathLike[str],
    fixed_image: str | os.PathLike[str],
    pe_dir: str,
    registration_config: Mapping[str, Any],
) -> list[list[Any]]:
    """sdcflows' ``_mm2vox``: the settings' ``transform_parameters`` with each level's third
    value (SyN's total-field variance, mm) divided by the fixed image's voxel size along the
    axis closest to the moving image's phase-encoding axis (nibabel's ``voxel_sizes``)."""
    params = registration_config["transform_parameters"]
    moving = nibabel_affine(_header_of(moving_image))
    moving_pe_axis = _unsigned_axcodes(moving)["ijk".index(pe_dir[0])]
    fixed = nibabel_affine(_header_of(fixed_image))
    fixed_axcodes = _unsigned_axcodes(fixed)
    zooms = np.sqrt(np.sum(fixed[:-1, :-1] ** 2, axis=0))
    pe_res = zooms[fixed_axcodes.index(moving_pe_axis)]
    return [[*level[:2], level[2] / pe_res] for level in params]


def adjust_zooms(
    in_anat: str | os.PathLike[str],
    in_epi: str | os.PathLike[str],
    z_max: float = 2.2,
    z_min: float = 1.8,
) -> tuple[Any, Any, Any]:
    """sdcflows' ``_adjust_zooms``: ``min(round(max(0.5 * (anat + epi), z_min), 2), z_max)``
    three times, ``anat`` and ``epi`` the smallest of each header's three voxel sizes; numpy
    2's arithmetic of the header's float32 values (a float32 result, or the Python bound)."""
    f32 = np.float32
    anat_res = min(f32(z) for z in _header_of(in_anat).zooms[:3])
    epi_res = min(f32(z) for z in _header_of(in_epi).zooms[:3])
    half = f32(f32(anat_res + epi_res) * f32(0.5))
    # Python's max and min keep the first argument unless the second is larger (smaller); a
    # float32 is compared with a Python float in float32 (NEP 50); numpy rounds a float32 in
    # float32 (x * 100, rint, / 100)
    value: Any
    if f32(z_min) > half:
        value = round(float(z_min), 2)
        if z_max < value:
            value = z_max
    else:
        value = f32(np.rint(f32(half * f32(100.0))) / f32(100.0))
        if f32(z_max) < value:
            value = z_max
    return (value, value, value)


def norm_lap(
    in_file: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str] = ".",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> str:
    """sdcflows' ``_norm_lap`` (from nirodents): the image (float64) minus its median, its
    positive values divided by their 99.8th percentile and its negative ones by minus their
    0.2th (numpy's linear percentiles), clipped to [-1, 1] and written as float32 with the
    input's header as ``<name>_norm`` in ``out_dir``.

    An image with no positive or no negative value after the median's subtraction (a constant
    image, for example) has no such percentile: sdcflows fails with numpy's ``IndexError``
    (``"reproduce"``); larmorx raises a ``ValueError`` saying so (SDC-14, rule B)."""
    from larmorx import _core

    _defects.check(upstream_defects)
    in_file = os.fspath(in_file)
    img = load_nibabel(in_file, dtype=np.float64, n_threads=n_threads)
    assert img.header is not None
    data = np.array(img.data, dtype=np.float64)
    data -= np.float64(_core.numpy_median(data))
    pos = data[data > 0]
    neg = data[data < 0]
    if pos.size == 0 or neg.size == 0:
        if upstream_defects == "reproduce":
            raise IndexError("index -1 is out of bounds for axis 0 with size 0")
        side = "positive" if pos.size == 0 else "negative"
        raise ValueError(
            f"{in_file}: the Laplacian has no {side} value after its median is subtracted "
            "(a constant image?), so it cannot be normalised (sdcflows fails here with "
            "numpy's IndexError: SDC-14)"
        )
    l_max = _core.numpy_percentile(pos, [99.8])[0]
    l_min = _core.numpy_percentile(neg, [0.2])[0]
    data[data < 0] *= -1.0 / l_min
    data[data > 0] *= 1.0 / l_max
    data = np.clip(data, -1.0, 1.0)
    os.makedirs(out_dir, exist_ok=True)
    out_file = _nipype.fname_presuffix(
        os.path.basename(in_file), suffix="_norm", newpath=os.path.abspath(os.fspath(out_dir))
    )
    save_nibabel(
        Image(data.astype(np.float32), img.affine, img.header),
        out_file,
        dtype=np.dtype(np.float32),
        n_threads=n_threads,
    )
    return out_file


def _laplacian(op1: str, cwd: str, upstream_defects: str, n_threads: int) -> tuple[str, str]:
    """sdcflows' ``ImageMath(operation='Laplacian', op2='1.5 1', copy_header=True)``: nipype's
    line (``ImageMath 3 <op1>_maths Laplacian <op1> 1.5 1``) run by larmorx's port, then
    nipype's ``copy_header`` (``keep_dtype``)."""
    cmd = _nipype.image_math(cwd=cwd, op1=op1, operation="Laplacian", op2="1.5 1", copy_header=True)
    _nipype.run(cmd, n_threads=n_threads, upstream_defects=upstream_defects)
    out = cmd.outputs["output_image"]
    _copy_header_keep_dtype(op1, out, n_threads)
    return out, cmd.cmdline


@dataclass(frozen=True)
class SynSdcResult:
    """``init_syn_sdc_wf``'s outputs: ``fmap`` (the field map, Hz, on the EPI reference's
    grid: ``unwarp``'s ``out_field``), ``fmap_ref`` (the EPI reference unwarped:
    ``out_corrected``), ``fmap_coeff`` (the B-spline coefficients, one file per control grid),
    ``fmap_mask`` (the anatomical mask on the field's grid), ``out_warp`` (declared by sdcflows'
    output node and never connected: ``None``), ``method``; the readout time and
    phase-encoding direction read from the metadata; ``zooms`` (``find_zooms``' value); the
    files of the other nodes and the command lines nipype would write."""

    fmap: str
    fmap_ref: str
    fmap_coeff: tuple[str, ...]
    fmap_mask: str
    out_warp: None
    method: str
    readout_time: float
    pe_direction: str
    zooms: tuple[Any, Any, Any]
    files: Mapping[str, Any]
    commands: Mapping[str, str]


def syn_sdc_workflow(
    epi_ref: tuple[str | os.PathLike[str], Mapping[str, Any]] | Sequence[Any],
    epi_mask: str | os.PathLike[str],
    anat_ref: str | os.PathLike[str],
    anat_mask: str | os.PathLike[str],
    sd_prior: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str],
    name: str = "syn_sdc_wf",
    use_metadata_estimates: bool = False,
    fallback_total_readout_time: float | None = None,
    sloppy: bool = False,
    debug: bool = False,
    omp_nthreads: int = 1,
    laplacian_weight: Sequence[float] | None = None,
    ants_random_seed: int | None = None,
    niworkflows_version: str = NIWORKFLOWS_VERSION,
    numpy_version: str = "2.5.4",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> SynSdcResult:
    """sdcflows' ``init_syn_sdc_wf``: the fieldmap-less ("SyN") estimation of the
    susceptibility distortion, from :func:`syn_preprocessing_workflow`'s outputs.

    The EPI reference (``epi_ref``: its path and metadata) is clipped (``IntensityClip(35,
    99.9)``) and registered to the anatomical reference in its space (``anat_ref``) with
    ``antsRegistration`` and sdcflows' ``sd_syn.json`` (``sd_syn_sloppy.json`` with
    ``sloppy``): two images per side (the image and its normalised Laplacian, ANTs'
    ``ImageMath ... Laplacian 1.5 1``), the deformation restricted to the axis of the phase
    encoding (:func:`warp_dir`, SyN's field variance in voxels: :func:`mm2vox`), the fixed
    masks ``[anat_mask, sd_prior]``, the moving ones ``[NULL, epi_mask | anat_mask dilated]``.
    The registration's displacement field becomes a field map (Hz:
    :func:`displacements_field_to_fieldmap` with the metadata's readout time,
    :func:`get_readout_time`), is brought onto the EPI reference's grid (``antsApplyTransforms
    -t identity``, B-spline, ``-u float``), smoothed by sdcflows' B-spline fit
    (:func:`bspline_approx`, ``recenter=False``; ``zooms_min=4`` with ``sloppy``) and applied
    to the EPI reference (:func:`apply_coeffs_field`, no Jacobian). The nodes write into
    ``out_dir/<name>/<node>`` as nipype's do (fMRIPrep names the workflow ``wf_<estimator>``).

    ``laplacian_weight`` (two weights, each clipped to [0, 0.5]) replaces the settings' metric
    weights by ``[[1 - w0, w0], [1 - w1, w1]]``. ``debug`` writes the registration's warped
    images and every second iteration's volumes (``--write-interval-volumes 2``) and fits
    without extrapolation, as sdcflows does. ``ants_random_seed`` is what ANTs reads from
    ``ANTS_RANDOM_SEED`` (fMRIPrep sets it); ``omp_nthreads`` the registration's thread count
    in nipype's line; ``n_threads`` larmorx's (the results do not depend on either).
    ``numpy_version`` selects the clip's percentile arithmetic; ``niworkflows_version`` the
    version written into ``descrip``.

    One node is not computed: ``zooms_epi`` (niworkflows' ``RegridToZooms`` of the clipped EPI
    to ``find_zooms``' voxel size), whose output sdcflows never connects (SDC-15, rule C: no
    result depends on it); ``zooms`` holds ``find_zooms``' value."""
    import json

    from larmorx.niworkflows import intensity_clip
    from larmorx.niworkflows.header import copy_xform
    from larmorx.sdcflows.approx import bspline_approx
    from larmorx.sdcflows.bspline import DEFAULT_HF_ZOOMS_MM
    from larmorx.sdcflows.fmap import displacements_field_to_fieldmap
    from larmorx.sdcflows.masks import binary_dilation, union
    from larmorx.sdcflows.pepolar import get_readout_time
    from larmorx.sdcflows.registration import registration_settings
    from larmorx.sdcflows.transform import apply_coeffs_field

    _defects.check(upstream_defects)
    wf = os.path.join(os.path.abspath(os.fspath(out_dir)), name)

    def node(n: str) -> str:
        p = os.path.join(wf, n)
        os.makedirs(p, exist_ok=True)
        return p

    epi_file = os.path.abspath(os.fspath(_front(epi_ref)))
    metadata = _last(epi_ref)
    epi_mask = os.path.abspath(os.fspath(epi_mask))
    anat_ref = os.path.abspath(os.fspath(anat_ref))
    anat_mask = os.path.abspath(os.fspath(anat_mask))
    sd_prior = os.path.abspath(os.fspath(sd_prior))
    ants_threads = max(int(omp_nthreads), 1)
    files: dict[str, Any] = {}
    commands: dict[str, str] = {}
    at_message = f"FixHeaderApplyTransforms (niworkflows v{niworkflows_version})"

    # readout_time
    node("readout_time")
    ro = get_readout_time(
        metadata,
        epi_file,
        use_estimate=use_metadata_estimates,
        fallback=fallback_total_readout_time,
    )
    # clip_epi, and the two Laplacians
    clipped = intensity_clip(
        epi_file,
        p_min=35.0,
        p_max=99.9,
        out_dir=node("clip_epi"),
        numpy_version=numpy_version,
        n_threads=n_threads,
    )
    assert clipped.out_file is not None
    clip_epi = clipped.out_file
    lap_anat, commands["lap_anat"] = _laplacian(
        anat_ref, node("lap_anat"), upstream_defects, ants_threads
    )
    lap_anat_norm = norm_lap(
        lap_anat, out_dir=node("lap_anat_norm"), upstream_defects=upstream_defects,
        n_threads=n_threads,
    )  # fmt: skip
    lap_epi, commands["lap_epi"] = _laplacian(
        clip_epi, node("lap_epi"), upstream_defects, ants_threads
    )
    lap_epi_norm = norm_lap(
        lap_epi, out_dir=node("lap_epi_norm"), upstream_defects=upstream_defects,
        n_threads=n_threads,
    )  # fmt: skip
    # the masks: anat_dilmsk, amask2epi, epi_umask
    _, anat_dil = binary_dilation(anat_mask, out_dir=node("anat_dilmsk"), n_threads=n_threads)
    assert anat_dil is not None
    amask2epi, commands["amask2epi"] = _apply_transforms(
        node("amask2epi"),
        anat_dil,
        epi_mask,
        ["identity"],
        None,
        "MultiLabel",
        at_message,
        upstream_defects,
        n_threads,
    )
    _, umask = union(epi_mask, amask2epi, out_dir=node("epi_umask"), n_threads=n_threads)
    assert umask is not None
    # find_zooms (zooms_epi's RegridToZooms is not computed: SDC-15)
    node("find_zooms")
    zooms = adjust_zooms(anat_ref, epi_file)
    # warp_dir, vox_params
    node("warp_dir")
    restrict = warp_dir(
        clip_epi, anat_ref, ro.pe_direction, nlevels=2, upstream_defects=upstream_defects
    )
    settings_file = registration_settings(f"sd_syn{'_sloppy' * bool(sloppy)}.json")
    node("vox_params")
    config = json.loads(settings_file.read_text(encoding="utf-8"))
    transform_parameters = mm2vox(clip_epi, anat_ref, ro.pe_direction, config)
    # syn
    extra: dict[str, Any] = {}
    if laplacian_weight is not None:
        w = (
            max(min(laplacian_weight[0], MAX_LAPLACIAN_WEIGHT), 0.0),
            max(min(laplacian_weight[1], MAX_LAPLACIAN_WEIGHT), 0.0),
        )
        extra["metric_weight"] = [[1.0 - w[0], w[0]], [1.0 - w[1], w[1]]]
    if debug:
        extra["args"] = "--write-interval-volumes 2"
    reg = _nipype.registration(
        cwd=node("syn"),
        from_file=settings_file,
        output_warped_image=bool(debug),
        output_inverse_warped_image=bool(debug),
        restrict_deformation=restrict,
        fixed_image=[anat_ref, lap_anat_norm],
        fixed_image_masks=[anat_mask, sd_prior],
        moving_image=[clip_epi, lap_epi_norm],
        moving_image_masks=["NULL", umask],
        transform_parameters=transform_parameters,
        num_threads=ants_threads,
        **extra,
    )
    _nipype.run(
        reg,
        n_threads=ants_threads,
        upstream_defects=upstream_defects,
        ants_random_seed=ants_random_seed,
    )
    commands["syn"] = reg.cmdline
    message = f"FixHeaderRegistration (niworkflows v{niworkflows_version})"
    for key, ref_image in (("warped_image", anat_ref), ("inverse_warped_image", clip_epi)):
        produced = reg.outputs.get(key)
        if produced:
            copy_xform(ref_image, produced, message=message, n_threads=n_threads)
            files[f"syn_{key}"] = produced
    forward = list(reg.outputs["forward_transforms"])
    # extract_field, zooms_field, zooms_bmask
    extracted = displacements_field_to_fieldmap(
        _front(forward),
        clip_epi,
        ro.readout_time,
        ro.pe_direction,
        out_dir=node("extract_field"),
    )
    assert extracted.out_file is not None
    zooms_field, commands["zooms_field"] = _apply_transforms(
        node("zooms_field"),
        os.fspath(extracted.out_file),
        epi_file,
        ["identity"],
        None,
        "BSpline",
        at_message,
        upstream_defects,
        n_threads,
        args="-u float",
    )
    zooms_bmask, commands["zooms_bmask"] = _apply_transforms(
        node("zooms_bmask"),
        anat_mask,
        zooms_field,
        ["identity"],
        None,
        "MultiLabel",
        at_message,
        upstream_defects,
        n_threads,
        args="-u uchar",
    )
    # bs_filter, unwarp
    fit_kwargs: dict[str, Any] = {"zooms_min": 4.0} if sloppy else {}
    fit = bspline_approx(
        zooms_field,
        None,
        bs_spacing=[DEFAULT_HF_ZOOMS_MM],
        recenter=False,
        extrapolate=not debug,
        debug=debug,
        out_dir=node("bs_filter"),
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        **fit_kwargs,
    )
    coeffs = tuple(os.fspath(p) for p in fit.out_coeff)
    unwarped = apply_coeffs_field(
        epi_file,
        list(coeffs),
        ro.readout_time,
        ro.pe_direction,
        jacobian=False,
        out_dir=node("unwarp"),
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    assert unwarped.out_corrected is not None and unwarped.out_field is not None
    files.update(
        {
            "clip_epi": clip_epi,
            "lap_anat": lap_anat,
            "lap_anat_norm": lap_anat_norm,
            "lap_epi": lap_epi,
            "lap_epi_norm": lap_epi_norm,
            "anat_dilmsk": anat_dil,
            "amask2epi": amask2epi,
            "epi_umask": umask,
            "warp_dir": restrict,
            "vox_params": transform_parameters,
            "syn": forward,
            "extract_field": os.fspath(extracted.out_file),
            "zooms_field": zooms_field,
            "zooms_bmask": zooms_bmask,
            "bs_filter": coeffs,
        }
    )
    return SynSdcResult(
        fmap=os.fspath(unwarped.out_field),
        fmap_ref=os.fspath(unwarped.out_corrected),
        fmap_coeff=coeffs,
        fmap_mask=zooms_bmask,
        out_warp=None,
        method=SYN_METHOD,
        readout_time=ro.readout_time,
        pe_direction=ro.pe_direction,
        zooms=zooms,
        files=files,
        commands=commands,
    )
