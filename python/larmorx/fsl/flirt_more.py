# SPDX-License-Identifier: Apache-2.0
"""FLIRT's programs and scripts added in its part 3: ``midtrans`` (the transform to the middle
of several transforms), ``pointflirt`` (an affine matrix from point correspondences),
``applyxfm4D`` (a series resampled through one matrix or one per volume), and the scripts
``pairreg`` (brain and skull registration), ``extracttxt`` (lines of a text file) and
``aff2rigid`` (a 6-parameter approximation of a standard-space matrix), FSL 6.0.7.23's.

Each has two ports (``docs/licensing.md``): the bit-exact replica from ``larmorx-nc`` (FSL
Licence), run as a separate program, and the clean room (Apache-2.0, written from
``specs/flirt.md``), run in this process; ``implementation="auto"`` runs the replica when it is
installed, else the clean room. A run that fails raises :class:`RuntimeError` with its messages.
"""

from __future__ import annotations

import os
import tempfile
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.flirt import _matrix_file, _run, _warn
from larmorx.fsl.flirt_tools import _image_file

__all__ = [
    "aff2rigid",
    "applyxfm4d",
    "extracttxt",
    "midtrans",
    "pairreg",
    "pointflirt",
]

Interp4D = Literal["sinc", "trilinear", "nearestneighbour", "spline"]

#: The output types by the file name's extension (FSL replaces the extension by its own).
_OUTPUT_TYPES = (
    (".nii.gz", "NIFTI_GZ"),
    (".nii", "NIFTI"),
    (".hdr.gz", "NIFTI_PAIR_GZ"),
    (".img.gz", "NIFTI_PAIR_GZ"),
    (".hdr", "NIFTI_PAIR"),
    (".img", "NIFTI_PAIR"),
)


def _call(tool: str, args: list[str], tmp: str, implementation: str, upstream_defects: str):
    """``tool`` with ``args`` in ``tmp`` (its replica or its clean room, :func:`_run`; ``FSLDIR``
    passed on when set, the clean room's ``pairreg`` carrying FSL's schedules itself); a failure
    (exit status not 0) raises :class:`RuntimeError` with its messages."""
    selected = _replica.select("fsl", tool, implementation)
    extra = (("FSLDIR", os.environ["FSLDIR"]),) if os.environ.get("FSLDIR") else ()
    process = _run(selected, args, tmp, 1, upstream_defects, program=tool, extra_env=extra)
    if process.returncode != 0:
        raise RuntimeError(f"{tool} failed (exit {process.returncode}):\n{process.stderr}")
    return process


def _output_type(path: str) -> tuple[str, str]:
    """The path FSL writes for ``path`` and the ``FSLOUTPUTTYPE`` that writes it (a name without
    a known extension: ``.nii.gz``, FSL's default)."""
    for ext, kind in _OUTPUT_TYPES:
        if path.endswith(ext):
            return path, kind
    return path + ".nii.gz", "NIFTI_GZ"


def _points_file(points: Any, tmp: str, name: str) -> str:
    if isinstance(points, (str, os.PathLike)):
        return os.path.abspath(os.fspath(points))
    a = np.atleast_2d(np.asarray(points, dtype=np.float64))
    if a.ndim != 2 or a.shape[1] != 3:
        raise ValueError(f"{name} must be N x 3, not of shape {a.shape}")
    path = os.path.join(tmp, f"{name}.txt")
    np.savetxt(path, a, fmt="%.17g")
    return path


def midtrans(
    transforms: list[Any],
    *,
    template: Any = None,
    separate: str | None = None,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """FSL's ``midtrans``: the 4 x 4 transform from the template space of ``transforms`` (4 x 4
    arrays or files, at least two) to their "middle" (FSL's Procrustes average about the MNI
    origin, or about the centre of gravity of ``template``). With ``separate`` (a path prefix)
    each transform composed with it is also written to ``<separate>0001.mat`` and so on.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-flirt.md``, "Defects of the original, corrected by default").
    """
    if len(transforms) < 2:
        raise ValueError("midtrans needs at least 2 transforms")
    with tempfile.TemporaryDirectory(prefix="larmorx-midtrans-") as tmp:
        out = os.path.join(tmp, "mid.mat")
        args = ["-o", out]
        if template is not None:
            args += ["--template", _image_file(template, tmp, "template")]
        if separate is not None:
            args += ["--separate", os.path.abspath(separate)]
        args += [_matrix_file(t, tmp, f"t{k}") for k, t in enumerate(transforms)]
        _call("midtrans", args, tmp, implementation, upstream_defects)
        return np.loadtxt(out)


def pointflirt(
    in_points: Any,
    ref_points: Any,
    *,
    in_image: Any = None,
    ref_image: Any = None,
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``pointflirt``: the affine 4 x 4 matrix that maps ``in_points`` onto ``ref_points``
    (N x 3 arrays or text files, in mm; with ``in_image`` and ``ref_image``, in voxels, scaled
    by their voxel sizes: ``--vox``) in the least-squares sense.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-pointflirt-") as tmp:
        out = os.path.join(tmp, "aff.mat")
        args = [
            "-i",
            _points_file(in_points, tmp, "in"),
            "-r",
            _points_file(ref_points, tmp, "ref"),
        ]
        args += ["-o", out]
        if (in_image is None) != (ref_image is None):
            raise ValueError("pointflirt's voxel coordinates need both in_image and ref_image")
        if in_image is not None:
            args += [
                "--vox",
                "--invol=" + _image_file(in_image, tmp, "invol"),
                "--refvol=" + _image_file(ref_image, tmp, "refvol"),
            ]
        _call("pointflirt", args, tmp, implementation, "fix")
        return np.loadtxt(out)


def applyxfm4d(
    in_image: str | os.PathLike,
    ref_image: str | os.PathLike,
    out_image: str | os.PathLike,
    transforms: str | os.PathLike,
    *,
    single_matrix: bool = False,
    four_digit: bool = False,
    user_prefix: str | None = None,
    interp: Interp4D = "sinc",
    three_d: bool = False,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """FSL's ``applyxfm4D``: each volume of ``in_image`` resampled onto ``ref_image``'s grid
    through ``transforms`` (one matrix file with ``single_matrix``, else a directory of
    ``MAT_0000n`` files, ``MAT_000n`` with ``four_digit``, ``<user_prefix>000n`` with
    ``user_prefix``), written to ``out_image`` in the input's data type (its extension chooses
    the format: ``.nii.gz``, ``.nii``, ``.hdr``/``.img``; none: ``.nii.gz``); ``three_d``
    resamples one volume (``-3D``). Returns the path written.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-flirt.md``, "Defects of the original, corrected by default").
    """
    selected = _replica.select("fsl", "applyxfm4D", implementation)
    with tempfile.TemporaryDirectory(prefix="larmorx-applyxfm4d-") as tmp:
        out, kind = _output_type(os.path.abspath(os.fspath(out_image)))
        args = [
            os.path.abspath(os.fspath(in_image)),
            os.path.abspath(os.fspath(ref_image)),
            out,
            os.path.abspath(os.fspath(transforms)),
        ]
        if single_matrix:
            args.append("-singlematrix")
        if four_digit:
            args.append("-fourdigit")
        if user_prefix is not None:
            args += ["-userprefix", user_prefix]
        args += ["-interp", interp]
        if three_d:
            args.append("-3D")
        if selected is None:
            # the clean room, in this process
            env = [("FSLOUTPUTTYPE", kind), ("PATH", tmp), (_defects.ENV, upstream_defects)]
            code, _, err = _core.fsl_run_original("applyxfm4D", args, env, tmp)
            _warn(err)
        else:
            process = _replica.run(
                selected,
                args,
                cwd=tmp,
                n_threads=1,
                upstream_defects=upstream_defects,
                env={"FSLOUTPUTTYPE": kind, "PATH": tmp},
                exclude=lambda name: name.upper().startswith("FSL"),
            )
            _warn(process.stderr)
            code, err = process.returncode, process.stderr
        if code != 0:
            raise RuntimeError(f"applyxfm4D failed (exit {code}):\n{err}")
    return out


def pairreg(
    brain1: str | os.PathLike,
    brain2: str | os.PathLike,
    skull1: str | os.PathLike,
    skull2: str | os.PathLike,
    *,
    flirt_args: list[str] | None = None,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """FSL's ``pairreg``: the matrix from ``brain2`` to ``brain1`` (``brain1`` and ``skull1``
    the references), found with the brains, refined with the skulls and finished with the
    brains (three FLIRT runs with FSL's ``pairreg1.sch`` to ``pairreg3.sch`` schedules, which the
    clean room carries: no ``$FSLDIR`` needed); ``flirt_args`` are passed to every FLIRT run.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-flirt.md``, "Defects of the original, corrected by default").
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-pairreg-") as tmp:
        out = os.path.join(tmp, "pair.mat")
        files = [os.path.abspath(os.fspath(p)) for p in (brain1, brain2, skull1, skull2)]
        args = [*files, out, *(flirt_args or [])]
        process = _call("pairreg", args, tmp, implementation, upstream_defects)
        if not os.path.exists(out):
            raise RuntimeError(f"pairreg wrote no matrix:\n{process.stderr}")
        return np.loadtxt(out)


def extracttxt(
    search_word: str,
    file: str | os.PathLike,
    trailing_lines: int | None = None,
    relative_start: int | None = None,
    *,
    implementation: ImplementationName = "auto",
) -> str:
    """FSL's ``extracttxt``: the lines of ``file`` from the first that matches ``search_word``
    (a basic regular expression, as grep reads it) to the end, or ``trailing_lines`` of them,
    starting ``relative_start`` lines after (or before) the match.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    if relative_start is not None and trailing_lines is None:
        raise ValueError("extracttxt's relative_start needs trailing_lines")
    with tempfile.TemporaryDirectory(prefix="larmorx-extracttxt-") as tmp:
        args = [search_word, os.path.abspath(os.fspath(file))]
        if trailing_lines is not None:
            args.append(str(int(trailing_lines)))
        if relative_start is not None:
            args.append(str(int(relative_start)))
        return _call("extracttxt", args, tmp, implementation, "fix").stdout


def aff2rigid(
    matrix: Any,
    *,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """FSL's ``aff2rigid``: the rigid (6-parameter) 4 x 4 matrix that keeps the anterior
    commissure, the AC-PC line and the mid-sagittal plane of the 12-parameter input-to-standard
    ``matrix`` (array or file). FSL 6.0.7.23's script stops before writing it (FSL-81); the
    replica and the clean room compute it as the script defines it (``upstream_defects="reproduce"``
    fails as FSL).

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-flirt.md``, "Defects of the original, corrected by default").
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-aff2rigid-") as tmp:
        out = os.path.join(tmp, "rigid.mat")
        args = [_matrix_file(matrix, tmp, "in"), out]
        _call("aff2rigid", args, tmp, implementation, upstream_defects)
        return np.loadtxt(out)
