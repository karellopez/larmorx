# SPDX-License-Identifier: Apache-2.0
"""sdcflows' PEPOLAR estimation with AFNI's ``3dQwarp`` (``init_3dQwarp_wf``): its Python steps
as plain functions, the workflow waiting for its programs.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1 and 2.15.2,
fMRIPrep 25.2.x's): ``interfaces/utils.py`` (``Flatten`` with ``_flatten``, ``ConvertWarp`` with
``_qwarp2ants``) and ``workflows/fit/pepolar.py`` (``_sorted_pe``, ``init_3dQwarp_wf``), with
nibabel 5.4.2's (MIT) ``four_to_three``, ``slicer`` and writer through :mod:`larmorx.io`.

- :func:`flatten`: the EPI files split into their first ``max_trs`` volumes, with each one's
  metadata (``Flatten``).
- :func:`sorted_pe`: the files of the first phase-encoding direction and of its opposite, and
  ``3dQwarp``'s options that keep the warp on that axis (``_sorted_pe``).
- :func:`qwarp2ants`: ``3dQwarp``'s displacement field as ANTs reads one (``ConvertWarp``).
- :func:`qwarp_workflow`: ``init_3dQwarp_wf`` needs AFNI's ``3dQwarp`` (``QwarpPlusMinus``),
  FreeSurfer's ``mri_robust_template`` (niworkflows' ``StructuralReference`` of several files)
  and niworkflows' ``init_enhance_and_skullstrip_bold_wf``, which larmorx has not ported yet:
  it raises ``NotImplementedError`` before any work. sdcflows never chooses it (its
  ``get_workflow`` estimates every PEPOLAR field map with ``topup``, and fMRIPrep's image has no
  ``3dQwarp``).

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows' result):

- SDC-24: ``_qwarp2ants`` squeezes every axis of length one before it adds the vector axis, so
  the warp of a one-slice (or one-row) image loses that spatial axis and is written as a 4-D
  image ANTs does not read as a displacement field; larmorx drops only the axes after the
  third (rule A).

Documented, not run (the workflow waits for its programs): SDC-23, ``init_3dQwarp_wf``
connects ``inputnode.in_reference`` to the reference's resampling, a field its ``inputnode``
does not have (``in_data``, ``metadata``), so its ``fmap_ref`` can never be computed; when the
workflow is ported, a clear error (rule B).
"""

from __future__ import annotations

import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import header_for_image, load_nibabel
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = [
    "QWARP_ARGS",
    "FlattenResult",
    "flatten",
    "qwarp2ants",
    "qwarp_workflow",
    "sorted_pe",
]

#: ``3dQwarp``'s options by the phase-encoding axis (``_sorted_pe``).
QWARP_ARGS = {"i": "-noYdis -noZdis", "j": "-noXdis -noZdis", "k": "-noXdis -noYdis"}
_INTENT_VECTOR = 1007

PathLike = str | os.PathLike[str]


def _multi(value: Any) -> list[Any]:
    """nipype's ``InputMultiObject``: one item becomes a one-item list."""
    if isinstance(value, (str, os.PathLike, Mapping)):
        return [value]
    return list(value)


def _existing(path: Any, trait: str) -> str:
    """nipype's ``File(exists=True)``."""
    p = os.fspath(path)
    if not os.path.isfile(p):
        raise FileNotFoundError(
            f"{trait}: the path {p!r} does not exist (nipype's File(exists=True))"
        )
    return p


@dataclass(frozen=True)
class FlattenResult:
    """``Flatten``'s outputs: ``(file, metadata)`` pairs (``out_list``) and the same unzipped
    (``out_data``, ``out_meta``)."""

    out_list: tuple[tuple[str, Mapping[str, Any]], ...]
    out_data: tuple[str, ...]
    out_meta: tuple[Mapping[str, Any], ...]


def flatten(
    in_data: PathLike | Sequence[PathLike],
    in_meta: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    max_trs: int = 50,
    *,
    out_dir: PathLike | None = None,
    n_threads: int = 1,
) -> FlattenResult:
    """sdcflows' ``Flatten``: each 3-D file kept with its metadata; each 4-D file's first
    ``max_trs`` volumes (nibabel's ``img.slicer[..., :max_trs]`` and ``four_to_three``) written
    as ``<stem>_idx-NNN.nii.gz`` in ``out_dir`` (absolute; the current folder without it, as
    ``_flatten``) with that file's metadata. Files and metadata are paired in order, the longer
    list cut (``zip``); nothing to pair is a ``ValueError`` (sdcflows' unpacking)."""
    from larmorx.niworkflows.images import _constructed, four_to_three

    files = [_existing(f, "in_data") for f in _multi(in_data)]
    metas = _multi(in_meta)
    for m in metas:
        if not isinstance(m, Mapping) or not all(isinstance(k, str) for k in m):
            raise TypeError("in_meta: every item must be a dictionary with string keys")
    if int(max_trs) != max_trs or isinstance(max_trs, bool):
        raise TypeError(f"max_trs must be an integer, not {max_trs!r}")
    base = Path(out_dir) if out_dir is not None else Path()
    output: list[tuple[str, Mapping[str, Any]]] = []
    for path, meta in zip(files, metas, strict=False):
        img = load_nibabel(path, n_threads=n_threads)
        if len(img.shape) == 3:
            output.append((path, meta))
            continue
        data = np.asanyarray(img.data)
        sliced = data[:, :, :, : int(max_trs)]
        if any(d == 0 for d in sliced.shape):
            raise IndexError("Empty slice requested")  # nibabel's slicer
        stem = Path(path).name.rpartition(".nii")[0]
        for j, nii in enumerate(four_to_three(_constructed(img, sliced.copy()))):
            out_name = (base / f"{stem}_idx-{j:03}.nii.gz").absolute()
            out_name.parent.mkdir(parents=True, exist_ok=True)
            save_nibabel(nii, out_name, n_threads=n_threads)
            output.append((str(out_name), meta))
    if not output:
        raise ValueError("not enough values to unpack (expected 2, got 0)")
    out_data, out_meta = zip(*output, strict=False)
    return FlattenResult(tuple(output), tuple(out_data), tuple(out_meta))


def sorted_pe(
    inlist: Sequence[tuple[PathLike, Mapping[str, Any]]],
) -> tuple[list[list[str]], str]:
    """sdcflows' ``_sorted_pe``: the files whose ``PhaseEncodingDirection`` is the first one's
    and those of the opposite direction (``[same, opposite]``, in order), with ``3dQwarp``'s
    options for that axis (:data:`QWARP_ARGS`). Another axis: ``ValueError('Cannot handle
    orthogonal PE encodings.')``."""
    out_ref = [inlist[0][0]]
    out_opp = []
    ref_pe = inlist[0][1]["PhaseEncodingDirection"]
    for d, m in inlist[1:]:
        pe = m["PhaseEncodingDirection"]
        if pe == ref_pe:
            out_ref.append(d)
        elif pe[0] == ref_pe[0]:
            out_opp.append(d)
        else:
            raise ValueError("Cannot handle orthogonal PE encodings.")
    return [out_ref, out_opp], QWARP_ARGS[ref_pe[0]]  # type: ignore[return-value]


def qwarp2ants(
    in_file: PathLike,
    *,
    out_dir: PathLike | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> str:
    """sdcflows' ``ConvertWarp`` (``_qwarp2ants``): the displacement field ``3dQwarp`` writes,
    as float32 with the intent 'vector' (no parameters, no name) and its three components on
    the fifth axis (``X x Y x Z x 1 x 3``), the input's header otherwise, written as
    ``_warpfield<in>`` in ``out_dir`` (the input's folder without it; sdcflows gives
    ``'_warpfield'`` as nipype's ``fname_presuffix``'s prefix: the name is kept). SDC-24
    (module docstring)."""
    from larmorx.sdcflows.approx import _split_filename

    _defects.check(upstream_defects)
    in_file = _existing(in_file, "in_file")
    img = load_nibabel(in_file, dtype=np.float32, n_threads=n_threads)
    data = np.asarray(img.data, dtype=np.float32)
    if upstream_defects == "reproduce" or data.ndim <= 3:
        squeezed = np.squeeze(data)
    else:
        squeezed = data.reshape(data.shape[:3] + tuple(n for n in data.shape[3:] if n != 1))
        if np.squeeze(data).shape != squeezed.shape:
            _defects.warn(
                [
                    (
                        "SDC-24",
                        "sdcflows ConvertWarp",
                        upstream_defects,
                        f"the warp's spatial axes {data.shape[:3]} keep their length-one "
                        "axes (sdcflows drops them)",
                    )
                ],
                stacklevel=2,
            )
    out = squeezed[..., np.newaxis, :]
    fields = img.header.to_dict()
    fields.update(
        intent_code=_INTENT_VECTOR,
        intent_p1=0.0,
        intent_p2=0.0,
        intent_p3=0.0,
        intent_name=b"",
    )
    affine = np.ascontiguousarray(img.affine, dtype=np.float64)
    hdr = header_for_image(list(out.shape), "float32", affine, fields, None)
    newpath = out_dir if out_dir is not None else os.path.dirname(os.path.abspath(in_file))
    # nipype's fname_presuffix(in_file, '_warpfield', newpath=...): its second argument is the
    # prefix
    _, stem, ext = _split_filename(in_file)
    out_file = os.path.join(os.path.abspath(os.fspath(newpath)), "_warpfield" + stem + ext)
    os.makedirs(newpath, exist_ok=True)
    save_nibabel(Image(out, affine, NiftiHeader.from_dict(hdr)), out_file, n_threads=n_threads)
    return out_file


def qwarp_workflow(
    in_data: Any = None,
    metadata: Any = None,
    *,
    omp_nthreads: int = 1,
    debug: bool = False,
    name: str = "pepolar_estimate_wf",
    out_dir: PathLike | None = None,
    n_threads: int = 1,
) -> Any:
    """sdcflows' ``init_3dQwarp_wf``: not available yet (``NotImplementedError``, before any
    work). Its steps: :func:`flatten` of the EPI files, :func:`sorted_pe`, a template of each
    direction (FreeSurfer's ``mri_robust_template``), niworkflows'
    ``init_enhance_and_skullstrip_bold_wf`` on both, their rigid alignment (ANTs'
    ``antsRegistration`` with sdcflows' ``translation_rigid.json``), AFNI's ``3dQwarp -plusminus``
    (``blur -1 -1``, ``minpatch 9``, ``nopadWARP``, ``noweight``, ``pblur 0.05 0.05``, at most
    four threads), the warp's header from the first EPI (``CopyHeader``), :func:`qwarp2ants`
    and the reference unwarped (``antsApplyTransforms``, ``LanczosWindowedSinc``), whose input
    does not exist in sdcflows (SDC-23)."""
    del in_data, metadata, omp_nthreads, debug, name, out_dir, n_threads
    raise NotImplementedError(
        "sdcflows' init_3dQwarp_wf needs AFNI's 3dQwarp (QwarpPlusMinus), FreeSurfer's "
        "mri_robust_template (StructuralReference of several files) and niworkflows' "
        "init_enhance_and_skullstrip_bold_wf, which larmorx has not ported yet; its Python "
        "steps are larmorx.sdcflows.flatten, sorted_pe and qwarp2ants (sdcflows itself "
        "estimates every PEPOLAR field map with topup: larmorx.sdcflows.topup_workflow)"
    )
