# SPDX-License-Identifier: Apache-2.0
"""``bet`` and ``betsurf``: FSL's brain extraction and its skull and scalp surfaces (FSL
6.0.7.23's ``bet``, ``bet2`` and ``betsurf``). ``bet`` has two implementations: the bit-exact
replica from ``larmorx-nc``, run as a separate program (``docs/licensing.md``), and the
clean-room original of this package (``crates/larmorx-fsl``, written from ``specs/bet.md``),
run in this process; ``implementation="auto"`` runs the replica when it is installed, else the
original. ``betsurf``'s clean-room original is still to be written: it needs the replica.

``bet`` deforms a tessellated sphere onto the brain's surface (S. M. Smith, Human Brain Mapping
17(3), 2002) and returns the brain, its mask, the surface's outline and mesh, and an approximate
skull, as ``bet2`` writes them, or one of the ``bet`` script's variations (``-R``, ``-F``,
``-Z``). ``betsurf`` finds the inner and outer skull and the scalp from the brain's mesh, the T1
(and a T2 registered to it) and the T1's registration to MNI152.

FSL's deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``);
``upstream_defects="reproduce"`` gives FSL's output. FSL's environment variables are settings of
the call, never read from the process environment (CLAUDE.md rule 6): the outputs are NIfTI-1
files read back here, and FSL's programs are never looked for on the ``PATH``."""

from __future__ import annotations

import os
import tempfile
import warnings
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

__all__ = ["VARIATIONS", "BetResult", "BetsurfResult", "Surface", "bet", "betsurf", "read_vtk"]

#: ``bet``'s variations: ``"robust"`` (``-R``: bet2 run again from the brain's centre until it
#: moves less than a voxel), ``"small_fov"`` (``-Z``: the image padded to 100 mm in z first),
#: ``"fmri"`` (``-F``: the mask of the first volume, dilated, applied to every volume), and the
#: ones that need FSL programs larmorx does not replicate yet: ``"eyes"`` (``-S``),
#: ``"bias"`` (``-B``), ``"surfaces"`` (``-A``), ``"surfaces_t2"`` (``-A2``).
VARIATIONS = ("robust", "small_fov", "fmri", "eyes", "bias", "surfaces", "surfaces_t2")

Variation = Literal["robust", "small_fov", "fmri", "eyes", "bias", "surfaces", "surfaces_t2"]

_FLAGS = {"robust": "-R", "small_fov": "-Z", "fmri": "-F"}
_PENDING = {
    "eyes": ("-S", "standard_space_roi, FLIRT and fslmaths' morphology"),
    "bias": ("-B", "standard_space_roi and FAST"),
    "surfaces": ("-A", "FLIRT (lx.fsl.betsurf runs the surfaces from a matrix you give it)"),
    "surfaces_t2": ("-A2", "FLIRT (lx.fsl.betsurf runs the surfaces from a matrix you give it)"),
}


@dataclass(frozen=True)
class Surface:
    """A triangle mesh as FSL's programs write it (VTK polydata).

    Attributes
    ----------
    vertices
        ``(n, 3)`` float64: FSL's coordinates, millimetres along the voxel axes of the image as
        FSL stores it (voxel index times voxel size, the x axis in radiological order), not world
        coordinates.
    triangles
        ``(m, 3)`` int64: each triangle's vertex indices.
    """

    vertices: np.ndarray
    triangles: np.ndarray


@dataclass(frozen=True)
class BetResult:
    """What :func:`bet` computed (``None`` for what was not asked for).

    Attributes
    ----------
    brain
        The input with every voxel outside the brain 0 (float32; 4D with ``"fmri"``).
    mask
        The brain mask (int16, 1 inside).
    outline
        The surface drawn on the input at its maximum (float32; ``-o``).
    skull
        An approximate skull (int16; ``-s``).
    mesh
        The brain's surface (``-e``).
    """

    brain: Image | None = None
    mask: Image | None = None
    outline: Image | None = None
    skull: Image | None = None
    mesh: Surface | None = None


@dataclass(frozen=True)
class BetsurfResult:
    """What :func:`betsurf` computed.

    Attributes
    ----------
    inskull, outskull, outskin
        The inner skull, outer skull and scalp surfaces.
    inskull_mask, outskull_mask, outskin_mask
        Their masks (int16, 1 inside), with ``masks=True``.
    skull_mask
        The skull between the two skull surfaces (int16), with ``skull_mask=True``.
    inskull_outline, outskull_outline, outskin_outline
        The surfaces drawn as 1 on the T1's grid (int16), with ``outlines=True``.
    """

    inskull: Surface
    outskull: Surface
    outskin: Surface
    inskull_mask: Image | None = None
    outskull_mask: Image | None = None
    outskin_mask: Image | None = None
    skull_mask: Image | None = None
    inskull_outline: Image | None = None
    outskull_outline: Image | None = None
    outskin_outline: Image | None = None


def read_vtk(path: str | os.PathLike[str]) -> Surface:
    """A mesh from a VTK polydata file as FSL writes it (``POINTS``, then ``POLYGONS`` of
    triangles)."""
    with open(path, encoding="utf-8") as f:
        words = f.read().split()
    try:
        k = words.index("POINTS")
        n = int(words[k + 1])
        start = k + 3
        vertices = np.array(words[start : start + 3 * n], dtype=np.float64).reshape(n, 3)
        k = words.index("POLYGONS", start + 3 * n)
        m = int(words[k + 1])
        faces = np.array(words[k + 3 : k + 3 + 4 * m], dtype=np.int64).reshape(m, 4)
    except (ValueError, IndexError) as e:
        raise ValueError(f"{os.fspath(path)}: not a VTK triangle mesh as FSL writes it") from e
    if not (faces[:, 0] == 3).all():
        raise ValueError(f"{os.fspath(path)}: the mesh has faces that are not triangles")
    vertices.setflags(write=False)
    triangles = faces[:, 1:].copy()
    triangles.setflags(write=False)
    return Surface(vertices, triangles)


def _write_vtk(surface: Surface, path: str) -> None:
    v = np.asarray(surface.vertices, dtype=np.float64)
    t = np.asarray(surface.triangles, dtype=np.int64)
    if v.ndim != 2 or v.shape[1] != 3 or t.ndim != 2 or t.shape[1] != 3:
        raise ValueError("a Surface has (n, 3) vertices and (m, 3) triangles")
    if t.size and (t.min() < 0 or t.max() >= len(v)):
        raise ValueError("a triangle names a vertex the surface does not have")
    lines = ["# vtk DataFile Version 3.0", "surface file", "ASCII", "DATASET POLYDATA"]
    lines.append(f"POINTS {len(v)}  float")
    lines += [" ".join(repr(float(c)) for c in row) for row in v]
    lines.append(f"POLYGONS {len(t)} {4 * len(t)}")
    lines += [f"3 {a} {b} {c} " for a, b, c in t]
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def _number(name: str, value: Any) -> str:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(f"{name} must be a number, not {value!r}")
    return repr(float(value))


def _threads(n_threads: int) -> int:
    n = int(n_threads)
    if n < 0:
        raise ValueError(f"n_threads must be 0 (all) or positive, not {n_threads}")
    return n


def _write_image(image: Any, tmp: str, name: str, n_threads: int) -> str:
    """The input as the program reads it: an absolute path for a file, else the in-memory image
    written to ``tmp`` (its data type kept when FSL stores it as such, else float32)."""
    if isinstance(image, (str, os.PathLike)):
        return os.path.abspath(os.fspath(image))
    img = as_image(image)
    array = np.asarray(img.data)
    if array.ndim not in (2, 3, 4):
        raise ValueError(f"the image must be 2D, 3D or 4D, not {array.ndim}D")
    if array.ndim == 2:
        array = array[:, :, np.newaxis]
    kept = (np.uint8, np.int16, np.int32, np.float32, np.float64)
    if array.dtype not in kept or not array.dtype.isnative:
        array = array.astype(np.float32)
    template = None if img.header is None else img.header.to_dict()
    fields = _core.nifti_header_for_image(
        list(array.shape), array.dtype.name, img.affine, template, None
    )
    fields.update(scl_slope=1.0, scl_inter=0.0)
    path = os.path.join(tmp, f"{name}.nii")
    _core.nifti_write(path, array, fields, 0, n_threads)
    return path


def _read_image(path: str, n_threads: int, impl: _replica.Implementation) -> Image:
    data = _core.nifti_read(path, "raw", n_threads)[0]
    header = NiftiHeader.from_dict(_core.nifti_read_header(path, False))
    scaling = header.slope_inter
    if scaling is not None and scaling != (1.0, 0.0):
        slope, inter = scaling
        data = data.astype(np.float64) * slope + inter
    return Image(data, header.affine, header, implementation=impl)


def _is_fsl_variable(name: str) -> bool:
    return name.upper().startswith("FSL")


def _run(
    selected: _replica.Selected,
    args: Sequence[str],
    tmp: str,
    *,
    n_threads: int,
    upstream_defects: str,
) -> None:
    """Runs the replica in ``tmp`` with FSL's settings of this call (NIfTI-1 outputs; no FSL
    program on the ``PATH``); its notices and FSL's warnings become Python warnings."""
    process = _replica.run(
        selected,
        list(args),
        cwd=tmp,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp},
        exclude=_is_fsl_variable,
    )
    for line in process.stderr.splitlines():
        notice = _defects.parse_line(line)
        if notice is not None:
            warnings.warn(notice, stacklevel=4)
        elif line.startswith("Warning: "):
            warnings.warn(line.removeprefix("Warning: "), stacklevel=4)


def _run_original(
    tool: str,
    args: Sequence[str],
    tmp: str,
    *,
    n_threads: int,
    upstream_defects: str,
) -> None:
    """Runs the clean-room original of ``fsl tool`` in this process, in ``tmp``, with FSL's
    settings of this call (NIfTI-1 outputs; no FSL program on the ``PATH``); its notices and
    FSL's warnings become Python warnings; a failure raises :class:`RuntimeError`."""
    env = [
        ("FSLOUTPUTTYPE", "NIFTI"),
        ("PATH", tmp),
        (_defects.ENV, upstream_defects),
    ]
    if n_threads > 0:
        env.append(("OMP_NUM_THREADS", str(n_threads)))
    code, _out, err = _core.fsl_run_original(tool, list(args), env, tmp)
    for line in err.splitlines():
        notice = _defects.parse_line(line)
        if notice is not None:
            warnings.warn(notice, stacklevel=4)
        elif line.startswith("Warning: "):
            warnings.warn(line.removeprefix("Warning: "), stacklevel=4)
    if code != 0:
        tail = "\n".join(err.strip().splitlines()[-20:])
        raise RuntimeError(
            f"larmorx fsl {tool} (the clean-room original) failed with exit code {code}:\n{tail}"
        )


def bet(
    image: Any,
    *,
    frac: float = 0.5,
    gradient: float = 0.0,
    radius: float | None = None,
    centre: Sequence[float] | None = None,
    smoothness: float = 1.0,
    threshold: bool = False,
    head_ct: bool = False,
    variation: Variation | None = None,
    brain: bool = True,
    mask: bool = False,
    outline: bool = False,
    skull: bool = False,
    mesh: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> BetResult:
    """The brain, as FSL's ``bet2`` (or, with ``variation``, the ``bet`` script) extracts it.

    Parameters
    ----------
    image
        A path to an image FSL reads (NIfTI-1 or -2, ANALYZE, gzipped or not) or an in-memory
        image (:class:`~larmorx.Image`, nibabel image, ``(array, affine)``): 3D (a 4D image's
        first volume is used, with a warning), or 4D with ``variation="fmri"``.
    frac
        The fractional intensity threshold, 0 to 1 (``-f``): smaller values give larger brain
        outlines.
    gradient
        The vertical gradient of the threshold, -1 to 1 (``-g``): positive values give a larger
        outline at the bottom, smaller at the top.
    radius
        The head's radius in mm (``-r``; the initial sphere is half of it); by default estimated.
    centre
        The initial surface's centre in voxels (``-c x y z``); by default the centre of gravity.
    smoothness
        The smoothness factor (``-w``; bet2 only): values below 1 give a more detailed surface.
    threshold
        Also apply thresholding to the brain and mask (``-t``).
    head_ct
        The input is a CT in Hounsfield units (``--ct``; bet2 only).
    variation
        One of :data:`VARIATIONS`, or ``None`` for bet2 itself. ``"eyes"``, ``"bias"``,
        ``"surfaces"`` and ``"surfaces_t2"`` run FSL programs larmorx does not replicate yet:
        :class:`NotImplementedError`.
    brain, mask, outline, skull, mesh
        Which outputs to return: the brain (unless ``brain=False``, ``-n``), its mask (``-m``),
        the outline (``-o``), the skull (``-s``), the mesh (``-e``). ``"fmri"`` returns the brain
        and the mask, and no others; ``"robust"`` always computes the brain.
    n_threads
        Threads (0 = all); the result does not depend on it.
    implementation
        ``"auto"`` (default): the replica in ``larmorx-nc`` when it is installed, else the
        clean-room original; ``"replica"``: the replica
        (:class:`~larmorx.ReplicaNotFoundError` without it); ``"original"``: the clean-room
        original (FSL's behaviour within the thresholds of its validation record,
        ``docs/validation/fsl-bet.md``).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"`` (FSL's output); a
        :class:`~larmorx.UpstreamDefectWarning` names each defect that changed the result.

    Returns
    -------
    BetResult
        Each image's :attr:`~larmorx.Image.implementation` names the implementation that ran.

    Raises
    ------
    ValueError
        Invalid arguments.
    larmorx.ReplicaError
        The replica failed (its message says why: FSL's own errors, and the cases where FSL's
        result is undefined).
    RuntimeError
        The clean-room original failed (its message says why, as the replica's does).
    """
    _defects.check(upstream_defects)
    n_threads = _threads(n_threads)
    if variation is not None and variation not in VARIATIONS:
        raise ValueError(f"variation must be one of {VARIATIONS} or None, not {variation!r}")
    if variation in _PENDING:
        flag, need = _PENDING[variation]
        raise NotImplementedError(
            f"bet's variation {variation!r} ({flag}) runs FSL's {need}, which larmorx does not "
            f"replicate yet"
        )
    f, g = float(frac), float(gradient)
    if f < 0.0 or f > 1.0:
        raise ValueError(f"frac must be between 0 and 1, not {frac!r}")
    if g < -1.0 or g > 1.0:
        raise ValueError(f"gradient must be between -1 and 1, not {gradient!r}")
    options: list[str] = []
    if outline:
        options.append("-o")
    if mask:
        options.append("-m")
    if skull:
        options.append("-s")
    if not brain:
        options.append("-n")
    if threshold:
        options.append("-t")
    if mesh:
        options.append("-e")
    if frac != 0.5:
        options += ["-f", _number("frac", frac)]
    if gradient != 0.0:
        options += ["-g", _number("gradient", gradient)]
    if radius is not None:
        options += ["-r", _number("radius", radius)]
    if centre is not None:
        c = list(centre)
        if len(c) != 3:
            raise ValueError(f"centre must be 3 numbers (voxels), not {centre!r}")
        options += ["-c", *(_number("centre", v) for v in c)]
    if variation is None:
        tool = "bet2"
        if smoothness != 1.0:
            options += ["-w", _number("smoothness", smoothness)]
        if head_ct:
            options.append("--ct")
    else:
        tool = "bet"
        if smoothness != 1.0 or head_ct:
            raise ValueError(
                "smoothness and head_ct are bet2's options, which bet's variations do not pass on"
            )
        if variation == "fmri" and (outline or skull or mesh):
            raise ValueError(
                "variation='fmri' returns the brain and the mask only (bet -F writes the other "
                "outputs to temporary files it removes)"
            )
        options.append(_FLAGS[variation])
    selected = _replica.select("fsl", tool, implementation)
    impl = _replica.ORIGINAL if selected is None else selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-bet-", ignore_cleanup_errors=True) as tmp:
        source = _write_image(image, tmp, "input", n_threads)
        if selected is None:
            _run_original(
                tool,
                [source, "out", *options],
                tmp,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
            )
        else:
            _run(
                selected,
                [source, "out", *options],
                tmp,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
            )

        def read(name: str) -> Image:
            return _read_image(os.path.join(tmp, f"out{name}.nii"), n_threads, impl)

        fmri = variation == "fmri"
        return BetResult(
            brain=read("") if brain else None,
            mask=read("_mask") if mask or fmri else None,
            outline=read("_overlay") if outline else None,
            skull=read("_skull") if skull else None,
            mesh=read_vtk(os.path.join(tmp, "out_mesh.vtk")) if mesh else None,
        )


def betsurf(
    t1: Any,
    brain_surface: Surface | str | os.PathLike[str],
    t1_to_standard: Any,
    *,
    t2: Any = None,
    masks: bool = True,
    skull_mask: bool = False,
    outlines: bool = False,
    precision: int = 0,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> BetsurfResult:
    """The inner skull, outer skull and scalp surfaces, as FSL's ``betsurf`` finds them.

    Parameters
    ----------
    t1
        The T1-weighted image (a path or an in-memory image), not brain-extracted, on the grid
        ``brain_surface`` was found on.
    brain_surface
        The brain's surface from :func:`bet` (``mesh=True``), or a VTK file of it.
    t1_to_standard
        The T1's registration to MNI152 (FLIRT's convention, as ``bet -A`` computes it with
        ``flirt -ref MNI152_T1_2mm``): a path to a 4 x 4 text matrix, or a 4 x 4 array. It only
        decides which profiles cross the eyes.
    t2
        A T2-weighted image registered to the T1 (on its grid); without it the surfaces come
        from the T1 alone (``--t1only``).
    masks
        Also return the three surfaces' masks (``-m``).
    skull_mask
        Also return the skull's mask (``-s``).
    outlines
        Also return the surfaces drawn on the T1's grid (``-o``).
    precision
        Retessellate the surfaces this many times for 100 more iterations each (``-p``).
    n_threads, implementation, upstream_defects
        As for :func:`bet`.

    Returns
    -------
    BetsurfResult
    """
    _defects.check(upstream_defects)
    n_threads = _threads(n_threads)
    if isinstance(precision, bool) or not isinstance(precision, int) or precision < 0:
        raise ValueError(f"precision must be a non-negative int, not {precision!r}")
    options: list[str] = [] if t2 is not None else ["--t1only"]
    if outlines:
        options.append("-o")
    if masks:
        options.append("-m")
    if skull_mask:
        options.append("-s")
    if precision:
        options += ["-p", str(precision)]
    selected = _replica.select("fsl", "betsurf", implementation)
    if selected is None:  # pragma: no cover - there is no original yet
        raise NotImplementedError("larmorx has no clean-room original of fsl betsurf yet")
    impl = selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-betsurf-", ignore_cleanup_errors=True) as tmp:
        images = [_write_image(t1, tmp, "t1", n_threads)]
        if t2 is not None:
            images.append(_write_image(t2, tmp, "t2", n_threads))
        if isinstance(brain_surface, Surface):
            mesh_path = os.path.join(tmp, "brain_mesh.vtk")
            _write_vtk(brain_surface, mesh_path)
        else:
            mesh_path = os.path.abspath(os.fspath(brain_surface))
        if isinstance(t1_to_standard, (str, os.PathLike)):
            mat_path = os.path.abspath(os.fspath(t1_to_standard))
        else:
            m = np.asarray(t1_to_standard, dtype=np.float64)
            if m.shape != (4, 4) or not np.isfinite(m).all():
                raise ValueError("t1_to_standard must be a finite 4 x 4 matrix or a path")
            mat_path = os.path.join(tmp, "t1_to_std.mat")
            with open(mat_path, "w", encoding="utf-8", newline="\n") as f:
                f.writelines(" ".join(repr(float(v)) for v in row) + "\n" for row in m)
        _run(
            selected,
            [*options, *images, mesh_path, mat_path, "out"],
            tmp,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )

        def read(name: str) -> Image:
            return _read_image(os.path.join(tmp, f"out_{name}.nii"), n_threads, impl)

        def surface(name: str) -> Surface:
            return read_vtk(os.path.join(tmp, f"out_{name}_mesh.vtk"))

        return BetsurfResult(
            inskull=surface("inskull"),
            outskull=surface("outskull"),
            outskin=surface("outskin"),
            inskull_mask=read("inskull_mask") if masks else None,
            outskull_mask=read("outskull_mask") if masks else None,
            outskin_mask=read("outskin_mask") if masks else None,
            skull_mask=read("skull_mask") if skull_mask else None,
            inskull_outline=read("inskull_mesh") if outlines else None,
            outskull_outline=read("outskull_mesh") if outlines else None,
            outskin_outline=read("outskin_mesh") if outlines else None,
        )
