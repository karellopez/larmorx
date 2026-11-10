# SPDX-License-Identifier: Apache-2.0 AND BSD-2-Clause
"""niworkflows' header fixes, its ``ApplyMask`` and ``KeySelect``, and its
``init_skullstrip_bold_wf`` as a plain function.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``utils/images.py`` (``_copyxform``: :func:`copy_xform`),
``interfaces/header.py`` (``CopyHeader``: :func:`copy_header`; ``ValidateImage``:
:func:`validate_image`, with transforms3d 0.4.2's ``decompose44`` and ``mat2axangle``, BSD-2-Clause,
for its report's text), ``interfaces/nibabel.py``
(``ApplyMask``: :func:`apply_mask`), ``interfaces/utility.py`` (``KeySelect``:
:func:`key_select`) and ``func/util.py`` (``init_skullstrip_bold_wf``:
:func:`skullstrip_bold_workflow`, FSL's ``bet``, AFNI's ``3dAutomask`` and ``fslmaths`` run with
nipype's command lines through larmorx's ports, :mod:`larmorx._nipype`). Files are read and
written as nibabel reads and writes them (:func:`larmorx.io.load_nibabel`,
:func:`larmorx.io.save_nibabel`).

transforms3d's notice (for the translated ``decompose44`` and ``mat2axangle``; also in NOTICE)::

    Copyright (c) 2009-2024, Matthew Brett and Christoph Gohlke
    All rights reserved.

    Redistribution and use in source and binary forms, with or without
    modification, are permitted provided that the following conditions are
    met:

    1. Redistributions of source code must retain the above copyright notice,
    this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in the
    documentation and/or other materials provided with the distribution.

    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
    IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
    THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
    PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
    CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
    EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
    PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
    PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
    LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
    NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
    SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
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
    "ValidateImageResult",
    "apply_mask",
    "copy_header",
    "copy_xform",
    "key_select",
    "skullstrip_bold_workflow",
    "validate_image",
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


#: nibabel's ``unit_codes`` labels of the spatial units.
_XYZ_UNITS = {0: "unknown", 1: "meter", 2: "mm", 3: "micron"}


@dataclass(frozen=True)
class ValidateImageResult:
    """``ValidateImage``'s outputs: ``out_file`` (the input itself, or ``<in>_valid`` with the
    fixed x-forms), ``out_report`` (``report.html`` in ``out_dir``: empty, or the HTML
    snippet) and ``warning`` (the snippet's title, ``None`` when nothing changed)."""

    out_file: str
    out_report: str
    warning: str | None


def _srow_sform(fields: dict) -> np.ndarray:
    """nibabel's ``get_sform()`` (the srow rows, whatever the code)."""
    out = np.eye(4)
    for i, row in enumerate(("srow_x", "srow_y", "srow_z")):
        out[i, :] = np.asarray(fields[row], dtype=np.float64)
    return out


def _as_stored(fields: dict) -> dict:
    """Header fields with the x-form values rounded to the header's storage (``float32`` in
    NIfTI-1), as nibabel's header holds them after ``set_qform`` / ``set_sform``."""
    if int(fields.get("version", 1)) != 1:
        return fields
    out = dict(fields)
    for k in ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z"):
        out[k] = float(np.float32(out[k]))
    for k in ("pixdim", "srow_x", "srow_y", "srow_z"):
        out[k] = [float(v) for v in np.asarray(out[k], dtype=np.float32)]
    return out


def _det_is_zero(m: np.ndarray) -> bool:
    """``np.linalg.det(m) == 0`` for a 4 x 4 affine (its last row 0, 0, 0, 1): LAPACK's LU of
    the 3 x 3 block, as numpy computes it, has a zero pivot (``det(-a) = -det(a)`` exactly, so
    neither is positive only then)."""
    from larmorx import _core

    a = np.ascontiguousarray(m[:3, :3], dtype=np.float64)
    pos = _core.numpy_linalg_small("det_positive", a)
    neg = _core.numpy_linalg_small("det_positive", np.ascontiguousarray(-a))
    return not pos and not neg


def _axangle_and_translation(diff: np.ndarray) -> tuple[float, float]:
    """transforms3d 0.4.2's ``decompose44(diff)``: the translation's norm and the rotation's
    angle (``mat2axangle``). The decomposition is computed as transforms3d computes it (numpy's
    vector dot products: :func:`larmorx._core.numpy_dot`); the rotation's axis comes from
    ``numpy.linalg.eig`` and the angle from ``math.atan2``, as there. The report prints two
    significant digits; for a change at the precision of a qform the angle is rounding noise."""
    import math

    from larmorx import _core

    t = diff[:-1, -1]
    m0, m1, m2 = (np.array(c, dtype=np.float64) for c in np.array(diff[:-1, :-1]).T)
    sx = math.sqrt(np.sum(m0**2))
    m0 /= sx
    sx_sxy = _core.numpy_dot(m0, m1)
    m1 -= sx_sxy * m0
    sy = math.sqrt(np.sum(m1**2))
    m1 /= sy
    sx_sxz = _core.numpy_dot(m0, m2)
    sy_syz = _core.numpy_dot(m1, m2)
    m2 -= sx_sxz * m0 + sy_syz * m1
    sz = math.sqrt(np.sum(m2**2))
    m2 /= sz
    r = np.array([m0, m1, m2]).T
    if np.linalg.det(r) < 0:
        r[:, 0] *= -1
    lam, w = np.linalg.eig(r.T)
    i = np.where(np.abs(lam - 1.0) < 1e-5)[0]
    if not len(i):
        raise ValueError("no unit eigenvector corresponding to eigenvalue 1")
    direction = np.real(w[:, i[-1]]).squeeze()
    cosa = (np.trace(r) - 1.0) / 2.0
    if abs(direction[2]) > 1e-8:
        sina = (r[1, 0] + (cosa - 1.0) * direction[0] * direction[1]) / direction[2]
    elif abs(direction[1]) > 1e-8:
        sina = (r[0, 2] + (cosa - 1.0) * direction[0] * direction[2]) / direction[1]
    else:
        sina = (r[2, 1] + (cosa - 1.0) * direction[1] * direction[2]) / direction[0]
    angle = math.atan2(sina, cosa)
    return angle, float(np.sqrt(np.sum(t * t)))


def validate_image(
    in_file: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str] = ".",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> ValidateImageResult:
    """niworkflows' ``ValidateImage``: the x-forms of ``in_file`` checked against
    niworkflows' truth table. Valid, matching qform and sform with codes above 0 (or a qform
    that only differs by the precision a qform can hold): the input itself and an empty
    report. Otherwise ``<in>_valid`` in ``out_dir`` with the sform copied from a valid qform
    (``Note on orientation: sform matrix set``), the qform from a valid sform (``Note on
    orientation: qform matrix overwritten``, with the angle and translation of the change, or
    ``WARNING - Invalid qform information``), or both set to nibabel's base affine with code 1
    (``WARNING - Missing orientation information``); the report (``report.html``) holds the
    HTML snippet. Files are read and written as nibabel does.

    PIPE-24: the report names the translation's unit with nibabel's ``get_xyzt_units``, which
    raises ``KeyError`` for a spatial unit code 4 to 7 (no NIfTI unit); larmorx reports such a
    translation in mm, as for an unknown unit (``upstream_defects="reproduce"``: the
    ``KeyError``)."""
    from textwrap import indent

    from larmorx import _defects
    from larmorx.io.nibabel_header import _get_qform, nibabel_affine

    _defects.check(upstream_defects)
    in_file = os.fspath(in_file)
    img = load_nibabel(in_file, n_threads=n_threads)
    assert img.header is not None
    fields = img.header.to_dict()
    os.makedirs(out_dir, exist_ok=True)
    report = os.path.join(os.path.abspath(os.fspath(out_dir)), "report.html")

    def unchanged() -> ValidateImageResult:
        open(report, "w").close()
        return ValidateImageResult(in_file, report, None)

    sform_code = int(fields["sform_code"])
    qform_code = int(fields["qform_code"])
    valid_qform = False
    qform = None
    try:
        qform = _get_qform(img.header)
        valid_qform = True
    except ValueError:
        pass
    sform = _srow_sform(fields)
    if _det_is_zero(sform):
        valid_sform = False
    else:
        rzs = sform[:3, :3]
        zooms = np.sqrt(np.sum(rzs * rzs, axis=0))
        hdr_zooms = np.asarray(img.header.pixdim, dtype=np.float32)[1 : int(fields["dim"][0]) + 1]
        valid_sform = bool(np.allclose(zooms, tuple(hdr_zooms)[:3]))
    matching_affines = valid_qform and bool(np.allclose(qform, sform))
    if matching_affines and qform_code > 0 and sform_code > 0:
        return unchanged()
    if valid_qform and qform_code > 0 and (sform_code == 0 or not valid_sform):
        fields = set_xform(fields, "sform", qform, qform_code)
        warning_txt = "Note on orientation: sform matrix set"
        description = '<p class="elem-desc">The sform has been copied from qform.</p>\n'
    elif (valid_sform and sform_code > 0) and (not matching_affines or qform_code == 0):
        fields = set_xform(fields, "qform", sform, sform_code)
        new_qform = _get_qform(NiftiHeader.from_dict(_as_stored(fields)))
        if valid_qform:
            if np.allclose(new_qform, qform) and qform_code > 0:
                return unchanged()
            from larmorx.transforms._linalg import inv, matmul

            diff = matmul(inv(qform), new_qform)
            angle, total_trans = _axangle_and_translation(diff)
            xyz_code = int(fields["xyzt_units"]) % 8
            if xyz_code not in _XYZ_UNITS:
                if upstream_defects == "reproduce":
                    raise KeyError(xyz_code)
                _defects.warn(
                    [
                        (
                            "PIPE-24",
                            "niworkflows ValidateImage",
                            upstream_defects,
                            f"spatial unit code {xyz_code} is no NIfTI unit: the report gives "
                            "the translation in mm, as for an unknown unit (niworkflows raises "
                            "KeyError)",
                        )
                    ]
                )
            xyz_unit = _XYZ_UNITS.get(xyz_code, "unknown")
            if xyz_unit == "unknown":
                xyz_unit = "mm"
            warning_txt = "Note on orientation: qform matrix overwritten"
            description = f"""\
    <p class="elem-desc">
    The qform has been copied from sform.
    The difference in angle is {angle:.02g} radians.
    The difference in translation is {total_trans:.02g}{xyz_unit}.
    </p>
    """
        elif qform_code > 0:
            warning_txt = "WARNING - Invalid qform information"
            description = """\
<p class="elem-desc">
    The qform matrix found in the file header is invalid.
    The qform has been copied from sform.
    Checking the original qform information from the data produced
    by the scanner is advised.
</p>
"""
        else:
            warning_txt = "Note on orientation: qform matrix overwritten"
            description = '<p class="elem-desc">The qform has been copied from sform.</p>'
    else:
        from larmorx.io.nibabel_header import _base_affine

        affine = _base_affine(img.header)
        fields = set_xform(fields, "sform", affine, 1)
        fields = set_xform(fields, "qform", affine, 1)
        warning_txt = "WARNING - Missing orientation information"
        description = """\
<p class="elem-desc">
    FMRIPREP could not retrieve orientation information from the image header.
    The qform and sform matrices have been set to a default, LAS-oriented affine.
    Analyses of this dataset MAY BE INVALID.
</p>
"""
    snippet = f'<h3 class="elem-title">{warning_txt}</h3>\n{description}\n'
    out_fname = _nipype.fname_presuffix(in_file, suffix="_valid", newpath=out_dir)
    header = NiftiHeader.from_dict(fields)
    save_nibabel(Image(img.data, nibabel_affine(header), header), out_fname, n_threads=n_threads)
    with open(report, "w", encoding="utf-8") as fobj:
        fobj.write(indent(snippet, "\t" * 3))
    return ValidateImageResult(out_fname, report, warning_txt)


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
