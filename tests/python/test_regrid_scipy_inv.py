# SPDX-License-Identifier: Apache-2.0
"""SciPy's ``linalg.inv`` (``larmorx.transforms._linalg.scipy_inv``), SciPy's Gaussian filter and
niworkflows' ``RegridToZooms`` (``lx.niworkflows.regrid_to_zooms``): the structures, errors,
warnings and the regridded grid. Bit-for-bit agreement with SciPy and niworkflows is checked in
``validation/investigations/sdcflows-fieldmap-b/check_scipy_inv.py`` and ``check_regrid.py``."""

from __future__ import annotations

import warnings

import numpy as np
import pytest

import larmorx as lx
from larmorx._defects import UpstreamDefectWarning
from larmorx.transforms._linalg import LinAlgWarning, scipy_inv

OBLIQUE = np.array(
    [[0.96, -0.28, 0.0, -20.0], [0.28, 0.96, 0.0, 12.0], [0.0, 0.0, 2.0, 7.5], [0, 0, 0, 1.0]]
)


@pytest.mark.parametrize("version", ["1.15.3", "1.18.1"])
def test_scipy_inv_inverts_every_structure(version):
    plumb = np.diag([-2.0, 2.0, 2.5, 1.0])
    plumb[:3, 3] = (90.0, -126.0, -72.0)
    lower = np.tril(np.arange(1.0, 17.0).reshape(4, 4)) + 3 * np.eye(4)
    for m in (OBLIQUE, plumb, lower, np.diag([2.0, -4.0, 0.5, 1.0])):
        x = scipy_inv(m, scipy_version=version)
        assert np.allclose(x @ m, np.eye(4), atol=1e-12)
    # 1.18 sets the other triangle of a triangular matrix to +0
    if version.startswith("1.18"):
        x = scipy_inv(lower, scipy_version=version)
        assert all(
            np.signbit(x[r, c]) == 0 and x[r, c] == 0 for r in range(4) for c in range(r + 1, 4)
        )


def test_scipy_inv_errors_and_messages():
    s = OBLIQUE.copy()
    s[1] = s[0]
    with pytest.raises(np.linalg.LinAlgError, match=r"^singular matrix$"):
        scipy_inv(s, scipy_version="1.15.3")
    with pytest.raises(np.linalg.LinAlgError, match=r"slice\(s\) \[0\] are singular"):
        scipy_inv(s, scipy_version="1.18.1")
    with pytest.raises(ValueError, match="infs or NaNs"):
        scipy_inv(np.diag([1.0, np.nan, 1.0, 1.0]))
    with pytest.raises(ValueError, match="expected square matrix"):
        scipy_inv(np.ones((3, 4)), scipy_version="1.15.3")
    with pytest.raises(ValueError, match="Expected square matrix"):
        scipy_inv(np.ones((3, 4)))
    with pytest.raises(NotImplementedError):
        scipy_inv(np.eye(3))
    sym = np.array([[0, 1, 0, 0], [1, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1.0]])
    with pytest.raises(np.linalg.LinAlgError, match=r"slice\(s\) \[0\] are singular"):
        scipy_inv(sym)
    with pytest.raises(ValueError, match=r"1\.15 or 1\.18"):
        scipy_inv(OBLIQUE, scipy_version="1.16.0")


def test_scipy_inv_symmetric_paths():
    """1.18's symmetric path: Cholesky for a definite matrix, Bunch-Kaufman (a 2 x 2 pivot)
    for an axis swap with a zero origin; the lower triangle filled from the upper."""
    spd = np.array(
        [[4.0, 1.0, 0.5, 0.25], [1.0, 3.0, 0.2, 0.1], [0.5, 0.2, 2.0, 0.3], [0.25, 0.1, 0.3, 1.5]]
    )
    swap = np.array([[0, 2, 0, 0], [2, 0, 0, 0], [0, 0, -1.5, 0], [0, 0, 0, 1.0]])
    for m in (spd, swap):
        x = scipy_inv(m)
        assert np.array_equal(x, x.T)
        assert np.allclose(x @ m, np.eye(4), atol=1e-14)
    # SciPy 1.18.1's bits (its wheel, OPENBLAS_CORETYPE=Haswell), signed zeros included.
    expected = np.array(
        [
            [-0.0, 0.5, 0.0, 0.0],
            [0.5, -0.0, 0.0, 0.0],
            [0.0, 0.0, -2 / 3, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    )
    assert scipy_inv(swap).tobytes() == expected.tobytes()


@pytest.mark.parametrize(
    ("matrix", "rcond"),
    [
        # SciPy 1.18.1's warnings, every path (the values its LAPACK estimates print).
        (np.diag([1.0, 2.0, 1e-17, 1.0]), "2e+17"),  # diagonal: the condition number (SCI-20)
        (
            np.array([[1, 1, 0, 0], [1, 1 + 2**-52, 0, 0], [0, 0, 1, 0], [0, 0, 1, 1.0]]),
            "5.551115123125783e-17",
        ),  # general: gecon
        (
            np.array([[1e-17, 1, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]]),
            "5e-18",
        ),  # upper triangular: trcon
        (
            np.array([[1, 1, 0, 0], [1, 1 + 2**-51, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]]),
            "1.110223024625156e-16",
        ),  # symmetric definite: pocon
        (
            np.array([[1e-17, 1, 0, 0], [1, 1e-17, 0, 0], [0, 0, 1e-17, 0], [0, 0, 0, 1.0]]),
            "1e-17",
        ),  # symmetric indefinite: sycon
    ],
)
def test_scipy_inv_warning_texts(matrix, rcond):
    with pytest.warns(LinAlgWarning) as w:
        scipy_inv(matrix)
    assert [str(x.message) for x in w] == [
        f"An ill-conditioned matrix detected: slice 0 has rcond = {rcond}."
    ]
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        scipy_inv(matrix, scipy_version="1.15.3")  # 1.15 never warns


def test_scipy_inv_warning_and_sci18():
    with pytest.warns(LinAlgWarning, match="slice 0 has rcond = "):
        scipy_inv(np.diag([1.0, 2.0, 1e-17, 1.0]))
    big = np.array([[1.7e308, 1.7e308, 0, 1], [0.5, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]])
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        lu = scipy_inv(big, upstream_defects="reproduce")
        fixed = scipy_inv(big)
        old = scipy_inv(big, scipy_version="1.15.3")
    ids = [x.message.entry for x in w if isinstance(x.message, UpstreamDefectWarning)]
    assert ids == ["SCI-18", "SCI-18"]
    assert lu[0, 0] == 1.7e308  # the LU factors: U's first pivot
    assert np.array_equal(fixed, old)


def test_gaussian_filter():
    a = np.zeros((9, 7))
    a[4, 3] = 1.0
    g = lx.ndimage.gaussian_filter(a, 1.0)
    assert abs(g.sum() - 1.0) < 1e-12 and g[4, 3] == g.max()
    assert np.array_equal(lx.ndimage.gaussian_filter(a, 0.0), a)
    assert lx.ndimage.gaussian_filter(a.astype(np.int16), 1.0).dtype == np.int16
    with pytest.raises(ValueError, match="Radius must be a nonnegative integer"):
        lx.ndimage.gaussian_filter1d(a, 1.0, radius=-1)
    with pytest.raises(RuntimeError, match="no filter weights given"):
        lx.ndimage.correlate1d(a, [])
    c = lx.ndimage.correlate1d(np.arange(5.0), [1.0, 0.0, -1.0], mode="nearest")
    assert c.tolist() == [-1.0, -2.0, -2.0, -2.0, -1.0]


def _hex(values) -> list[float]:
    return [float.fromhex(v) for v in values]


def test_gaussian_derivatives():
    """SciPy 1.18.1's values (numpy 2.5.4 and 2.3.5 alike): the derivative kernels (numpy's
    dot through OpenBLAS's Haswell dgemv: the 4 x 4 kernel and the tails for order 3, every
    column kernel and a two-row tail for order 5, a dot product for radius 0) and a filter."""
    from larmorx._ndimage_filters import _gaussian_kernel1d

    want = {
        (1.5, 3, 2): ["-0x1.daf95d98e30e3p-5", "-0x1.e3964dba7c92bp-4", "0x0.0p+0",
                      "0x1.e3964dba7c92bp-4", "0x1.daf95d98e30e3p-5"],
        (0.8, 5, 3): ["0x1.748b02cfabf98p-2", "-0x1.6912234e19544p+0", "0x1.94fc40c9cdce9p+0",
                      "0x0.0p+0", "-0x1.94fc40c9cdce9p+0", "0x1.6912234e19544p+0",
                      "-0x1.748b02cfabf98p-2"],
        (1.0, 2, 0): ["-0x1.0000000000000p+0"],
    }  # fmt: skip
    for (sigma, order, radius), values in want.items():
        got = _gaussian_kernel1d(sigma, order, radius, "2.5.4")
        assert got.tobytes() == np.array(_hex(values)).tobytes()
    g = lx.ndimage.gaussian_filter1d(np.arange(9.0) ** 2, 1.0, order=1)
    assert g.tobytes() == np.array(_hex([
        "0x1.45b4963028e16p-1", "0x1.13405c3ef342cp+1", "0x1.00ef6dad9e59ep+2",
        "0x1.8001b15c92e93p+2", "0x1.fff6901339537p+2", "0x1.3faf8cf3b08a0p+3",
        "0x1.77ddac1c5ca50p+3", "0x1.6e12282e221e6p+3", "0x1.4bcd61001bc7dp+2",
    ])).tobytes()  # fmt: skip
    with pytest.raises(ValueError, match="order must be non-negative"):
        lx.ndimage.gaussian_filter1d(g, 1.0, order=-1)
    with pytest.raises(TypeError, match="expected a sequence of integers"):
        lx.ndimage.gaussian_filter1d(g, 1.0, order=1.0)
    m = np.arange(12.0).reshape(3, 4) - 5.5
    assert lx._core.numpy_matvec_f64(m, np.array([1.0, -2.0, 0.5, 3.0])).tolist() == [
        m[i] @ [1.0, -2.0, 0.5, 3.0] for i in range(3)
    ]


def test_complex_correlate():
    """SciPy's complex correlation through the parts (the weights conjugated, a complex cval),
    its output rules and the C parser's messages."""
    c = lx.ndimage.correlate1d(
        np.array([1 + 2j, 3 - 1j, -2 + 0.5j, 4j]),
        np.array([0.5 + 0.25j, 1 - 1j, 0.125j]),
        mode="constant",
        cval=1 - 1j,
    )
    want = [(-0.875, 1.875), (5.0625, 3.0), (-0.75, -2.75), (-5.0, 4.625)]
    assert c.dtype == np.complex128 and [(v.real, v.imag) for v in c] == want
    a = np.arange(12.0).reshape(3, 4)
    assert lx.ndimage.correlate1d(a.astype(np.float32), [1.0, 2j, 1.0]).dtype == np.complex64
    assert lx.ndimage.correlate1d(a.astype(np.int16), [1.0, 2j, 1.0]).dtype == np.complex64
    with pytest.warns(UserWarning, match="promoting specified output dtype to complex"):
        out = lx.ndimage.correlate1d(a + 1j, [1.0, 2.0], output=np.float64)
    assert out.dtype == np.complex128
    with pytest.raises(RuntimeError, match="output must have complex dtype"):
        lx.ndimage.correlate1d(a + 1j, [1.0, 2.0], output="float64")
    with pytest.raises(ValueError, match="complex-valued cval when the input is real"):
        lx.ndimage.correlate1d(a, [1.0, 2j], cval=1j)
    with pytest.raises(TypeError, match="must be real number, not complex"):
        lx.ndimage.correlate1d(a, [1.0, 2.0], cval=1j)
    with pytest.raises(RuntimeError, match=r"array type dtype\('complex128'\) not supported"):
        lx.ndimage.gaussian_filter(a, 1.0, output=np.complex128)
    with pytest.raises(RuntimeError, match=r"array type dtype\('float16'\) not supported"):
        lx.ndimage.correlate1d(a.astype(np.float16), [1.0])
    with pytest.raises(RuntimeError, match="output must have numeric dtype"):
        lx.ndimage.correlate1d(a, [1.0], output="bool")
    ro = np.zeros_like(a)
    ro.flags.writeable = False
    with pytest.raises(ValueError, match="output array is read-only"):
        lx.ndimage.correlate1d(a, [1.0], output=ro)
    z = lx.ndimage.gaussian_filter((a + 2j * a).astype(np.complex64), 1.0, order=(0, 1))
    assert z.dtype == np.complex64
    re = lx.ndimage.gaussian_filter(a.astype(np.float32), 1.0, order=(0, 1))
    assert np.array_equal(z.real, re) and np.array_equal(z.imag, 2 * re)


def test_regrid_to_zooms(tmp_path):
    src = tmp_path / "in.nii.gz"
    data = (np.arange(20 * 18 * 12) % 997).reshape(20, 18, 12).astype(np.int16)
    lx.io.save_nibabel(lx.Image(data, OBLIQUE, None), src)
    res = lx.niworkflows.regrid_to_zooms(src, (2.0, 2.0, 2.0), out_dir=tmp_path / "out")
    assert res.out_file.endswith("in_regrid.nii.gz")
    out = lx.io.load_nibabel(res.out_file)
    # extent 20 x 1 mm, 18 x 1 mm, 12 x 2 mm on a 2 mm grid
    assert out.shape == (10, 9, 12)
    assert out.header is not None
    assert np.allclose(np.linalg.norm(np.asarray(out.affine)[:3, :3], axis=0), 2.0)
    vals = np.asarray(out.data)
    assert vals.min() >= data.min() and vals.max() <= data.max()  # clipped
    four = tmp_path / "four.nii.gz"
    lx.io.save_nibabel(lx.Image(np.zeros((4, 4, 4, 2), np.float32), np.eye(4), None), four)
    with pytest.raises(RuntimeError, match="invalid shape for coordinate array"):
        lx.niworkflows.regrid_to_zooms(four, (2.0, 2.0, 2.0), out_dir=tmp_path)
