# SPDX-License-Identifier: Apache-2.0
"""``antsRegistration``'s linear stages: :func:`registration`.

The function builds ``antsRegistration``'s command line from typed arguments and runs exactly
the command-line code (``larmorx ants antsRegistration``) in-process: in-memory images are
passed without files, the transforms are written to a temporary directory and read back as
:class:`~larmorx.transforms.ItkTransform` lists, the warped images are returned in memory.
The option-mapping table is in ``docs/api/ants-registration.md``.
"""

from __future__ import annotations

import os
import re
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, parse_line
from larmorx.ants._filters import memory_image
from larmorx.image import Image
from larmorx.transforms.itk import ItkTransform, read, write

__all__ = [
    "AlignCenters",
    "RegistrationError",
    "RegistrationIteration",
    "RegistrationMetric",
    "RegistrationResult",
    "RegistrationStage",
    "registration",
]

MetricType = Literal["Mattes", "MI", "MI2", "MeanSquares", "CC", "GC", "Demons"]
TransformType = Literal["Translation", "Rigid", "Similarity", "Affine", "CompositeAffine"]
Sampling = Literal["None", "Regular", "Random"]
Precision = Literal["float64", "float32"]


class RegistrationError(RuntimeError):
    """``antsRegistration`` failed; the message is what the program printed."""


@dataclass(frozen=True)
class RegistrationMetric:
    """One image metric of a stage (``-m Type[fixed,moving,weight,parameter,sampling,
    percentage,gradientFilter]``).

    - ``type``: ``"Mattes"`` (Mattes mutual information; ANTs' ``MI`` is the same metric),
      ``"MI2"`` (joint-histogram mutual information), ``"MeanSquares"``, ``"CC"``
      (neighbourhood cross-correlation), ``"GC"`` (global correlation) or ``"Demons"`` (which
      needs a displacement field: a linear stage with it fails).
    - ``fixed``, ``moving``: paths (read as ANTs reads them) or in-memory images.
    - ``weight``: the metric's weight in a stage of several metrics.
    - ``bins``: histogram bins of ``Mattes``, ``MI`` and ``MI2``; ``radius``: ``CC``'s window
      radius. The other metrics have no parameter.
    - ``sampling``, ``sampling_percentage``: ``"None"`` (every voxel), ``"Regular"`` or
      ``"Random"``, with the fraction of the virtual domain's voxels.
    - ``use_gradient_filter``: the gradient from a recursive Gaussian derivative image
      instead of central differences.
    """

    type: MetricType
    fixed: Any
    moving: Any
    weight: float = 1.0
    bins: int = 32
    radius: int = 4
    sampling: Sampling = "None"
    sampling_percentage: float = 1.0
    use_gradient_filter: bool = False


@dataclass(frozen=True)
class RegistrationStage:
    """One stage (``-t``, ``-m``, ``-c``, ``-f``, ``-s``, and per stage ``-x`` and ``-g``).

    ``iterations``, ``shrink_factors`` and ``smoothing_sigmas`` have one value per level
    (``-c [10x5,...]``, ``-f 2x1``, ``-s 1x0vox``); ``sigmas_in_mm`` gives the sigmas in
    millimetres (``mm``) instead of voxels (``vox``). ``masks`` is a fixed mask, or a pair
    ``(fixed, moving)`` where ``None`` means no mask. ``restrict_deformation`` weighs each
    parameter of the stage's transform (``-g``).
    """

    transform: TransformType
    gradient_step: float
    metrics: RegistrationMetric | Sequence[RegistrationMetric]
    iterations: Sequence[int]
    shrink_factors: Sequence[int]
    smoothing_sigmas: Sequence[float]
    sigmas_in_mm: bool = False
    convergence_threshold: float = 1e-6
    convergence_window: int = 10
    masks: Any = None
    restrict_deformation: Sequence[float] | None = None


@dataclass(frozen=True)
class AlignCenters:
    """An initial transform from two images (``-r [fixed,moving,feature]``): a translation
    aligning their geometric centres (``"geometry"``, 0), centres of mass (``"moments"``, 1)
    or origins (``"origins"``, 2)."""

    fixed: Any
    moving: Any
    feature: Literal["geometry", "moments", "origins"] = "geometry"


@dataclass(frozen=True)
class RegistrationIteration:
    """One optimizer iteration as ANTs reports it (a ``DIAGNOSTIC`` line): the stage (from
    0), the level (from 0), the iteration (from 1), the metric's value, the convergence
    value, and the full-scale similarity when it was computed."""

    stage: int
    level: int
    iteration: int
    value: float
    convergence: float
    full_scale_value: float | None = None


@dataclass(frozen=True)
class RegistrationResult:
    """What ``antsRegistration`` wrote.

    - ``transforms``: the transform files in ``antsApplyTransforms``' ``-t`` order (the last
      one is applied first to a point of the fixed image), each the list of transforms the
      file holds; pass them to :func:`~larmorx.ants.apply_transforms` as they are. Collapsed
      (the default) they hold the initial transforms too; otherwise, as with ANTs, initial
      transforms given as files are not among them (one from images is:
      ``DerivedInitialMovingTranslation``).
    - ``inverse_transforms``: the inverse composite (with ``write_composite_transform``).
    - ``files``: every transform file, by its name after the output prefix
      (``"0GenericAffine.mat"``, ``"Composite.h5"``, ``"0DerivedInitialMovingTranslation.mat"``).
    - ``warped``, ``inverse_warped``: the moving image on the fixed grid and the other way
      round, when asked for.
    - ``iterations``: every optimizer iteration; ``log``: what the program printed.
    """

    transforms: list[list[ItkTransform]]
    inverse_transforms: list[list[ItkTransform]] | None
    files: dict[str, list[ItkTransform]]
    warped: Image | None
    inverse_warped: Image | None
    iterations: list[RegistrationIteration] = field(default_factory=list)
    log: str = ""


_FEATURES = {"geometry": 0, "moments": 1, "origins": 2}


def _num(v: float) -> str:
    """A number as its shortest round-trip text (an integral value without a fraction)."""
    f = float(v)
    return str(int(f)) if f.is_integer() and abs(f) < 1e15 else repr(f)


class _Args:
    """The command line under construction, with the in-memory inputs and temporary files."""

    def __init__(self, tmp: Path) -> None:
        self.tmp = tmp
        self.inputs: dict[str, Any] = {}
        self._images: dict[int, str] = {}
        self._files = 0

    def image(self, value: Any) -> str:
        """A path as given, or a placeholder name for an in-memory image."""
        if isinstance(value, (str, os.PathLike)):
            return os.fspath(value)
        key = id(value)
        if key not in self._images:
            name = f"lx-input-{len(self._images)}.nii.gz"
            self.inputs[name] = memory_image(value)
            self._images[key] = name
        return self._images[key]

    def transform(self, value: Any) -> str:
        """A transform file: a path as given, transform objects written to a temporary file."""
        if isinstance(value, (str, os.PathLike)):
            return os.fspath(value)
        items = [value] if isinstance(value, ItkTransform) else list(value)
        self._files += 1
        path = self.tmp / f"lx-initial-{self._files}.h5"
        write(path, items)
        return os.fspath(path)


def _initial(args: _Args, value: Any) -> str:
    if isinstance(value, AlignCenters):
        return f"[{args.image(value.fixed)},{args.image(value.moving)},{_FEATURES[value.feature]}]"
    if isinstance(value, tuple) and len(value) == 2 and isinstance(value[1], bool):
        return f"[{args.transform(value[0])},{int(value[1])}]"
    return args.transform(value)


def _metric(args: _Args, m: RegistrationMetric) -> str:
    parameter = m.radius if m.type == "CC" else m.bins
    fields = [
        args.image(m.fixed),
        args.image(m.moving),
        _num(m.weight),
        str(int(parameter)),
        m.sampling,
        _num(m.sampling_percentage),
        str(int(m.use_gradient_filter)),
    ]
    return f"{m.type}[{','.join(fields)}]"


def _mask(args: _Args, value: Any) -> str:
    if isinstance(value, tuple):
        names = ["NULL" if v is None else args.image(v) for v in value]
        return f"[{','.join(names)}]"
    return args.image(value)


_DIAGNOSTIC = re.compile(r"^ ?\*?2DIAGNOSTIC,\s+(\d+), ([^,]+), ([^,]+), [^,]+, [^,]+, ?(.*)$")


def _iterations(log: str) -> list[RegistrationIteration]:
    out: list[RegistrationIteration] = []
    stage = -1
    level = -1
    for line in log.splitlines():
        if line.startswith("Stage ") and line[6:].strip().isdigit():
            stage = int(line[6:])
            level = -1
        elif line.startswith("DIAGNOSTIC,Iteration"):
            level += 1
        else:
            m = _DIAGNOSTIC.match(line)
            if m:
                extra = m.group(4).strip()
                out.append(
                    RegistrationIteration(
                        stage=stage,
                        level=level,
                        iteration=int(m.group(1)),
                        value=float(m.group(2)),
                        convergence=float(m.group(3)),
                        full_scale_value=float(extra) if extra else None,
                    )
                )
    return out


def registration(
    stages: RegistrationStage | Sequence[RegistrationStage],
    *,
    dimension: int | None = None,
    initial_moving_transforms: Sequence[Any] = (),
    initial_fixed_transforms: Sequence[Any] = (),
    restore_state: Any = None,
    winsorize: tuple[float, float] | None = None,
    use_histogram_matching: bool = False,
    collapse_output_transforms: bool = True,
    write_composite_transform: bool = False,
    initialize_transforms_per_stage: bool = False,
    warped: bool = False,
    inverse_warped: bool = False,
    interpolation: str = "Linear",
    precision: Precision = "float64",
    random_seed: int | None = None,
    print_similarity_measure_interval: int = 0,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> RegistrationResult:
    """Register the moving images to the fixed ones in linear stages, as ``antsRegistration``
    (ANTs v2.6.5) does.

    Each :class:`RegistrationStage` optimises one transform (``Translation``, ``Rigid``, ``Similarity``,
    ``Affine`` or ``CompositeAffine``) against its :class:`RegistrationMetric` objects, level by level
    (shrunk and smoothed images); the stages chain, each starting where the previous left.
    The deformable stages (``SyN``, ``BSpline``, ...) come in milestone A5.

    - ``initial_moving_transforms`` (``-r``): transform files, transform lists,
      ``(transform, invert)`` pairs or :class:`AlignCenters`; ``initial_fixed_transforms``
      (``-q``) likewise. ``restore_state`` (``-k``): a state file saved before.
    - ``winsorize``: the lower and upper quantiles (``-w``); ``use_histogram_matching``
      (``-u``).
    - ``collapse_output_transforms`` (``-z``), ``write_composite_transform`` (``-a``),
      ``initialize_transforms_per_stage`` (``-i``; it needs ``write_composite_transform`` and
      no collapsing).
    - ``warped``, ``inverse_warped``: return the warped images (``-o [prefix,warped,inverse]``),
      resampled with ``interpolation`` (``-n``, as ``antsApplyTransforms`` takes it).
    - ``precision``: ``"float32"`` runs ANTs' single-precision pipeline (``--float 1``).
    - ``random_seed`` (``--random-seed``): the seed of the sampling. Without one, ANTs seeds
      from the clock (its result changes from run to run); larmorx then uses a fixed seed.
    - ``print_similarity_measure_interval``: report the full-scale similarity (CC of radius 4
      between the original images) every so many iterations (``iterations[...].full_scale_value``).

    The result does not depend on ``n_threads``: it is ANTs' single-thread result (ANTs' own
    changes with its thread count, ``docs/validation/ants-registration.md``). ANTs' defects
    are corrected as on the command line (``docs/api/upstream-defects.md``);
    ``upstream_defects="reproduce"`` gives ANTs' result, and an
    :class:`~larmorx.UpstreamDefectWarning` names each defect that changed it.

    Raises :class:`RegistrationError` with the program's messages when it fails.
    """
    check(upstream_defects)
    stage_list = [stages] if isinstance(stages, RegistrationStage) else list(stages)
    if not stage_list:
        raise ValueError("at least one stage is needed")
    with tempfile.TemporaryDirectory(prefix="larmorx-registration-") as tmpdir:
        tmp = Path(tmpdir)
        a = _Args(tmp)
        first = stage_list[0].metrics
        first_metric = first if isinstance(first, RegistrationMetric) else first[0]
        if dimension is None:
            fixed = first_metric.fixed
            if isinstance(fixed, (str, os.PathLike)):
                import nibabel as nib

                ndim = len(nib.load(os.fspath(fixed)).shape)
            else:
                ndim = np.asarray(memory_image(fixed)[0]).ndim
            dimension = min(max(ndim, 2), 4)
        prefix = os.fspath(tmp / "lx")
        out_spec = [prefix]
        if warped or inverse_warped:
            out_spec.append("lx-warped.nii.gz" if warped else "")
        if inverse_warped:
            out_spec.append("lx-inverse-warped.nii.gz")
        argv = ["-d", str(int(dimension)), "-v", "1", "-o", f"[{','.join(out_spec)}]"]
        argv += ["--float", "1" if precision == "float32" else "0"]
        argv += ["-n", interpolation]
        if winsorize is not None:
            argv += ["-w", f"[{_num(winsorize[0])},{_num(winsorize[1])}]"]
        argv += ["-u", str(int(use_histogram_matching))]
        argv += ["-z", str(int(collapse_output_transforms))]
        argv += ["-a", str(int(write_composite_transform))]
        argv += ["-i", str(int(initialize_transforms_per_stage))]
        if random_seed is not None:
            argv += ["--random-seed", str(int(random_seed))]
        if print_similarity_measure_interval:
            argv += [
                "--print-similarity-measure-interval",
                str(int(print_similarity_measure_interval)),
            ]
        for t in initial_moving_transforms:
            argv += ["-r", _initial(a, t)]
        for t in initial_fixed_transforms:
            argv += ["-q", _initial(a, t)]
        if restore_state is not None:
            argv += ["-k", _initial(a, restore_state)]
        for s in stage_list:
            argv += ["-t", f"{s.transform}[{_num(s.gradient_step)}]"]
            metrics = [s.metrics] if isinstance(s.metrics, RegistrationMetric) else list(s.metrics)
            for m in metrics:
                argv += ["-m", _metric(a, m)]
            its = "x".join(str(int(i)) for i in s.iterations)
            argv += ["-c", f"[{its},{_num(s.convergence_threshold)},{int(s.convergence_window)}]"]
            argv += ["-f", "x".join(str(int(f)) for f in s.shrink_factors)]
            unit = "mm" if s.sigmas_in_mm else "vox"
            argv += ["-s", "x".join(_num(v) for v in s.smoothing_sigmas) + unit]
            if s.masks is not None:
                argv += ["-x", _mask(a, s.masks)]
            if s.restrict_deformation is not None:
                argv += ["-g", "x".join(_num(v) for v in s.restrict_deformation)]
        captures = ["lx-warped.nii.gz", "lx-inverse-warped.nii.gz"]
        code, out, err, outputs = _core.ants_program(
            "antsRegistration", argv, a.inputs, captures, n_threads, upstream_defects
        )
        import warnings

        messages = []
        for line in err.splitlines():
            warning = parse_line(line)
            if warning is not None:
                warnings.warn(warning, stacklevel=2)
            else:
                messages.append(line)
        if code != 0:
            raise RegistrationError(
                "\n".join(messages).strip() or f"antsRegistration exited {code}"
            )
        files: dict[str, list[ItkTransform]] = {}
        for path in sorted(tmp.iterdir()):
            ours = path.name.startswith("lx") and not path.name.startswith(
                ("lx-initial-", "lx-input-")
            )
            if ours and path.suffix in (".mat", ".txt", ".h5"):
                files[path.name[2:]] = read(path, upstream_defects=upstream_defects)

        def index(name: str) -> int:
            m = re.match(r"(\d+)", name)
            return int(m.group(1)) if m else -1

        if "Composite.h5" in files:
            transforms = [files["Composite.h5"]]
            inverse = [files["InverseComposite.h5"]] if "InverseComposite.h5" in files else None
        else:
            numbered = [n for n in files if index(n) >= 0 and "DerivedInitialFixed" not in n]
            transforms = [files[n] for n in sorted(numbered, key=index, reverse=True)]
            inverse = None

        def image(name: str) -> Image | None:
            if name not in outputs:
                return None
            data, affine, _descrip = outputs[name]
            return Image(data, affine)

        return RegistrationResult(
            transforms=transforms,
            inverse_transforms=inverse,
            files=files,
            warped=image("lx-warped.nii.gz"),
            inverse_warped=image("lx-inverse-warped.nii.gz"),
            iterations=_iterations(out),
            log=out,
        )
