# SPDX-License-Identifier: Apache-2.0
"""D13, "correct by default, faithful on request": the ``upstream_defects`` switch, its
warnings and its environment variable, for every defect larmorx corrects
(``docs/api/upstream-defects.md``). Each test runs both modes and checks that the warning
names the defect exactly when the two differ."""

from __future__ import annotations

import contextlib
import io
import os
import warnings

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx import _defects
from larmorx.cli import run as cli_run

pytestmark = pytest.mark.usefixtures("no_replica")

FIX, REPRODUCE = "fix", "reproduce"


def defect_warnings(fn, *args, **kwargs):
    """``fn``'s result and the IDs and modes of the ``UpstreamDefectWarning``s it raised."""
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        result = fn(*args, **kwargs)
    found = [
        (w.message.entry, w.message.mode)
        for w in record
        if isinstance(w.message, lx.UpstreamDefectWarning)
    ]
    return result, found


# --- The mechanism -------------------------------------------------------------------------------


def test_the_registry():
    entries = {d.id: d for d in _defects.defects()}
    for entry in ("PIPE-03", "PIPE-04", "SCI-03", "AFNI-01", "FSL-01", "FSL-14"):
        assert entries[entry].fixed_by_default
    assert not entries["FSL-02"].fixed_by_default  # measured, not decided
    assert _defects.UPSTREAM_DEFECTS == ("fix", "reproduce")
    with pytest.raises(ValueError, match="must be 'fix' or 'reproduce'"):
        _defects.check("faithful")


def test_warnings_and_notice_lines():
    w = lx.UpstreamDefectWarning("FSL-01", "mcflirt", FIX, "the last volume starts from 238")
    assert issubclass(lx.UpstreamDefectWarning, UserWarning)
    assert str(w).startswith("FSL-01 (mcflirt) corrected: the last volume starts from 238.")
    assert "upstream_defects='reproduce'" in str(w)
    # The line of a command line or replica (larmorx_core::defects::Notice::line).
    line = (
        "larmorx: upstream defect FSL-14 (mcflirt) reproduced: volume 3 has NaN voxels "
        "[LARMORX_UPSTREAM_DEFECTS=fix corrects it]"
    )
    parsed = _defects.parse_line(line)
    assert (parsed.entry, parsed.tool, parsed.mode, parsed.detail) == (
        "FSL-14",
        "mcflirt",
        REPRODUCE,
        "volume 3 has NaN voxels",
    )
    assert _defects.parse_line("*+ WARNING: something else") is None


# --- SciPy (SCI-03) ------------------------------------------------------------------------------


@pytest.mark.parametrize("mode", ["reflect", "grid-mirror"])
def test_reflect_splines(mode):
    data = (np.arange(6 * 5 * 4, dtype=np.float64).reshape(6, 5, 4) * 37 % 101) / 7
    coords = np.indices(data.shape).reshape(3, -1).astype(np.float64)
    fixed, wf = defect_warnings(lx.ndimage.map_coordinates, data, coords, order=5, mode=mode)
    scipy, wr = defect_warnings(
        lx.ndimage.map_coordinates, data, coords, order=5, mode=mode, upstream_defects=REPRODUCE
    )
    np.testing.assert_allclose(fixed, data.ravel(), atol=1e-9)  # an interpolant
    assert np.abs(scipy - data.ravel()).max() > 1e-3  # SciPy's is not, on short axes
    assert wf == [("SCI-03", FIX)] and wr == [("SCI-03", REPRODUCE)]
    _, wf = defect_warnings(lx.ndimage.spline_filter, data, order=3, mode=mode)
    assert wf == [("SCI-03", FIX)]
    # Other boundaries: one result and no warning.
    a, wa = defect_warnings(lx.ndimage.map_coordinates, data, coords, order=3, mode="mirror")
    b, wb = defect_warnings(
        lx.ndimage.map_coordinates, data, coords, order=3, mode="mirror", upstream_defects=REPRODUCE
    )
    np.testing.assert_array_equal(a, b)
    assert wa == wb == []


# --- fMRIPrep's resampler (PIPE-03) and nitransforms (PIPE-04) -----------------------------------


def _series(shape=(6, 5, 4), n=2):
    data = np.arange(np.prod(shape) * n, dtype=np.float32).reshape(*shape, n, order="F") % 17
    return np.asfortranarray(data + 100)


def test_jacobian_axis():
    src = (_series(), np.diag([3.0, 3.0, 3.0, 1.0]))
    fmap = (np.indices((6, 5, 4)).sum(axis=0) % 5).astype(np.float32) * 3.0
    kwargs = dict(fieldmap=fmap, pe_dir="j", ro_time=0.05, order=1, mode="nearest")
    # The source's own grid: fMRIPrep's Jacobian is right, both modes agree, no warning.
    native = ((6, 5, 4), np.diag([3.0, 3.0, 3.0, 1.0]))
    a, wa = defect_warnings(lx.transforms.resample_series, src, native, **kwargs)
    b, wb = defect_warnings(
        lx.transforms.resample_series, src, native, upstream_defects=REPRODUCE, **kwargs
    )
    np.testing.assert_array_equal(a.data, b.data)
    assert wa == wb == []
    # A grid with half the voxel size along y: the derivative per source voxel is twice the
    # derivative per target voxel; fMRIPrep's is per target voxel.
    fine = ((6, 10, 4), np.diag([3.0, 1.5, 3.0, 1.0]))
    fmap2 = (np.indices((6, 10, 4)).sum(axis=0) % 5).astype(np.float32) * 3.0
    kwargs["fieldmap"] = fmap2
    a, wa = defect_warnings(lx.transforms.resample_series, src, fine, **kwargs)
    b, wb = defect_warnings(
        lx.transforms.resample_series, src, fine, upstream_defects=REPRODUCE, **kwargs
    )
    assert not np.array_equal(a.data, b.data)
    assert wa == [("PIPE-03", FIX)] and wb == [("PIPE-03", REPRODUCE)]


def _composite(path, n):
    """An ITK composite .h5 file of ``n`` translations (group "0" is the composite itself)."""
    xfm = lx.transforms.ItkTransform
    rows = [xfm("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0))]
    for i in range(1, n + 1):
        params = np.array([1, 0, 0, 0, 1, 0, 0, 0, 1, i, 0, 0], np.float64)
        rows.append(xfm("AffineTransform_double_3_3", params, np.zeros(3)))
    lx.transforms.write(path, rows)


def test_composite_order(tmp_path):
    small, large = tmp_path / "small.h5", tmp_path / "large.h5"
    _composite(small, 3)
    _composite(large, 11)
    _, w = defect_warnings(lx.transforms.load_itk_composite, small)
    assert w == []
    order = lambda c: [-round(s.matrix[0, 3]) for s in c]  # noqa: E731 (LPS: x negated)
    fixed, wf = defect_warnings(lx.transforms.load_itk_composite, large)
    named, wr = defect_warnings(lx.transforms.load_itk_composite, large, upstream_defects=REPRODUCE)
    # The chain lists the transforms in reverse file order.
    assert order(fixed) == list(range(11, 0, -1))
    assert order(named) == [9, 8, 7, 6, 5, 4, 3, 2, 11, 10, 1]  # name order: 1, 10, 11, 2, ...
    assert wf == [("PIPE-04", FIX)] and wr == [("PIPE-04", REPRODUCE)]
    _, w = defect_warnings(lx.transforms.load_transforms, [large])
    assert w == [("PIPE-04", FIX)]


# --- 3dTshift (AFNI-01, -02, -03, -05, -07) ------------------------------------------------------


def _raw_nifti(path, data, slope, inter=0.0, units=(2, 8), affine=None, tweak=None):
    """A NIfTI file with ``data`` stored as is and the scaling fields set directly."""
    img = nib.Nifti1Image(data, np.diag([3.0, 3.0, 3.5, 1.0]) if affine is None else affine)
    hdr = img.header
    hdr.set_data_dtype(data.dtype)
    hdr["xyzt_units"] = units[0] | units[1]
    hdr["pixdim"][4] = 2.0
    hdr["vox_offset"] = 352
    hdr.set_slope_inter(slope, inter)
    if tweak:
        tweak(hdr)
    with open(path, "wb") as f:
        f.write(hdr.binaryblock + b"\0" * 4 + np.asfortranarray(data).tobytes(order="F"))
    return path


def _bold(dtype=np.int16, shape=(3, 2, 4, 12)):
    t = np.arange(shape[3])
    data = 300 + 20 * ((t * 7) % 11) + np.arange(np.prod(shape[:3])).reshape(*shape[:3], 1)
    return np.asfortranarray(data.astype(dtype))


def _cli(*args, env=None):
    err = io.StringIO()
    old = os.environ.get(_defects.ENV)
    try:
        if env is not None:
            os.environ[_defects.ENV] = env
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            code = cli_run(["larmorx", *map(str, args)])
    finally:
        if old is None:
            os.environ.pop(_defects.ENV, None)
        else:
            os.environ[_defects.ENV] = old
    return code, err.getvalue()


def test_negative_slope(tmp_path):
    raw = _bold()
    src = _raw_nifti(tmp_path / "neg.nii", raw, -0.5)
    fixed, wf = defect_warnings(lx.afni.tshift, src, slice_times="alt+z", method="linear")
    afni, wr = defect_warnings(
        lx.afni.tshift, src, slice_times="alt+z", method="linear", upstream_defects=REPRODUCE
    )
    true = raw * -0.5
    assert (fixed.data < 0).all() and abs(float(fixed.data.mean() - true.mean())) < 5
    assert (afni.data[..., 1, :] > 0).all()  # AFNI's shifted slices are sign-flipped
    assert ("AFNI-01", FIX) in wf and ("AFNI-01", REPRODUCE) in wr
    # The command line: the same choice through LARMORX_UPSTREAM_DEFECTS.
    for env, mode, expected in ((None, "corrected", fixed), ("reproduce", "reproduced", afni)):
        out = tmp_path / f"cli-{env}.nii"
        code, err = _cli(
            "afni", "3dTshift", "-linear", "-tpattern", "alt+z", "-prefix", out, src, env=env
        )
        assert code == 0 and f"upstream defect AFNI-01 (3dTshift) {mode}" in err
        np.testing.assert_array_equal(np.asanyarray(nib.load(out).dataobj), expected.data)
    code, err = _cli(
        "afni", "3dTshift", "-tpattern", "alt+z", "-prefix", tmp_path / "x.nii", src, env="exact"
    )
    assert code == 2 and "LARMORX_UPSTREAM_DEFECTS must be 'fix' or 'reproduce'" in err


def test_float32_slope_and_intercept(tmp_path):
    raw = _bold(np.float32)
    src = _raw_nifti(tmp_path / "si.nii", raw, 2.0, 10.0)
    fixed, wf = defect_warnings(lx.afni.tshift, src, slice_times="alt+z", method="linear")
    afni, wr = defect_warnings(
        lx.afni.tshift, src, slice_times="alt+z", method="linear", upstream_defects=REPRODUCE
    )
    assert fixed.header.scl_slope == 0  # scaled once, no factor kept
    assert afni.header.scl_slope == 2.0  # AFNI keeps the slope: scaled twice
    assert abs(float(fixed.data.mean()) - float((2 * raw + 10).mean())) < 5
    assert abs(float(afni.data.mean()) - float(2 * (2 * raw + 10).mean())) < 10
    assert ("AFNI-02", FIX) in wf and ("AFNI-02", REPRODUCE) in wr


def test_no_detrend(tmp_path):
    src = _raw_nifti(tmp_path / "b.nii", _bold(np.float32), 1.0)
    fixed, wf = defect_warnings(
        lx.afni.tshift, src, slice_times="alt+z", method="cubic", detrend=False
    )
    afni, wr = defect_warnings(
        lx.afni.tshift,
        src,
        slice_times="alt+z",
        method="cubic",
        detrend=False,
        upstream_defects=REPRODUCE,
    )
    # The first voxel of each pair is treated the same; the second differs.
    np.testing.assert_array_equal(fixed.data[0, 0], afni.data[0, 0])
    assert not np.array_equal(fixed.data[1, 0], afni.data[1, 0])
    assert wf == [("AFNI-05", FIX)] and wr == [("AFNI-05", REPRODUCE)]


def test_metre_units(tmp_path):
    affine = np.array(
        [[0.003, 0, 0, -0.1], [0, 0.003, 0, 0.05], [0, 0, 0.0035, 0.02], [0, 0, 0, 1]]
    )
    src = _raw_nifti(tmp_path / "m.nii", _bold(np.float32), 1.0, units=(1, 8), affine=affine)
    fixed, wf = defect_warnings(lx.afni.tshift, src, slice_times="alt+z")
    afni, wr = defect_warnings(lx.afni.tshift, src, slice_times="alt+z", upstream_defects=REPRODUCE)
    np.testing.assert_allclose(fixed.affine[:3, 3], [-100, 50, 20], atol=1e-4)
    np.testing.assert_allclose(afni.affine[:3, 3], [-0.1, 0.05, 0.02], atol=1e-6)
    np.testing.assert_allclose(fixed.affine[:3, :3], afni.affine[:3, :3])
    np.testing.assert_array_equal(fixed.data, afni.data)
    assert ("AFNI-07", FIX) in wf and ("AFNI-07", REPRODUCE) in wr


def test_header_slice_code_2(tmp_path):
    def code2(h):
        h["dim_info"] = 48
        h["slice_code"] = 2
        h["slice_start"] = 0
        h["slice_end"] = 3
        h["slice_duration"] = 0.5

    src = _raw_nifti(tmp_path / "c2.nii", _bold(np.float32), 1.0, tweak=code2)
    outs = {}
    for env in ("fix", "reproduce"):
        out = tmp_path / f"{env}.nii"
        code, err = _cli("afni", "3dTshift", "-linear", "-prefix", out, src, env=env)
        assert code == 0 and "upstream defect AFNI-03" in err
        outs[env] = np.asanyarray(nib.load(out).dataobj)
    np.testing.assert_array_equal(outs["reproduce"], np.asanyarray(nib.load(src).dataobj))
    assert not np.array_equal(outs["fix"], outs["reproduce"])


# --- Head-motion correction (FSL-01, -02, -03, -05 to -10, -13, -14) -----------------------------


def _phantom(shift=(0.0, 0.0, 0.0), shape=(16, 18, 12)):
    idx = np.indices(shape).astype(np.float64)
    c = [(n - 1) / 2 + s for n, s in zip(shape, shift, strict=True)]
    r2 = sum(((idx[k] - c[k]) / (shape[k] / 3)) ** 2 for k in range(3))
    lump = ((idx[0] - 5 - shift[0]) ** 2 + (idx[1] - 7 - shift[1]) ** 2) / 6
    return 800 / (1 + r2**2) + 300 / (1 + lump**2) + 20


def _hmc_series(dtype=np.float32):
    shifts = [(0, 0, 0), (0.4, 0, 0), (0, -0.3, 0.2), (-0.2, 0.3, 0), (0.3, 0.2, -0.1)]
    data = np.stack([_phantom(s) for s in shifts], axis=-1)
    return np.asfortranarray(data.astype(dtype)), np.diag([3.5, 3.5, 4.0, 1.0])


def test_hmc_last_volume_and_nan():
    data, affine = _hmc_series()
    reference = (np.asfortranarray(data[..., 0]), affine)
    fixed, wf = defect_warnings(lx.mri.hmc, (data, affine), reference=reference, resample=False)
    mcflirt, wr = defect_warnings(
        lx.mri.hmc, (data, affine), reference=reference, resample=False, upstream_defects=REPRODUCE
    )
    np.testing.assert_array_equal(fixed.matrices[:-1], mcflirt.matrices[:-1])
    assert not np.array_equal(fixed.matrices[-1], mcflirt.matrices[-1])
    assert wf == [("FSL-01", FIX)] and wr == [("FSL-01", REPRODUCE)]
    # FSL-14: one NaN voxel.
    data[3, 4, 5, 2] = np.nan
    fixed, wf = defect_warnings(lx.mri.hmc, (data, affine), ref_index=0, fudge=True, resample=False)
    mcflirt, wr = defect_warnings(
        lx.mri.hmc,
        (data, affine),
        ref_index=0,
        fudge=True,
        resample=False,
        upstream_defects=REPRODUCE,
    )
    assert np.isnan(mcflirt.matrices[2, :3]).all()
    assert np.isfinite(fixed.matrices).all()
    assert ("FSL-14", FIX) in wf and ("FSL-14", REPRODUCE) in wr


def test_hmc_correlation_count():
    data, affine = _hmc_series()
    a = lx.mri.hmc((data, affine), ref_index=0, resample=False, normcorr_count="weights")
    b = lx.mri.hmc((data, affine), ref_index=0, resample=False)
    assert not np.array_equal(a.matrices, b.matrices)
    with pytest.raises(ValueError, match="cannot reproduce mcflirt"):
        lx.mri.hmc((data, affine), normcorr_count="weights", upstream_defects=REPRODUCE)
    with pytest.raises(ValueError, match="normcorr_count must be"):
        lx.mri.hmc((data, affine), normcorr_count="pearson")


def test_hmc_command_line(tmp_path, monkeypatch):
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_GZ")
    data, affine = _hmc_series()
    img = nib.Nifti1Image(np.round(data).astype(np.int16), affine)
    img.header.set_xyzt_units("mm", "msec")
    img.header["pixdim"][4] = 2000.0
    src = tmp_path / "bold.nii.gz"
    img.to_filename(src)
    results = {}
    for env in ("fix", "reproduce"):
        out = tmp_path / env / "bold_mcf.nii.gz"
        out.parent.mkdir()
        code, err = _cli("mri", "hmc", "-in", src, "-out", out, "-rmsrel", "-stats", env=env)
        assert code == 0, err
        for entry in ("FSL-08", "FSL-09", "FSL-10", "FSL-13"):
            assert f"upstream defect {entry}" in err, (entry, err)
        results[env] = sorted(p.name for p in out.parent.iterdir())
        hdr = nib.load(out).header
        assert hdr.get_xyzt_units() == (("mm", "msec") if env == "fix" else ("mm", "sec"))
    # FSL-08 names, FSL-13 directory.
    assert "bold_mcf_meanvol.nii.gz" in results["fix"]
    assert "bold_mcf.nii.gz.mat" not in results["fix"]
    assert "bold_mcf.nii.gz_meanvol.nii.gz" in results["reproduce"]
    assert "bold_mcf.nii.gz.mat" in results["reproduce"]
    # FSL-06 and FSL-07 on the command line.
    code, err = _cli("mri", "hmc", "-in", src, "-cost", "bogus", env="fix")
    assert code == 255 and "Unrecognised cost function type: bogus" in err
    code, err = _cli("mri", "hmc", "-help", env="fix")
    assert code == 0 and "FSL-07" in err
    code, err = _cli("mri", "hmc", "-help", env="reproduce")
    assert code == 1
