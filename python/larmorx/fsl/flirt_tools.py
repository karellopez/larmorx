# SPDX-License-Identifier: Apache-2.0
"""FLIRT's other programs: ``avscale`` (an affine matrix decomposed), ``img2imgcoord``,
``std2imgcoord`` and ``img2stdcoord`` (coordinates mapped between images), ``rmsdiff`` (the
deviation between two matrices) and ``makerot`` (a rotation matrix), FSL 6.0.7.23's.

Each has two ports (``docs/licensing.md``): the bit-exact replica from ``larmorx-nc`` (FSL
Licence), run as a separate program, and the clean room (Apache-2.0, written from
``specs/flirt.md``), run in this process; ``implementation="auto"`` runs the replica when it is
installed, else the clean room. The results are what the programs print, parsed: FSL prints matrices and parameters with 6 decimals, and coordinates with 6
significant digits (with 6 decimals once a matrix was printed in verbose mode), and so do these.
"""

from __future__ import annotations

import os
import tempfile
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _write_image
from larmorx.fsl.flirt import _matrix_file, _run

__all__ = [
    "AvscaleResult",
    "avscale",
    "img2imgcoord",
    "img2stdcoord",
    "makerot",
    "rmsdiff",
    "std2imgcoord",
]


def _image_file(image: Any, tmp: str, name: str) -> str:
    """A path as given (absolute), or an in-memory image (:class:`larmorx.Image` or an array)
    written into ``tmp``."""
    return _write_image(image, tmp, name, 1)


def _coords_file(coords: Any, tmp: str) -> str:
    if isinstance(coords, (str, os.PathLike)):
        return os.path.abspath(os.fspath(coords))
    a = np.atleast_2d(np.asarray(coords, dtype=np.float64))
    if a.ndim != 2 or a.shape[1] != 3:
        raise ValueError(f"coordinates must be N x 3, not of shape {a.shape}")
    path = os.path.join(tmp, "coords.txt")
    with open(path, "w", encoding="utf-8") as f:
        for row in a:
            f.write(" ".join(repr(float(v)) for v in row) + "\n")
    return path


@dataclass(frozen=True)
class AvscaleResult:
    """``avscale``'s decomposition of ``matrix`` (``matrix = rotation * skew * scale``), as it
    prints them (6 decimals, or 6 significant digits where FSL prints those).

    Attributes
    ----------
    rotation_translation
        The rotation-and-translation matrix (4 x 4).
    rotations
        The rotation's angles about x, y and z, in radians (``all_params``; else ``None``).
    translations
        The translation about the centre of gravity of ``volume``, or about the origin without
        one (``all_params``; else ``None``).
    scales
        The scales along x, y and z.
    skews
        The skews.
    average_scale
        The scales' average.
    determinant
        The matrix's determinant.
    left_right_preserved
        Whether the left-right order is kept.
    forward_half
        The forward half transform (the matrix's square root).
    backward_half
        The backward half transform.
    matrix_check
        The check matrix (``inverted_eddies``; else ``None``).
    inverted_eddy
        The inverted eddy-current matrix (``inverted_eddies``; else ``None``).
    stdout
        The program's text.
    """

    rotation_translation: np.ndarray
    rotations: np.ndarray | None
    translations: np.ndarray | None
    scales: np.ndarray
    skews: np.ndarray
    average_scale: float
    determinant: float
    left_right_preserved: bool
    forward_half: np.ndarray
    backward_half: np.ndarray
    matrix_check: np.ndarray | None
    inverted_eddy: np.ndarray | None
    stdout: str


def _after(lines: list[str], head: str, n: int) -> np.ndarray | None:
    for i, line in enumerate(lines):
        if line.startswith(head):
            return np.array([[float(v) for v in lines[i + 1 + k].split()] for k in range(n)])
    return None


def _values(lines: list[str], head: str) -> np.ndarray | None:
    for line in lines:
        if line.startswith(head):
            return np.array([float(v) for v in line[len(head) :].split()])
    return None


def avscale(
    matrix: Any,
    volume: Any = None,
    *,
    all_params: bool = False,
    inverted_eddies: bool = False,
    implementation: ImplementationName = "auto",
) -> AvscaleResult:
    """FSL's ``avscale``: ``matrix`` (4 x 4 array or file) decomposed, its translations about the
    centre of gravity of ``volume`` (a path or :class:`larmorx.Image`; the origin without one).
    ``all_params`` also returns the rotation angles and translations; ``inverted_eddies`` the
    eddy-current inversion (the two options are exclusive, as on the command line).

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    if all_params and inverted_eddies:
        raise ValueError("avscale takes one of all_params and inverted_eddies")
    selected = _replica.select("fsl", "avscale", implementation)
    with tempfile.TemporaryDirectory(prefix="larmorx-avscale-") as tmp:
        args = []
        if all_params:
            args.append("--allparams")
        if inverted_eddies:
            args.append("--inverteddies")
        args.append(_matrix_file(matrix, tmp, "in"))
        if volume is not None:
            args.append(_image_file(volume, tmp, "vol"))
        process = _run(selected, args, tmp, 1, "fix", program="avscale")
    text = process.stdout
    lines = text.splitlines()
    scales = _values(lines, "Scales (x,y,z) = ")
    skews = _values(lines, "Skews (xy,xz,yz) = ")
    avg = _values(lines, "Average scaling = ")
    det = _values(lines, "Determinant = ")
    rt = _after(lines, "Rotation & Translation Matrix:", 4)
    fwd = _after(lines, "Forward half transform =", 4)
    bwd = _after(lines, "Backward half transform =", 4)
    if any(x is None for x in (scales, skews, avg, det, rt, fwd, bwd)):
        raise RuntimeError(f"avscale printed no decomposition:\n{text}")
    return AvscaleResult(
        rotation_translation=rt,
        rotations=_values(lines, "Rotation Angles (x,y,z) [rads] = "),
        translations=_values(lines, "Translations (x,y,z) [mm] = "),
        scales=scales,
        skews=skews,
        average_scale=float(avg[0]),
        determinant=float(det[0]),
        left_right_preserved="Left-Right orientation: preserved" in text,
        forward_half=fwd,
        backward_half=bwd,
        matrix_check=_after(lines, "Matrix check: mat^-1 * rotmat * skew * scale =", 4),
        inverted_eddy=_after(lines, "Inverted eddy matrix:", 4),
        stdout=text,
    )


def _coords(tool: str, args: list[str], implementation: str) -> np.ndarray:
    selected = _replica.select("fsl", tool, implementation)
    with tempfile.TemporaryDirectory(prefix=f"larmorx-{tool}-") as tmp:
        process = _run(selected, args, tmp, 1, "fix", program=tool)
    if process.returncode != 0:
        raise RuntimeError(f"{tool} failed (exit {process.returncode}):\n{process.stderr}")
    rows = []
    for line in process.stdout.splitlines():
        words = line.split()
        if len(words) == 3:
            try:
                rows.append([float(w) for w in words])
            except ValueError:
                continue
    return np.array(rows, dtype=np.float64).reshape(-1, 3)


def img2imgcoord(
    coords: Any,
    src: Any,
    dest: Any,
    *,
    xfm: Any = None,
    mm: bool = False,
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``img2imgcoord``: ``coords`` (N x 3, or a file of rows ``x y z``) in ``src``'s voxels
    (NIfTI's), or millimetres with ``mm``, mapped through ``xfm`` (FLIRT's matrix from ``src`` to
    ``dest``; the identity without one) to ``dest``'s; returned as N x 3.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-coords-") as tmp:
        args = ["-src", _image_file(src, tmp, "src"), "-dest", _image_file(dest, tmp, "dest")]
        if xfm is not None:
            args += ["-xfm", _matrix_file(xfm, tmp, "xfm")]
        args.append("-mm" if mm else "-vox")
        args.append(_coords_file(coords, tmp))
        return _coords("img2imgcoord", args, implementation)


def std2imgcoord(
    coords: Any,
    img: Any,
    std: Any = None,
    *,
    xfm: Any = None,
    vox: bool = False,
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``std2imgcoord``: ``coords`` (N x 3, or a file) in ``std``'s millimetres (its
    sform or qform; without ``std``, the voxel sizes of a 1 mm grid) mapped through the inverse
    of ``xfm`` (FLIRT's matrix from ``img`` to ``std``) to ``img``'s millimetres, or its NIfTI
    voxels with ``vox``; returned as N x 3.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-coords-") as tmp:
        args = ["-img", _image_file(img, tmp, "img")]
        if std is not None:
            args += ["-std", _image_file(std, tmp, "std")]
        if xfm is not None:
            args += ["-xfm", _matrix_file(xfm, tmp, "xfm")]
        args.append("-vox" if vox else "-mm")
        args.append(_coords_file(coords, tmp))
        return _coords("std2imgcoord", args, implementation)


def img2stdcoord(
    coords: Any,
    img: Any,
    std: Any = None,
    *,
    xfm: Any = None,
    mm: bool = False,
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``img2stdcoord``: ``coords`` (N x 3, or a file) in ``img``'s NIfTI voxels, or its
    millimetres with ``mm``, mapped through ``xfm`` (FLIRT's matrix from ``img`` to ``std``) to
    ``std``'s millimetres; returned as N x 3.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-coords-") as tmp:
        args = ["-img", _image_file(img, tmp, "img")]
        if std is not None:
            args += ["-std", _image_file(std, tmp, "std")]
        if xfm is not None:
            args += ["-xfm", _matrix_file(xfm, tmp, "xfm")]
        args.append("-mm" if mm else "-vox")
        args.append(_coords_file(coords, tmp))
        return _coords("img2stdcoord", args, implementation)


def rmsdiff(
    matrix1: Any,
    matrix2: Any,
    ref: Any,
    mask: Any = None,
    *,
    implementation: ImplementationName = "auto",
) -> float | tuple[float, float]:
    """FSL's ``rmsdiff``: the RMS deviation (mm) between two matrices (4 x 4 arrays or files)
    over a sphere of radius 80 mm about ``ref``'s centre of gravity; with ``mask``, the largest
    and the RMS displacement over the mask's voxels, as ``(largest, rms)``.

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    """
    selected = _replica.select("fsl", "rmsdiff", implementation)
    with tempfile.TemporaryDirectory(prefix="larmorx-rmsdiff-") as tmp:
        args = [
            _matrix_file(matrix1, tmp, "m1"),
            _matrix_file(matrix2, tmp, "m2"),
            _image_file(ref, tmp, "ref"),
        ]
        if mask is not None:
            args.append(_image_file(mask, tmp, "mask"))
        process = _run(selected, args, tmp, 1, "fix", program="rmsdiff")
    if process.returncode != 0:
        raise RuntimeError(f"rmsdiff failed (exit {process.returncode}):\n{process.stderr}")
    values = [float(v) for v in process.stdout.split()]
    if mask is None:
        return values[0]
    return values[0], values[1]


def makerot(
    theta: float,
    axis: Any = None,
    centre: Any = None,
    *,
    cov: Any = None,
    upstream_defects: UpstreamDefectsName = "fix",
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``makerot``: the 4 x 4 matrix of a rotation by ``theta`` degrees about ``axis``
    (three values of any length, z by default) through ``centre`` (mm, the origin by default),
    or through the centre of the image ``cov`` (half its extent in mm). The axis is divided by
    its length (FSL divides by the square root of it, FSL-76; ``upstream_defects="reproduce"``
    gives FSL's matrix); an axis or centre of other than three values is an error here (FSL-77
    is about the command line, where one word holds them).

    Parameters
    ----------
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-flirt.md``, "Defects of the original, corrected by default").
    """
    selected = _replica.select("fsl", "makerot", implementation)

    def three(v: Any, what: str) -> str:
        a = np.asarray(v, dtype=np.float64).ravel()
        if a.shape != (3,):
            raise ValueError(f"{what} must hold three values, not {a.size}")
        return ",".join(repr(float(x)) for x in a)

    with tempfile.TemporaryDirectory(prefix="larmorx-makerot-") as tmp:
        args = [f"--theta={float(theta)!r}"]
        if axis is not None:
            args.append("--axis=" + three(axis, "axis"))
        if centre is not None:
            args.append("--centre=" + three(centre, "centre"))
        if cov is not None:
            args.append("--cov=" + _image_file(cov, tmp, "cov"))
        out = os.path.join(tmp, "rot.mat")
        args += ["-o", out]
        p = _run(selected, args, tmp, 1, upstream_defects, program="makerot")
        if p.returncode != 0 or not os.path.isfile(out):
            raise RuntimeError(f"makerot failed (exit {p.returncode}):\n{p.stderr}")
        return np.loadtxt(out)
