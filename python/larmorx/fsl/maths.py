# SPDX-License-Identifier: Apache-2.0
"""``fslmaths``: FSL's image calculator (FSL 6.0.7.23, fsl-avwutils 2209.6), the generic
:func:`fslmaths` and the typed wrappers of its operations.

:func:`fslmaths` mirrors the command line, ``fslmaths [-dt <type>] <input> [operations and
inputs] <output> [-odt <type>]``: the operations, applied left to right to the current image,
and their operands, in the command line's order; it returns the output image. The typed
functions (:func:`arithmetic`, :func:`threshold`, :func:`spatial_filter`, :func:`collapse`, ...)
build the same command line from Python arguments, one per operation family of fslmaths' usage
text, and those fMRIPrep runs (:func:`arithmetic` with ``"multiply"``: the masks niworkflows
combines for each BOLD reference; :func:`collapse`, :func:`apply_mask`, :func:`binarise`,
:func:`smooth`, :func:`spatial_filter` and :func:`threshold`: the goodvoxels mask of
``--project-goodvoxels``).

Two ports (``docs/licensing.md``): the bit-exact replica from ``larmorx-nc``, run as a separate
program on one thread, and the clean room (Apache-2.0, written from ``specs/fslmaths.md``, run in
this process on ``n_threads`` threads, its results the same at any thread count);
``implementation="auto"`` runs the replica when it is installed, the clean room otherwise. The
clean room takes every operation of fslmaths' usage text and the hidden ones
(``docs/api/fsl-maths.md``). FSL's deterministic defects are corrected
by default (D13, ``docs/api/fsl-maths.md``); ``upstream_defects="reproduce"`` gives FSL's
results. FSL's environment variables are settings of the call, never read from the process
environment (CLAUDE.md rule 6): the program writes NIfTI-1 (``FSLOUTPUTTYPE=NIFTI``).
"""

from __future__ import annotations

import glob
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
from larmorx.fsl.bet import _read_image, _threads
from larmorx.image import Image, as_image

__all__ = [
    "BINARY",
    "COLLAPSE",
    "DATATYPES",
    "SPATIAL_FILTERS",
    "STATISTICAL",
    "UNARY",
    "RocResult",
    "TensorDecomposition",
    "add_noise",
    "apply_mask",
    "arithmetic",
    "bandpass_temporal_filter",
    "binarise",
    "collapse",
    "dog_edge",
    "fslmaths",
    "grid",
    "normalise_intensity",
    "roc",
    "roi",
    "smooth",
    "spatial_filter",
    "statistical",
    "subsample2",
    "tensor_decomp",
    "tfce",
    "tfce_support",
    "threshold",
    "unary",
]

#: The data types of ``dt`` (``-dt``: the type the operations compute in) and ``odt`` (``-odt``:
#: the output's type), as fslmaths names them: ``char`` (unsigned 8-bit), ``short``, ``int``,
#: ``float``, ``double``, and ``input`` (the input image's type).
DATATYPES = ("char", "short", "int", "float", "double", "input")

DataType = Literal["char", "short", "int", "float", "double", "input"]

#: The binary operations of :func:`arithmetic`, by their friendly names, with fslmaths' options:
#: each takes a number or an image (a 3D image is cloned over a 4D one's volumes).
BINARY = {
    "add": "-add",
    "subtract": "-sub",
    "multiply": "-mul",
    "divide": "-div",
    "remainder": "-rem",
    "max": "-max",
    "min": "-min",
}

#: The unary operations of :func:`unary`, as fslmaths names them (without the ``-``).
UNARY = (
    "exp",
    "log",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "sqr",
    "sqrt",
    "recip",
    "abs",
    "bin",
    "binv",
    "fillh",
    "fillh26",
    "index",
    "edge",
    "nan",
    "nanm",
    "range",
)

#: The spatial filters of :func:`spatial_filter` (they use ``kernel``), as fslmaths names them.
SPATIAL_FILTERS = ("dilM", "dilD", "dilF", "dilall", "ero", "eroF", "fmedian", "fmean", "fmeanu")

#: The statistics of :func:`collapse` (fslmaths' ``-T<statistic>``, and ``-X``, ``-Y``, ``-Z``).
COLLAPSE = ("mean", "std", "max", "maxn", "min", "median", "perc", "ar1")

#: The statistical transforms of :func:`statistical`, as fslmaths names them.
STATISTICAL = ("pval", "pval0", "cpval", "ztop", "ptoz", "rank", "ranknorm")

#: The kernels' types (``-kernel``) and how many numbers each takes.
_KERNELS = {"3D": 0, "2D": 0, "box": 1, "boxv": 1, "boxv3": 3, "gauss": 1, "sphere": 1, "file": 1}

#: The images the program writes without the caller naming them: the output's base name.
_OUT = "lx-out"

#: The kinds of array fslmaths reads as they are; any other is passed as float32.
_STORED = (
    np.int8,
    np.uint8,
    np.int16,
    np.uint16,
    np.int32,
    np.uint32,
    np.int64,
    np.uint64,
    np.float32,
    np.float64,
)


@dataclass(frozen=True)
class TensorDecomposition:
    """What ``-tensor_decomp`` writes (:func:`tensor_decomp`): the eigenvalues ``l1`` >= ``l2``
    >= ``l3``, the fractional anisotropy ``fa`` (also the pipeline's current image, ``image``),
    the mean diffusivity ``md``, the mode ``mo``, and the eigenvectors ``v1``, ``v2``, ``v3``
    (4D, three volumes each)."""

    image: Image
    l1: Image
    l2: Image
    l3: Image
    fa: Image
    md: Image
    mo: Image
    v1: Image
    v2: Image
    v3: Image


@dataclass(frozen=True)
class RocResult:
    """What ``-roc`` gives (:func:`roc`): the area under the ROC curve up to the false-positive
    limit, normalised by it, and its first and third quartiles over the truth's volumes, as
    fslmaths prints them (``%g``); the curve (one row per threshold: false-positive rate,
    true-positive rate, threshold) read from the file fslmaths writes; and the current image
    (unchanged by ``-roc``)."""

    aroc: float
    aroc_quartiles: tuple[float, float]
    curve: np.ndarray
    image: Image


@dataclass(frozen=True)
class _Ran:
    image: Image
    stdout: str
    tmp: str


def _input(image: Any, tmp: str, name: str, n_threads: int) -> str:
    """An image operand as the program reads it: a path as given, else the in-memory image
    written to ``tmp`` (its data type kept when NIfTI stores it, booleans as uint8, any other
    as float32)."""
    if isinstance(image, (str, os.PathLike)):
        return os.fspath(image)
    img = as_image(image)
    array = np.asarray(img.data)
    if array.ndim not in (2, 3, 4):
        raise ValueError(f"an image operand must be 2D, 3D or 4D, not {array.ndim}D")
    if array.ndim == 2:
        array = array[:, :, np.newaxis]
    if array.dtype == np.bool_:
        array = array.astype(np.uint8)
    elif array.dtype.type not in _STORED or not array.dtype.isnative:
        array = array.astype(np.float32)
    template = None if img.header is None else img.header.to_dict()
    fields = _core.nifti_header_for_image(
        list(array.shape), array.dtype.name, img.affine, template, None
    )
    fields.update(scl_slope=1.0, scl_inter=0.0)
    path = os.path.join(tmp, f"{name}.nii")
    _core.nifti_write(path, array, fields, 0, n_threads)
    return path


def _number(value: Any) -> str:
    """A number as the command line gives it: integers as they are, floats as their shortest
    text that reads back as the same double (``repr``)."""
    if isinstance(value, (bool, np.bool_)):
        return str(int(value))
    if isinstance(value, (int, np.integer)):
        return str(int(value))
    return repr(float(value))


def _operand(value: Any, index: int, tmp: str, n_threads: int) -> str:
    """One argument after the input: a number, a string (an operation, a type, a file name: as
    it is), a path, or an image (written to ``tmp``)."""
    if isinstance(value, (bool, int, float, np.integer, np.floating, np.bool_)):
        return _number(value)
    if isinstance(value, str):
        return value
    if isinstance(value, os.PathLike):
        return os.fspath(value)
    return _input(value, tmp, f"lx-in-{index}", n_threads)


def _datatype(name: str, value: str | None) -> str | None:
    if value is not None and value not in DATATYPES:
        raise ValueError(f"{name} must be one of {', '.join(DATATYPES)}, not {value!r}")
    return value


def _clean_stdout(stdout: str) -> str:
    """fslmaths' standard output without its usage text (printed after a command-line error)."""
    cut = stdout.find("\nUsage: fslmaths")
    return (stdout if cut < 0 else stdout[:cut]).strip()


def _warn_lines(text: str) -> None:
    for line in text.splitlines():
        notice = _defects.parse_line(line)
        if notice is not None:
            warnings.warn(notice, stacklevel=4)
        elif line.strip().upper().startswith("WARNING"):
            warnings.warn(line.strip(), stacklevel=4)


def _run(
    image: Any,
    operations: Sequence[Any],
    *,
    dt: str | None,
    odt: str | None,
    n_threads: int,
    implementation: ImplementationName,
    upstream_defects: UpstreamDefectsName,
    tmp: str,
) -> _Ran:
    """``fslmaths [-dt dt] image operations... <tmp>/lx-out [-odt odt]`` by the replica, in the
    caller's working directory (relative names are the caller's, as in a shell); the output read
    back."""
    _defects.check(upstream_defects)
    n_threads = _threads(n_threads)
    _datatype("dt", dt)
    _datatype("odt", odt)
    selected = _replica.select("fsl", "fslmaths", implementation)
    impl = _replica.ORIGINAL if selected is None else selected.implementation()
    args = [] if dt is None else ["-dt", dt]
    args.append(_input(image, tmp, "lx-in", n_threads))
    args += [_operand(v, i, tmp, n_threads) for i, v in enumerate(operations)]
    out = os.path.join(tmp, _OUT)
    args.append(out)
    if odt is not None:
        args += ["-odt", odt]
    if selected is None:
        # the clean room, in this process (its notices become warnings)
        env = [("FSLOUTPUTTYPE", "NIFTI"), (_defects.ENV, upstream_defects)]
        if n_threads > 0:
            env.append(("OMP_NUM_THREADS", str(n_threads)))
        code, stdout, stderr = _core.fsl_run_original("fslmaths", args, env, os.getcwd())
        _warn_lines(stderr)
        _warn_lines(stdout)
        written = glob.glob(glob.escape(out) + ".nii")
        if code != 0 or not written:
            message = "\n".join(s for s in (stderr.strip(), _clean_stdout(stdout)) if s)
            raise RuntimeError(
                f"larmorx fsl fslmaths (the clean-room original) failed with exit code {code}"
                + (f":\n{message}" if message else "")
            )
        return _Ran(_read_image(written[0], n_threads, impl), stdout, tmp)
    try:
        process = _replica.run(
            selected,
            args,
            cwd=os.getcwd(),
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp},
            exclude=lambda name: name.upper().startswith("FSL"),
        )
    except _replica.ReplicaError as e:
        message = "\n".join(s for s in (e.stderr.strip(), _clean_stdout(e.stdout)) if s)
        raise _replica.ReplicaError(
            f"fslmaths failed with exit code {e.returncode}" + (f":\n{message}" if message else ""),
            command=e.command,
            returncode=e.returncode,
            stderr=e.stderr,
            stdout=e.stdout,
        ) from None
    _warn_lines(process.stderr)
    _warn_lines(process.stdout)
    written = glob.glob(glob.escape(out) + ".nii")
    if not written:
        raise _replica.ReplicaError(
            "fslmaths wrote no output image",
            command=list(process.args),
            returncode=process.returncode,
            stderr=process.stderr,
            stdout=process.stdout,
        )
    return _Ran(_read_image(written[0], n_threads, impl), process.stdout, tmp)


def fslmaths(
    image: Any,
    *operations: Any,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """Run fslmaths' ``operations`` on ``image``, as ``fslmaths [-dt <dt>] <image>
    <operations...> <output> [-odt <odt>]`` does, and return the output image.

    ``image`` and the image operands are :class:`~larmorx.Image`, nibabel images,
    ``(array, affine)`` pairs or paths; in-memory images are written for the program in their
    own data type (FSL's results depend on it with ``dt="input"`` and ``odt="input"``).
    Operations are fslmaths' own words (``"-mul"``, ``"-kernel"``, ``"gauss"``, ``"-Tmean"``,
    ...), numbers are passed as their shortest round-trip text, strings as they are: so
    ``fslmaths(a, "-mul", b)`` is exactly ``fslmaths a.nii -mul b.nii out`` (niworkflows'
    combination of two masks), and ``fslmaths(bold, "-Tstd")`` fMRIPrep's first goodvoxels step.
    The program runs in the caller's working directory: relative file names are read there, and
    the files an operation names (``-save <name>``, ``-roc``'s curve) are written there, as
    uncompressed NIfTI-1 (``FSLOUTPUTTYPE=NIFTI``), as the command line writes them.

    ``dt`` is ``-dt`` (the type the operations compute in: float, or double for a double input,
    by default) and ``odt`` ``-odt`` (the output's type: float, or double, by default); see
    :data:`DATATYPES`. The image returned has fslmaths' type and geometry. The typed wrappers
    (:func:`arithmetic`, :func:`threshold`, :func:`collapse`, ...) build these command lines from
    Python arguments; :func:`tensor_decomp` and :func:`roc` also return the files those
    operations write.

    ``n_threads`` is the clean room's thread count (0: all; its results are the same at any
    count); the replica runs on one thread. ``implementation`` chooses between them (``"auto"``:
    the replica when ``larmorx-nc`` is installed, else the clean room).

    Raises ``ValueError`` for invalid arguments, :class:`larmorx.ReplicaError` when the replica
    fails and ``RuntimeError`` when the clean room fails (their messages are in the error), and
    :class:`larmorx.ReplicaNotFoundError` for ``implementation="replica"`` without
    ``larmorx-nc``.

    Parameters
    ----------
    image
        The first input: a path, an :class:`~larmorx.Image`, a nibabel image or an ``(array,
        affine)`` pair.
    *operations
        fslmaths' operations and their operands in its own words, applied left to right
        (``"-mul"``, an image, ``"-kernel"``, ``"gauss"``, 2.5, ``"-Tmean"``, ...).
    dt
        ``-dt``: the type the operations compute in (:data:`DATATYPES`).
    odt
        ``-odt``: the output's type (:data:`DATATYPES`).
    n_threads
        Threads (0 = all) for the in-memory images and the clean room's operations; the
        replica runs on one thread; the same results at any count.
    implementation
        ``"auto"`` (default): the replica when ``larmorx-nc`` is installed, else the clean room;
        ``"replica"`` or ``"clean-room"`` (``"original"``, 0.1.0rc1's spelling, still works).
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"``: FSL's defects corrected, or FSL's behaviour
        (``docs/api/fsl-maths.md``, "Defects of the original, corrected by default").
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-fslmaths-") as tmp:
        return _run(
            image,
            operations,
            dt=dt,
            odt=odt,
            n_threads=n_threads,
            implementation=implementation,
            upstream_defects=upstream_defects,
            tmp=tmp,
        ).image


def _typed(
    image: Any,
    operations: Sequence[Any],
    dt: DataType | None,
    odt: DataType | None,
    n_threads: int,
    implementation: ImplementationName,
    upstream_defects: UpstreamDefectsName,
) -> Image:
    return fslmaths(
        image,
        *operations,
        dt=dt,
        odt=odt,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


def _choice(name: str, value: str, allowed: Sequence[str]) -> str:
    if value not in allowed:
        raise ValueError(f"{name} must be one of {', '.join(allowed)}, not {value!r}")
    return value


def _finite(name: str, value: float) -> float:
    v = float(value)
    if not np.isfinite(v):
        raise ValueError(f"{name} must be a finite number, not {value!r}")
    return v


def _whole(name: str, value: int) -> int:
    if int(value) != value:
        raise ValueError(f"{name} takes a whole number, not {value!r}")
    return int(value)


# --- binary operations ---------------------------------------------------------------------------


def arithmetic(
    image: Any,
    operation: str,
    operand: Any,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """fslmaths' binary operations: ``image`` combined voxel by voxel with ``operand``, a number
    or an image (``-add``, ``-sub``, ``-mul``, ``-div``, ``-rem``, ``-max``, ``-min``; the
    friendly names of :data:`BINARY`, or fslmaths' own).

    An image operand has ``image``'s size, or is 3D against a 4D ``image`` (cloned over its
    volumes). niworkflows combines two masks of each BOLD reference as
    ``arithmetic(mask_a, "multiply", mask_b)`` (``fslmaths <a> -mul <b> <out>``); fMRIPrep's
    goodvoxels mask divides and subtracts images and numbers the same way.
    """
    flags = set(BINARY.values())
    op = operation if operation in flags else BINARY.get(operation)
    if op is None:
        raise ValueError(
            f"unknown operation {operation!r}; use one of {sorted(BINARY)} or fslmaths' "
            f"{sorted(flags)}"
        )
    return _typed(image, [op, operand], dt, odt, n_threads, implementation, upstream_defects)


def apply_mask(
    image: Any,
    mask: Any,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-mas``: ``image`` where ``mask`` is above 0, zero elsewhere (fMRIPrep's goodvoxels mask
    keeps its ribbon this way)."""
    return _typed(image, ["-mas", mask], dt, odt, n_threads, implementation, upstream_defects)


# --- thresholds ----------------------------------------------------------------------------------


def threshold(
    image: Any,
    lower: float | None = None,
    upper: float | None = None,
    *,
    of: Literal["value", "robust_range", "positive_robust_range"] = "value",
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """Zero the voxels below ``lower`` and then those above ``upper``.

    With ``of="value"`` (the default) the limits are intensities (``-thr``, ``-uthr``; fMRIPrep's
    goodvoxels mask thresholds its normalised coefficient of variation with ``lower``); with
    ``"robust_range"`` percentages (0-100) of the image's robust range (``-thrp``, ``-uthrp``),
    with ``"positive_robust_range"`` of its positive voxels' (``-thrP``, ``-uthrP``).
    """
    flags = {
        "value": ("-thr", "-uthr"),
        "robust_range": ("-thrp", "-uthrp"),
        "positive_robust_range": ("-thrP", "-uthrP"),
    }
    _choice("of", of, list(flags))
    if lower is None and upper is None:
        raise ValueError("threshold needs lower, upper or both")
    ops: list[Any] = []
    if lower is not None:
        ops += [flags[of][0], _finite("lower", lower)]
    if upper is not None:
        ops += [flags[of][1], _finite("upper", upper)]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


# --- unary operations ----------------------------------------------------------------------------


def unary(
    image: Any,
    operation: str,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """fslmaths' basic unary operations (:data:`UNARY`, with or without the ``-``): the maths
    functions (``exp``, ``log``, the trigonometric ones, ``sqr``, ``sqrt``, ``recip``, ``abs``),
    binarisation (``bin``, ``binv``), hole filling (``fillh``, ``fillh26``), ``index``,
    ``edge``, NaN handling (``nan``, ``nanm``) and ``range`` (the display range set to the data's).
    """
    name = _choice("operation", operation.removeprefix("-"), UNARY)
    return _typed(image, [f"-{name}"], dt, odt, n_threads, implementation, upstream_defects)


def binarise(
    image: Any,
    *,
    invert: bool = False,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-bin``: 1 where ``image`` is above 0, else 0 (``invert``: ``-binv``, the opposite).
    fMRIPrep's goodvoxels mask binarises the BOLD series' mean this way."""
    op = "-binv" if invert else "-bin"
    return _typed(image, [op], dt, odt, n_threads, implementation, upstream_defects)


def add_noise(
    image: Any,
    distribution: Literal["uniform", "gaussian"] = "uniform",
    *,
    seed: int | None = None,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-rand`` (uniform noise on [0, 1)) or ``-randn`` (Gaussian noise, mean 0 and standard
    deviation 1) added to ``image``, after ``-seed <seed>`` when ``seed`` is given (fslmaths'
    generator is otherwise seeded as FSL seeds it)."""
    op = {"uniform": "-rand", "gaussian": "-randn"}.get(distribution)
    if op is None:
        raise ValueError(f"distribution must be 'uniform' or 'gaussian', not {distribution!r}")
    ops: list[Any] = [] if seed is None else ["-seed", _whole("seed", seed)]
    return _typed(image, [*ops, op], dt, odt, n_threads, implementation, upstream_defects)


def grid(
    image: Any,
    value: float,
    spacing: int,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-grid <value> <spacing>``: a 3D grid of intensity ``value`` drawn into ``image``: every
    voxel on each ``spacing``-th plane along x, y or z set to ``value``."""
    ops = ["-grid", _finite("value", value), _whole("spacing", spacing)]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def dog_edge(
    image: Any,
    sigma1: float,
    sigma2: float,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-dog_edge <sigma1> <sigma2>``: the zero crossings of a difference-of-Gaussians filter
    (sigmas in voxels, in each slice's plane; typically 1.0 and 1.6)."""
    ops = ["-dog_edge", _finite("sigma1", sigma1), _finite("sigma2", sigma2)]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def normalise_intensity(
    image: Any,
    mean: float,
    *,
    per_volume: bool = True,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """Intensity normalisation to ``mean``: of each 3D volume (``-inm``, ``per_volume``) or of
    the 4D image's global mean (``-ing``)."""
    ops = ["-inm" if per_volume else "-ing", _finite("mean", mean)]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


# --- kernels and spatial filters -----------------------------------------------------------------


def _kernel(kernel: Any) -> list[Any]:
    """``-kernel <type> [numbers | file]`` from ``"3D"``, ``"2D"``, or a tuple ``(type,
    *numbers)`` (``("gauss", 2.0)``, ``("boxv3", 3, 3, 1)``) or ``("file", image)``."""
    parts = [kernel] if isinstance(kernel, str) else list(kernel)
    if not parts:
        raise ValueError("a kernel is a type and its numbers, e.g. ('gauss', 2.0)")
    kind = _choice("the kernel's type", parts[0], list(_KERNELS))
    if len(parts) - 1 != _KERNELS[kind]:
        raise ValueError(
            f"the {kind!r} kernel takes {_KERNELS[kind]} argument(s), not {len(parts) - 1}"
        )
    if kind == "file":
        return ["-kernel", "file", parts[1]]
    if kind in ("boxv", "boxv3"):
        return ["-kernel", kind, *(_whole("the kernel's size", p) for p in parts[1:])]
    return ["-kernel", kind, *(_finite("the kernel's size", p) for p in parts[1:])]


def spatial_filter(
    image: Any,
    operation: str,
    *,
    kernel: Any = None,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """fslmaths' spatial filters (:data:`SPATIAL_FILTERS`, with or without the ``-``) with
    ``kernel`` (``-kernel``; ``None``: fslmaths' default 3 x 3 x 3 box): mean (``dilM``), modal
    (``dilD``) and maximum (``dilF``) dilation, ``dilall`` (``dilM`` until the field of view is
    filled), erosion (``ero``) and minimum filtering (``eroF``), median (``fmedian``) and
    kernel-weighted mean filtering (``fmean``, ``fmeanu`` un-normalised).

    ``kernel`` is ``"3D"``, ``"2D"``, or a tuple of a type and its numbers: ``("box", mm)``,
    ``("boxv", voxels)``, ``("boxv3", x, y, z)``, ``("gauss", sigma_mm)``, ``("sphere",
    radius_mm)``, ``("file", image)``. fMRIPrep's goodvoxels mask dilates by the mode (``"dilD"``)
    after smoothing.
    """
    name = _choice("operation", operation.removeprefix("-"), SPATIAL_FILTERS)
    ops = [] if kernel is None else _kernel(kernel)
    return _typed(image, [*ops, f"-{name}"], dt, odt, n_threads, implementation, upstream_defects)


def smooth(
    image: Any,
    sigma: float,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-s <sigma>``: mean filtering with a Gaussian kernel of ``sigma`` mm (fMRIPrep's
    goodvoxels mask smooths with 5)."""
    ops = ["-s", _finite("sigma", sigma)]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def subsample2(
    image: Any,
    *,
    centred: bool = True,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-subsamp2``: ``image`` downsampled by 2, its new voxels centred on the old
    (``centred=False``: ``-subsamp2offc``, not centred)."""
    op = "-subsamp2" if centred else "-subsamp2offc"
    return _typed(image, [op], dt, odt, n_threads, implementation, upstream_defects)


# --- dimensionality reduction, statistics, multi-argument operations -------------------------------


def collapse(
    image: Any,
    statistic: str,
    *,
    axis: Literal["T", "X", "Y", "Z"] = "T",
    percentile: float | None = None,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """fslmaths' dimensionality reductions: ``statistic`` (:data:`COLLAPSE`) across ``axis``
    (``-Tmean``, ``-Xmax``, ...): the mean, standard deviation, maximum, the index of the maximum
    (``maxn``), minimum, median, the ``percentile`` (0-100 of the full range: ``perc``) and the
    AR(1) coefficient (``ar1``). fMRIPrep's goodvoxels mask starts from the BOLD series'
    ``collapse(bold, "std")`` and ``collapse(bold, "mean")``.
    """
    name = _choice("statistic", statistic, COLLAPSE)
    _choice("axis", axis, ["T", "X", "Y", "Z"])
    ops: list[Any] = [f"-{axis}{name}"]
    if name == "perc":
        if percentile is None:
            raise ValueError("the 'perc' statistic needs percentile (0-100)")
        ops.append(_finite("percentile", percentile))
    elif percentile is not None:
        raise ValueError("percentile is the 'perc' statistic's argument")
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def statistical(
    image: Any,
    operation: str,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """fslmaths' statistical transforms (:data:`STATISTICAL`, with or without the ``-``):
    nonparametric uncorrected P values with the volumes as permutations, the first the actual
    statistic (``pval``; ``pval0`` with zeros as missing data; ``cpval`` FWE-corrected), Z to P
    (``ztop``) and P to Z (``ptoz``), ranks over time (``rank``) and their normal transform
    (``ranknorm``)."""
    name = _choice("operation", operation.removeprefix("-"), STATISTICAL)
    return _typed(image, [f"-{name}"], dt, odt, n_threads, implementation, upstream_defects)


def roi(
    image: Any,
    x: tuple[int, int] = (0, -1),
    y: tuple[int, int] = (0, -1),
    z: tuple[int, int] = (0, -1),
    t: tuple[int, int] = (0, -1),
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-roi``: zero outside a region given per axis as ``(first voxel, size)`` in voxel
    coordinates; a size of -1 is the image's whole extent along that axis."""
    ops: list[Any] = ["-roi"]
    for name, pair in (("x", x), ("y", y), ("z", z), ("t", t)):
        if len(pair) != 2:
            raise ValueError(f"{name} is (first voxel, size), not {pair!r}")
        ops += [_whole(name, pair[0]), _whole(name, pair[1])]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def bandpass_temporal_filter(
    image: Any,
    highpass_sigma: float,
    lowpass_sigma: float,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-bptf <hp_sigma> <lp_sigma>``: temporal band-pass filtering, a nonlinear high-pass and a
    Gaussian linear low-pass with sigmas in volumes (a negative sigma skips that filter)."""
    ops = [
        "-bptf",
        _finite("highpass_sigma", highpass_sigma),
        _finite("lowpass_sigma", lowpass_sigma),
    ]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def _tfce_settings(min_t: float | None, delta: float | None) -> list[Any]:
    ops: list[Any] = []
    if min_t is not None:
        ops += ["-tfce_minT", _finite("min_t", min_t)]
    if delta is not None:
        ops += ["-tfce_delta", _finite("delta", delta)]
    return ops


def tfce(
    image: Any,
    h: float = 2.0,
    e: float = 0.5,
    connectivity: Literal[6, 18, 26] = 6,
    *,
    min_t: float | None = None,
    delta: float | None = None,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-tfce <H> <E> <connectivity>``: threshold-free cluster enhancement (e.g. 2, 0.5, 6; 26
    for skeletons), with ``-tfce_minT`` and ``-tfce_delta`` before it when ``min_t`` and
    ``delta`` are given (the lowest height and the step; by default 0 and a hundredth of the
    image's maximum)."""
    ops = [
        *_tfce_settings(min_t, delta),
        "-tfce",
        _finite("h", h),
        _finite("e", e),
        _whole("connectivity", connectivity),
    ]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def tfce_support(
    image: Any,
    voxel: tuple[int, int, int],
    tfce_threshold: float,
    h: float = 2.0,
    e: float = 0.5,
    connectivity: Literal[6, 18, 26] = 6,
    *,
    min_t: float | None = None,
    delta: float | None = None,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``-tfceS <H> <E> <connectivity> <X> <Y> <Z> <tfce_thresh>``: the support area of TFCE at
    ``voxel`` (voxel coordinates) for the enhanced value ``tfce_threshold``."""
    if len(voxel) != 3:
        raise ValueError(f"voxel is (x, y, z), not {voxel!r}")
    ops = [
        *_tfce_settings(min_t, delta),
        "-tfceS",
        _finite("h", h),
        _finite("e", e),
        _whole("connectivity", connectivity),
        *(_whole("voxel", v) for v in voxel),
        _finite("tfce_threshold", tfce_threshold),
    ]
    return _typed(image, ops, dt, odt, n_threads, implementation, upstream_defects)


def roc(
    image: Any,
    truth: Any,
    aroc_threshold: float = 0.05,
    *,
    noise: Any = None,
    dt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> RocResult:
    """``-roc``: ``image`` tested against ``truth`` (normally binary; negative voxels are left
    out) in an ROC analysis, the area under the curve up to the false-positive rate
    ``aroc_threshold`` (usually 0.05).

    With ``noise`` (a 4D noise-only image) the false-positive rate is the fraction of its volumes
    with any false positive (the family-wise rate; fslmaths' positive threshold); without it the
    rate is the voxelwise one over ``truth``'s zero voxels, averaged over the volumes (its
    negative threshold). The true-positive rate is the mean fraction of ``truth``'s positive
    voxels found.
    """
    limit = _finite("aroc_threshold", aroc_threshold)
    if limit <= 0:
        raise ValueError(f"aroc_threshold must be positive, not {aroc_threshold!r}")
    with tempfile.TemporaryDirectory(prefix="larmorx-fslmaths-") as tmp:
        curve_file = os.path.join(tmp, "lx-roc.txt")
        if noise is None:
            ops: list[Any] = ["-roc", -limit, curve_file, truth]
        else:
            ops = ["-roc", limit, curve_file, noise, truth]
        ran = _run(
            image,
            ops,
            dt=dt,
            odt=None,
            n_threads=n_threads,
            implementation=implementation,
            upstream_defects=upstream_defects,
            tmp=tmp,
        )
        words = ran.stdout.split()
        if len(words) < 3:
            raise _replica.ReplicaError(
                f"fslmaths -roc printed no area: {ran.stdout!r}",
                command=["fslmaths", "-roc"],
                returncode=0,
                stderr="",
                stdout=ran.stdout,
            )
        rows = np.loadtxt(curve_file, ndmin=2) if os.path.getsize(curve_file) else np.empty((0, 3))
        return RocResult(
            aroc=float(words[-3]),
            aroc_quartiles=(float(words[-2]), float(words[-1])),
            curve=rows,
            image=ran.image,
        )


def tensor_decomp(
    image: Any,
    *,
    dt: DataType | None = None,
    odt: DataType | None = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> TensorDecomposition:
    """``-tensor_decomp``: a 4D tensor image of six volumes (the upper triangle, xx xy xz yy yz
    zz) decomposed into its eigenvalues, fractional anisotropy, mean diffusivity, mode and
    eigenvectors (:class:`TensorDecomposition`; the outputs ``<output>_L1`` ... ``_V3`` fslmaths
    writes beside its output, which is the FA)."""
    with tempfile.TemporaryDirectory(prefix="larmorx-fslmaths-") as tmp:
        ran = _run(
            image,
            ["-tensor_decomp"],
            dt=dt,
            odt=odt,
            n_threads=n_threads,
            implementation=implementation,
            upstream_defects=upstream_defects,
            tmp=tmp,
        )
        impl = ran.image.implementation
        assert impl is not None

        def read(suffix: str) -> Image:
            return _read_image(os.path.join(tmp, f"{_OUT}_{suffix}.nii"), n_threads, impl)

        return TensorDecomposition(
            image=ran.image,
            l1=read("L1"),
            l2=read("L2"),
            l3=read("L3"),
            fa=read("FA"),
            md=read("MD"),
            mo=read("MO"),
            v1=read("V1"),
            v2=read("V2"),
            v3=read("V3"),
        )
