# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' PEPOLAR estimation (``init_topup_wf``) and its steps, as plain functions.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers) and, for fMRIPrep 25.2.x's
wiring, sdcflows 2.15.1/2.15.2 (``workflows/fit/pepolar.py``, identical in both): ``get_trt``
(``utils/epimanip.py``), ``GetReadoutTime``, ``SelectPEVolumes`` (2.17.1), ``SortPEBlips``
(``interfaces/epi.py``), ``UniformGrid``, ``ReorientImageAndMetadata``, ``PadSlices``
(``interfaces/utils.py``), ``_select_topup_config`` and ``_topup_mats_to_world``; nipype 1.12.0's
``TOPUP`` command line and encoding file (Apache-2.0); nitransforms 25.1.0's resampling and FSL
matrices (MIT). Each node runs as nipype runs it, writing its files where nipype's node
directory holds them, through larmorx's ports: niworkflows' ``RobustAverage``, ``MergeSeries``
and ``ReorientImage`` (:mod:`larmorx.niworkflows`, AFNI's ``3dvolreg`` through
:mod:`larmorx.afni`), FSL's ``topup`` (``larmorx fsl topup``: the replica when ``larmorx-nc``
is installed), part A's ``TOPUPCoeffReorient`` and ``ApplyCoeffsField``, and
:func:`larmorx.sdcflows.brainextraction_workflow`.

**Two wirings** (``wiring``): ``"2.17"`` (sdcflows 2.17.1, the default: at most
``max_vols_per_pe`` volumes per phase-encoding direction, regridded, merged, reoriented to LAS,
``topup`` with FSL's ``b02b0_4/_2/_1.cnf`` chosen by the grid's divisibility) and ``"2.15"``
(sdcflows 2.15, fMRIPrep 25.2.x's: each run averaged with ``RobustAverage``, regridded, merged,
an odd slice count padded, the set averaged again with head-motion correction, ``topup`` with
sdcflows' own ``b02b0.cnf``).

**FSL's configurations** (the user's decisions D9 and of 2026-11-09): the main package ships
no FSL file. sdcflows' own configurations (``b02b0.cnf``, ``b02b0_quick.cnf``,
``b02b0_{1,2,4}_quick.cnf``, Apache-2.0) are in ``larmorx/sdcflows/data/flirtsch/``; FSL's
``b02b0_1/2/4.cnf`` are read from ``fsldir``'s ``etc/flirtsch``, else from the copy ``larmorx-nc``
carries (FSL 6.0.7.23's files, byte for byte), else refused (:func:`topup_config_file`). The main
package's own settings, a clean-room original from ``specs/topup.md``, come with ``t2smap``'s
clean-room part; this lookup is the one place that changes then.

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows' behaviour):

- SDC-12: ``ReorientImageAndMetadata`` checks the upper-cased orientation code but reorients to
  the code as given, so a lower-case code (``"las"``) fails in nibabel; larmorx uses the
  upper-cased code it checked.
"""

from __future__ import annotations

import os
import sys
import warnings
from collections import defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _nipype, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import load_nibabel
from larmorx.io.nibabel_writer import save_nibabel

__all__ = [
    "PEPOLAR_METHOD",
    "SDCFLOWS_CONFIGS",
    "GetReadoutTimeResult",
    "SelectPEVolumesResult",
    "SortPEBlipsResult",
    "TopupNodeResult",
    "TopupWorkflowResult",
    "UniformGridResult",
    "get_readout_time",
    "get_trt",
    "pad_slices",
    "reorient_image_and_metadata",
    "select_pe_volumes",
    "select_topup_config",
    "sort_pe_blips",
    "topup_config_file",
    "topup_mats_to_world",
    "topup_node",
    "topup_workflow",
    "uniform_grid",
]

#: ``init_topup_wf``'s ``method`` output.
PEPOLAR_METHOD = "PEB/PEPOLAR (phase-encoding based / PE-POLARity)"
#: sdcflows' own ``topup`` configurations (Apache-2.0), shipped with larmorx.
SDCFLOWS_CONFIGS = (
    "b02b0.cnf",
    "b02b0_quick.cnf",
    "b02b0_1_quick.cnf",
    "b02b0_2_quick.cnf",
    "b02b0_4_quick.cnf",
)
#: FSL's configurations sdcflows 2.17.1 picks by the grid (FSL Licence: never shipped here).
FSL_CONFIGS = ("b02b0_1.cnf", "b02b0_2.cnf", "b02b0_4.cnf")
_DATA = Path(__file__).resolve().parent / "data" / "flirtsch"
_LAS = "LAS"


# ------------------------------------------------------------------------------------------
# get_trt and GetReadoutTime


def _shape(in_file: Any) -> tuple[int, ...]:
    """``nb.load(in_file).shape`` (nibabel's ``TypeError`` for ``None``)."""
    if in_file is None:
        raise TypeError(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, "
            "not 'NoneType'"
        )
    if isinstance(in_file, Image):
        return tuple(int(n) for n in in_file.shape)
    from larmorx.io.nifti import read_header

    return tuple(int(n) for n in read_header(os.fspath(in_file)).shape)


def get_trt(
    in_meta: Mapping[str, Any],
    in_file: Any = None,
    *,
    use_estimate: bool = False,
    fallback: Any = None,
) -> Any:
    """sdcflows' ``get_trt``: the total readout time of an EPI from its BIDS metadata.

    ``TotalReadoutTime`` (or, with ``use_estimate``, ``EstimatedTotalReadoutTime``) is returned
    as given (a falsy value is a ``ValueError`` naming it); otherwise, when the metadata has
    ``PhaseEncodingDirection`` and one of ``EffectiveEchoSpacing``,
    ``EstimatedEffectiveEchoSpacing``, ``EchoSpacing``, ``WaterFatShift``, the number of voxels
    along the phase encoding (``in_file``'s shape) gives ``EES * (N - 1)``,
    ``EchoSpacing * (N // ParallelReductionFactorInPlane - 1)`` or Philips'
    ``WaterFatShift / (wfs_hz * (EPIFactor + 1)) * (N - 1)``; then ``fallback`` when truthy;
    else ``ValueError('Unknown total-readout time specification')``."""
    if "TotalReadoutTime" in in_meta:
        trt = in_meta.get("TotalReadoutTime")
        if not trt:
            raise ValueError(f"'{trt}'")
        return trt
    elif use_estimate and "EstimatedTotalReadoutTime" in in_meta:
        trt = in_meta.get("EstimatedTotalReadoutTime")
        if not trt:
            raise ValueError(f"'{trt}'")
        return trt

    keys = {
        "PhaseEncodingDirection",
        "EffectiveEchoSpacing",
        "EstimatedEffectiveEchoSpacing",
        "EchoSpacing",
        "WaterFatShift",
    }
    if in_meta.keys() & keys > {"PhaseEncodingDirection"}:
        pe_index = "ijk".index(in_meta["PhaseEncodingDirection"][0])
        npe = _shape(in_file)[pe_index]
        ees = in_meta.get("EffectiveEchoSpacing")
        if ees:
            return ees * (npe - 1)
        elif use_estimate and "EstimatedEffectiveEchoSpacing" in in_meta:
            return in_meta.get("EstimatedEffectiveEchoSpacing") * (npe - 1)
        try:
            echospacing = in_meta["EchoSpacing"]
            acc_factor = in_meta["ParallelReductionFactorInPlane"]
        except KeyError:
            pass
        else:
            etl = npe // acc_factor
            return echospacing * (etl - 1)
        try:
            wfs = in_meta["WaterFatShift"]
            epifactor = in_meta["EPIFactor"]
        except KeyError:
            pass
        else:
            wfs_hz = (
                (in_meta.get("ImagingFrequency", 0) * 3.39941)
                or (in_meta.get("MagneticFieldStrength", 0) * 144.7383333)
                or None
            )
            if wfs_hz:
                ees = wfs / (wfs_hz * (epifactor + 1))
                return ees * (npe - 1)
    if fallback:
        return fallback
    raise ValueError("Unknown total-readout time specification")


@dataclass(frozen=True)
class GetReadoutTimeResult:
    """``GetReadoutTime``'s outputs: the total readout time (s) and the phase-encoding
    direction in BIDS' (``i``, ``j-``, ...) and FSL's (``x``, ``y-``, ...) letters."""

    readout_time: float
    pe_direction: str
    pe_dir_fsl: str


def _float_trait(value: Any, name: str, spec: str) -> float:
    """nipype's ``traits.Float`` check of an output of a ``spec`` instance: an int or float
    (bools are ints), else traits' ``TraitError``."""
    if isinstance(value, (bool, int, float, np.integer, np.floating)):
        return float(value)
    raise _nipype.trait_error(spec, name, "a float", value)


def _enum_trait(value: Any, name: str, spec: str, values: tuple[str, ...]) -> Any:
    """nipype's ``traits.Enum`` check of an output."""
    if value in values:
        return value
    raise _nipype.trait_error(spec, name, " or ".join(repr(v) for v in values), value)


def get_readout_time(
    metadata: Mapping[str, Any],
    in_file: Any = None,
    use_estimate: bool = False,
    fallback: float | None = None,
) -> GetReadoutTimeResult:
    """sdcflows' ``GetReadoutTime``: :func:`get_trt` (``fallback`` 0 or ``None`` is no
    fallback) and the metadata's ``PhaseEncodingDirection`` (``KeyError`` when absent), in
    FSL's letters too (``i`` -> ``x``, ``j`` -> ``y``, ``k`` -> ``z``). The outputs are
    checked as nipype checks them (traits' ``TraitError``), in their declared order (nipype's
    order follows the process's string hashes: PIPE-39)."""
    trt = get_trt(metadata, in_file, use_estimate=use_estimate, fallback=fallback or None)
    pe = metadata["PhaseEncodingDirection"]
    fsl = pe.replace("i", "x").replace("j", "y").replace("k", "z")
    spec = "_GetReadoutTimeOutputSpec"
    return GetReadoutTimeResult(
        _float_trait(trt, "readout_time", spec),
        _enum_trait(pe, "pe_direction", spec, ("i", "i-", "j", "j-", "k", "k-")),
        _enum_trait(fsl, "pe_dir_fsl", spec, ("x", "x-", "y", "y-", "z", "z-")),
    )


# ------------------------------------------------------------------------------------------
# SelectPEVolumes, SortPEBlips


@dataclass(frozen=True)
class SelectPEVolumesResult:
    """``SelectPEVolumes``' outputs: the kept volumes (``<in>_vol<i>`` files) with their
    directions and readout times, in input order."""

    out_data: tuple[str, ...]
    pe_dirs_fsl: tuple[str, ...]
    readout_times: tuple[float, ...]


def select_pe_volumes(
    in_data: Sequence[str | os.PathLike[str]],
    pe_dirs_fsl: Sequence[str],
    readout_times: Sequence[float],
    max_vols_per_pe: int = 3,
    *,
    out_dir: str | os.PathLike[str],
    n_threads: int = 1,
) -> SelectPEVolumesResult:
    """sdcflows 2.17.1's ``SelectPEVolumes``: the runs in order, each squeezed and split into
    volumes, keeping at most ``max_vols_per_pe`` volumes per direction (a full direction's later
    volumes and runs dropped); each kept volume written as ``<in>_vol<i>`` in ``out_dir``
    (``ValueError`` when the three lists differ in length, as ``zip(strict=True)``)."""
    from larmorx.niworkflows.images import _squeezed, four_to_three
    from larmorx.sdcflows.approx import _fname_presuffix

    files = [os.fspath(f) for f in in_data]
    dirs, times = list(pe_dirs_fsl), list(readout_times)
    os.makedirs(out_dir, exist_ok=True)
    kept: dict[str, int] = defaultdict(int)
    out: list[str] = []
    out_dirs: list[str] = []
    out_times: list[float] = []
    for in_file, pe_dir, ro in zip(files, dirs, times, strict=True):
        remaining = int(max_vols_per_pe) - kept[pe_dir]
        if remaining <= 0:
            continue
        img = _squeezed(load_nibabel(in_file))
        volumes = four_to_three(img) if np.asarray(img.data).ndim == 4 else [img]
        for i, volume in enumerate(volumes[:remaining]):
            path = _fname_presuffix(in_file, f"_vol{i}", out_dir)
            save_nibabel(volume, path, n_threads=n_threads)
            out.append(path)
            out_dirs.append(pe_dir)
            out_times.append(ro)
        kept[pe_dir] += min(remaining, len(volumes))
    return SelectPEVolumesResult(tuple(out), tuple(out_dirs), tuple(out_times))


@dataclass(frozen=True)
class SortPEBlipsResult:
    """``SortPEBlips``' outputs: the data, directions (FSL's and BIDS' letters) and readout
    times sorted by the blip (``+`` before ``-``), then the readout time, then the path."""

    out_data: tuple[str, ...]
    pe_dirs: tuple[str, ...]
    pe_dirs_fsl: tuple[str, ...]
    readout_times: tuple[float, ...]


def sort_pe_blips(
    in_data: Sequence[str | os.PathLike[str]],
    pe_dirs_fsl: Sequence[str],
    readout_times: Sequence[float],
) -> SortPEBlipsResult:
    """sdcflows' ``SortPEBlips``: the inputs sorted as ``sorted(zip(blips, readout_times,
    in_data))`` with the blips written sign first (``+y``, ``-y``), then the sign put back last;
    ``zip`` stops at the shortest list, as sdcflows' does."""
    blips = [f"+{pe[0]}" if len(pe) == 1 else f"-{pe[0]}" for pe in pe_dirs_fsl]
    srt = sorted(zip(blips, readout_times, [os.fspath(f) for f in in_data], strict=False))
    fsl = [pe[1] if pe.startswith("+") else f"{pe[1]}-" for pe, _, _ in srt]
    bids = [pe.replace("x", "i").replace("y", "j").replace("z", "k") for pe in fsl]
    return SortPEBlipsResult(
        tuple(f for _, _, f in srt), tuple(bids), tuple(fsl), tuple(t for _, t, _ in srt)
    )


# ------------------------------------------------------------------------------------------
# UniformGrid, PadSlices, ReorientImageAndMetadata


@dataclass(frozen=True)
class UniformGridResult:
    """``UniformGrid``'s outputs: one file per input on the reference's grid (the input itself
    when already there; ``<in>_regrid<i>`` otherwise) and the reference."""

    out_data: tuple[str, ...]
    reference: str
    resampled: tuple[str, ...] = ()


def _regridded(nii: Image, ref: Image, n_threads: int) -> np.ndarray:
    """nitransforms' ``Affine(reference=ref).apply(nii)`` for a 3D image without header: the
    data (its own type) sampled at the reference's voxel centres, cubic spline, ``constant``
    0, prefiltered (SciPy's ``map_coordinates``, output in the input's type)."""
    from larmorx.ndimage import map_coordinates

    data = np.asanyarray(nii.data)
    # (an image of other than three dimensions: SciPy's RuntimeError "invalid shape for
    # coordinate array" from map_coordinates, as nitransforms' apply raises it)
    shape3 = tuple(int(n) for n in ref.shape[:3])
    coords = _core.sdcflows_affine_coordinates(
        shape3,
        np.ascontiguousarray(ref.affine, dtype=np.float64),
        np.ascontiguousarray(nii.affine, dtype=np.float64),
        None,
        int(n_threads),
    )
    values = map_coordinates(
        data, coords, order=3, mode="constant", cval=0.0, prefilter=True, n_threads=n_threads
    )
    return np.asarray(values).reshape(shape3, order="F")


def uniform_grid(
    in_data: Sequence[str | os.PathLike[str]],
    reference: int = 0,
    *,
    out_dir: str | os.PathLike[str],
    n_threads: int = 1,
) -> UniformGridResult:
    """sdcflows' ``UniformGrid``: every input on the grid of input ``reference``. An input of
    the same shape whose affine is close (``np.allclose``) is kept when the affine is equal, or
    written with the reference's affine (``<in>_regrid<i>``); any other is resampled onto the
    reference's grid (nitransforms' cubic resampling, :func:`_regridded`) and written with the
    reference's affine and its own header (stored type) as ``<in>_regrid<i>``. sdcflows 2.17.1
    warns when a resampled input's rotation differs from the reference's."""
    from larmorx.niworkflows.images import _constructed
    from larmorx.sdcflows.approx import _fname_presuffix

    files = [os.fspath(f) for f in in_data]
    os.makedirs(out_dir, exist_ok=True)
    retval: list[str | None] = [None] * len(files)
    ref_file = files[reference]
    retval[reference] = ref_file
    refnii = load_nibabel(ref_file)
    refshape = tuple(refnii.shape[:3])
    refaff = np.asarray(refnii.affine, dtype=np.float64)
    resampled: list[str] = []
    for i, fname in enumerate(files):
        if retval[i] is not None:
            continue
        nii = load_nibabel(fname)
        out = _fname_presuffix(fname, f"_regrid{i:03d}", out_dir)
        retval[i] = out
        aff = np.asarray(nii.affine, dtype=np.float64)
        if np.allclose(nii.shape[:3], refshape) and np.allclose(aff, refaff):
            if np.all(aff == refaff):
                retval[i] = fname
            else:
                save_nibabel(
                    _constructed(Image(nii.data, refaff, nii.header), np.asanyarray(nii.data)),
                    out,
                    n_threads=n_threads,
                )
            continue
        if not np.allclose(aff[:3, :3], refaff[:3, :3]):
            resampled.append(fname)
        data = _regridded(Image(np.asanyarray(nii.data), aff, None), refnii, n_threads)
        save_nibabel(_constructed(Image(data, refaff, nii.header), data), out, n_threads=n_threads)
    if resampled:
        warnings.warn(
            f"UniformGrid resampled {len(resampled)} input image(s) onto the reference grid "
            f"because they were not on a common sampling grid: {', '.join(resampled)}. When "
            "feeding TOPUP, the total readout time is referenced to the original acquisition "
            "grid, so resampling opposing phase-encoding runs that differ in matrix size or voxel "
            "spacing may bias the estimated field. Verify that these inputs belong together.",
            stacklevel=2,
        )
    return UniformGridResult(tuple(retval), ref_file, tuple(resampled))  # type: ignore[arg-type]


def pad_slices(
    in_file: str | os.PathLike[str],
    axis: int = 2,
    *,
    out_dir: str | os.PathLike[str],
    n_threads: int = 1,
) -> tuple[str, bool]:
    """sdcflows' ``PadSlices`` (``_pad_num_slices``): an image with an odd number of slices
    along ``axis`` gets one empty slice at the end (``np.pad`` of its values with 0), written
    with its header as ``<in>_padded``; an even count returns the input. ``(file, padded)``."""
    from larmorx.niworkflows.images import _constructed
    from larmorx.sdcflows.approx import _fname_presuffix

    in_file = os.fspath(in_file)
    img = load_nibabel(in_file)
    if img.shape[axis] % 2 == 0:
        return in_file, False
    pwidth = [(0, 0)] * len(img.shape)
    pwidth[axis] = (0, 1)
    padded = np.pad(np.asanyarray(img.data), pwidth)
    out_file = _fname_presuffix(in_file, "_padded", out_dir)
    os.makedirs(out_dir, exist_ok=True)
    save_nibabel(_constructed(img, padded), out_file, n_threads=n_threads)
    return out_file, True


def reorient_image_and_metadata(
    in_file: str | os.PathLike[str],
    target_orientation: str,
    pe_dir: Sequence[str],
    *,
    out_dir: str | os.PathLike[str],
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> tuple[str, tuple[str, ...]]:
    """sdcflows' ``ReorientImageAndMetadata``: the image reoriented to ``target_orientation``
    (``ValueError('Invalid orientation code ...')`` for letters other than RASLPI) with its
    phase-encoding directions (``reorient_pedir``), written as ``<in><TARGET>``; the input and
    its directions when no reorientation is needed. SDC-12 (module docstring)."""
    from larmorx.niworkflows.images import as_reoriented
    from larmorx.niworkflows.nibabel import axcodes2ornt, ornt_transform
    from larmorx.sdcflows.approx import _fname_presuffix
    from larmorx.sdcflows.coeffs import reorient_pedir
    from larmorx.transforms.resample import _io_orientation

    _defects.check(upstream_defects)
    in_file = os.fspath(in_file)
    target = target_orientation.upper()
    if not all(code in "RASLPI" for code in target):
        raise ValueError(f"Invalid orientation code {target_orientation}")
    img = load_nibabel(in_file)
    code = target_orientation if upstream_defects == "reproduce" else target
    notices = []
    if code != target_orientation:
        notices.append(
            (
                "SDC-12",
                "sdcflows",
                "fix",
                f"ReorientImageAndMetadata reorients to the code it checked ({target}); sdcflows "
                f"passes {target_orientation!r} to nibabel, which fails",
            )
        )
    img2target = ornt_transform(
        _io_orientation(np.asarray(img.affine, dtype=np.float64)), axcodes2ornt(list(code))
    ).astype(int)
    _defects.warn(notices)
    if np.array_equal(img2target, [[0, 1], [1, 1], [2, 1]]):
        return in_file, tuple(pe_dir)
    reoriented = as_reoriented(img, img2target)
    pe_dirs = tuple(reorient_pedir(p, img2target) for p in pe_dir)
    out_file = _fname_presuffix(in_file, target, out_dir)
    os.makedirs(out_dir, exist_ok=True)
    save_nibabel(reoriented, out_file, n_threads=n_threads)
    return out_file, pe_dirs


# ------------------------------------------------------------------------------------------
# TOPUP: the configurations, nipype's command line, the matrices


def topup_config_file(name: str, fsldir: str | os.PathLike[str] | None = None) -> str:
    """The path of the ``topup`` configuration ``name`` (D9, in one place): sdcflows' own
    configurations from larmorx's copy (:data:`SDCFLOWS_CONFIGS`); FSL's (:data:`FSL_CONFIGS`)
    from ``fsldir``'s ``etc/flirtsch``, else from ``larmorx-nc``'s copy of FSL's files
    (``share/larmorx-nc/fsl-data/etc/flirtsch``) when it is installed; otherwise
    ``FileNotFoundError`` naming both places. The main package's own settings (a clean-room
    original, the user's decision of 2026-11-09) come with ``t2smap``'s clean-room part."""
    if name in SDCFLOWS_CONFIGS:
        return str(_DATA / name)
    if name not in FSL_CONFIGS:
        raise ValueError(f"not a topup configuration of sdcflows: {name!r}")
    tried = []
    if fsldir is not None:
        p = Path(os.path.expanduser(os.fspath(fsldir))) / "etc" / "flirtsch" / name
        if p.is_file():
            return str(p.resolve())
        tried.append(str(p))
    for root in _nc_data_roots():
        p = root / "etc" / "flirtsch" / name
        if p.is_file():
            return str(p.resolve())
        tried.append(str(p))
    raise FileNotFoundError(
        f"FSL's topup configuration {name} (FSL Licence; larmorx's main package does not ship "
        "it, and its own clean-room settings come with t2smap's clean-room part, "
        "docs/next-steps.md) was not found: give fsldir, an FSL installation whose etc/flirtsch "
        "holds it, or install larmorx-nc, which carries FSL's files (looked in: "
        f"{', '.join(tried) or 'nowhere'})"
    )


def _nc_data_roots() -> list[Path]:
    """Where ``larmorx-nc`` installs FSL's files: ``share/larmorx-nc/fsl-data`` beside its
    program, and under this interpreter's prefix."""
    roots = []
    program = _replica.find("fsl", "topup")
    if program:
        roots.append(Path(program).resolve().parent.parent / "share" / "larmorx-nc" / "fsl-data")
    roots.append(Path(sys.prefix) / "share" / "larmorx-nc" / "fsl-data")
    return list(dict.fromkeys(roots))


def select_topup_config(
    in_file: str | os.PathLike[str],
    sloppy: bool = False,
    *,
    fsldir: str | os.PathLike[str] | None = None,
) -> str:
    """sdcflows 2.17.1's ``_select_topup_config``: ``b02b0_4.cnf`` when every dimension of the
    image is divisible by 4, ``b02b0_2.cnf`` when every one is even, else ``b02b0_1.cnf``
    (their ``_quick`` variants with ``sloppy``), through :func:`topup_config_file`."""
    dims = _shape(os.fspath(in_file))[:3]
    if all(d % 4 == 0 for d in dims):
        tier = "4"
    elif all(d % 2 == 0 for d in dims):
        tier = "2"
    else:
        tier = "1"
    return topup_config_file(f"b02b0_{tier}{'_quick' * bool(sloppy)}.cnf", fsldir)


@dataclass(frozen=True)
class TopupNodeResult:
    """nipype's ``TOPUP`` outputs (``out_enc_file`` the encoding file it wrote; the warps,
    Jacobians and matrices numbered from 01) and the command line it ran."""

    out_fieldcoef: str
    out_movpar: str
    out_enc_file: str
    out_field: str
    out_corrected: str
    out_logfile: str
    out_warps: tuple[str, ...]
    out_jacs: tuple[str, ...]
    out_mats: tuple[str, ...]
    cmdline: tuple[str, ...]


def _encfile(path: str, encoding_direction: Sequence[str], readout_times: Sequence[float]) -> None:
    """nipype's ``TOPUP._generate_encfile``: one row per direction, ``"%d %d %d %.8f"`` of the
    unit vector (sign from a trailing ``-``) and the readout time (one time for every row, or
    one per row: ``ValueError`` otherwise)."""
    durations = list(readout_times)
    if len(encoding_direction) != len(durations):
        if len(durations) != 1:
            raise ValueError(
                "Readout time must be a float or match thelength of encoding directions"
            )
        durations = durations * len(encoding_direction)
    lines = []
    for idx, encdir in enumerate(encoding_direction):
        direction = -1.0 if encdir.endswith("-") else 1.0
        row = [float(val[0] == encdir[0]) * direction for val in ["x", "y", "z"]] + [durations[idx]]
        # np.savetxt formats with "%"
        lines.append("%d %d %d %.8f" % tuple(np.array(row, dtype=np.float64)))  # noqa: UP031
    with open(path, "w", encoding="utf-8") as f:
        f.write("".join(line + "\n" for line in lines))


def topup_node(
    in_file: str | os.PathLike[str],
    encoding_direction: Sequence[str],
    readout_times: Sequence[float],
    config: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str],
    fsldir: str | os.PathLike[str] | None = None,
    implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> TopupNodeResult:
    """sdcflows' ``topup`` node: nipype's ``TOPUP(config=..., in_file, encoding_direction,
    readout_times)`` run in ``out_dir``: ``topup --config=<config> --datain=<out_dir>/<in>
    _encfile.txt --imain=<in> --out=<in>_base --iout=<in>_corrected.nii.gz
    --fout=<in>_field.nii.gz --jacout=jac --logout=<in>_topup.log --rbmout=xfm
    --dfout=warpfield`` with ``FSLOUTPUTTYPE=NIFTI_GZ``, through larmorx's ``topup`` (the
    replica when ``larmorx-nc`` is installed, ``implementation``)."""
    from larmorx import _nipype
    from larmorx.fsl.topup import _run_original

    in_file = os.path.abspath(os.fspath(in_file))
    cwd = os.path.abspath(os.fspath(out_dir))
    os.makedirs(cwd, exist_ok=True)
    base = _nipype.split_filename(in_file)[1]
    cmd = _nipype.topup(
        cwd=cwd,
        in_file=in_file,
        encoding_direction=list(encoding_direction),
        readout_times=list(readout_times),
        config=os.fspath(config),
    )
    for path, text in cmd.files.items():
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
    enc = cmd.outputs["out_enc_file"]
    args = cmd.argv()
    selected = _replica.select("fsl", "topup", implementation)
    env = {
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "FSLDIR": os.path.abspath(os.path.expanduser(os.fspath(fsldir))) if fsldir else cwd,
    }
    threads = max(int(n_threads), 1)
    if selected is None:
        _run_original(args, cwd, env, threads, upstream_defects)
    else:
        from larmorx.fsl.flirt import _warn

        process = _replica.run(
            selected,
            args,
            cwd=cwd,
            n_threads=threads,
            upstream_defects=upstream_defects,
            env=env,
            exclude=lambda name: name.upper().startswith("FSL"),
        )
        _warn(process.stderr)
    n_vols = _shape(in_file)[-1]
    at = lambda n: os.path.join(cwd, n)  # noqa: E731
    return TopupNodeResult(
        out_fieldcoef=at(f"{base}_base_fieldcoef.nii.gz"),
        out_movpar=at(f"{base}_base_movpar.txt"),
        out_enc_file=enc,
        out_field=at(f"{base}_field.nii.gz"),
        out_corrected=at(f"{base}_corrected.nii.gz"),
        out_logfile=at(f"{base}_topup.log"),
        out_warps=tuple(at(f"warpfield_{i:02d}.nii.gz") for i in range(1, n_vols + 1)),
        out_jacs=tuple(at(f"jac_{i:02d}.nii.gz") for i in range(1, n_vols + 1)),
        out_mats=tuple(at(f"xfm_{i:02d}.mat") for i in range(1, n_vols + 1)),
        cmdline=("topup", *args),
    )


def _fsl_aff_adapt(affine: np.ndarray, shape: Sequence[int]) -> tuple[np.ndarray, np.ndarray]:
    """nitransforms' ``_fsl_aff_adapt``: the swap (x flipped for a positive determinant) and the
    voxel-size scaling of FSL's coordinates."""
    zooms = [*np.sqrt(np.sum(affine[:3, :3] ** 2, axis=0)).tolist(), 1]
    swp = np.eye(4)
    if np.linalg.det(affine) > 0:
        swp[0, 0] = -1.0
        swp[0, 3] = (shape[0] - 1) * zooms[0]
    return swp, np.diag(zooms)


def topup_mats_to_world(
    in_mats: Sequence[str | os.PathLike[str]], in_reference: str | os.PathLike[str]
) -> list[list[list[float]]]:
    """sdcflows 2.17.1's ``_topup_mats_to_world``: each of TOPUP's FSL matrices as the RAS
    affine nitransforms reads (``nt.linear.load(mat, fmt='fsl', reference=moving=in_reference)
    .matrix.tolist()``: ``post @ inv(mat).T @ pre`` with the reference's FSL swap and scaling,
    numpy's ``inv`` and ``@``)."""
    from larmorx.transforms._linalg import inv, matmul

    ref = load_nibabel(in_reference)
    aff = np.asarray(ref.affine, dtype=np.float64)
    swp, spc = _fsl_aff_adapt(aff, ref.shape)
    pre = matmul(matmul(swp, spc), inv(aff))
    post = matmul(aff, inv(matmul(swp, spc)))
    out = []
    for path in in_mats:
        with open(os.fspath(path), encoding="utf-8") as f:
            lines = [ln.strip() for ln in f.read().splitlines() if ln.strip()]
        if len(lines) < 4:
            from larmorx.transforms.chain import TransformFileError

            raise TransformFileError
        mat = np.array([[float(v) for v in ln.split()] for ln in lines], dtype=np.float64).T
        m = matmul(matmul(post, np.swapaxes(inv(mat), 0, 1)), pre)
        if not np.allclose(m[3, :], (0.0, 0.0, 0.0, 1.0)):
            raise ValueError(f"The last row of a transform matrix must be (0, 0, 0, 1): {m[3]}")
        m = np.array(m, copy=True)
        m[3, :] = (0.0, 0.0, 0.0, 1.0)
        out.append(m.tolist())
    return out


# ------------------------------------------------------------------------------------------
# init_topup_wf


@dataclass(frozen=True)
class TopupWorkflowResult:
    """``init_topup_wf``'s outputs and its nodes' results: ``fmap`` (the field in Hz on the
    reference's orientation), ``fmap_ref`` and ``fmap_mask`` (the brain-extracted average of the
    corrected volumes), ``fmap_coeff`` (the coefficients placed in space), ``jacobians``,
    ``xfms`` and ``out_warps`` (TOPUP's), ``method``, ``topup_config`` (2.17); then the nodes'
    results (``nodes``: name -> result) as larmorx computed them."""

    fmap: str | None
    fmap_ref: str | None
    fmap_mask: str | None
    fmap_coeff: tuple[str, ...]
    jacobians: tuple[str, ...]
    xfms: tuple[str, ...]
    out_warps: tuple[str, ...]
    method: str
    topup_config: str | None
    wiring: str
    nodes: Mapping[str, Any]


def _front(x: Any) -> Any:
    return x[0] if isinstance(x, (list, tuple)) else x


def topup_workflow(
    in_data: Sequence[str | os.PathLike[str]],
    metadata: Sequence[Mapping[str, Any]],
    *,
    wiring: Literal["2.17", "2.15"] = "2.17",
    grid_reference: int = 0,
    use_metadata_estimates: bool = False,
    fallback_total_readout_time: float | None = None,
    omp_nthreads: int = 1,
    sloppy: bool = False,
    debug: bool = False,
    topup_config: str | os.PathLike[str] | None = None,
    max_vols_per_pe: int = 3,
    out_dir: str | os.PathLike[str] | None = None,
    fsldir: str | os.PathLike[str] | None = None,
    numpy_version: str | None = None,
    scipy_version: str | None = None,
    topup_implementation: str = "auto",
    volreg_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> TopupWorkflowResult:
    """sdcflows' ``init_topup_wf``: the B0 field from EPIs of opposite phase encodings, with
    FSL's ``topup``.

    ``in_data`` are the EPI files (3D or 4D) and ``metadata`` their BIDS metadata (the
    ``inputnode``). ``wiring`` chooses sdcflows 2.17.1's graph (the default) or 2.15's
    (fMRIPrep 25.2.x's; ``topup_config`` and ``max_vols_per_pe`` are 2.17.1's arguments), as the
    module docstring says; ``grid_reference``, ``use_metadata_estimates``,
    ``fallback_total_readout_time``, ``omp_nthreads`` (3dvolreg's threads), ``sloppy`` (the
    ``_quick`` configurations) and ``debug`` (the corrected volumes and the field from
    ``ApplyCoeffsField`` instead of TOPUP's) are ``init_topup_wf``'s. ``fsldir`` gives FSL's
    configurations (:func:`topup_config_file`). The library releases that change results
    (``numpy_version``, ``scipy_version``: the averages' percentiles and summation, the brain
    mask) default to the newest. Every node's files are in ``out_dir`` as in nipype's working
    directory (a temporary directory without it: the paths are then ``None``)."""
    from larmorx.niworkflows import merge_series, reorient_file, robust_average
    from larmorx.niworkflows.images import NUMPY_NEWEST
    from larmorx.sdcflows.coeffs import topup_coeff_reorient
    from larmorx.sdcflows.masks import SCIPY_NEWEST
    from larmorx.sdcflows.transform import apply_coeffs_field
    from larmorx.sdcflows.workflows import _brainextraction, _Workdir

    _defects.check(upstream_defects)
    if wiring not in ("2.17", "2.15"):
        raise ValueError(f"wiring must be '2.17' or '2.15', not {wiring!r}")
    npv = numpy_version or NUMPY_NEWEST
    spv = scipy_version or SCIPY_NEWEST
    files = [os.fspath(f) for f in in_data]
    metas = list(metadata)
    wd = _Workdir(out_dir)
    nodes: dict[str, Any] = {}
    try:

        def ravg(in_file: str, *names: str) -> Any:
            return robust_average(
                in_file,
                num_threads=omp_nthreads,
                out_dir=wd.node(*names),
                numpy_version=npv,
                volreg_implementation=volreg_implementation,
                upstream_defects=upstream_defects,
                n_threads=n_threads,
            )

        if wiring == "2.15":
            runwise = [
                ravg(f, "runwise_avg", "mapflow", f"_runwise_avg{i}") for i, f in enumerate(files)
            ]
            nodes["runwise_avg"] = runwise
            regrid = uniform_grid(
                [r.out_file for r in runwise],
                grid_reference,
                out_dir=wd.node("regrid"),
                n_threads=n_threads,
            )
            ro_files = list(regrid.out_data)
        else:
            ro_files = files
        if len(ro_files) != len(metas):
            raise ValueError("in_data and metadata have different lengths")
        readout = []
        for i, (meta, f) in enumerate(zip(metas, ro_files, strict=True)):
            wd.node("readout_time", "mapflow", f"_readout_time{i}")
            readout.append(
                get_readout_time(meta, f, use_metadata_estimates, fallback_total_readout_time)
            )
        nodes["readout_time"] = readout
        pe_fsl = [r.pe_dir_fsl for r in readout]
        ro_times = [r.readout_time for r in readout]
        if wiring == "2.17":
            sel = select_pe_volumes(
                files,
                pe_fsl,
                ro_times,
                max_vols_per_pe,
                out_dir=wd.node("select_volumes"),
                n_threads=n_threads,
            )
            nodes["select_volumes"] = sel
            regrid = uniform_grid(
                sel.out_data, grid_reference, out_dir=wd.node("regrid"), n_threads=n_threads
            )
            pe_fsl, ro_times = list(sel.pe_dirs_fsl), list(sel.readout_times)
        nodes["regrid"] = regrid
        srt = sort_pe_blips(regrid.out_data, pe_fsl, ro_times)
        nodes["sort_pe_blips"] = srt
        concat = merge_series(
            list(srt.out_data),
            affine_tolerance=1e-4,
            out_dir=wd.node("concat_blips"),
            n_threads=n_threads,
        )
        nodes["concat_blips"] = concat
        if wiring == "2.15":
            padded, _ = pad_slices(concat, out_dir=wd.node("pad_blip_slices"), n_threads=n_threads)
            nodes["pad_blip_slices"] = padded
            setwise = ravg(padded, "setwise_avg")
            nodes["setwise_avg"] = setwise
            to_las_in, fmap_ref_file = setwise.out_hmc_volumes, setwise.out_file
        else:
            to_las_in, fmap_ref_file = concat, regrid.reference
        las_file, las_pe = reorient_image_and_metadata(
            to_las_in, _LAS, list(srt.pe_dirs_fsl), out_dir=wd.node("to_las"),
            upstream_defects=upstream_defects, n_threads=n_threads,
        )  # fmt: skip
        nodes["to_las"] = (las_file, las_pe)
        if wiring == "2.15":
            config = topup_config_file(f"b02b0{'_quick' * bool(sloppy)}.cnf", fsldir)
            out_config = None
        elif topup_config:
            config = out_config = os.fspath(topup_config)
        else:
            wd.node("select_config")
            config = out_config = select_topup_config(las_file, sloppy, fsldir=fsldir)
        nodes["topup_config"] = config
        tu = topup_node(
            las_file, las_pe, list(srt.readout_times), config, out_dir=wd.node("topup"),
            fsldir=fsldir, implementation=topup_implementation,
            upstream_defects=upstream_defects, n_threads=n_threads,
        )  # fmt: skip
        nodes["topup"] = tu
        fix = topup_coeff_reorient(
            tu.out_fieldcoef, fmap_ref_file, _front(las_pe), out_dir=wd.node("fix_coeff"),
            upstream_defects=upstream_defects,
        )  # fmt: skip
        nodes["fix_coeff"] = fix
        fmap_coeff = tuple(os.fspath(p) for p in fix.out_coeff)
        if not debug:
            from_las = reorient_file(
                tu.out_corrected,
                target_file=fmap_ref_file,
                newpath=wd.node("from_las"),
                n_threads=n_threads,
            )
            from_las_fmap = reorient_file(
                tu.out_field,
                target_file=fmap_ref_file,
                newpath=wd.node("from_las_fmap"),
                n_threads=n_threads,
            )
            nodes["from_las"], nodes["from_las_fmap"] = from_las, from_las_fmap
            avg_in, fmap, warps = from_las, from_las_fmap, tu.out_warps
        else:
            xfms = None
            if wiring == "2.17":
                wd.node("convert_xfms")
                xfms = topup_mats_to_world(tu.out_mats, las_file)
                nodes["convert_xfms"] = xfms
                unwarp_in = concat
            else:
                unwarp_in = setwise.out_hmc_volumes
            unwarp = apply_coeffs_field(
                unwarp_in, list(fmap_coeff), list(srt.readout_times), list(srt.pe_dirs),
                in_xfms=xfms, jacobian=True, out_dir=wd.node("unwarp"),
                upstream_defects=upstream_defects, n_threads=n_threads,
            )  # fmt: skip
            nodes["unwarp"] = unwarp
            corrected = [os.fspath(p) for p in np.atleast_1d(unwarp.out_corrected)]
            avg_in = merge_series(
                corrected, out_dir=wd.node("concat_corrected"), n_threads=n_threads
            )
            nodes["concat_corrected"] = avg_in
            fmap, warps = os.fspath(unwarp.out_field), ()
        ref_avg = ravg(avg_in, "ref_average")
        nodes["ref_average"] = ref_avg
        bx = _brainextraction(
            wd,
            ("brainextraction_wf",),
            ref_avg.out_file,
            None,
            npv,
            spv,
            upstream_defects,
            n_threads,
        )
        nodes["brainextraction_wf"] = bx
        result = TopupWorkflowResult(
            fmap=fmap,
            fmap_ref=bx.out_file,
            fmap_mask=bx.out_mask,
            fmap_coeff=fmap_coeff,
            jacobians=tu.out_jacs,
            xfms=tu.out_mats,
            out_warps=tuple(warps),
            method=PEPOLAR_METHOD,
            topup_config=out_config,
            wiring=wiring,
            nodes=nodes,
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(result)
