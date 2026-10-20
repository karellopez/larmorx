# SPDX-License-Identifier: Apache-2.0
"""``antsRegistration``'s linear and SyN stages: :func:`registration`.

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
    "IntensityPointSetMetric",
    "PointSetMetric",
    "RegistrationError",
    "RegistrationIteration",
    "RegistrationMetric",
    "RegistrationResult",
    "RegistrationStage",
    "registration",
]

MetricType = Literal["Mattes", "MI", "MI2", "MeanSquares", "CC", "GC", "Demons"]
PointSetMetricType = Literal["ICP", "PSE", "JHCT"]
TransformType = Literal["Translation", "Rigid", "Similarity", "Affine", "CompositeAffine", "SyN"]
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
class PointSetMetric:
    """One labelled point-set metric of a stage (``-m ICP|PSE|JHCT[fixed,moving,weight,
    samplingPercentage,boundaryPointsOnly,pointSetSigma,kNeighborhood,alpha,
    useAnisotropicCovariances]``): points of the same label are matched.

    - ``type``: ``"ICP"`` (the distance to the nearest moving point), ``"PSE"`` (a Gaussian
      expectation over the ``k_neighborhood`` nearest moving points, of width ``sigma``) or
      ``"JHCT"`` (the Jensen-Havrda-Charvát-Tsallis divergence of a manifold Parzen density of
      the moving points, of regularisation ``sigma``, order ``alpha``, over the
      ``k_neighborhood`` nearest points, with ``anisotropic_covariances`` estimated from each
      point's 5 nearest).
    - ``fixed``, ``moving``: a label image (path or in-memory; every voxel whose label is not 0
      is a point, or with ``boundary_points_only`` the voxels on a label's contour), a point
      set file (``.txt``: ``x y z label`` per line, ``x y 0 label`` in 2D; ``.vtk`` polydata
      whose scalars are the labels), or a pair ``(points, labels)`` of arrays (``N × D``
      physical coordinates in ITK's LPS convention, ``N`` labels).
    - ``sampling_percentage``: keep a random fraction of each point set (ANTs seeds this from
      the clock; larmorx from ``random_seed``).
    - ``weight``: the metric's weight in a stage of several metrics.

    A stage of point-set metrics only needs a fixed mask (``RegistrationStage.masks``): its
    grid is the virtual domain (ANTs crashes without one). Warped images and
    ``print_similarity_measure_interval`` need an image metric first.
    """

    type: PointSetMetricType
    fixed: Any
    moving: Any
    weight: float = 1.0
    sampling_percentage: float = 1.0
    boundary_points_only: bool = False
    sigma: float = 1.0
    k_neighborhood: int = 50
    alpha: float = 1.1
    anisotropic_covariances: bool = False


@dataclass(frozen=True)
class IntensityPointSetMetric:
    """ANTs' intensity point-set metric of a stage (``-m IGDM[fixed,moving,weight,fixedMask,
    movingMask,neighborhoodRadius,intensitySigma,distanceSigma,kNeighborhood,gradientSigma]``):
    each masked voxel of ``fixed`` (whose neighbourhood of ``neighborhood_radius`` lies in the
    image) is compared with the nearest masked voxel of ``moving`` by its intensity and its
    distance.

    - ``fixed``, ``moving``: images; ``fixed_mask``, ``moving_mask``: their masks (on their
      grids).
    - ``intensity_sigma``, ``distance_sigma``: the widths of the two terms; 0 estimates them.
    - ``k_neighborhood`` and ``gradient_sigma`` are passed on as ANTs takes them; ANTs does not
      use them (ANTS-67).
    """

    fixed: Any
    moving: Any
    fixed_mask: Any
    moving_mask: Any
    weight: float = 1.0
    neighborhood_radius: int | Sequence[int] = 0
    intensity_sigma: float = 0.0
    distance_sigma: float = 0.0
    k_neighborhood: int = 1
    gradient_sigma: float = 1.0


AnyMetric = RegistrationMetric | PointSetMetric | IntensityPointSetMetric


@dataclass(frozen=True)
class RegistrationStage:
    """One stage (``-t``, ``-m``, ``-c``, ``-f``, ``-s``, and per stage ``-x`` and ``-g``).

    ``iterations``, ``shrink_factors`` and ``smoothing_sigmas`` have one value per level
    (``-c [10x5,...]``, ``-f 2x1``, ``-s 1x0vox``); ``sigmas_in_mm`` gives the sigmas in
    millimetres (``mm``) instead of voxels (``vox``). ``masks`` is a fixed mask, or a pair
    ``(fixed, moving)`` where ``None`` means no mask. ``restrict_deformation`` weighs each
    parameter of the stage's transform (``-g``); for ``SyN``, each axis of the displacement.

    ``SyN`` (symmetric normalisation, ``-t SyN[gradientStep,updateFieldVariance,
    totalFieldVariance]``) takes ``update_field_variance`` and ``total_field_variance``: the
    variances (in voxels²) of the Gaussian smoothing of each update and of the total fields
    (ANTs' defaults 3 and 0). The linear transforms ignore them.
    """

    transform: TransformType
    gradient_step: float
    metrics: AnyMetric | Sequence[AnyMetric]
    iterations: Sequence[int]
    shrink_factors: Sequence[int]
    smoothing_sigmas: Sequence[float]
    sigmas_in_mm: bool = False
    convergence_threshold: float = 1e-6
    convergence_window: int = 10
    masks: Any = None
    restrict_deformation: Sequence[float] | None = None
    update_field_variance: float = 3.0
    total_field_variance: float = 0.0


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
      (``"0GenericAffine.mat"``, ``"1Warp.nii.gz"``, ``"1InverseWarp.nii.gz"``, ``"Composite.h5"``,
      ``"0DerivedInitialMovingTranslation.mat"``). A SyN stage's transform is a displacement
      field (``1Warp.nii.gz``, read as a ``DisplacementFieldTransform``) and its inverse field
      (``1InverseWarp.nii.gz``, not in ``transforms``).
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

    def point_set(self, value: Any, dimension: int) -> str:
        """A point-set argument: a path or an image as :meth:`image` takes it, or
        ``(points, labels)`` written to a temporary text file."""
        # (points, labels): labels are one-dimensional; an image's (array, affine) is not.
        if isinstance(value, tuple) and len(value) == 2 and np.ndim(value[1]) == 1:
            points = np.asarray(value[0], dtype=np.float64)
            labels = np.asarray(value[1])
            if points.ndim != 2 or points.shape[1] != dimension or len(labels) != len(points):
                raise ValueError(
                    f"a point set is (points, labels) with points of shape (N, {dimension}) and N labels"
                )
            self._files += 1
            path = self.tmp / f"lx-points-{self._files}.txt"
            rows = []
            for p, label in zip(points, labels, strict=True):
                coords = [repr(float(c)) for c in p]
                if dimension == 2:
                    coords.append("0")
                rows.append(" ".join([*coords, str(int(label))]))
            # No final newline: ANTs' reader would read one more record (ANTS-62).
            path.write_text("\n".join(rows), encoding="utf-8")
            return os.fspath(path)
        return self.image(value)

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


def _metric(args: _Args, m: AnyMetric, dimension: int) -> str:
    if isinstance(m, PointSetMetric):
        fields = [
            args.point_set(m.fixed, dimension),
            args.point_set(m.moving, dimension),
            _num(m.weight),
            _num(m.sampling_percentage),
            str(int(m.boundary_points_only)),
            _num(m.sigma),
            str(int(m.k_neighborhood)),
            _num(m.alpha),
            str(int(m.anisotropic_covariances)),
        ]
        return f"{m.type}[{','.join(fields)}]"
    if isinstance(m, IntensityPointSetMetric):
        radius = m.neighborhood_radius
        r = [int(radius)] * dimension if isinstance(radius, int) else [int(v) for v in radius]
        fields = [
            args.image(m.fixed),
            args.image(m.moving),
            _num(m.weight),
            args.image(m.fixed_mask),
            args.image(m.moving_mask),
            "x".join(str(v) for v in r),
            _num(m.intensity_sigma),
            _num(m.distance_sigma),
            str(int(m.k_neighborhood)),
            _num(m.gradient_sigma),
        ]
        return f"IGDM[{','.join(fields)}]"
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


_DIAGNOSTIC = re.compile(r"^ ?\*?[12]DIAGNOSTIC,\s+(\d+), ([^,]+), ([^,]+), [^,]+, [^,]+, ?(.*)$")


def _iterations(log: str) -> list[RegistrationIteration]:
    out: list[RegistrationIteration] = []
    stage = -1
    level = -1
    for line in log.splitlines():
        if line.startswith("Stage ") and line[6:].strip().isdigit():
            stage = int(line[6:])
            level = -1
        elif line.startswith(("DIAGNOSTIC,Iteration", "XXDIAGNOSTIC,Iteration")):
            # A linear stage's header (2DIAGNOSTIC lines) or SyN's (1DIAGNOSTIC lines).
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
    """Register the moving images to the fixed ones in linear and SyN stages, as
    ``antsRegistration`` (ANTs v2.6.5) does.

    Each :class:`RegistrationStage` optimises one transform (``Translation``, ``Rigid``,
    ``Similarity``, ``Affine``, ``CompositeAffine``, or ``SyN``'s symmetric displacement
    fields) against its metrics (image metrics, :class:`RegistrationMetric`; point-set metrics,
    :class:`PointSetMetric` and :class:`IntensityPointSetMetric`, in linear stages), level by
    level (shrunk and smoothed images); the stages chain, each starting where the previous
    left. The other deformable stages (``BSplineSyN``, ``BSpline``, displacement and velocity
    fields) come in milestone A5's next parts.

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
        first_metric = first if isinstance(first, AnyMetric) else first[0]
        if dimension is None:
            fixed = first_metric.fixed
            if isinstance(fixed, tuple) and len(fixed) == 2 and np.ndim(fixed[0]) == 2:
                ndim = np.shape(fixed[0])[1]
            elif isinstance(fixed, (str, os.PathLike)):
                if os.fspath(fixed).lower().endswith((".txt", ".vtk")):
                    raise ValueError(
                        "give the dimension for point sets read from text or VTK files"
                    )
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
            if s.transform == "SyN":
                parameters = [
                    _num(s.gradient_step),
                    _num(s.update_field_variance),
                    _num(s.total_field_variance),
                ]
            else:
                parameters = [_num(s.gradient_step)]
            argv += ["-t", f"{s.transform}[{','.join(parameters)}]"]
            metrics = [s.metrics] if isinstance(s.metrics, AnyMetric) else list(s.metrics)
            for m in metrics:
                argv += ["-m", _metric(a, m, int(dimension))]
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
                ("lx-initial-", "lx-input-", "lx-points-")
            )
            field = path.name.endswith(("Warp.nii.gz", "Warp.nii"))
            if ours and (path.suffix in (".mat", ".txt", ".h5") or field):
                files[path.name[2:]] = read(path, upstream_defects=upstream_defects)

        def index(name: str) -> int:
            m = re.match(r"(\d+)", name)
            return int(m.group(1)) if m else -1

        if "Composite.h5" in files:
            transforms = [files["Composite.h5"]]
            inverse = [files["InverseComposite.h5"]] if "InverseComposite.h5" in files else None
        else:
            numbered = [
                n
                for n in files
                if index(n) >= 0 and "DerivedInitialFixed" not in n and "InverseWarp" not in n
            ]
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
