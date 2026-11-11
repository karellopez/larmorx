# SPDX-License-Identifier: Apache-2.0
"""The memory layouts numpy gives the arrays the confounds' reductions run on.

numpy sums along an axis pairwise when that axis is the innermost one in memory, and in index
order otherwise (``larmorx_core::numpy::reduce``), so the result of ``mean``, ``std`` or ``sum``
at a call site depends on whether its operand is C- or Fortran-ordered. This probe rebuilds each
operand the way nipype 1.12.0 (``algorithms/confounds.py``), niworkflows 1.15.4 and fMRIPrep
25.2.7 build it (images written and loaded with nibabel, whose arrays are Fortran-ordered), and
prints its layout: ``C`` (C-contiguous), ``F`` (Fortran-contiguous), ``CF`` (both: one axis of
length > 1 at most), or ``-`` (neither). Run it in every environment::

    python -I probe_reduction_layouts.py
    apptainer exec --cleanenv <fmriprep-25.2.6.sif> python -I probe_reduction_layouts.py

It writes into a temporary folder only. The facts it prints are the input of
``check_numpy_reductions.py``'s layout assertions.
"""

from __future__ import annotations

import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np


def layout(a) -> str:
    a = np.asarray(a)
    c, f = a.flags.c_contiguous, a.flags.f_contiguous
    return {(True, True): "CF", (True, False): "C", (False, True): "F"}.get((c, f), "-")


def row(name: str, a) -> None:
    a = np.asarray(a)
    print(f"{name:58s} {layout(a):3s} {a.dtype.str:5s} {a.shape} {a.strides}")


def images(tmp: Path, dtype: str, shape=(7, 6, 5, 12)):
    rng = np.random.default_rng(3)
    data = (rng.integers(0, 2000, size=shape)).astype(dtype)
    img = nb.Nifti1Image(data, np.eye(4))
    img.header.set_data_dtype(np.dtype(dtype))
    fn = tmp / f"bold_{np.dtype(dtype).str.replace('<', 'l').replace('>', 'b')}.nii.gz"
    img.to_filename(fn)
    mask = (rng.random(shape[:3]) > 0.3).astype("uint8")
    mfn = tmp / "mask.nii.gz"
    nb.Nifti1Image(mask, np.eye(4)).to_filename(mfn)
    return fn, mfn


def main() -> int:
    print(f"numpy {np.__version__}  nibabel {nb.__version__}  python {sys.version.split()[0]}")
    from numpy.polynomial import Legendre

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        for dt in ("<f4", ">f4", "<i2"):
            fn, mfn = images(tmp, dt)
            print(f"--- input dtype {dt}")
            img = nb.load(fn)
            # nipype compute_dvars (ComputeDVARS)
            func = np.float32(nb.load(fn).dataobj)
            mask = np.bool_(nb.load(mfn).dataobj)
            row("dvars: func = np.float32(dataobj)", func)
            mfunc = func[mask]
            row("dvars: mfunc = func[mask]", mfunc)
            mfunc = (mfunc / np.median(mfunc)) * 1000
            row("dvars: (mfunc / median) * 1000", mfunc)
            keep = np.ones(mfunc.shape[0], bool)
            keep[1] = False
            row("dvars: mfunc[zero_variance_voxels, :]", mfunc[keep, :])
            fd = np.diff(mfunc, axis=1)
            row("dvars: func_diff = np.diff(mfunc, axis=1)", fd)
            row("dvars: np.square(func_diff)", np.square(fd))
            sdhat = np.ones(mfunc.shape[0])
            q = fd / np.array([sdhat] * fd.shape[-1]).T
            row("dvars: func_diff / np.array([sdhat]*k).T", q)
            row("dvars: np.square(that)", np.square(q))
            # nipype regress_poly as called with a C (N, T) float32 block
            X = np.ones((mfunc.shape[1], 1))
            betas = np.linalg.pinv(X).dot(mfunc.T)
            datahat = X.dot(betas).T
            row("regress_poly: datahat = X.dot(betas).T", datahat)
            row("regress_poly: data - datahat (data = mfunc)", mfunc - datahat)
            # TCompCor._process_masks: timeseries = np.asanyarray(imgseries.dataobj)
            ts = np.asanyarray(img.dataobj)
            row("tcompcor: np.asanyarray(dataobj)", ts)
            row("tcompcor: timeseries[mask, :]", ts[mask, :])
            sk = img.__class__(img.dataobj[..., 2:], img.affine, img.header)
            row("compcor: dataobj[..., skip:] (skip_vols)", sk.dataobj)
            row("tcompcor: np.asanyarray(skipped)[mask, :]", np.asanyarray(sk.dataobj)[mask, :])
            # compute_noise_components: imgseries = get_fdata(float32)
            ims = img.get_fdata(dtype=np.float32)
            row("compcor: get_fdata(dtype=float32)", ims)
            row("compcor: get_fdata(float32) of the skipped", sk.get_fdata(dtype=np.float32))
            vt = ims[mask, :]
            row("noise: voxel_timecourses = imgseries[mask, :]", vt)
            T = vt.shape[1]
            Xc = np.ones((T, 2))
            Xc[:, 0] = np.arange(T)
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                b = np.linalg.lstsq(Xc, vt.T)[0]
            res = vt - Xc.dot(b).T
            row("cosine_filter: Xc.dot(betas).T", Xc.dot(b).T)
            row("cosine_filter: data - X.dot(betas).T", res)
            row("noise: M = residuals.reshape(shape).T", res.reshape(vt.shape).T)
            # TCompCor's regress_poly(2, imgseries) of the masked dataobj rows
            xs = np.ones((T, 1))
            for i in range(2):
                xs = np.hstack((xs, Legendre.basis(i + 1)(np.linspace(-1, 1, T))[:, None]))
            tsm = ts[mask, :]
            r2 = tsm.reshape((-1, T)) - xs.dot(np.linalg.pinv(xs).dot(tsm.reshape((-1, T)).T)).T
            row("tcompcor: regress_poly(2, timeseries[mask, :])[0]", r2.reshape(tsm.shape))
            # nipype TSNR
            data = np.concatenate(
                [img.get_fdata(dtype=np.float32).reshape((*img.shape[:3], -1))], axis=3
            )
            row("tsnr: concatenate([get_fdata(float32)], axis=3)", data)
            row("tsnr: np.nan_to_num(data)", np.nan_to_num(data))
            d2 = data.reshape((-1, T))
            r = d2 - xs.dot(np.linalg.pinv(xs).dot(d2.T)).T
            row("tsnr: regress_poly(2, data)[0] (reshaped back)", r.reshape(data.shape))
            # nipype NonSteadyStateDetector
            g = img.dataobj[:, :, :, :50]
            row("nss: dataobj[:, :, :, :50]", g)
            g0 = g.mean(axis=0)
            row("nss: .mean(axis=0)", g0)
            g1 = g0.mean(axis=0)
            row("nss: .mean(axis=0).mean(axis=0)", g1)
            # niworkflows SignalExtraction
            fd64 = img.get_fdata()
            row("signals: get_fdata()", fd64)
            row("signals: data[mask, :]", fd64[mask, :])
        # nipype ComputeDVARS: np.mean(dvars, axis=1) of the three series
        a, b2, c = np.ones(5), np.ones(5, np.float32), np.ones(5)
        row("ComputeDVARS: np.asarray((stdz, nstd, vxstdz))", np.asarray((a, b2, c)))
        # nipype FramewiseDisplacement: mpars = np.loadtxt(...) then apply_along_axis
        with open(Path(td) / "m.txt", "w") as fh:
            fh.write("\n".join(" ".join(["0.1"] * 6) for _ in range(5)))
        mp = np.loadtxt(Path(td) / "m.txt")
        row("fd: np.loadtxt(par)", mp)
        mp2 = np.apply_along_axis(lambda r: r, 1, mp)
        row("fd: np.apply_along_axis(..., axis=1)", mp2)
        diff = mp2[:-1, :6] - mp2[1:, :6]
        row("fd: diff = mpars[:-1, :6] - mpars[1:, :6]", diff)
        row("fd: np.abs(diff)", np.abs(diff))
    return 0


if __name__ == "__main__":
    sys.exit(main())
