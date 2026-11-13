# SPDX-License-Identifier: Apache-2.0 AND MIT
"""niworkflows' ``interfaces/images.py`` (niworkflows 1.15.4, the same in 1.15.2):
``IntraModalMerge`` and its ``reorient``, with nibabel's (5.4.2, MIT) ``squeeze_image``,
``four_to_three``, ``concat_images`` and ``as_closest_canonical``; and ``SignalExtraction``
(:func:`signal_extraction`: the mean of a series over each region, as fMRIPrep's confounds take
the global, CSF, white-matter and tCompCor signals; PIPE-78).

``IntraModalMerge`` merges images of one modality into a series and averages it: sdcflows
averages a field map's magnitude images with it (``magmrg``) and its direct field maps
(``fmapmrg``). The arithmetic is numpy's and nibabel's, as niworkflows runs it:

- the series is the files' values as nibabel's ``dataobj`` gives them (scaled to ``float64``
  where the header scales them), gathered by ``concat_images`` into a ``float64`` array of
  shape ``(n, ...)`` rolled to ``(..., n)``;
- the average is ``get_fdata(dtype='float32').mean(axis=3)``: the ``float32`` series keeps
  that memory order, so numpy sums each voxel's volumes one after the other from ``+0.0`` in
  ``float32`` (not pairwise: the summed axis is the outermost in memory) and divides by the
  count in ``float64`` (an ``intp`` count, NEP 50), cast to ``float32``; numpy 2.2.6, 2.3.5 and
  2.5.4 alike (measured: ``validation/investigations/sdcflows-fieldmap-b/check_merge.py``);
- the files are written by nibabel's scaled writer (:func:`larmorx.io.save_nibabel`) in the
  first image's stored type: the merged series as ``<first>_merged``, the average as
  ``<first>_avg``.

Corrected by default (``upstream_defects='reproduce'`` gives niworkflows' behaviour):

- PIPE-18: ``grand_mean_scaling`` indexes the series with the mask's ``[..., np.newaxis]``, a
  boolean index numpy refuses for every series of two or more volumes (an ``IndexError``), so
  the option can never run; larmorx scales each volume by the ratio of the largest median to
  its median inside the mask, as the code means.
- PIPE-19: a 6- or 7-dimensional image passes niworkflows' 5D check and is averaged over its
  fourth axis; larmorx raises niworkflows' error for it, as for a 5D image.
- PIPE-34 (``robust_average``): a gathered volume whose mean is 0 is divided by 0; larmorx
  raises a ``ValueError`` naming it (rule B).
"""

from __future__ import annotations

import contextlib
import os
import tempfile
from collections.abc import Iterator, Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import header_for_image, io_orientation, load_nibabel
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = [
    "IntraModalMergeResult",
    "RobustAverageResult",
    "as_closest_canonical",
    "as_reoriented",
    "four_to_three",
    "intra_modal_merge",
    "reorient",
    "robust_average",
    "signal_extraction",
    "squeeze_image",
]

#: The newest numpy release (the arithmetic of ``grand_mean_scaling``'s percentile).
NUMPY_NEWEST = "2.5.4"
_IDENTITY_ORNT = [[0, 1], [1, 1], [2, 1]]


@dataclass(frozen=True)
class IntraModalMergeResult:
    """``IntraModalMerge``'s outputs: ``out_file`` (the merged series, or mcflirt's corrected
    series with ``hmc``), ``out_avg`` (the average; the merged image itself when the merge has
    fewer than four dimensions), ``out_mats`` and ``out_movpar`` (mcflirt's matrices and
    parameters with ``hmc``); the images as niworkflows holds them before writing (``merged``:
    the series' values, ``float64`` when several images were gathered; ``average``: the
    ``float32`` average). Paths are ``None`` when nothing was written (no ``out_dir``)."""

    merged: Image
    average: Image
    out_file: str | None
    out_avg: str | None
    out_mats: tuple[str, ...] = ()
    out_movpar: str | None = None
    warnings: tuple[str, ...] = ()


@contextlib.contextmanager
def _workdir(out_dir: str | os.PathLike[str] | None) -> Iterator[tuple[str, bool]]:
    """The node's working directory: ``out_dir`` (made), or a temporary one removed after
    the call (the files are read back exactly as nipype's next node reads them)."""
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        yield os.path.abspath(os.fspath(out_dir)), True
        return
    with tempfile.TemporaryDirectory(prefix="larmorx-") as tmp:
        yield tmp, False


def _fname_presuffix(fname: str, suffix: str, newpath: str) -> str:
    from larmorx.sdcflows.approx import _fname_presuffix as fp

    return fp(fname, suffix, newpath)


def _dim_info(hdr: NiftiHeader) -> tuple[int | None, int | None, int | None]:
    """nibabel's ``get_dim_info``: the frequency, phase and slice axes (``None`` unset)."""
    info = int(hdr.dim_info)
    vals = (info & 3, (info >> 2) & 3, (info >> 4) & 3)
    return tuple(v - 1 if v else None for v in vals)  # type: ignore[return-value]


def _set_dim_info(freq: int | None, phase: int | None, slc: int | None) -> int:
    """nibabel's ``set_dim_info``: the ``dim_info`` byte."""
    info = 0
    if freq is not None:
        info |= (freq + 1) & 3
    if phase is not None:
        info |= ((phase + 1) & 3) << 2
    if slc is not None:
        info |= ((slc + 1) & 3) << 4
    return info


def as_closest_canonical(img: Image) -> Image:
    """nibabel's ``as_closest_canonical(img)`` (``as_reoriented(io_orientation(affine))``):
    the image itself when its axes are already closest to RAS; otherwise its data flipped and
    transposed, the affine composed with the inverse orientation, and the header's
    ``dim_info`` axes moved with them (``Nifti1Image.as_reoriented``)."""
    return as_reoriented(img, io_orientation(np.asarray(img.affine, dtype=np.float64)))


def as_reoriented(img: Image, ornt: Any) -> Image:
    """nibabel's ``Nifti1Image.as_reoriented(ornt)``: the image itself for the identity;
    otherwise ``apply_orientation`` of its data, the affine composed with
    ``inv_ornt_aff(ornt, shape)`` (numpy's ``dot``) and the header made by the image
    constructor, with the ``dim_info`` axes moved."""
    from larmorx.transforms._linalg import matmul
    from larmorx.transforms.resample import _apply_orientation, _inv_ornt_aff

    ornt = np.asarray(ornt)
    if np.array_equal(ornt, _IDENTITY_ORNT):
        return img
    data = _apply_orientation(np.asanyarray(img.data), ornt)
    affine = matmul(np.asarray(img.affine, dtype=np.float64), _inv_ornt_aff(ornt, img.shape))
    out = _constructed(Image(img.data, affine, img.header), data)
    hdr = out.header
    if hdr is None:
        return out
    new_dim = [None if d is None else int(ornt[d, 0]) for d in _dim_info(hdr)]
    return out.replace(header=hdr.replace(dim_info=_set_dim_info(*new_dim)))


def reorient(in_file: str | os.PathLike[str], newpath: str | os.PathLike[str] | None = None) -> str:
    """niworkflows' ``reorient(in_file, newpath)``: ``nb.as_closest_canonical(nb.load(in_file))``
    written as ``<in_file>_ras`` in ``newpath`` (the working directory by default)."""
    out = _fname_presuffix(os.fspath(in_file), "_ras", os.fspath(newpath or os.getcwd()))
    save_nibabel(as_closest_canonical(load_nibabel(in_file)), out)
    return out


def _constructed(img: Image, data: np.ndarray) -> Image:
    """``img.__class__(data, img.affine, img.header)``: nibabel's constructor copies the header
    and updates it for ``data`` (``update_header``: a changed shape is set with
    ``set_data_shape``, which sets every later ``dim`` to 1 and every later ``pixdim`` to 1.0;
    the affine into the qform and sform); the stored type stays the header's."""
    hdr = img.header
    if hdr is None:
        return Image(data, img.affine, None)
    dtype = hdr.data_dtype if hdr.data_dtype is not None else np.asarray(data).dtype
    fields = header_for_image(
        [int(n) for n in np.asarray(data).shape],
        np.dtype(dtype).name,
        np.ascontiguousarray(img.affine, dtype=np.float64),
        hdr.to_dict(),
        None,
    )
    return Image(data, img.affine, NiftiHeader.from_dict(fields))


def squeeze_image(img: Image) -> Image:
    """nibabel's ``squeeze_image``: trailing axes of length 1 after the third dropped (the
    image itself when there are none)."""
    return _squeezed(img)


def four_to_three(img: Image) -> list[Image]:
    """nibabel's ``four_to_three``: the volumes of a 4D image, each made by the image
    constructor from the 4D header (``ValueError`` for other shapes)."""
    arr = np.asanyarray(img.data)
    if arr.ndim != 4:
        raise ValueError("Expecting four dimensions")
    return [_constructed(img, arr[..., i]) for i in range(arr.shape[3])]


def _squeezed(img: Image) -> Image:
    """nibabel's ``squeeze_image``: trailing axes of length 1 after the third dropped."""
    shape = tuple(np.asarray(img.data).shape)
    slen = len(shape)
    if slen < 4:
        return img
    for b in shape[3:][::-1]:
        if b == 1:
            slen -= 1
        else:
            break
    if slen == len(shape):
        return img
    return _constructed(img, np.asanyarray(img.data).reshape(shape[:slen]))


def _concat(images: Sequence[Image], check_affines: bool = True) -> Image:
    """nibabel's ``concat_images(images, check_affines)`` (``axis=None``): a ``float64``
    array of shape ``(n,) + shape0`` filled with each image's values and rolled to
    ``shape0 + (n,)``, with the first image's affine and header."""
    img0 = images[0]
    shape0 = tuple(np.asarray(img0.data).shape)
    n_dim = len(shape0)
    out = np.empty((len(images), *shape0))
    affine = np.asarray(img0.affine, dtype=np.float64)
    for i, img in enumerate(images):
        shape = tuple(np.asarray(img.data).shape)
        if len(shape) != n_dim:
            raise ValueError(f"Image {i} has {len(shape)} dimensions, image 0 has {n_dim}")
        if shape != shape0:
            raise ValueError(
                f"shape {shape} for image {i} not compatible with first image shape {shape0} "
                "with axis == None"
            )
        if check_affines and not np.all(np.asarray(img.affine, dtype=np.float64) == affine):
            raise ValueError(f"Affine for image {i} does not match affine for first image")
        out[i] = np.asanyarray(img.data)
    return _constructed(img0, np.rollaxis(out, 0, out.ndim))


def _sequential_mean(series: np.ndarray) -> np.ndarray:
    """``series.mean(axis=3)`` of a ``float32`` 4D series whose fourth axis is the outermost in
    memory: each voxel's volumes summed in order from ``+0.0`` in ``float32``, divided by the
    count in ``float64`` and cast to ``float32``."""
    acc = np.zeros(series.shape[:3], dtype=np.float32)
    for i in range(series.shape[3]):
        acc += series[..., i]
    return (acc.astype(np.float64) / np.float64(series.shape[3])).astype(np.float32)


_PIPE40 = (
    "IntraModalMerge with to_ras reorients the series to RAS but not in_mask, so "
    "grand_mean_scaling indexes the RAS series with the mask on the input's grid (numpy's "
    "IndexError when the shapes differ, a mirrored mask when they agree); the mask was "
    "reoriented with the series"
)


def _grand_mean_scaled(
    series: np.ndarray,
    in_mask: Any,
    numpy_version: str,
    upstream_defects: str,
) -> tuple[np.ndarray, list[tuple[str, str, str, str]]]:
    """``grand_mean_scaling``: the mask (``in_mask > 0.5``, else the voxels whose median over
    time is above the 25th percentile of those medians), then each volume multiplied by the
    largest masked median over its own (PIPE-18)."""
    nimgs = series.shape[-1]
    if in_mask is None:
        med = np.asarray(_core.numpy_median(np.ascontiguousarray(series), axis=series.ndim - 1))
        thres = np.float32(
            _core.numpy_percentile_weak(np.ascontiguousarray(med), 25.0, numpy_version)
        )
        mask = med > thres
    else:
        mimg = (
            load_nibabel(in_mask, dtype=np.float32)
            if isinstance(in_mask, (str, os.PathLike))
            else in_mask
        )
        mask = np.asarray(getattr(mimg, "data", mimg), dtype=np.float32) > 0.5
    if upstream_defects == "reproduce":
        # niworkflows' own indexing: numpy's IndexError for every series of two or more volumes
        sel = series[mask[..., np.newaxis]].reshape((-1, nimgs)).T
        means = np.median(sel, axis=-1)
    else:
        if mask.shape != series.shape[:-1]:
            raise ValueError(
                f"the mask's shape {mask.shape} does not match the series' {series.shape[:-1]}"
            )
        sel = np.ascontiguousarray(series[mask].T)
        means = np.array(
            [_core.numpy_median(np.ascontiguousarray(sel[i]), axis=None) for i in range(nimgs)],
            dtype=series.dtype,
        ).reshape(nimgs)
    max_mean = means.max()
    out = np.array(series, copy=True)
    for i in range(nimgs):
        out[..., i] *= max_mean / means[i]
    notice = (
        "PIPE-18",
        "niworkflows",
        "fix",
        "IntraModalMerge's grand_mean_scaling indexes the series with a mask of the wrong "
        "shape and fails with numpy's IndexError; each volume was scaled by the largest "
        "median over its own inside the mask, as intended",
    )
    return out, [notice]


def intra_modal_merge(
    in_files: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    in_mask: str | os.PathLike[str] | None = None,
    hmc: bool = True,
    zero_based_avg: bool = True,
    to_ras: bool = True,
    grand_mean_scaling: bool = False,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    hmc_implementation: str = "auto",
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> IntraModalMergeResult:
    """niworkflows' ``IntraModalMerge``: the images (paths) merged into one series and
    averaged over its fourth axis.

    - ``to_ras``: each file first reoriented to the closest RAS orientation and written as
      ``<name>_ras`` (:func:`reorient`).
    - Trailing axes of length 1 are dropped; a 5D image is an error (``RuntimeError``); a 4D
      image contributes its volumes. Several images are gathered into one series (nibabel's
      ``concat_images``: equal shapes and affines, ``ValueError`` otherwise), written as
      ``<first>_merged``. A merge of fewer than four dimensions is the result (``out_avg`` =
      ``out_file``).
    - ``hmc``, when more than one file is given: FSL's ``mcflirt`` on the merged series as
      nipype's ``MCFLIRT(cost='normcorr', save_mats=True, save_plots=True, ref_vol=0)`` runs it
      (``larmorx fsl mcflirt``: the replica when ``larmorx-nc`` is installed, else the
      clean-room original; ``hmc_implementation``), and the corrected series averaged.
    - ``grand_mean_scaling``: each volume scaled to the largest median inside ``in_mask`` (or
      the brighter voxels; PIPE-18, see the module).
    - The average of the ``float32`` series (module docstring), shifted to start at 0 with
      ``zero_based_avg``, written as ``<first>_avg`` with the series' header.

    Files go into ``out_dir`` (nipype's node directory); without it they are written to a
    temporary directory and only the images are returned."""
    _defects.check(upstream_defects)
    files = [
        os.fspath(f) for f in (in_files if isinstance(in_files, (list, tuple)) else [in_files])
    ]
    if not files:
        raise ValueError("in_files is empty")
    notices: list[tuple[str, str, str, str]] = []
    with _workdir(out_dir) as (cwd, keep):
        inputs = [reorient(f, newpath=cwd) for f in files] if to_ras else files
        run_hmc = hmc and len(inputs) > 1
        nii_list: list[Image] = []
        for f in inputs:
            img = _squeezed(load_nibabel(f))
            ndim = np.asarray(img.data).ndim
            if ndim == 5 or (ndim > 5 and upstream_defects != "reproduce"):
                raise RuntimeError(f"Input image ({f}) is {ndim}D.")
            if ndim == 4:
                arr = np.asanyarray(img.data)
                nii_list += [_constructed(img, arr[..., i]) for i in range(arr.shape[3])]
            elif ndim > 5:
                # PIPE-19 reproduced: kept whole, read in nibabel's (Fortran) memory order
                nii_list.append(img.replace(data=np.asfortranarray(np.asanyarray(img.data))))
            else:
                nii_list.append(img)
        merged = _concat(nii_list) if len(nii_list) > 1 else nii_list[0]
        merged_fname = _fname_presuffix(files[0], "_merged", cwd)
        save_nibabel(merged, merged_fname, n_threads=n_threads)
        out_file = merged_fname
        out_mats: tuple[str, ...] = ()
        out_movpar = None
        mdata = np.asanyarray(merged.data)
        if mdata.ndim < 4:
            _defects.warn(notices)
            path = merged_fname if keep else None
            return IntraModalMergeResult(merged, merged, path, path, warnings=())
        if run_hmc:
            out_file, out_mats, out_movpar = _mcflirt(merged_fname, cwd, hmc_implementation)
            series = np.asarray(load_nibabel(out_file, dtype=np.float32).data)
            src = load_nibabel(out_file)
            affine, header = src.affine, src.header
        else:
            series = np.asarray(mdata, dtype=np.float32)
            affine, header = merged.affine, merged.header
        if grand_mean_scaling:
            mask_in = in_mask
            if to_ras and in_mask is not None and upstream_defects != "reproduce":
                # PIPE-40: niworkflows reorients the series to RAS but not the mask; the mask
                # reoriented with it (a mask already in RAS is unchanged, no notice)
                given = load_nibabel(in_mask, dtype=np.float32)
                mask_in = as_closest_canonical(given)
                if np.asarray(mask_in.data).tobytes() != np.asarray(given.data).tobytes() or (
                    np.asarray(mask_in.data).shape != np.asarray(given.data).shape
                ):
                    notices.append(("PIPE-40", "niworkflows", "fix", _PIPE40))
            series, more = _grand_mean_scaled(series, mask_in, numpy_version, upstream_defects)
            if upstream_defects != "reproduce":
                notices += more
        # a 6/7-D series (PIPE-19 reproduced): numpy's own mean over the fourth axis
        avg = _sequential_mean(series) if series.ndim == 4 else series.mean(axis=3)
        if zero_based_avg:
            avg -= avg.min()
        average = Image(avg, affine, header)
        out_avg = _fname_presuffix(files[0], "_avg", cwd)
        save_nibabel(average, out_avg, n_threads=n_threads)
        _defects.warn(notices)
        return IntraModalMergeResult(
            merged,
            average,
            out_file if keep else None,
            out_avg if keep else None,
            out_mats if keep else (),
            out_movpar if keep else None,
            tuple(n[3] for n in notices),
        )


def _mcflirt(in_file: str, cwd: str, implementation: str) -> tuple[str, tuple[str, ...], str]:
    """nipype's ``MCFLIRT(cost='normcorr', save_mats=True, save_plots=True, ref_vol=0)`` on
    ``in_file`` in ``cwd``: ``mcflirt -in <in> -cost normcorr -out <cwd>/<in>_mcf.nii.gz
    -refvol 0 -mats -plots`` through larmorx's ``fsl mcflirt``; the corrected series, its
    matrices (``<out>.mat/MAT_NNNN``) and parameters (``<out>.par``), as nipype lists them."""
    import io

    from larmorx import cli

    out = _fname_presuffix(in_file, "_mcf", cwd)
    for ext in (".nii.gz", ".nii"):
        if out.endswith(ext):
            out = out[: -len(ext)] + ".nii.gz"
            break
    argv = ["larmorx", "fsl", "mcflirt", "-in", in_file, "-cost", "normcorr", "-out", out]
    argv += ["-refvol", "0", "-mats", "-plots"]
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = cli.run(argv, implementation=implementation)
    if code != 0:
        raise RuntimeError(f"mcflirt failed (exit {code}): {err.getvalue().strip()}")
    nvols = np.asarray(load_nibabel(in_file).data).shape[3]
    mats = tuple(
        os.path.join(cwd, os.path.basename(out) + ".mat", f"MAT_{t:04d}") for t in range(nvols)
    )
    return out, mats, out + ".par"


@dataclass(frozen=True)
class RobustAverageResult:
    """``RobustAverage``'s outputs: ``out_file`` (the average: ``<in>_average``, or the input
    itself for a 3D image, ``<in>_squeezed`` for one volume), ``out_volumes`` (the clipped,
    drift-corrected volumes it registered: ``<in>_sliced``, or the input), ``out_drift`` (the
    global signal's drift per volume), ``out_hmc`` (3dvolreg's ``.aff12.1D`` matrices, or
    mcflirt's matrices with ``mc_method="FSL"``) and ``out_hmc_volumes`` (the registered
    series). Paths are ``None`` when nothing was written (no ``out_dir``) or not produced."""

    out_file: str | None
    out_volumes: str | None
    out_drift: tuple[float, ...]
    out_hmc: str | tuple[str, ...] | None = None
    out_hmc_volumes: str | None = None


def _gs_drift(data: np.ndarray, numpy_version: str) -> np.ndarray:
    """``np.mean(data, axis=(0, 1, 2))`` of the ``float32`` series ``concat_images`` made (its
    fourth axis outermost in memory, each volume a C-ordered block): each volume's values
    summed in numpy ``numpy_version``'s order (pairwise over the block from numpy 2.3, in chunks
    of 8192 before), divided by the count in ``float64`` and cast to ``float32``."""
    count = np.float64(int(np.prod(data.shape[:3])))
    sums = [
        _core.numpy_add_reduce(np.ascontiguousarray(data[..., t]), numpy_version)
        for t in range(data.shape[3])
    ]
    return (np.asarray(sums, dtype=np.float32).astype(np.float64) / count).astype(np.float32)


def _volreg_nipype(
    in_file: str,
    cwd: str,
    two_pass: bool,
    num_threads: int | None,
    implementation: str,
    upstream_defects: str,
) -> tuple[str, str]:
    """nipype's ``Volreg(in_file, interp='Fourier', args='-twopass', zpad=4,
    outputtype='NIFTI_GZ')`` run in ``cwd`` (:func:`larmorx._nipype.volreg`): ``3dvolreg
    -twopass -Fourier -1Dfile <stem>.1D -1Dmatrix_save <stem>.aff12.1D -prefix
    <stem>_volreg.nii.gz -zpad 4 -maxdisp1D <stem>_md.1D <in>`` (``-twopass`` only with
    ``two_pass``), through larmorx's 3dvolreg (the bit-exact replica by default). The
    registered series and the matrices file."""
    from larmorx import _nipype
    from larmorx.afni import _common
    from larmorx.afni.volreg import _run_original

    cmd = _nipype.volreg(
        cwd=cwd,
        in_file=in_file,
        args="-twopass" if two_pass else None,
        interp="Fourier",
        zpad=4,
        outputtype="NIFTI_GZ",
    )
    selected = _replica.select("afni", "3dvolreg", implementation)
    threads = 1 if num_threads is None else max(int(num_threads), 1)
    if selected is None:
        # in this process: the output names in the node's folder
        _run_original(_nipype.words(cmd, upstream_defects, run=True), threads, upstream_defects, {})
    else:
        _common.run_replica(
            selected,
            _nipype.words(cmd, upstream_defects),
            cwd,
            n_threads=threads,
            upstream_defects=upstream_defects,
            settings={},
        )
    return cmd.outputs["out_file"], cmd.outputs["oned_matrix_save"]


def _mcflirt_sinc(in_file: str, cwd: str, implementation: str) -> str:
    """nipype's ``MCFLIRT(in_file, ref_vol=0, interpolation='sinc')`` in ``cwd``: ``mcflirt -in
    <in> -out <cwd>/<in>_mcf.nii.gz -refvol 0 -sinc_final`` through larmorx's ``fsl
    mcflirt``; the corrected series."""
    import io

    from larmorx import cli

    out = _fname_presuffix(in_file, "_mcf", cwd)
    for ext in (".nii.gz", ".nii"):
        if out.endswith(ext):
            out = out[: -len(ext)] + ".nii.gz"
            break
    argv = ["larmorx", "fsl", "mcflirt", "-in", in_file, "-out", out, "-refvol", "0"]
    argv += ["-sinc_final"]
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = cli.run(argv, implementation=implementation)
    if code != 0:
        raise RuntimeError(f"mcflirt failed (exit {code}): {err.getvalue().strip()}")
    return out


def robust_average(
    in_file: str | os.PathLike[str],
    t_mask: Sequence[bool] | None = None,
    mc_method: Literal["AFNI", "FSL"] | None = "AFNI",
    nonnegative: bool = True,
    two_pass: bool = True,
    *,
    num_threads: int | None = None,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    volreg_implementation: str = "auto",
    mcflirt_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> RobustAverageResult:
    """niworkflows' ``RobustAverage``: a robust average of a series.

    - A 3D image is the result itself; a series of one volume (after nibabel's
      ``squeeze_image``) is written as ``<in>_squeezed``.
    - Otherwise the volumes of ``t_mask`` (all by default; its length must be the series',
      and it must keep one volume: ``ValueError``s) are gathered (nibabel's ``four_to_three``
      and ``concat_images``), read as ``float32``, clipped to ``[0, p99.8]`` (``[p0.2, p99.8]``
      without ``nonnegative``; numpy ``numpy_version``'s percentile), divided by the global
      signal's drift (each volume's mean over the brightest's, numpy's summation order:
      :func:`_gs_drift`), clipped again, and written as ``<in>_sliced`` with the input's
      header (its extensions cleared).
    - One volume kept: the gathered volume is the average. Otherwise the volumes are
      registered: AFNI's ``3dvolreg -Fourier [-twopass] -zpad 4`` with nipype's output names
      (``mc_method="AFNI"``, the default; ``volreg_implementation``) or FSL's ``mcflirt -refvol
      0 -sinc_final`` (``"FSL"``), or not at all (``None``); the result clipped once more and
      its median over the volumes written as ``<in>_average`` with the gathered volumes' header.

    ``num_threads`` is nipype's (3dvolreg's ``OMP_NUM_THREADS``: no effect on its result).
    Files go into ``out_dir`` (nipype's node directory), else into a temporary directory.

    PIPE-34 (rule B; ``upstream_defects="reproduce"`` gives niworkflows' result): a gathered
    volume whose mean is 0 after the clipping (an empty volume, or a series of zeros) is divided
    by 0: NaN (or infinite) voxels, which niworkflows registers and averages; by default a
    ``ValueError`` naming the volumes."""
    _defects.check(upstream_defects)
    in_file = os.fspath(in_file)
    with _workdir(out_dir) as (cwd, keep):

        def kept(p: Any) -> Any:
            return p if keep or p is None or p == in_file else None

        img = load_nibabel(in_file)
        if np.asarray(img.data).ndim == 3:
            return RobustAverageResult(in_file, in_file, (1.0,))
        if img.header is not None:
            img = img.replace(header=img.header.replace(extensions=()))
        img = _squeezed(img)
        if np.asarray(img.data).ndim == 3:
            out = _fname_presuffix(in_file, "_squeezed", cwd)
            save_nibabel(img, out, n_threads=n_threads)
            return RobustAverageResult(kept(out), in_file, (1.0,))
        img_len = int(img.shape[3])
        mask = [True] * img_len if t_mask is None else list(t_mask)
        if len(mask) != img_len:
            raise ValueError(f"Image length ({img_len} timepoints) unmatched by mask ({len(mask)})")
        n_volumes = sum(mask)
        if n_volumes < 1:
            raise ValueError("At least one volume should be selected for slicing")
        out_file = _fname_presuffix(in_file, "_average", cwd)
        out_volumes = _fname_presuffix(in_file, "_sliced", cwd)
        sliced = _concat([v for v, t in zip(four_to_three(img), mask, strict=False) if t])
        data = np.asarray(sliced.data, dtype=np.float32)
        flat = np.ascontiguousarray(data).reshape(-1)
        hi = np.float32(_core.numpy_percentile_weak(flat, 99.8, numpy_version))
        lo = (
            0.0
            if nonnegative
            else np.float32(_core.numpy_percentile_weak(flat, 0.2, numpy_version))
        )
        data = np.clip(data, a_min=lo, a_max=hi)
        gs_drift = _gs_drift(data, numpy_version)
        zero = [i for i, v in enumerate(gs_drift) if v == 0]
        if zero and upstream_defects == "fix":
            raise ValueError(
                f"RobustAverage: the gathered volume(s) {zero} of {in_file} have a mean of 0 after "
                "the clipping, and niworkflows divides by it (NaN or infinite voxels; PIPE-34, "
                "upstream_defects='reproduce' gives its result)"
            )
        if zero:
            _defects.warn(
                [
                    (
                        "PIPE-34",
                        "niworkflows RobustAverage",
                        "reproduce",
                        f"the gathered volume(s) {zero} have a mean of 0: divided by 0",
                    )
                ],
                stacklevel=2,
            )
        gs_drift /= gs_drift.max()
        drift = tuple(float(i) for i in gs_drift)
        data /= gs_drift
        data = np.clip(data, a_min=0.0 if nonnegative else data.min(), a_max=data.max())
        save_nibabel(_constructed(sliced, data), out_volumes, n_threads=n_threads)
        if n_volumes == 1:
            save_nibabel(_squeezed(sliced), out_file, n_threads=n_threads)
            return RobustAverageResult(kept(out_file), kept(out_volumes), (1.0,))
        out_hmc: str | tuple[str, ...] | None = None
        out_hmc_volumes = None
        if mc_method == "AFNI":
            out_hmc_volumes, out_hmc = _volreg_nipype(
                out_volumes, cwd, two_pass, num_threads, volreg_implementation, upstream_defects
            )
        elif mc_method == "FSL":
            out_hmc_volumes = _mcflirt_sinc(out_volumes, cwd, mcflirt_implementation)
        elif mc_method is not None:
            raise ValueError(f"mc_method must be 'AFNI', 'FSL' or None, not {mc_method!r}")
        if out_hmc_volumes is not None:
            data = np.asarray(load_nibabel(out_hmc_volumes, dtype=np.float32).data)
        data = np.clip(data, a_min=0.0 if nonnegative else data.min(), a_max=data.max())
        median = np.asarray(_core.numpy_median(data, axis=3))
        save_nibabel(_constructed(sliced, median), out_file, n_threads=n_threads)
        return RobustAverageResult(
            kept(out_file),
            kept(out_volumes),
            drift,
            kept(out_hmc),
            kept(out_hmc_volumes),
        )


# --- SignalExtraction --------------------------------------------------------------------------


def _region_means(data: np.ndarray, mask: np.ndarray, numpy_version: str, n_threads: int):
    """``data[mask, :].mean(axis=0)`` of nibabel's ``get_fdata()`` (``data``: the values
    ``dataobj`` gives, widened to ``float64`` as ``get_fdata()`` does: exactly), with numpy's
    errors for a mask of another shape or more axes. A 4-D series and a 3-D mask stream through
    :func:`larmorx._core.niworkflows_signal_means` (no masked copy); other shapes gather the
    voxels as numpy does and take numpy ``numpy_version``'s mean of the copy."""
    if mask.ndim + 1 > data.ndim:
        raise IndexError(
            f"too many indices for array: array is {data.ndim}-dimensional, but "
            f"{mask.ndim + 1} were indexed"
        )
    for i, (a, b) in enumerate(zip(data.shape, mask.shape, strict=False)):
        if a != b:
            raise IndexError(
                f"boolean index did not match indexed array along axis {i}; size of axis is "
                f"{a} but size of corresponding boolean axis is {b}"
            )
    if data.ndim == 4 and mask.ndim == 3:
        if data.dtype not in (np.float32, np.float64) or not data.dtype.isnative:
            data = data.astype(np.float64)
        if not (data.flags.c_contiguous or data.flags.f_contiguous):
            data = np.asfortranarray(data)
        return np.asarray(
            _core.niworkflows_signal_means(
                data, np.ascontiguousarray(mask), numpy_version, n_threads
            )
        )
    gathered = np.ascontiguousarray(data[mask, ...], dtype=np.float64)
    return np.asarray(_core.numpy_reduce(gathered, "mean", 0, numpy_version))


def signal_extraction(
    in_file: os.PathLike[str] | str,
    label_files: os.PathLike[str] | str | Sequence[os.PathLike[str] | str],
    class_labels: Sequence[Any],
    *,
    prob_thres: float = 0.5,
    out_file: os.PathLike[str] | str = "signals.tsv",
    out_dir: os.PathLike[str] | str = ".",
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``SignalExtraction`` interface: the mean of the series ``in_file`` over each
    region of ``label_files`` per volume, as a table (``class_labels`` as the header, numpy's
    ``repr`` of each mean, tabs between, ``\\n`` line ends, UTF-8) at ``out_file`` (relative
    to ``out_dir``). Several files (or one 4-D file of several volumes) are probability maps
    (the voxels ``>= prob_thres`` in each map's stored type); one 3-D file is a label image
    (one region per non-zero value, in ascending order). The means are numpy's
    ``data[mask, :].mean(axis=0)`` of ``get_fdata()`` (in the mask's C order, numpy
    ``numpy_version``'s summation; NaN for an empty region). niworkflows' errors are kept: the
    first map's shape against the series' (``NotImplementedError``), the number of regions
    against the labels (``ValueError``), numpy's for the other maps. A label holding a tab or a
    line break raises ``ValueError`` (PIPE-78; ``reproduce``: written as it is). Its path."""
    mode = _defects.check(upstream_defects)
    spec = "_SignalExtractionInputSpec"

    from larmorx.confounds.masks import _nibabel_data, _trait_file

    in_path = _trait_file(spec, "in_file", in_file)
    files = [label_files] if isinstance(label_files, (str, os.PathLike)) else list(label_files)
    if not files:
        raise ValueError(
            "SignalExtraction requires a value for input 'label_files'. For a list of required "
            "inputs, see SignalExtraction.help()"
        )
    files = [_trait_file(spec, "label_files", f, element=True) for f in files]
    if not isinstance(prob_thres, (int, float, np.integer, np.floating)):
        raise ValueError(
            f"The 'prob_thres' trait of a {spec} instance must be 0.0 <= a floating point "
            f"number <= 1.0, but a value of {prob_thres!r} {type(prob_thres)!r} was specified."
        )
    if not 0.0 <= float(prob_thres) <= 1.0:
        raise ValueError(
            f"The 'prob_thres' trait of a {spec} instance must be 0.0 <= a floating point "
            f"number <= 1.0, but a value of {prob_thres!r} {type(prob_thres)!r} was specified."
        )
    labels = list(class_labels)
    from larmorx.io.nifti import read_header

    shape = tuple(int(s) for s in read_header(in_path).shape)
    mask_arrays = [_nibabel_data(load_nibabel(f, n_threads=n_threads)) for f in files]
    if len(mask_arrays) == 1 and mask_arrays[0].ndim == 4:
        four = mask_arrays[0]
        mask_arrays = [four[..., i] for i in range(four.shape[3])]
    if not mask_arrays:
        raise IndexError("list index out of range")
    # This check assumes all input masks have same dimensions
    if shape[:3] != tuple(mask_arrays[0].shape[:3]):
        raise NotImplementedError(
            "Input image and mask should be of same dimensions before running SignalExtraction"
        )
    if len(mask_arrays) > 1:
        threshold = float(prob_thres)  # traits' float: a Python number (numpy's weak scalar)
        masks = [m >= threshold for m in mask_arrays]
    else:
        labelsmap = mask_arrays[0]
        values = np.unique(labelsmap)
        values = values[values != 0]
        masks = [labelsmap == v for v in values]
    if len(masks) != len(labels):
        raise ValueError("Number of masks must match number of labels")
    if len(shape) < 4:
        raise IndexError("tuple index out of range")
    series = np.zeros((shape[3], len(masks)))
    data = _nibabel_data(load_nibabel(in_path, n_threads=n_threads))
    for j, mask in enumerate(masks):
        series[:, j] = _region_means(data, mask, numpy_version, n_threads)
    del data
    output = np.vstack((labels, series.astype(str)))
    broken = [str(s) for s in output[0] if any(c in str(s) for c in "\t\n\r")]
    if broken:
        if mode == "fix":
            raise ValueError(
                f"the class labels {broken!r} hold a tab or a line break, which breaks the "
                "table (PIPE-78: niworkflows writes them as they are; "
                "upstream_defects='reproduce' does too)"
            )
        _defects.warn(
            [
                (
                    "PIPE-78",
                    "niworkflows SignalExtraction",
                    mode,
                    f"the class labels {broken!r} hold a tab or a line break, written as they are",
                )
            ],
            stacklevel=2,
        )
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(os.path.abspath(os.fspath(out_dir)), os.fspath(out_file))
    with open(path, "wb") as fh:  # numpy's savetxt: a text file in UTF-8 (Linux's), "\n"
        for row in output:
            fh.write(("\t".join(str(v) for v in row) + "\n").encode("utf-8"))
    return path
