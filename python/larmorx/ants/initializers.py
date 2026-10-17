# SPDX-License-Identifier: Apache-2.0
"""Initial linear transforms: ``antsAI`` (:func:`ai`) and ``antsAffineInitializer``
(:func:`affine_initializer`).

Each function builds its program's command line from typed arguments and runs exactly the
command-line code (``larmorx ants antsAI``, ``larmorx ants antsAffineInitializer``)
in-process: in-memory images are passed without files, the transform is written to a
temporary directory and read back as an :class:`~larmorx.transforms.ItkTransform`. The
option-mapping tables are in ``docs/api/ants-ai.md`` and ``docs/api/ants-affine-initializer.md``.
"""

from __future__ import annotations

import os
import re
import tempfile
import warnings
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, parse_line
from larmorx.ants._filters import memory_image
from larmorx.ants.registration import _Args, _num
from larmorx.transforms.itk import ItkTransform, read

__all__ = [
    "AIError",
    "AIResult",
    "AffineInitializerError",
    "affine_initializer",
    "ai",
]

AIMetric = Literal["Mattes", "GC", "MI"]
AITransform = Literal[
    "Rigid", "Affine", "Similarity", "AlignGeometricCenters", "AlignCentersOfMass"
]
Sampling = Literal["None", "Regular", "Random"]


class AIError(RuntimeError):
    """``antsAI`` failed; the message is what the program printed."""


class AffineInitializerError(RuntimeError):
    """``antsAffineInitializer`` failed; the message is what the program printed."""


@dataclass(frozen=True)
class AIResult:
    """What :func:`ai` returns.

    - ``transform``: the transform ``antsAI`` writes (``Euler2DTransform`` /
      ``Euler3DTransform`` for ``Rigid`` and the two alignments, ``AffineTransform``,
      ``Similarity2DTransform`` / ``Similarity3DTransform``), mapping fixed-space points to the
      moving image, as ``antsApplyTransforms -t`` takes it.
    - ``start_points``: the number of start points of the search (``None`` without one).
    - ``values``: the metric values the search printed (every tenth start point, or each
      when there are at most 10), as ``(start point, value)``.
    - ``best_value``: the best start point's metric value (``None`` without a search).
    - ``log``: what the program printed on standard output (with ``--verbose 1``).
    """

    transform: ItkTransform
    start_points: int | None
    values: list[tuple[int, float]]
    best_value: float | None
    log: str


def _dimension(image: Any) -> int:
    if isinstance(image, (str, os.PathLike)):
        import nibabel as nib

        ndim = len(nib.load(os.fspath(image)).shape)
    else:
        ndim = np.asarray(memory_image(image)[0]).ndim
    return min(max(ndim, 2), 4)


def _messages(err: str) -> list[str]:
    """The lines of standard error that are not upstream-defect notices (which become
    :class:`~larmorx.UpstreamDefectWarning` warnings)."""
    messages = []
    for line in err.splitlines():
        warning = parse_line(line)
        if warning is not None:
            warnings.warn(warning, stacklevel=3)
        else:
            messages.append(line)
    return messages


_START = re.compile(r"^Starting optimizer with (\d+) starting points$")
_VALUE = re.compile(r"^(\d+)/\d+,(\S+)$")
_BEST = re.compile(r"^Optimization complete\. Best metric value: (\S+)$")


def ai(
    fixed: Any,
    moving: Any,
    *,
    transform: AITransform = "Rigid",
    gradient_step: float = 0.1,
    metric: AIMetric = "Mattes",
    bins: int = 32,
    sampling: Sampling = "None",
    sampling_percentage: float = 1.0,
    align_principal_axes: bool = False,
    search_factor: float = 20.0,
    arc_fraction: float = 1.0,
    translation_search_grid: tuple[float, Sequence[float]] | None = None,
    iterations: int = 0,
    convergence_threshold: float = 1e-6,
    convergence_window: int = 5,
    fixed_mask: Any = None,
    moving_mask: Any = None,
    dimension: int | None = None,
    random_seed: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> AIResult:
    """The linear transform that best aligns ``moving`` to ``fixed``, as ``antsAI`` (ANTs
    v2.6.5) finds it.

    - ``transform``: ``"Rigid"``, ``"Affine"`` or ``"Similarity"`` (with ``gradient_step``,
      the conjugate-gradient step), or ``"AlignGeometricCenters"`` / ``"AlignCentersOfMass"``
      (a translation, written as a rigid transform).
    - ``metric``: ``"Mattes"`` (with ``bins``), ``"GC"`` (global correlation) or ``"MI"``
      (joint-histogram mutual information, with ``bins``); ``sampling`` (``"None"``: every
      voxel, ``"Regular"``, ``"Random"``) and ``sampling_percentage`` pick the fixed image's
      points.
    - ``align_principal_axes``: start from the rotation between the images' principal axes
      (otherwise from their centres of mass only).
    - ``search_factor`` (degrees) and ``arc_fraction``: the start points' rotations, from
      ``-180°·arc_fraction`` to ``180°·arc_fraction`` in steps of ``search_factor``, about each
      axis (the principal axes with ``align_principal_axes``).
    - ``translation_search_grid``: ``(step, extents)`` in mm, translations of the start
      points (``-g [step,AxBxC]``).
    - ``iterations``, ``convergence_threshold``, ``convergence_window``: the local
      optimization of each start point (``-c``; 0 iterations: the initial alignment only).
    - ``fixed_mask``, ``moving_mask``: masks of the metric (``-x``).
    - ``random_seed``: the seed of ``Regular`` sampling's perturbations (``--random-seed``;
      1234 by default). ANTs draws ``Random`` sampling's voxels, and everything with seed 0,
      from a generator seeded by the clock (its result changes from run to run); larmorx
      seeds it from ``random_seed`` (ANTS-58).

    Paths are read as ANTs reads them (as float images); in-memory images are
    :class:`~larmorx.Image` objects or ``(array, affine)`` pairs. The result does not depend
    on ``n_threads``. ANTs' defects are corrected as on the command line
    (``docs/api/upstream-defects.md``); ``upstream_defects="reproduce"`` gives ANTs' result,
    and an :class:`~larmorx.UpstreamDefectWarning` names each defect that changed it.

    Raises :class:`AIError` with the program's messages when it fails.
    """
    check(upstream_defects)
    with tempfile.TemporaryDirectory(prefix="larmorx-ai-") as tmpdir:
        tmp = Path(tmpdir)
        a = _Args(tmp)
        if dimension is None:
            dimension = _dimension(fixed)
        out_name = os.fspath(tmp / "lx-transform.mat")
        metric_fields = [a.image(fixed), a.image(moving), str(int(bins))]
        metric_fields += [sampling, _num(sampling_percentage)]
        argv = ["-d", str(int(dimension)), "-v", "1"]
        argv += ["-m", f"{metric}[{','.join(metric_fields)}]"]
        if transform in ("AlignGeometricCenters", "AlignCentersOfMass"):
            argv += ["-t", transform]
        else:
            argv += ["-t", f"{transform}[{_num(gradient_step)}]"]
        argv += ["-p", str(int(align_principal_axes))]
        argv += ["-s", f"[{_num(search_factor)},{_num(arc_fraction)}]"]
        if translation_search_grid is not None:
            step, extents = translation_search_grid
            grid = "x".join(_num(v) for v in extents)
            argv += ["-g", f"[{_num(step)},{grid}]"]
        its = f"[{int(iterations)},{_num(convergence_threshold)},{int(convergence_window)}]"
        argv += ["-c", its]
        if fixed_mask is not None or moving_mask is not None:
            if moving_mask is None:
                argv += ["-x", a.image(fixed_mask)]
            else:
                if fixed_mask is None:
                    raise ValueError("a moving mask needs a fixed mask (antsAI's -x)")
                argv += ["-x", f"[{a.image(fixed_mask)},{a.image(moving_mask)}]"]
        if random_seed is not None:
            argv += ["--random-seed", str(int(random_seed))]
        argv += ["-o", out_name]
        code, out, err, _outputs = _core.ants_program(
            "antsAI", argv, a.inputs, [], n_threads, upstream_defects
        )
        messages = _messages(err)
        if code != 0 or not os.path.exists(out_name):
            raise AIError("\n".join(messages).strip() or f"antsAI exited {code}")
        transforms = read(out_name, upstream_defects=upstream_defects)
    if not transforms:
        raise AIError(
            "antsAI wrote no transform (the alignments with iterations, ANTS-53; "
            "upstream_defects='fix' writes the alignment)"
        )
    start_points = None
    values: list[tuple[int, float]] = []
    best = None
    for line in out.splitlines():
        if m := _START.match(line):
            start_points = int(m.group(1))
        elif m := _VALUE.match(line):
            values.append((int(m.group(1)), float(m.group(2))))
        elif m := _BEST.match(line):
            best = float(m.group(1))
    return AIResult(
        transform=transforms[0],
        start_points=start_points,
        values=values,
        best_value=best,
        log=out,
    )


def affine_initializer(
    fixed: Any,
    moving: Any,
    *,
    search_factor: float = 10.0,
    radian_fraction: float = 0.25,
    principal_axes: bool = False,
    local_search_iterations: int = 20,
    fixed_mask: Any = None,
    dimension: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ItkTransform:
    """An affine transform that aligns ``moving`` to ``fixed``, as ``antsAffineInitializer``
    (ANTs v2.6.5) finds it: the images' centres of mass (and, with ``principal_axes``, their
    principal axes) aligned, then the best of a multi-start search over rotations about the
    fixed image's principal axes (2D and 3D; in 4D the alignment alone).

    - ``search_factor``: the rotation step in degrees; ``radian_fraction``: the rotations run
      over ``±π·radian_fraction`` (clamped to ``[0.01, 1]``).
    - ``local_search_iterations``: the command line's eighth argument, which ANTs does not
      read: each start point runs 20 conjugate-gradient iterations whatever it says
      (ANTS-54; a value other than 20 gives an :class:`~larmorx.UpstreamDefectWarning`).
    - ``fixed_mask``: the mask of the metric (Mattes mutual information, 32 bins, every 10th
      voxel of the fixed image).

    Paths are read as ANTs reads them (as float images); in-memory images are
    :class:`~larmorx.Image` objects or ``(array, affine)`` pairs. The result does not depend
    on ``n_threads``.

    Raises :class:`AffineInitializerError` with the program's messages when it fails.
    """
    check(upstream_defects)
    with tempfile.TemporaryDirectory(prefix="larmorx-affine-initializer-") as tmpdir:
        tmp = Path(tmpdir)
        a = _Args(tmp)
        if dimension is None:
            dimension = _dimension(fixed)
        out_name = os.fspath(tmp / "lx-transform.mat")
        argv = [str(int(dimension)), a.image(fixed), a.image(moving), out_name]
        argv += [_num(search_factor), _num(radian_fraction), str(int(principal_axes))]
        argv += [str(int(local_search_iterations))]
        if fixed_mask is not None:
            argv += [a.image(fixed_mask)]
        code, _out, err, _outputs = _core.ants_program(
            "antsAffineInitializer", argv, a.inputs, [], n_threads, upstream_defects
        )
        messages = _messages(err)
        if code != 0 or not os.path.exists(out_name):
            raise AffineInitializerError(
                "\n".join(messages).strip() or f"antsAffineInitializer exited {code}"
            )
        return read(out_name, upstream_defects=upstream_defects)[0]
