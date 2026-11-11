# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.fslroi`` and ``lx.fsl.standard_space_roi``: since 2026-11-05 their clean-room
originals run (``implementation="clean-room"``, and ``"auto"`` without the replica's program);
the bit-exact replicas are ``larmorx-nc``'s. Their outputs are checked against FSL by the BET
suite (``fsl-bet``, both implementations) and on every platform by the golden cases; here, the
wrappers' arguments, the outputs' grids and types, and the search of FSL's standard images
(``specs/bet.md`` §5.4) without FSL or the network."""

from __future__ import annotations

import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica

pytestmark = pytest.mark.usefixtures("no_replica")


def _image(shape=(6, 5, 4), dtype=np.float32, t: int | None = None) -> lx.Image:
    n = int(np.prod(shape)) * (t or 1)
    data = (np.arange(n) % 97).astype(dtype).reshape((*shape, t) if t else shape, order="F")
    affine = np.diag([2.0, 3.0, 4.0, 1.0])
    affine[:3, 3] = [-10.0, 20.0, 5.0]
    return lx.Image(data, affine)


def test_fslroi_needs_a_region():
    with pytest.raises(ValueError, match="needs a region"):
        lx.fsl.fslroi(_image())


@pytest.mark.parametrize("implementation", ["auto", "clean-room"])
def test_fslroi_cuts_a_box(implementation):
    img = _image()
    out = lx.fsl.fslroi(img, x=(-1, 3), y=(1, -1), implementation=implementation)
    data = np.asarray(out.data)
    assert data.shape == (3, 4, 4)
    src = np.asarray(img.data)
    # voxel 0 along x is outside the image (zeros); the rest is the input's
    assert (data[0] == 0).all()
    np.testing.assert_array_equal(data[1:], src[0:2, 1:5, :])
    # the box's corner keeps its world position: x -1 -> -12 mm, y 1 -> 23 mm
    np.testing.assert_allclose(out.affine[:3, 3], [-12.0, 23.0, 5.0])
    assert out.implementation.kind == "clean-room"


def test_fslroi_volumes_and_types():
    img = _image(dtype=np.int16, t=5)
    out = lx.fsl.fslroi(img, t=(2, -1), implementation="clean-room")
    data = np.asarray(out.data)
    assert data.shape == (6, 5, 4, 3)
    assert data.dtype == np.int16
    np.testing.assert_array_equal(data, np.asarray(img.data)[..., 2:])


def test_fslroi_keeps_int8_by_default(tmp_path):
    # an int8 file (in memory, the wrapper writes int8 images as float32)
    path = tmp_path / "i8.nii"
    lx.save(lx.Image(np.full((3, 3, 3), -5, np.int8), np.eye(4)), path)
    img = str(path)
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-217"):
        out = lx.fsl.fslroi(img, t=(0, 1), implementation="clean-room")
    assert np.asarray(out.data).dtype == np.int8
    assert (np.asarray(out.data) == -5).all()
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-217"):
        out = lx.fsl.fslroi(
            img, t=(0, 1), implementation="clean-room", upstream_defects="reproduce"
        )
    assert (np.asarray(out.data) == 251).all()


def test_fslroi_invalid_region_raises():
    # FSL-215: the error exits 1 by default (FSL's fslroi exits 0)
    with (
        pytest.warns(lx.UpstreamDefectWarning, match="FSL-215"),
        pytest.raises(RuntimeError, match="Invalid ROI dimensions"),
    ):
        lx.fsl.fslroi(_image(), t=(0, 0), implementation="clean-room")


def _head(shape=(20, 22, 18)) -> np.ndarray:
    z, y, x = np.meshgrid(*[np.arange(n, dtype=np.float32) for n in shape[::-1]], indexing="ij")
    c = [(n - 1) / 2 for n in shape]
    r = ((x - c[0]) / 7) ** 2 + ((y - c[1]) / 8) ** 2 + ((z - c[2]) / 6) ** 2
    return np.transpose(np.where(r < 1, 100.0 + 10 * x, 5.0).astype(np.float32), (2, 1, 0))


def test_standard_space_roi_with_its_own_reference():
    head = lx.Image(_head(), np.diag([2.0, 2.0, 2.0, 1.0]))
    result = lx.fsl.standard_space_roi(
        head,
        reference=head,
        flirt_args=["-dof", "6", "-nosearch"],
        implementation="clean-room",
    )
    # the image registered to itself: the identity, the field of view less its outer voxel
    np.testing.assert_allclose(result.to_standard, np.eye(4), atol=1e-3)
    np.testing.assert_allclose(result.from_standard, np.eye(4), atol=1e-3)
    data = np.asarray(result.image.data)
    assert data.dtype == np.float32
    assert data.shape[0] <= 20 and data.shape[0] >= 17
    assert result.image.implementation.kind == "clean-room"


def test_standard_space_roi_says_where_it_looked(monkeypatch, tmp_path):
    # no FSLDIR, no named directory, an empty cache and no package: refused, every place named
    monkeypatch.delenv("FSLDIR", raising=False)
    monkeypatch.delenv("LARMORX_FSL_DATA", raising=False)
    monkeypatch.setenv("LARMORX_DATA_DIR", str(tmp_path / "cache"))
    monkeypatch.setenv("LARMORX_FSL_DATA_PACKAGE", str(tmp_path / "no-package.tar.bz2"))
    head = lx.Image(_head(), np.diag([2.0, 2.0, 2.0, 1.0]))
    with pytest.raises(RuntimeError) as e:
        lx.fsl.standard_space_roi(head, implementation="clean-room")
    text = str(e.value)
    assert "MNI152_T1_2mm was not found" in text
    assert "$FSLDIR (not set)" in text and "larmorx's data cache" in text
    # a named directory without the image: the only place looked
    with pytest.raises(RuntimeError, match="LARMORX_FSL_DATA="):
        lx.fsl.standard_space_roi(head, fsl_data=tmp_path, implementation="clean-room")


def test_the_replica_is_not_required():
    for implementation in ("replica",):
        with pytest.raises(_replica.ReplicaNotFoundError, match="larmorx-nc"):
            lx.fsl.fslroi(_image(), t=(0, 1), implementation=implementation)


def test_registry_lists_their_originals():
    reg = _replica.registry()
    for tool in ("fslroi", "standard_space_roi"):
        r = reg[("fsl", tool)]
        assert r.original
        assert r.package == "larmorx-nc"
