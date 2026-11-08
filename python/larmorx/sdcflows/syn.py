# SPDX-License-Identifier: Apache-2.0
"""sdcflows' fieldmap-less ("SyN") preprocessing as plain functions: the EPI reference, the
anatomical reference and its masks in the EPI's space, and the prior of strong distortions.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1, fMRIPrep
25.2.6's): ``workflows/fit/syn.py`` (``init_syn_preprocessing_wf``, ``_set_dtype``,
``_merge_meta``) and ``interfaces/utils.py`` (``Deoblique``, ``Reoblique``: ``_deoblique``,
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

Under measurement (SDC-13, rule D; decided with ``init_syn_sdc_wf``'s port): sdcflows compares
nibabel's ``obliquity`` (radians) with ``OBLIQUE_THRESHOLD_DEG = 0.5`` (meant as degrees), so
``Deoblique`` and ``Reoblique`` act only beyond 28.6 degrees; and ``_deoblique``'s plumb grid
centres the image in voxels, not millimetres, and compares the axis codes with ``RAS`` by
position. Both modes give sdcflows' behaviour until the measurement decides.
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
    "OBLIQUE_THRESHOLD_DEG",
    "SynPreprocessingResult",
    "deoblique",
    "fmap_atlas",
    "fmap_atlas_affine",
    "merge_meta",
    "obliquity",
    "reoblique",
    "set_dtype",
    "syn_preprocessing_workflow",
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
    hdr_img = load_nibabel(in_file, n_threads=n_threads)
    out_img = load_nibabel(out, n_threads=n_threads)
    assert hdr_img.header is not None and out_img.header is not None
    dtype = np.dtype(out_img.header.data_dtype)
    header = NiftiHeader.from_dict(hdr_img.header.to_dict())
    save_nibabel(
        Image(out_img.data, nibabel_affine(header), header), out, dtype=dtype, n_threads=n_threads
    )
    return out, cmd.cmdline


def _apply_transforms(
    cwd: str,
    input_image: str,
    reference_image: str,
    transforms: Sequence[str],
    invert: Sequence[bool],
    interpolation: str,
    message: str,
    upstream_defects: str,
    n_threads: int,
) -> tuple[str, str]:
    """niworkflows' ``FixHeaderApplyTransforms``: nipype's ``antsApplyTransforms`` line run by
    larmorx's port, then the reference's x-forms copied onto the output (``_copyxform``)."""
    from larmorx.niworkflows.header import copy_xform

    cmd = _nipype.apply_transforms(
        cwd=cwd,
        input_image=input_image,
        reference_image=reference_image,
        transforms=list(transforms),
        invert_transform_flags=list(invert),
        interpolation=interpolation,
        num_threads=max(int(n_threads), 1),
    )
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
