# SPDX-License-Identifier: Apache-2.0
"""``lx.afni.tshift`` and ``larmorx afni 3dTshift``: behaviour that does not need AFNI (parity
with AFNI itself is in ``tests/parity`` and ``validation/``)."""

from __future__ import annotations

import contextlib
import io

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

# These tests are about the clean-room original; tests/python/test_replica.py covers the choice
# of implementation.
pytestmark = pytest.mark.usefixtures("no_replica")

TR = 2.0
AFFINE = np.array(
    [[-3.0, 0.1, 0.0, 90.0], [0.0, 3.0, 0.2, -100.0], [0.0, -0.2, 3.5, -60.0], [0, 0, 0, 1]]
)


def bold(shape=(5, 4, 6, 40), dtype=np.float32, seed=0) -> np.ndarray:
    """Smooth oscillations on a linear drift, different in every voxel."""
    rng = np.random.default_rng(seed)
    nt = shape[3]
    t = np.arange(nt)
    phase = rng.uniform(0, 2 * np.pi, shape[:3])[..., None]
    data = 1000 + 40 * np.sin(2 * np.pi * t / 13 + phase) + 0.8 * t + rng.normal(0, 3, shape)
    return np.asfortranarray(data.astype(dtype))


def save(path, data, tr=TR, slope=None):
    img = nib.Nifti1Image(data, AFFINE)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = tr
    if slope is not None:
        img.header.set_slope_inter(slope, 0.0)
    img.to_filename(path)
    if slope is not None:  # nibabel rescales on save: write the raw values instead
        hdr = img.header
        hdr.set_data_dtype(data.dtype)
        with open(path, "wb") as f:
            hdr["vox_offset"] = 352
            hdr.set_slope_inter(slope, 0.0)
            f.write(hdr.binaryblock + b"\0" * 4 + np.asfortranarray(data).tobytes(order="F"))
    return path


def cli(*args) -> tuple[int, str]:
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = cli_run(["larmorx", "afni", "3dTshift", *map(str, args)])
    return code, err.getvalue()


@pytest.mark.parametrize("method", lx.afni.METHODS)
def test_python_matches_the_command_line(tmp_path, method):
    src = save(tmp_path / "bold.nii.gz", bold())
    out = tmp_path / "out.nii.gz"
    code, err = cli("-tpattern", "alt+z", f"-{method}", "-prefix", out, src)
    assert code == 0, err
    img = lx.afni.tshift(src, slice_times="alt+z", method=method, n_threads=3)
    expected = nib.load(out)
    np.testing.assert_array_equal(img.data, np.asanyarray(expected.dataobj))
    np.testing.assert_allclose(img.affine, expected.affine)
    assert img.header.toffset == expected.header["toffset"]
    assert img.implementation == lx.Implementation(
        "original", "larmorx", lx.__version__, "Apache-2.0"
    )


def test_in_memory_images_match_files(tmp_path):
    data = bold()
    src = save(tmp_path / "bold.nii", data)
    times = [0.0, 1.0, 0.333, 1.333, 0.667, 1.667]
    from_file = lx.afni.tshift(src, slice_times=times, method="heptic")
    from_image = lx.afni.tshift(lx.load(src), slice_times=times, method="heptic")
    from_pair = lx.afni.tshift((data, AFFINE), slice_times=times, method="heptic", tr=TR)
    np.testing.assert_array_equal(from_file.data, from_image.data)
    np.testing.assert_array_equal(from_file.data, from_pair.data)
    np.testing.assert_allclose(from_pair.affine, AFFINE, atol=1e-6)


def test_results_do_not_depend_on_threads(tmp_path):
    src = save(tmp_path / "bold.nii", bold(shape=(9, 7, 5, 64)))
    one = lx.afni.tshift(src, slice_times="seq-z", n_threads=1)
    many = lx.afni.tshift(src, slice_times="seq-z", n_threads=0)
    np.testing.assert_array_equal(one.data, many.data)


def test_output_header(tmp_path):
    src = save(tmp_path / "bold.nii", bold())
    img = lx.afni.tshift(src, slice_times="alt+z", tzero=0.5)
    h = img.header
    assert img.data.dtype == np.float32
    assert h.tr == TR
    assert h.toffset == 0.5
    assert (h.slice_code, h.slice_start, h.slice_end, h.slice_duration) == (0, 0, 5, 0.0)
    assert h.dim_info == 48
    assert (h.qform_code, h.sform_code) == (1, 1)
    np.testing.assert_allclose(img.affine, AFFINE, atol=1e-5)


def test_slice_at_tzero_is_unchanged_and_half_tr_shifts_half_a_sample(tmp_path):
    # Slice 0 at 0 (= tzero): left as stored. Slice 1 half a TR late: its output at n is its
    # input at n - 0.5 (spec §1), checked on a pure sine with the Fourier method.
    nt = 64
    t = np.arange(nt)
    data = np.zeros((2, 2, 2, nt), np.float32)
    data[:, :, 0] = 100 + 10 * np.sin(2 * np.pi * t / 16)
    data[:, :, 1] = 100 + 10 * np.sin(2 * np.pi * t / 16)
    img = lx.afni.tshift((data, AFFINE), slice_times=[0.0, TR / 2], tzero=0.0, tr=TR)
    np.testing.assert_array_equal(img.data[:, :, 0], data[:, :, 0])
    expected = 100 + 10 * np.sin(2 * np.pi * (t - 0.5) / 16)
    middle = slice(8, nt - 8)
    # Zero padding rings a little (Gibbs); a shift the wrong way would be off by ~4.
    np.testing.assert_allclose(img.data[0, 0, 1, middle], expected[middle], atol=0.25)


def test_ignored_points_and_restore_modes(tmp_path):
    src = save(tmp_path / "bold.nii", bold())
    shifted = lx.afni.tshift(src, slice_times="alt+z", ignore=3, method="cubic")
    original = lx.load(src).data
    np.testing.assert_array_equal(shifted.data[..., :3], original[..., :3])
    residual = lx.afni.tshift(src, slice_times="alt+z", restore="none", method="cubic")
    assert abs(float(residual.data[..., 1:3, :].mean())) < 5  # detrended: about zero
    with pytest.warns(UserWarning, match="first voxel of each pair"):
        mean_kept = lx.afni.tshift(src, slice_times="alt+z", detrend=False, method="cubic")
    assert abs(float(mean_kept.data.mean() - original.mean())) < 1


def test_brick_factors_are_kept(tmp_path):
    raw = (bold() - 900).astype(np.int16)
    src = save(tmp_path / "scaled.nii", raw, slope=0.25)
    img = lx.afni.tshift(src, slice_times="seq+z", method="linear")
    assert img.header.scl_slope == 0.25
    assert img.data.dtype == np.float64  # scaled as lx.load would return it
    out = tmp_path / "out.nii"
    assert cli("-tpattern", "seq+z", "-linear", "-prefix", out, src)[0] == 0
    np.testing.assert_array_equal(img.data, np.asanyarray(nib.load(out).dataobj))


def test_fmriprep_slice_timing():
    times, t0 = lx.afni.fmriprep_slice_timing([0.0, 0.5, 1.0, 1.5], "k-")
    assert times == [1.5, 1.0, 0.5, 0.0]
    assert t0 == 0.75
    assert lx.afni.fmriprep_slice_timing([0.0, 0.0625, 1.9375])[1] == 0.969


@pytest.mark.parametrize(
    ("kwargs", "match"),
    [
        ({"slice_times": [0.0, 1.0]}, "slice times for 6 slices"),
        ({"slice_times": [0.0, 0.1, 0.2, 0.3, 0.4, 2.5]}, "outside 0..TR"),
        ({"slice_times": "ALT+Z"}, "unknown slice pattern"),
        ({"slice_times": "alt+z", "method": "spline"}, "unknown method"),
        ({"slice_times": "alt+z", "restore": "mean"}, "restore must be"),
        (
            {"slice_times": "alt+z", "detrend": False, "restore": "none", "method": "linear"},
            "restore",
        ),
        ({"slice_times": "alt+z", "ignore": 36}, "too large"),
        ({"slice_times": "alt+z", "slice": 6}, "out of range"),
        ({"slice_times": "alt+z", "tzero": -1.0}, "non-negative"),
    ],
)
@pytest.mark.filterwarnings("ignore:detrend=False")
def test_invalid_arguments(tmp_path, kwargs, match):
    src = save(tmp_path / "bold.nii", bold())
    with pytest.raises(ValueError, match=match):
        lx.afni.tshift(src, **kwargs)


def test_in_memory_images_need_a_tr():
    with pytest.raises(ValueError, match="tr is required"):
        lx.afni.tshift((bold(), AFFINE), slice_times="alt+z")


# --- the command line ---------------------------------------------------------------------------


def test_cli_errors(tmp_path):
    src = save(tmp_path / "bold.nii", bold())
    out = tmp_path / "out.nii"
    assert cli("-bogus", src)[0] == 1
    assert "-prefix" in cli("-tpattern", "alt+z", src)[1]
    assert "must end in .nii" in cli("-tpattern", "alt+z", "-prefix", tmp_path / "x", src)[1]
    assert cli("-tpattern", "alt+z", "-prefix", out, src)[0] == 0
    code, err = cli("-tpattern", "alt+z", "-prefix", out, src)
    assert code == 1 and "conflicts with existing file" in err
    assert cli("-no_detrend", "-heptic", "-prefix", tmp_path / "y.nii", src)[0] == 1
    assert "Unknown tpattern" in cli("-tpattern", "bogus", "-prefix", tmp_path / "z.nii", src)[1]
    # -verbose names the implementation first.
    code, err = cli("-verbose", "-tpattern", "alt+z", "-prefix", tmp_path / "v.nii", src)
    assert code == 0 and err.startswith("++ implementation: original (larmorx "), err


def test_cli_copies_input_without_slice_timing(tmp_path):
    data = bold()
    src = save(tmp_path / "bold.nii", data)
    out = tmp_path / "copy.nii"
    code, err = cli("-prefix", out, src)
    assert code == 0
    assert "already aligned in time" in err
    np.testing.assert_array_equal(np.asanyarray(nib.load(out).dataobj), data)


def test_cli_reads_slice_timing_from_the_header(tmp_path):
    data = bold()
    img = nib.Nifti1Image(data, AFFINE)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = TR
    img.header.set_dim_info(slice=2)
    img.header["slice_code"] = 1  # sequential increasing
    img.header["slice_start"] = 0
    img.header["slice_end"] = 5
    img.header["slice_duration"] = TR / 6
    src = tmp_path / "timed.nii"
    img.to_filename(src)
    out = tmp_path / "out.nii"
    assert cli("-linear", "-prefix", out, src)[0] == 0
    expected = lx.afni.tshift(src, slice_times="seq+z", method="linear")
    np.testing.assert_allclose(np.asanyarray(nib.load(out).dataobj), expected.data, atol=1e-3)


def test_cli_tpattern_file_like_fmriprep(tmp_path):
    src = save(tmp_path / "bold.nii", bold())
    times = [0.0, 1.0, 0.333, 1.333, 0.667, 1.667]
    pattern = tmp_path / "slice_timing.1D"
    pattern.write_text("\t".join(str(float(t)) for t in times) + "\n", encoding="utf-8")
    out = tmp_path / "out.nii"
    t0 = round(min(times) + 0.5 * (max(times) - min(times)), 3)
    args = ["-ignore", 0, "-tzero", t0, "-TR", "2.0s", "-tpattern", f"@{pattern}"]
    assert cli(*args, "-prefix", out, src)[0] == 0
    img = lx.afni.tshift(src, slice_times=times, tzero=t0, tr=2.0)
    np.testing.assert_array_equal(np.asanyarray(nib.load(out).dataobj), img.data)
