# SPDX-License-Identifier: Apache-2.0
"""niworkflows' header fixes, its ``ApplyMask`` and ``KeySelect``, and its
``init_skullstrip_bold_wf`` as a plain function.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``utils/images.py`` (``_copyxform``: :func:`copy_xform`),
``interfaces/header.py`` (``CopyHeader``: :func:`copy_header`), ``interfaces/nibabel.py``
(``ApplyMask``: :func:`apply_mask`), ``interfaces/utility.py`` (``KeySelect``:
:func:`key_select`) and ``func/util.py`` (``init_skullstrip_bold_wf``:
:func:`skullstrip_bold_workflow`, FSL's ``bet``, AFNI's ``3dAutomask`` and ``fslmaths`` run with
nipype's command lines through larmorx's ports, :mod:`larmorx._nipype`). Files are read and
written as nibabel reads and writes them (:func:`larmorx.io.load_nibabel`,
:func:`larmorx.io.save_nibabel`).
"""

from __future__ import annotations

import os
import shutil
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _nipype
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import load_nibabel, set_xform
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = [
    "SkullstripBoldResult",
    "apply_mask",
    "copy_header",
    "copy_xform",
    "key_select",
    "skullstrip_bold_workflow",
]


def _qform_sform(hdr: NiftiHeader) -> tuple[np.ndarray, int, np.ndarray, int]:
    """nibabel's ``get_qform(coded=True)`` and ``get_sform(coded=True)``: the matrices and
    their codes (a matrix is ``None`` when its code is 0)."""
    from larmorx.io.nibabel_header import _get_qform

    fields = hdr.to_dict()
    qcode, scode = int(fields["qform_code"]), int(fields["sform_code"])
    qform = _get_qform(hdr) if qcode else None
    sform = None
    if scode:
        sform = np.eye(4)
        sform[0, :] = np.asarray(fields["srow_x"], dtype=np.float64)
        sform[1, :] = np.asarray(fields["srow_y"], dtype=np.float64)
        sform[2, :] = np.asarray(fields["srow_z"], dtype=np.float64)
    return qform, qcode, sform, scode  # type: ignore[return-value]


def copy_xform(
    ref_image: str | os.PathLike[str],
    out_image: str | os.PathLike[str],
    message: str | None = None,
    *,
    n_threads: int = 1,
) -> str:
    """niworkflows' ``_copyxform``: the qform and sform (with their codes) of ``ref_image``
    written into ``out_image``'s header (overwritten in place), ``descrip`` set to ``xform
    matrices modified by <message>.``, and the image made with the reference's affine
    (``FixHeaderApplyTransforms`` and ``FixHeaderRegistration`` call it on their outputs). The
    path of ``out_image``."""
    out_image = os.fspath(out_image)
    resampled = load_nibabel(out_image, n_threads=n_threads)
    orig = load_nibabel(ref_image, n_threads=n_threads)
    assert resampled.header is not None and orig.header is not None
    qform, qcode, sform, scode = _qform_sform(orig.header)
    fields = resampled.header.to_dict()
    for which, matrix, code in (("qform", qform, qcode), ("sform", sform, scode)):
        if matrix is None:
            # nibabel's set_qform(None, code) / set_sform(None, code): the code only
            fields[f"{which}_code"] = int(code)
        else:
            fields = set_xform(fields, which, matrix, code)
    text = ("xform matrices modified by %s." % (message or "(unknown)")).encode()
    size = len(fields["descrip"])
    fields["descrip"] = text[:size].ljust(size, b"\0")
    header = NiftiHeader.from_dict(fields)
    save_nibabel(Image(resampled.data, orig.affine, header), out_image, n_threads=n_threads)
    return out_image


def copy_header(
    hdr_file: str | os.PathLike[str],
    in_file: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str] = ".",
    n_threads: int = 1,
) -> str:
    """niworkflows' ``CopyHeader``: ``in_file``'s data with ``hdr_file``'s affine and header,
    in ``in_file``'s stored type, written to ``<in_file>_fixhdr`` in ``out_dir``. Its path."""
    hdr = load_nibabel(hdr_file, n_threads=n_threads)
    src = load_nibabel(in_file, n_threads=n_threads)
    assert hdr.header is not None and src.header is not None
    fields = hdr.header.to_dict()
    header = NiftiHeader.from_dict(fields)
    os.makedirs(out_dir, exist_ok=True)
    out = _nipype.fname_presuffix(in_file, suffix="_fixhdr", newpath=out_dir)
    save_nibabel(
        Image(src.data, hdr.affine, header), out, dtype=src.header.data_dtype, n_threads=n_threads
    )
    return out


def apply_mask(
    in_file: str | os.PathLike[str],
    in_mask: str | os.PathLike[str],
    *,
    threshold: float = 0.5,
    out_dir: str | os.PathLike[str] = ".",
    n_threads: int = 1,
) -> str:
    """niworkflows' ``ApplyMask``: ``in_file``'s data (as nibabel's ``dataobj`` reads it) times
    the mask's values above ``threshold``, with ``in_file``'s header, written to
    ``<in_file>_masked`` in ``out_dir`` (nibabel's writer: the header's stored type).
    ``ValueError`` when the sizes or the affines differ. Its path."""
    img = load_nibabel(in_file, n_threads=n_threads)
    msknii = load_nibabel(in_mask, dtype=np.float64, n_threads=n_threads)
    msk = np.asarray(msknii.data) > float(threshold)
    os.makedirs(out_dir, exist_ok=True)
    out = _nipype.fname_presuffix(in_file, suffix="_masked", newpath=out_dir)
    data = np.asanyarray(img.data)
    if tuple(data.shape[:3]) != tuple(msk.shape):
        raise ValueError("Image and mask sizes do not match.")
    if not np.allclose(img.affine, msknii.affine):
        raise ValueError("Image and mask affines are not similar enough.")
    if data.ndim == msk.ndim + 1:
        msk = msk[..., np.newaxis]
    save_nibabel(Image(data * msk, img.affine, img.header), out, n_threads=n_threads)
    return out


def key_select(keys: Sequence[Any], key: Any, **fields: Sequence[Any]) -> dict[str, Any]:
    """niworkflows' ``KeySelect``: the item of every field at ``key``'s index in ``keys``
    (``key`` too). ``ValueError`` (niworkflows' messages) for duplicated keys, a key not in
    ``keys``, no fields, a field that is a string, empty or of another length than ``keys``."""
    if not fields:
        raise ValueError("A list or multiplexed fields must be provided at instantiation time.")
    keys = list(keys)
    if len(set(keys)) != len(keys):
        raise ValueError("Found duplicated entries in the index of ordered keys")
    if key not in keys:
        raise ValueError(f'Selected key "{key}" not found in the index')
    for name, value in fields.items():
        if isinstance(value, str) or len(value) < 1:
            raise ValueError(f'Trying to set an invalid value ({value}) for input "{name}"')
        if len(value) != len(keys):
            raise ValueError(
                f'Length of value ({value}) for input field "{name}" does not match '
                "the length of the indexing list."
            )
    index = keys.index(key)
    out = {name: list(value)[index] for name, value in fields.items()}
    out["key"] = key
    return out


def _link(src: str, folder: str) -> str:
    """nipype's link of an input into the node's folder (``copyfile=False``): a symbolic link
    named as the file, else a copy where links cannot be made."""
    os.makedirs(folder, exist_ok=True)
    dst = os.path.join(folder, os.path.basename(src))
    if os.path.lexists(dst):
        if os.path.realpath(dst) == os.path.realpath(src):
            return dst
        os.remove(dst)
    try:
        os.symlink(os.path.abspath(src), dst)
    except OSError:
        shutil.copyfile(src, dst)
    return dst


@dataclass(frozen=True)
class SkullstripBoldResult:
    """``init_skullstrip_bold_wf``'s outputs and its nodes' files: BET's brain and mask
    (``skullstrip_first_pass``), 3dAutomask's mask and masked brain
    (``skullstrip_second_pass``), their product (``combine_masks``: ``mask_file``) and the input
    masked with it (``apply_mask``: ``skull_stripped_file``); the command lines nipype would
    write (``commands``)."""

    mask_file: str | None
    skull_stripped_file: str | None
    bet_brain: str | None
    bet_mask: str | None
    automask_mask: str | None
    automask_brain: str | None
    commands: Mapping[str, str]


def skullstrip_bold_workflow(
    in_file: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str] | None = None,
    bet_implementation: str = "auto",
    maths_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> SkullstripBoldResult:
    """niworkflows' ``init_skullstrip_bold_wf``: FSL's ``bet <in> <in>_brain -f 0.20 -m``,
    AFNI's ``3dAutomask -apply_prefix <in>_brain_masked.nii.gz -dilate 1 -prefix
    <in>_brain_mask.nii.gz <in>_brain`` on BET's brain, ``fslmaths <bet mask> -mul <automask>
    <bet mask>_maths`` (the mask) and :func:`apply_mask` of the input with it, each node in its
    folder under ``out_dir`` (``skullstrip_first_pass``, ``skullstrip_second_pass``,
    ``combine_masks``, ``apply_mask``; a temporary folder otherwise, the paths then ``None``).
    ``bet`` and ``fslmaths`` run through their replica (``larmorx-nc``) or clean-room original
    (``*_implementation``), ``3dAutomask`` through its replica; the mask report
    (``SimpleShowMaskRPT``) belongs with the reports and is not made."""
    from larmorx.sdcflows.workflows import _Workdir

    wd = _Workdir(out_dir)
    try:
        src = os.path.abspath(os.fspath(in_file))
        first = wd.node("skullstrip_first_pass")
        linked = _link(src, first)
        bet = _nipype.bet(cwd=first, in_file=linked, frac=0.2, mask=True)
        _nipype.run(
            bet,
            implementation=bet_implementation,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )
        second = wd.node("skullstrip_second_pass")
        brain_link = _link(bet.outputs["out_file"], second)
        am = _nipype.automask(cwd=second, in_file=brain_link, dilate=1, outputtype="NIFTI_GZ")
        _nipype.run(am, n_threads=n_threads, upstream_defects=upstream_defects)
        combine = wd.node("combine_masks")
        mul = _nipype.binary_maths(
            cwd=combine,
            in_file=bet.outputs["mask_file"],
            operation="mul",
            operand_file=am.outputs["out_file"],
        )
        _nipype.run(
            mul,
            implementation=maths_implementation,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )
        masked = apply_mask(
            src, mul.outputs["out_file"], out_dir=wd.node("apply_mask"), n_threads=n_threads
        )
        result = SkullstripBoldResult(
            mask_file=mul.outputs["out_file"],
            skull_stripped_file=masked,
            bet_brain=bet.outputs["out_file"],
            bet_mask=bet.outputs["mask_file"],
            automask_mask=am.outputs["out_file"],
            automask_brain=am.outputs["brain_file"],
            commands={
                "skullstrip_first_pass": bet.cmdline,
                "skullstrip_second_pass": am.cmdline,
                "combine_masks": mul.cmdline,
            },
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(result)
