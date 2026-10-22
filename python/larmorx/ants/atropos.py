# SPDX-License-Identifier: Apache-2.0
"""Finite-mixture segmentation with an MRF prior, as ANTs' ``Atropos`` computes it (ANTs v2.6.5
on ITK v5.4.5): :func:`atropos`.

The function builds Atropos' command line and runs larmorx's port of it on in-memory images
(``larmorx ants Atropos`` runs the same code from the command line), so every option behaves as
the program's option does. The defects of the original that larmorx corrects by default
(``upstream_defects="fix"``) are listed in ``docs/api/ants-atropos.md``; ``"reproduce"`` gives
ANTs' result bit for bit.
"""

from __future__ import annotations

import os
import re
import warnings
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, parse_line
from larmorx.ants._filters import memory_image
from larmorx.image import Image

__all__ = [
    "AtroposError",
    "AtroposIteration",
    "AtroposResult",
    "BoxPlot",
    "GaussianLikelihood",
    "GrubbsRosner",
    "HistogramParzenWindows",
    "JointShapeAndOrientationProbability",
    "LogEuclideanGaussian",
    "ManifoldParzenWindows",
    "atropos",
]


class AtroposError(RuntimeError):
    """Atropos stopped: its message (ANTs' own, explained by larmorx where ANTs says nothing)."""


# ------------------------------------------------------------------------------------------------
# Likelihood models (-k) and outlier handling (-w)


@dataclass(frozen=True)
class GaussianLikelihood:
    """A Gaussian per class, its mean and covariance estimated with the posterior weights."""


@dataclass(frozen=True)
class HistogramParzenWindows:
    """A smoothed histogram per class and intensity image: ``bins`` bins plus 3 ``sigma`` on
    each side, smoothed by a Gaussian of ``sigma`` bins."""

    sigma: float = 1.0
    bins: int = 32


@dataclass(frozen=True)
class ManifoldParzenWindows:
    """A Gaussian at every sample: covariance ``regularization_sigma`` times the identity, or
    (``covariance_k`` > 0) estimated from that many neighbours with a kernel of
    ``kernel_sigma``; evaluated over the ``evaluation_k`` nearest samples."""

    regularization_sigma: float = 1.0
    evaluation_k: int = 50
    covariance_k: int = 0
    kernel_sigma: float = 1.0


@dataclass(frozen=True)
class JointShapeAndOrientationProbability:
    """Joint histograms of diffusion tensors' shape and orientation (6 intensity images in 3D,
    the tensors' upper triangles xx, xy, xz, yy, yz, zz)."""

    shape_sigma: float = 2.0
    shape_bins: int = 64
    orientation_sigma: float = 1.0
    orientation_bins: int = 32


@dataclass(frozen=True)
class LogEuclideanGaussian:
    """A Gaussian of the log-Euclidean distance to the classes' mean tensors (6 intensity
    images in 3D)."""


LikelihoodModel = (
    GaussianLikelihood
    | HistogramParzenWindows
    | ManifoldParzenWindows
    | JointShapeAndOrientationProbability
    | LogEuclideanGaussian
)


@dataclass(frozen=True)
class BoxPlot:
    """Winsorize the samples beyond the whiskers ``whisker`` times the interquartile range
    (between the ``lower`` and ``upper`` percentiles) outside them."""

    lower: float = 0.25
    upper: float = 0.75
    whisker: float = 1.5


@dataclass(frozen=True)
class GrubbsRosner:
    """Remove outliers by Grubbs' test at ``significance``, and winsorize them at the
    ``winsorizing`` level."""

    significance: float = 0.05
    winsorizing: float = 0.10


# ------------------------------------------------------------------------------------------------
# Results


@dataclass(frozen=True)
class AtroposIteration:
    """One iteration as Atropos reports it before running it: the number of iterations done,
    the mean maximum posterior probability after the previous one, and the annealing
    temperature (6 significant digits, as printed)."""

    iteration: int
    posterior_probability: float
    annealing_temperature: float


@dataclass(frozen=True)
class AtroposResult:
    """The segmentation.

    - ``labels``: the classified image, 1 to ``classes`` in the mask, 0 outside (uint32).
    - ``posteriors``: one posterior probability image per class (float32, 0 outside the mask).
    - ``likelihoods``, ``distances``, ``smoothed``: per class, when asked for (the distance
      priors and the smoothed intensity images exist only with prior-based initializations).
    - ``icm_codes``: the ICM code image, when asked for.
    - ``iterations``: Atropos' iterations; ``log``: what the program printed.
    """

    labels: Image
    posteriors: tuple[Image, ...]
    likelihoods: tuple[Image, ...] = ()
    distances: tuple[Image, ...] = ()
    smoothed: tuple[Image, ...] = ()
    icm_codes: Image | None = None
    iterations: tuple[AtroposIteration, ...] = field(default_factory=tuple)
    log: str = ""


Initialization = Literal["kmeans", "otsu", "random", "prior-probabilities", "prior-label"]
Formulation = Literal["socrates", "plato", "aristotle", "sigmoid"]


def _num(v: float) -> str:
    """A number as its shortest round-trip text (an integral value without a fraction)."""
    f = float(v)
    return str(int(f)) if f.is_integer() and abs(f) < 1e15 else repr(f)


def _per_axis(v: int | Sequence[int], dimension: int, what: str) -> str:
    values = [int(v)] if isinstance(v, (int, np.integer)) else [int(x) for x in v]
    if len(values) not in (1, dimension):
        raise ValueError(f"{what} needs 1 or {dimension} values, not {len(values)}")
    return "x".join(str(x) for x in values)


_ITERATION = re.compile(
    r"^\s*Iteration (\d+) \(of \d+\): posterior probability = (\S+) \(annealing temperature = (\S+)\)"
)


class _Inputs:
    """In-memory inputs under placeholder names; paths are passed as given."""

    def __init__(self) -> None:
        self.images: dict[str, Any] = {}

    def add(self, value: Any, name: str) -> str:
        if isinstance(value, (str, os.PathLike)):
            return os.fspath(value)
        self.images[name] = memory_image(value)
        return name


def atropos(
    images: Any | Sequence[Any],
    mask: Any,
    *,
    classes: int = 3,
    initialization: Initialization = "kmeans",
    kmeans_centers: Sequence[float] | None = None,
    priors: Sequence[Any] | None = None,
    prior_label: Any = None,
    prior_weight: float = 0.0,
    prior_threshold: float | None = None,
    likelihood: LikelihoodModel | None = None,
    posterior_formulation: Formulation = "socrates",
    use_mixture_model_proportions: bool = True,
    annealing: tuple[float, float, float] = (1.0, 1.0, 0.1),
    iterations: int = 5,
    convergence_threshold: float = 0.001,
    mrf_smoothing: Any = 0.3,
    mrf_radius: int | Sequence[int] = 1,
    icm: bool = True,
    icm_iterations: int = 1,
    use_random_seed: bool = True,
    euclidean_distance: bool = False,
    label_propagation: Mapping[int, tuple[float, float]] | tuple[float, float] | None = None,
    minimize_memory: bool = False,
    outliers: BoxPlot | GrubbsRosner | None = None,
    partial_volume: Sequence[tuple[int, int]] = (),
    partial_volume_likelihoods: bool = False,
    adaptive_smoothing: float | Sequence[float] = 0.0,
    bspline: tuple[int | Sequence[int], int | Sequence[int], int] = (6, 1, 3),
    likelihood_images: bool = False,
    distance_images: bool = False,
    smoothed_images: bool = False,
    icm_code_image: bool = False,
    dimension: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> AtroposResult:
    """``Atropos -d D -a image... -x mask -i <initialization> -k <likelihood> -p
    <formulation> -c [iterations,threshold] -m [smoothing,radius] -g ... -r ... -o [...]``.

    ``images`` is one intensity image or several (paths, :class:`~larmorx.Image`, nibabel
    images or ``(array, affine)``), all on the mask's grid; ``mask`` defines the voxels to
    label (any non-zero value).

    - ``initialization``: ``"kmeans"`` (from ``kmeans_centers`` of the first image, or centres
      spread over its range), ``"otsu"``, ``"random"``, ``"prior-probabilities"`` (``priors``:
      one probability image per class) or ``"prior-label"`` (``prior_label``: an image of labels
      1 to ``classes``). ``prior_weight`` (0 to 1) weighs the priors against the likelihood and
      MRF; ``prior_threshold`` is the probability below which a prior counts as absent.
    - ``likelihood``: :class:`GaussianLikelihood` (the default, ``None``), :class:`HistogramParzenWindows`,
      :class:`ManifoldParzenWindows`, :class:`JointShapeAndOrientationProbability` or
      :class:`LogEuclideanGaussian`.
    - ``posterior_formulation``, ``use_mixture_model_proportions`` and ``annealing`` (initial
      temperature, rate, minimum) as Atropos' ``-p``.
    - ``iterations`` and ``convergence_threshold``: Atropos stops when the mean maximum
      posterior probability changes by less than the threshold.
    - ``mrf_smoothing``: the MRF's smoothing factor, or an image of factors; ``mrf_radius``:
      its neighbourhood radius (one value, or one per axis).
    - ``icm``: asynchronous (ICM) updating with ``icm_iterations`` passes, else synchronous.
    - ``use_random_seed``: Atropos' ``-r``. ANTs seeds its generator from the clock unless it
      is ``False`` (seed 19650218); larmorx never uses the clock: ``True`` gives the seed
      121212.
    - ``euclidean_distance``: propagate prior labels by Euclidean distance instead of fast
      marching along the mask; ``label_propagation``: ``(lambda, boundary probability)`` for
      every class, or per class ``{label: (lambda, probability)}``.
    - ``outliers``: :class:`BoxPlot` or :class:`GrubbsRosner` winsorizing.
    - ``partial_volume``: pairs of class labels; with ``partial_volume_likelihoods`` each pair
      is a class of its own.
    - ``adaptive_smoothing`` (per image, 0 to 1) and ``bspline`` (levels, mesh, order): with
      prior-based initializations, the intensities are mixed with their B-spline smoothing
      within each class.
    - ``likelihood_images``, ``distance_images``, ``smoothed_images``, ``icm_code_image``: the
      optional outputs.

    The result does not depend on ``n_threads``. Raises :class:`AtroposError` when Atropos
    stops (ANTs' message, which ANTs itself prints only with ``-v``), and ``ValueError`` for
    arguments that cannot be expressed on Atropos' command line.
    """
    check(upstream_defects)
    image_list = list(images) if isinstance(images, (list, tuple)) else [images]
    if not image_list:
        raise ValueError("at least one intensity image is needed")
    inputs = _Inputs()
    names = [inputs.add(img, f"lx-image-{i}") for i, img in enumerate(image_list)]
    if dimension is None:
        first = image_list[0]
        if isinstance(first, (str, os.PathLike)):
            import nibabel as nib

            dimension = min(max(len(nib.load(os.fspath(first)).shape), 2), 4)
        else:
            dimension = int(np.asarray(memory_image(first)[0]).ndim)
    dimension = int(dimension)
    argv: list[str] = ["-d", str(dimension)]
    weights = (
        [float(adaptive_smoothing)] * len(names)
        if isinstance(adaptive_smoothing, (int, float, np.floating, np.integer))
        else [float(w) for w in adaptive_smoothing]
    )
    if len(weights) != len(names):
        raise ValueError("adaptive_smoothing needs one weight per intensity image")
    for name, w in zip(names, weights, strict=True):
        argv += ["-a", f"[{name},{_num(w)}]" if w != 0 else name]
    argv += ["-x", inputs.add(mask, "lx-mask")]
    k = int(classes)
    if initialization == "kmeans":
        if kmeans_centers is None:
            argv += ["-i", f"KMeans[{k}]"]
        else:
            argv += ["-i", f"KMeans[{k},{'x'.join(_num(c) for c in kmeans_centers)}]"]
    elif initialization == "otsu":
        argv += ["-i", f"Otsu[{k}]"]
    elif initialization == "random":
        argv += ["-i", f"Random[{k}]"]
    elif initialization == "prior-probabilities":
        if priors is None or len(priors) != k:
            raise ValueError(f"prior-probabilities needs one prior image per class ({k})")
        # Atropos reads its priors as a numbered series: name them lx-prior01, lx-prior02, ...
        # (in-memory images, or placeholders standing for the files given).
        for i, p in enumerate(priors, 1):
            name = f"lx-prior{i:02d}"
            if isinstance(p, (str, os.PathLike)):
                inputs.images[name] = os.fspath(p)
            else:
                inputs.add(p, name)
        pattern = "lx-prior%02d"
        spec = f"PriorProbabilityImages[{k},{pattern},{_num(prior_weight)}"
        if prior_threshold is not None:
            spec += f",{_num(prior_threshold)}"
        argv += ["-i", spec + "]"]
    elif initialization == "prior-label":
        if prior_label is None:
            raise ValueError("prior-label needs prior_label")
        label = inputs.add(prior_label, "lx-prior-label")
        argv += ["-i", f"PriorLabelImage[{k},{label},{_num(prior_weight)}]"]
    else:
        raise ValueError(f"unknown initialization {initialization!r}")
    if likelihood is None or isinstance(likelihood, GaussianLikelihood):
        argv += ["-k", "Gaussian"]
    elif isinstance(likelihood, HistogramParzenWindows):
        argv += ["-k", f"HistogramParzenWindows[{_num(likelihood.sigma)},{int(likelihood.bins)}]"]
    elif isinstance(likelihood, ManifoldParzenWindows):
        m = likelihood
        argv += [
            "-k",
            f"ManifoldParzenWindows[{_num(m.regularization_sigma)},{int(m.evaluation_k)},"
            f"{int(m.covariance_k)},{_num(m.kernel_sigma)}]",
        ]
    elif isinstance(likelihood, JointShapeAndOrientationProbability):
        j = likelihood
        argv += [
            "-k",
            f"JointShapeAndOrientationProbability[{_num(j.shape_sigma)},{int(j.shape_bins)},"
            f"{_num(j.orientation_sigma)},{int(j.orientation_bins)}]",
        ]
    elif isinstance(likelihood, LogEuclideanGaussian):
        argv += ["-k", "LogEuclideanGaussian"]
    else:
        raise ValueError(f"unknown likelihood model {likelihood!r}")
    formulation = {
        "socrates": "Socrates",
        "plato": "Plato",
        "aristotle": "Aristotle",
        "sigmoid": "Sigmoid",
    }.get(posterior_formulation)
    if formulation is None:
        raise ValueError(f"unknown posterior formulation {posterior_formulation!r}")
    t0, rate, tmin = annealing
    argv += [
        "-p",
        f"{formulation}[{int(bool(use_mixture_model_proportions))},{_num(t0)},{_num(rate)},"
        f"{_num(tmin)}]",
    ]
    argv += ["-c", f"[{int(iterations)},{_num(convergence_threshold)}]"]
    if isinstance(mrf_smoothing, (int, float, np.floating, np.integer)):
        smoothing = _num(mrf_smoothing)
    else:
        smoothing = inputs.add(mrf_smoothing, "lx-mrf-coefficients.nii")
    argv += ["-m", f"[{smoothing},{_per_axis(mrf_radius, dimension, 'mrf_radius')}]"]
    captures = ["lx-labels.nii.gz"]
    # ``-g a[a,n]``: the documented ``[a,n]`` makes ANTs abort (ANTS-102); a name first works.
    icm_spec = f"{int(bool(icm))}[{int(bool(icm))},{int(icm_iterations)}"
    if icm_code_image:
        icm_spec += ",lx-icm.nii.gz"
        captures.append("lx-icm.nii.gz")
    argv += ["-g", icm_spec + "]"]
    argv += ["-r", "1" if use_random_seed else "0"]
    argv += ["-e", "1" if euclidean_distance else "0"]
    if label_propagation is not None:
        if isinstance(label_propagation, Mapping):
            for label, (lam, prob) in sorted(label_propagation.items()):
                argv += ["-l", f"{int(label)}[{_num(lam)},{_num(prob)}]"]
        else:
            lam, prob = label_propagation
            argv += ["-l", f"[{_num(lam)},{_num(prob)}]"]
    argv += ["-u", "1" if minimize_memory else "0"]
    if isinstance(outliers, BoxPlot):
        argv += [
            "-w",
            f"BoxPlot[{_num(outliers.lower)},{_num(outliers.upper)},{_num(outliers.whisker)}]",
        ]
    elif isinstance(outliers, GrubbsRosner):
        argv += ["-w", f"GrubbsRosner[{_num(outliers.significance)},{_num(outliers.winsorizing)}]"]
    elif outliers is not None:
        raise ValueError(f"unknown outlier handling {outliers!r}")
    for pair in partial_volume:
        a, b = pair
        argv += ["-s", f"{int(a)}x{int(b)}"]
    if partial_volume:
        argv += ["--use-partial-volume-likelihoods", "1" if partial_volume_likelihoods else "0"]
    levels, mesh, order = bspline
    argv += [
        "-b",
        f"[{_per_axis(levels, dimension, 'bspline levels')},"
        f"{_per_axis(mesh, dimension, 'bspline mesh')},{int(order)}]",
    ]
    # Up to one output per class, partial-volume classes included.
    n_out = k + len(partial_volume)
    out = ["lx-labels.nii.gz", "lx-post%02d.nii.gz"]
    captures += [f"lx-post{i:02d}.nii.gz" for i in range(1, n_out + 1)]
    for flag, stem in (
        (likelihood_images or distance_images or smoothed_images, "lx-lik"),
        (distance_images or smoothed_images, "lx-dist"),
        (smoothed_images, "lx-bsp"),
    ):
        if flag:
            out.append(f"{stem}%02d.nii.gz")
            captures += [f"{stem}{i:02d}.nii.gz" for i in range(1, n_out + 1)]
    argv += ["-o", "[" + ",".join(out) + "]", "-v", "1"]
    code, stdout, stderr, outputs = _core.ants_program(
        "Atropos", argv, inputs.images, captures, n_threads, upstream_defects
    )
    messages = []
    for line in stderr.splitlines():
        warning = parse_line(line)
        if warning is not None:
            warnings.warn(warning, stacklevel=2)
        else:
            messages.append(line)
    if code != 0:
        raise AtroposError("\n".join(messages).strip() or f"Atropos exited {code}")

    def image(name: str) -> Image | None:
        if name not in outputs:
            return None
        data, affine, _descrip = outputs[name]
        return Image(np.asfortranarray(data), affine)

    def series(stem: str) -> tuple[Image, ...]:
        found = [image(f"{stem}{i:02d}.nii.gz") for i in range(1, n_out + 1)]
        return tuple(i for i in found if i is not None)

    labels = image("lx-labels.nii.gz")
    if labels is None:
        raise AtroposError("Atropos wrote no classified image")
    its = []
    for line in stdout.splitlines():
        m = _ITERATION.match(line)
        if m:
            its.append(AtroposIteration(int(m.group(1)), float(m.group(2)), float(m.group(3))))
    return AtroposResult(
        labels=labels,
        posteriors=series("lx-post"),
        likelihoods=series("lx-lik") if likelihood_images else (),
        distances=series("lx-dist") if distance_images else (),
        smoothed=series("lx-bsp") if smoothed_images else (),
        icm_codes=image("lx-icm.nii.gz"),
        iterations=tuple(its),
        log=stdout,
    )
