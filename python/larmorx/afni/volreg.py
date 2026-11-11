# SPDX-License-Identifier: Apache-2.0
"""``3dvolreg``: rigid-body (6-parameter) registration of every volume of a series to a base
volume, as AFNI 26.3.05's program does.

``implementation="auto"`` (the default) and ``"replica"`` run the bit-exact replica (part of
larmorx, in this process), which gives AFNI's registered series, parameters, matrices and
displacements bit for bit (``docs/validation/afni-volreg-replica.md``); ``"clean-room"`` runs the
clean-room original (Apache-2.0, written from ``specs/3dvolreg.md``, built for speed), which
gives AFNI's results within the thresholds of the spec (§14.3).

AFNI's deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``):
``coarse_rotation=False`` turns the coarse rotation search off, as documented (AFNI reads
``-nocoarserot`` as ``-noclip``: AFNI-86), byte data are rounded, not stretched (AFNI-87),
``maxdisp=False`` does not change the registration (AFNI-92), an empty coarse-search region
is not taken (AFNI-93), and those of AFNI's dataset reading and writing (AFNI-01, -02, -07,
-16, -23). ``upstream_defects="reproduce"`` gives AFNI's output.

AFNI's environment variables are settings of the call (``afni_settings``), never read from the
process environment (CLAUDE.md rule 6); the command line reads them from it, as AFNI does."""

from __future__ import annotations

import math
import os
import warnings
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.afni import _common
from larmorx.image import Image

__all__ = ["INTERPOLATIONS", "VolregResult", "fmriprep_volreg_options", "volreg"]

#: The interpolation methods of the registration (``-Fourier``, ``-heptic``, ...) and of the
#: final resampling (``-final``), as 3dvolreg names them.
INTERPOLATIONS = ("Fourier", "heptic", "quintic", "cubic", "linear", "NN")

#: The AFNI variables 3dvolreg reads (``environ::VOLREG``): the datasets' reading and writing,
#: the weight's edges and the Gaussian blurs.
_APPLIED = _common.SETTINGS_APPLIED | {
    "AFNI_VOLREG_EDGING",
    "AFNI_BLUR_FIRFAC",
    "AFNI_BLUR_FIROLD",
}
#: Variables 3dvolreg accepts that change nothing :func:`volreg` returns.
_NO_EFFECT = _common.SETTINGS_NO_EFFECT | {"AFNI_BLUR_INTS_AS_OLD", "AFNI_ROTA_ZPAD"}


@dataclass(frozen=True)
class VolregResult:
    """What :func:`volreg` computed (one row per volume).

    Attributes
    ----------
    registered
        The registered series, on the input's grid (or ``gridparent``'s), in the input's
        storage type (float32 with ``float_output=True`` or a scaled input).
    parameters
        ``(n, 6)`` float64: roll, pitch, yaw (degrees, about the I, R and A axes) and dS, dL,
        dP (mm), as ``-1Dfile`` writes them (4 decimals).
    matrices
        ``(n, 12)`` float64: each volume's affine, base to input in DICOM coordinates, row by
        row (``-1Dmatrix_save``, 6 significant digits): the matrix fMRIPrep reads.
    rms
        ``(n, 2)`` float64: the root mean square difference from the base before and after
        registration (``-dfile``'s last two columns; 0 with ``rotparent``).
    max_displacement, max_delta_displacement
        ``(n,)`` float64: the largest displacement of the automask's boundary voxels, and its
        largest change from the previous volume (mm, ``-maxdisp1D``, 3 decimals); ``None``
        with ``maxdisp=False`` or no automask.
    displacements
        With ``save_displacements=True``: each voxel's displacement along the dataset's x, y
        and z axes (mm, float32 series, ``-savedisp``); else ``None``.
    """

    registered: Image
    parameters: np.ndarray
    matrices: np.ndarray
    rms: np.ndarray
    max_displacement: np.ndarray | None = None
    max_delta_displacement: np.ndarray | None = None
    displacements: tuple[Image, Image, Image] | None = None


def volreg(
    image: Any,
    *,
    base: int | Any = 0,
    interpolation: Literal["Fourier", "heptic", "quintic", "cubic", "linear"] = "heptic",
    final: Literal["Fourier", "heptic", "quintic", "cubic", "linear", "NN"] | None = None,
    clip: bool = True,
    zpad: int = 4,
    twopass: bool = False,
    twoblur: float = 2.0,
    coarse: tuple[int, int] = (10, 2),
    coarse_rotation: bool = True,
    sinit: bool = True,
    weight: Any = None,
    weight_index: int = 0,
    weight_input: bool = False,
    edging: float | str | None = None,
    max_iterations: int = 23,
    x_threshold: float = 0.01,
    rot_threshold: float = 0.02,
    delta: float = 0.70,
    tshift: int | None = None,
    float_output: bool = False,
    maxdisp: bool = True,
    save_displacements: bool = False,
    rotparent: str | os.PathLike[str] | None = None,
    gridparent: str | os.PathLike[str] | None = None,
    twodup: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    afni_settings: Mapping[str, str] | None = None,
) -> VolregResult:
    """Registers every volume of a series to a base volume, as AFNI's ``3dvolreg`` does.

    Each volume is moved by a rotation and a shift (6 parameters) found by iterated linearised
    weighted least squares; ``twopass=True`` first aligns blurred copies, from a coarse search
    over shifts and rotations, and starts the final pass there (fMRIPrep's call:
    :func:`fmriprep_volreg_options`).

    Parameters
    ----------
    image
        The series: a path to a NIfTI file (read with AFNI's rules) or an in-memory image
        (:class:`~larmorx.Image`, nibabel image, ``(array, affine)``), 4D (or 3D with an
        external ``base``).
    base
        The base volume: an index into the series (``-base n``, default 0), or another
        dataset (path or image) whose first volume is the base (``-base dset``).
    interpolation
        The interpolation of the registration (``-Fourier``, ``-heptic``, ``-quintic``,
        ``-cubic``, ``-linear``; default heptic).
    final
        The interpolation of the registered output (``-final``); default ``interpolation``.
    clip
        Clip each registered volume to its input's range (``-clipit``, the default;
        ``False`` is ``-noclip``).
    zpad
        Zero padding (voxels on every face) during rotations (``-zpad``, default 4).
    twopass, twoblur, coarse, coarse_rotation, sinit
        ``-twopass``; its blur (``-twoblur``, at least 2 voxels); its coarse search's step and
        steps (``-coarse del num``); ``coarse_rotation=False`` is ``-nocoarserot`` (AFNI-86:
        with ``upstream_defects="reproduce"`` it turns the clipping off instead, as AFNI does);
        ``sinit=False`` is ``-sinit`` (no scale initialisation).
    weight, weight_index
        A weight dataset (path or image) and its volume (``-weight 'wset[n]'``).
    weight_input
        Weight by the input's first volume (``-wtinp``).
    edging
        The weight's zeroed edge: voxels (a number) or a percentage (``"5%"``; ``-edging``);
        default 5% (or ``AFNI_VOLREG_EDGING`` in ``afni_settings``).
    max_iterations, x_threshold, rot_threshold, delta
        ``-maxite``, ``-x_thresh`` (voxels), ``-rot_thresh`` (degrees), ``-delta`` (voxels).
    tshift
        Slice-time correct the input first, ignoring this many volumes (``-tshift``; the
        input's slice times).
    float_output
        Store the registered series as float32 (``-float``).
    maxdisp
        Compute the maximum displacements (``-maxdisp``, the default; ``False`` is
        ``-nomaxdisp``).
    save_displacements
        Also return each voxel's displacements (``-savedisp``).
    rotparent, gridparent
        ``-rotparent``: apply the transformation a previous 3dvolreg stored in this dataset
        (an output of AFNI's own 3dvolreg, which keeps its ``VOLREG_*`` attributes) on top of
        each volume's; ``-gridparent``: the output's grid (more slices are zero-padded).
    twodup
        With ``twopass`` and an external ``base``: the output gets the base's origin
        (``-twodup``).
    n_threads
        Threads (0 = all); the result does not depend on it.
    implementation
        ``"auto"`` (default) and ``"replica"``: the bit-exact replica; ``"clean-room"``: the
        clean-room original.
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"`` (AFNI's output); a
        :class:`~larmorx.UpstreamDefectWarning` names each defect that changed the result.
    afni_settings
        AFNI's environment variables for this call (as for :func:`larmorx.afni.tshift`, plus
        ``AFNI_VOLREG_EDGING``, ``AFNI_BLUR_FIRFAC``, ``AFNI_BLUR_FIROLD``); the process
        environment is never read.

    Returns
    -------
    VolregResult
        The registered series, the parameters, the matrices, the RMS differences, and the
        displacements asked for; each image's :attr:`~larmorx.Image.implementation` names the
        implementation that ran.

    Raises
    ------
    ValueError
        Invalid arguments; with the original, AFNI's fatal errors (a base with fewer than 100
        non-zero voxels, mismatched grids, ...).
    larmorx.ReplicaError
        The replica ran and failed (AFNI's fatal errors: a base with fewer than 100 non-zero
        voxels, mismatched grids, ...).
    """
    _defects.check(upstream_defects)
    settings = _common.check_settings(
        afni_settings, "lx.afni.volreg", "3dvolreg", applied=_APPLIED, no_effect=_NO_EFFECT
    )
    n_threads = _common.check_threads(n_threads)
    options = _options(
        interpolation, final, clip, zpad, twopass, twoblur, coarse, coarse_rotation, sinit,
        weight_input, edging, max_iterations, x_threshold, rot_threshold, delta, tshift,
        float_output, maxdisp, twodup,
    )  # fmt: skip
    if isinstance(weight_index, bool) or not isinstance(weight_index, int) or weight_index < 0:
        raise ValueError(f"weight_index must be a non-negative int, not {weight_index!r}")
    selected = _replica.select("afni", "3dvolreg", implementation)
    impl = _replica.ORIGINAL if selected is None else selected.implementation()
    with _common.temporary_directory("larmorx-volreg-") as tmp:
        source = _common.write_input(image, tmp, n_threads)
        args = list(options)
        if isinstance(base, bool):
            raise TypeError(f"base must be an index or a dataset, not {base!r}")
        if isinstance(base, int):
            if base < 0:
                raise ValueError(f"base must be a non-negative index, not {base}")
            args += ["-base", str(base)]
        else:
            args += ["-base", _dataset(base, tmp, "base", n_threads)]
        if weight is not None:
            args += ["-weight", f"{_dataset(weight, tmp, 'weight', n_threads)}[{weight_index}]"]
        for flag, value in (("-rotparent", rotparent), ("-gridparent", gridparent)):
            if value is not None:
                if not isinstance(value, (str, os.PathLike)):
                    raise TypeError(f"{flag[1:]} must be a path to a NIfTI file of AFNI's")
                args += [flag, os.path.abspath(os.fspath(value))]
        path = lambda name: os.path.join(tmp, name)  # noqa: E731
        # Relative names for the replica (run in tmp), absolute ones for the original (run in
        # this process).
        out = path if selected is None else (lambda name: name)
        outputs = ["-prefix", out("out.nii"), "-1Dfile", out("p.1D")]
        outputs += ["-1Dmatrix_save", out("m.aff12.1D"), "-dfile", out("d.txt")]
        if maxdisp:
            outputs += ["-maxdisp1D", out("md.1D")]
        if save_displacements:
            outputs += ["-savedisp", out("sd.nii")]
        if selected is None:
            _run_original([*args, *outputs, source], n_threads, upstream_defects, settings)
        else:
            _common.run_replica(
                selected,
                [*args, *outputs, source],
                tmp,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
                settings=settings,
            )
        read = lambda name: _common.read_output(path(name), n_threads, impl)  # noqa: E731
        table = lambda name, cols: np.loadtxt(path(name), comments="#", ndmin=2)[:, cols]  # noqa: E731
        has_md = maxdisp and os.path.exists(path("md.1D"))
        return VolregResult(
            registered=read("out.nii"),
            parameters=table("p.1D", slice(0, 6)),
            matrices=table("m.aff12.1D", slice(0, 12)),
            rms=table("d.txt", slice(7, 9)),
            max_displacement=table("md.1D", 0) if has_md else None,
            max_delta_displacement=table("md_delt.1D", 0) if has_md else None,
            displacements=(
                (read("sd_DX.nii"), read("sd_DY.nii"), read("sd_DZ.nii"))
                if save_displacements
                else None
            ),
        )


def _run_original(
    args: list[str], n_threads: int, upstream_defects: str, settings: dict[str, str]
) -> None:
    """Runs the clean-room original's command line in this process; its warnings become Python
    warnings, a failure a ``ValueError`` with AFNI's message."""
    code, _, stderr = _core.afni_volreg_cli(
        args,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        afni_settings=sorted(settings.items()) or None,
    )
    for line in stderr.splitlines():
        if line.startswith("*+ WARNING: "):
            text = line.removeprefix("*+ WARNING: ")
            warnings.warn(_defects.parse_line(text) or text, stacklevel=4)
    if code != 0:
        errors = [ln for ln in stderr.splitlines() if ln.startswith("**")]
        raise ValueError(
            "3dvolreg (clean-room original) failed: " + (" ".join(errors) or stderr.strip())
        )


def fmriprep_volreg_options() -> dict[str, Any]:
    """The options fMRIPrep's BOLD reference gives 3dvolreg (niworkflows' ``RobustAverage``:
    ``3dvolreg -Fourier -twopass -zpad 4 -1Dfile -1Dmatrix_save -maxdisp1D``; sdcflows'
    PEPOLAR reference averages: ``-Fourier -twopass -zpad 4``), as :func:`volreg`'s keyword
    arguments."""
    return {"interpolation": "Fourier", "twopass": True, "zpad": 4}


def _dataset(value: Any, tmp: str, name: str, n_threads: int) -> str:
    """A path for a dataset argument: a file as given, an in-memory image written to ``tmp``."""
    if isinstance(value, (str, os.PathLike)):
        return os.path.abspath(os.fspath(value))
    sub = os.path.join(tmp, name)
    os.makedirs(sub, exist_ok=True)
    return _common.write_input(value, sub, n_threads)


def _options(
    interpolation: str,
    final: str | None,
    clip: bool,
    zpad: int,
    twopass: bool,
    twoblur: float,
    coarse: tuple[int, int],
    coarse_rotation: bool,
    sinit: bool,
    weight_input: bool,
    edging: float | str | None,
    max_iterations: int,
    x_threshold: float,
    rot_threshold: float,
    delta: float,
    tshift: int | None,
    float_output: bool,
    maxdisp: bool,
    twodup: bool,
) -> list[str]:
    """3dvolreg's options for :func:`volreg`'s arguments, checked before anything runs."""
    if interpolation not in INTERPOLATIONS[:5]:
        raise ValueError(
            f"interpolation must be one of {INTERPOLATIONS[:5]}, not {interpolation!r}"
        )
    args = [f"-{interpolation}"]
    if final is not None:
        if final not in INTERPOLATIONS:
            raise ValueError(f"final must be one of {INTERPOLATIONS} or None, not {final!r}")
        args += ["-final", final]
    if not clip:
        args.append("-noclip")
    for name, value in (("zpad", zpad), ("max_iterations", max_iterations)):
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise ValueError(f"{name} must be a non-negative int, not {value!r}")
    args += ["-zpad", str(zpad)]
    if max_iterations < 1:
        raise ValueError(f"max_iterations must be positive, not {max_iterations}")
    for name, value in (
        ("x_threshold", x_threshold),
        ("rot_threshold", rot_threshold),
        ("delta", delta),
    ):
        if not (isinstance(value, (int, float)) and math.isfinite(value) and value > 0):
            raise ValueError(f"{name} must be a positive number, not {value!r}")
    args += ["-maxite", str(max_iterations), "-x_thresh", repr(float(x_threshold))]
    args += ["-rot_thresh", repr(float(rot_threshold)), "-delta", repr(float(delta))]
    if twopass:
        if not (isinstance(twoblur, (int, float)) and math.isfinite(twoblur) and twoblur >= 2):
            raise ValueError(f"twoblur must be at least 2 voxels, not {twoblur!r}")
        # -twopass raises a -maxite below 10 to 66, as AFNI does.
        args += ["-twopass", "-twoblur", repr(float(twoblur))]
    c = tuple(coarse)
    if len(c) != 2 or any(isinstance(v, bool) or not isinstance(v, int) or v < 0 for v in c):
        raise ValueError(f"coarse must be two non-negative ints (step, steps), not {coarse!r}")
    args += ["-coarse", str(c[0]), str(c[1])]
    if not coarse_rotation:
        args.append("-nocoarserot")
    if not sinit:
        args.append("-sinit")
    if weight_input:
        args.append("-wtinp")
    if edging is not None:
        text = edging if isinstance(edging, str) else repr(float(edging))
        value = text.removesuffix("%")
        try:
            number = float(value)
        except ValueError:
            raise ValueError(f"edging must be a number of voxels or 'p%', not {edging!r}") from None
        if number < 0 or (text.endswith("%") and number > 25):
            raise ValueError(f"edging must be 0 or more voxels, or 0% to 25%, not {edging!r}")
        args += ["-edging", text]
    if tshift is not None:
        if isinstance(tshift, bool) or not isinstance(tshift, int) or tshift < 0:
            raise ValueError(
                f"tshift must be a non-negative int (volumes to ignore), not {tshift!r}"
            )
        args += ["-tshift", str(tshift)]
    if float_output:
        args.append("-float")
    if not maxdisp:
        args.append("-nomaxdisp")
    if twodup:
        args.append("-twodup")
    return args
