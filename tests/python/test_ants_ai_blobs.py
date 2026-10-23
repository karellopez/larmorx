# SPDX-License-Identifier: Apache-2.0
"""``antsAI -b`` (the blob alignment): ``lx.ants.blob_alignment`` and ``lx.ants.ai(blobs=...)``.

The phantoms are 2D images of Gaussian blobs (fast; the patch radius of 20 voxels needs blobs
at least 21 voxels from every edge). Their affine is ITK's identity direction (RAS diag(-1, -1)),
so ``fix`` and ``reproduce`` read the same physical points (ITK-02 drops a 2D image's direction
in ``reproduce``). The parity suite compares every step with ANTs itself (``ants-ai``, category
``blobs``)."""

from __future__ import annotations

import contextlib
import io
import warnings

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

SHAPE = (112, 108)
LPS = np.diag([-1.0, -1.0, 1.0, 1.0])


def blob_pair(shift=(0.0, 0.0), angle=0.0, n=10, seed=3):
    """A fixed image of ``n`` blobs and a moving image with the blobs rotated by ``angle``
    degrees about the centre and shifted by ``shift`` voxels."""
    rng = np.random.default_rng(seed)
    centres = np.stack([rng.uniform(24, s - 25, n) for s in SHAPE], 1)
    sigmas = rng.uniform(1.5, 3.0, n)
    amps = rng.uniform(80, 200, n)
    mid = (np.array(SHAPE) - 1) / 2
    t = np.deg2rad(angle)
    rot = np.array([[np.cos(t), -np.sin(t)], [np.sin(t), np.cos(t)]])
    moved = (centres - mid) @ rot.T + mid + np.asarray(shift)
    ii, jj = np.meshgrid(np.arange(SHAPE[0]), np.arange(SHAPE[1]), indexing="ij")
    images = []
    for cs in (centres, moved):
        img = np.full(SHAPE, 10.0)
        for (x, y), s, a in zip(cs, sigmas, amps, strict=True):
            img += a * np.exp(-((ii - x) ** 2 + (jj - y) ** 2) / (2 * s * s))
        img += rng.normal(0, 0.5, SHAPE)
        images.append(img.astype(np.float32))
    return images


def save(data, path):
    nib.Nifti1Image(np.asarray(data, np.float32), LPS).to_filename(path)
    return path


def files(tmp_path, **kw):
    a, b = blob_pair(**kw)
    return save(a, tmp_path / "fixed.nii.gz"), save(b, tmp_path / "moving.nii.gz")


def entries(fn, *args, **kwargs):
    """``fn``'s result and the IDs of the ``UpstreamDefectWarning``s it raised (other warnings
    as their text)."""
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        result = fn(*args, **kwargs)
    return result, [getattr(w.message, "entry", str(w.message)) for w in record]


def ants(*args):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "ants", *map(str, args)])
    return code, out.getvalue(), err.getvalue()


def test_a_translation_is_found_exactly_in_both_modes(tmp_path):
    a, b = files(tmp_path, shift=(3.0, -2.0))
    fixed, fw = entries(lx.ants.blob_alignment, a, b, blobs=10)
    ants_, aw = entries(lx.ants.blob_alignment, a, b, blobs=10, upstream_defects="reproduce")
    for r in (fixed, ants_):
        assert len(r.fixed_blobs) == len(r.moving_blobs) == 10
        assert r.correspondence.shape == (10, 10) and r.correspondence.dtype == np.float32
        assert all(i == j for i, j in r.pairs)  # every blob paired with its own copy
        assert r.fixed_landmarks.shape == r.moving_landmarks.shape == (len(r.landmarks), 2)
        assert r.transform.name == "AffineTransform_double_2_2"
        # Voxel (i, j) is LPS (i, j) mm here: the moving blobs are 3 and -2 mm further.
        np.testing.assert_allclose(r.transform.parameters, [1, 0, 0, 1, 3, -2], atol=1e-9)
        b0 = r.fixed_blobs[0]
        assert len(b0.index) == 2 and b0.radius == pytest.approx(np.sqrt(2) * b0.sigma)
        values = [blob.value for blob in r.fixed_blobs]
        assert values == sorted(values, reverse=True)
    # ANTS-111: once no positive correlation is left, ANTs pairs blob 0 with blob 0 again.
    assert "ANTS-111" in fw and "ANTS-111" in aw
    assert len(ants_.pairs) == 10 and ants_.pairs[-1] == (0, 0) and len(fixed.pairs) == 9
    assert fixed.fixed_blobs == ants_.fixed_blobs
    np.testing.assert_array_equal(fixed.correspondence, ants_.correspondence)


def test_the_result_does_not_depend_on_the_threads(tmp_path):
    a, b = files(tmp_path, shift=(2.0, 1.0), angle=10.0)
    one = lx.ants.blob_alignment(a, b, blobs=10, n_threads=1)
    four = lx.ants.blob_alignment(a, b, blobs=10, n_threads=4)
    assert one.fixed_blobs == four.fixed_blobs and one.pairs == four.pairs
    np.testing.assert_array_equal(one.correspondence, four.correspondence)
    np.testing.assert_array_equal(one.transform.parameters, four.transform.parameters)


def test_a_rotation_needs_the_corrected_distance_test(tmp_path):
    a, b = files(tmp_path, shift=(2.0, 1.0), angle=10.0)
    # ANTS-110: ANTs compares the lengths of the pairs' displacements, which a rotation
    # changes, so no pair is kept and its rigid fit is NaN (ANTS-114).
    ants_, aw = entries(lx.ants.blob_alignment, a, b, blobs=10, upstream_defects="reproduce")
    assert ants_.landmarks == [] and np.isnan(ants_.transform.parameters[4:]).all()
    assert "ANTS-110" in aw and "ANTS-114" in aw
    assert any("Less than 2 landmarks available" in w for w in aw)  # ITK's own warning
    # Corrected: the pairs whose distances to 3 others are preserved.
    fixed, fw = entries(lx.ants.blob_alignment, a, b, blobs=10)
    assert "ANTS-110" in fw and len(fixed.landmarks) >= 2
    m = fixed.transform.parameters[:4].reshape(2, 2)
    np.testing.assert_allclose(m @ m.T, np.eye(2), atol=1e-12)
    # The LPS axes are the voxel axes here, so the angle is the voxel grid's.
    angle = np.rad2deg(np.arctan2(m[1, 0], m[0, 0]))
    assert abs(abs(angle) - 10.0) < 3.0
    # The affine fit needs D + 1 landmarks: ITK's exception, which antsAI does not catch.
    with pytest.raises(lx.ants.AIError, match="insufficient number of landmarks"):
        lx.ants.blob_alignment(a, b, blobs=10, transform="Affine")


def test_ai_starts_from_the_blobs_as_on_the_command_line(tmp_path):
    a, b = files(tmp_path, shift=(3.0, -2.0))
    res = lx.ants.ai(a, b, blobs=10, search_factor=90, arc_fraction=0.5)
    out = tmp_path / "t.mat"
    code, _, _ = ants(
        "antsAI", "-d", 2, "-m", f"Mattes[{a},{b},32,None,1]", "-t", "Rigid[0.1]",
        "-b", 10, "-s", "[90,0.5]", "-o", out,
    )  # fmt: skip
    assert code == 0
    t = lx.transforms.read(out)[0]
    np.testing.assert_array_equal(t.parameters, res.transform.parameters)
    # The search's start points rotate the blob transform; the unrotated one is the best here.
    np.testing.assert_allclose(res.transform.parameters[-2:], [3, -2], atol=1e-6)
    # -b [n,m]: up to m pairs.
    few = lx.ants.blob_alignment(a, b, blobs=(10, 5))
    assert len(few.pairs) == 5
    res5 = lx.ants.ai(a, b, blobs=(10, 5), search_factor=90, arc_fraction=0.5)
    assert np.isfinite(res5.transform.parameters).all()


def test_failures(tmp_path):
    a, b = files(tmp_path, shift=(3.0, -2.0))
    with pytest.raises(ValueError, match=r"greater number of blobs \(>=3\)"):
        lx.ants.blob_alignment(a, b, blobs=2)
    with pytest.raises(lx.ants.AIError, match=r"greater number of blobs"):
        lx.ants.ai(a, b, blobs=2)
    with pytest.raises(ValueError, match="'Rigid', 'Affine' or 'Similarity'"):
        lx.ants.blob_alignment(a, b, transform="AlignCentersOfMass")
    with pytest.raises(ValueError, match="n or"):
        lx.ants.blob_alignment(a, b, blobs=(10, 5, 2))
    # Noise: no pair keeps its distances. Corrected, the rigid fit without a landmark is an
    # error (ANTS-114); ANTs pairs blob 0 with blob 0 again and again and fits those.
    rng = np.random.default_rng(0)
    n1 = save(rng.normal(100, 20, SHAPE), tmp_path / "n1.nii.gz")
    n2 = save(rng.normal(100, 20, SHAPE), tmp_path / "n2.nii.gz")
    with pytest.raises(lx.ants.AIError, match="ANTS-114"), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        lx.ants.blob_alignment(n1, n2, blobs=10)
    garbage, w = entries(lx.ants.blob_alignment, n1, n2, blobs=10, upstream_defects="reproduce")
    assert "ANTS-111" in w and garbage.pairs.count((0, 0)) > 1
    # An axis of fewer than 4 voxels: the Laplacian's recursive Gaussian throws.
    thin = save(np.ones((40, 3)), tmp_path / "thin.nii.gz")
    with pytest.raises(lx.ants.AIError, match="less than 4"):
        lx.ants.blob_alignment(thin, thin, blobs=5)
    with pytest.raises(lx.ants.AIError, match="does not exist|No such file|not found"):
        lx.ants.blob_alignment(a, tmp_path / "missing.nii.gz")
    # In memory, the same as from the files.
    mem = lx.ants.blob_alignment(lx.load(a), lx.load(b), blobs=10)
    disk = lx.ants.blob_alignment(a, b, blobs=10)
    np.testing.assert_array_equal(mem.transform.parameters, disk.transform.parameters)
