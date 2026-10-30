# SPDX-License-Identifier: Apache-2.0
"""The clean-room originals of 3dAutomask and 3dUnifize (``lx.afni.automask``,
``lx.afni.unifize`` with ``implementation="original"``, and ``larmorx afni 3dAutomask`` /
``3dUnifize``): the Python API gives the command line's outputs, the results do not depend on
the thread count, and the D13 switches act. Their agreement with AFNI is the parity suites'
(``validation/``), their bits on every platform the golden tests'."""

from __future__ import annotations

import contextlib
import io
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

ORIGINAL = "original"


def head(shape=(26, 24, 20), seed=3, t2=False) -> np.ndarray:
    """A small head: background noise, a bright shell, grey and white matter, a dark core, a
    thin protrusion (peeling cuts it), under a smooth shading."""
    rng = np.random.default_rng(seed)
    x, y, z = np.meshgrid(*(np.linspace(-1, 1, n) for n in shape), indexing="ij")
    r = np.sqrt(x**2 + y**2 + (1.1 * z) ** 2)
    wm, gm, csf, skull = (420.0, 700.0, 1400.0, 150.0) if t2 else (1000.0, 650.0, 250.0, 900.0)
    v = np.full(shape, 15.0)
    v[r < 0.92] = skull
    v[r < 0.8] = csf
    v[r < 0.72] = gm
    v[r < 0.5] = wm
    v[(np.abs(x) < 0.15) & (np.abs(y) < 0.15) & (np.abs(z) < 0.2)] = csf
    v[(np.abs(x) < 0.05) & (np.abs(y) < 0.3) & (z < -0.85)] = gm
    v *= 1.0 + 0.25 * x - 0.1 * y
    v += rng.normal(0, 8, shape) + np.abs(rng.normal(0, 5, shape))
    return np.clip(v, 0, None).astype(np.float32)


def affine(zoom=3.0, shape=(26, 24, 20)) -> np.ndarray:
    a = np.diag([zoom, zoom, zoom, 1.0])
    a[:3, 3] = [-(n - 1) * zoom / 2 for n in shape]
    return a


@pytest.fixture
def t1(tmp_path, no_replica) -> Path:
    path = tmp_path / "t1.nii"
    lx.save((head(), affine()), path)
    return path


@pytest.fixture
def t2(tmp_path, no_replica) -> Path:
    path = tmp_path / "t2.nii"
    lx.save((head(t2=True), affine()), path)
    return path


def cli(tool: str, *args: str) -> tuple[int, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "afni", tool, *args])
    return code, err.getvalue()


def same(a: lx.Image, b: lx.Image) -> bool:
    x, y = np.asarray(a.data), np.asarray(b.data)
    return (
        x.dtype == y.dtype
        and x.shape == y.shape
        and np.array_equal(
            np.ascontiguousarray(x).view(np.uint8), np.ascontiguousarray(y).view(np.uint8)
        )
        and np.array_equal(a.affine, b.affine)
    )


def test_automask_outputs(t1, tmp_path):
    r = lx.afni.automask(t1, apply=True, depth=True, implementation=ORIGINAL)
    mask = np.asarray(r.mask.data)
    assert mask.dtype == np.uint8 and mask.shape == (26, 24, 20)
    assert set(np.unique(mask)) == {0, 1}
    data = np.asarray(lx.load(t1).data)
    np.testing.assert_array_equal(np.asarray(r.masked.data), np.where(mask == 1, data, 0))
    depth = np.asarray(r.depth.data)
    assert depth.dtype == np.int16 and depth.max() >= 2
    assert np.array_equal(depth > 0, mask > 0)
    assert r.mask.implementation.kind == "original"
    # The mask covers the head's centre, not the corners.
    assert mask[13, 12, 10] == 1 and mask[0, 0, 0] == 0


def test_automask_python_gives_the_command_lines_outputs(t1, tmp_path):
    out = tmp_path / "out"
    out.mkdir()
    code, err = cli(
        "3dAutomask", "-q", "-eclip", "-dilate", "1", "-apply_prefix", str(out / "masked.nii"),
        "-depth", str(out / "depth.nii"), "-prefix", str(out / "mask.nii"), str(t1),
    )  # fmt: skip
    assert code == 0, err
    r = lx.afni.automask(
        t1, exterior_clip=True, dilate=1, apply=True, depth=True, implementation=ORIGINAL
    )
    assert same(r.mask, lx.load(out / "mask.nii"))
    assert same(r.masked, lx.load(out / "masked.nii"))
    assert same(r.depth, lx.load(out / "depth.nii"))


def test_unifize_python_gives_the_command_lines_outputs(t2, tmp_path):
    out = tmp_path / "out"
    out.mkdir()
    code, err = cli(
        "3dUnifize", "-quiet", "-clfrac", "0.2", "-rbt", "6", "65", "90", "-T2", "-ssave",
        str(out / "s.nii"), "-amsave", str(out / "a.nii"), "-prefix", str(out / "u.nii"),
        "-input", str(t2),
    )  # fmt: skip
    assert code == 0, err
    r = lx.afni.unifize(
        t2,
        clfrac=0.2,
        radius=6.0,
        percentiles=(65.0, 90.0),
        t2=True,
        save_scale=True,
        save_automask=True,
        implementation=ORIGINAL,
    )
    assert same(r.image, lx.load(out / "u.nii"))
    assert same(r.scale, lx.load(out / "s.nii"))
    assert same(r.automasked, lx.load(out / "a.nii"))
    assert r.image.implementation.kind == "original"


def test_results_do_not_depend_on_threads(t1, t2):
    a = lx.afni.automask(t1, depth=True, n_threads=1, implementation=ORIGINAL)
    b = lx.afni.automask(t1, depth=True, n_threads=3, implementation=ORIGINAL)
    assert same(a.mask, b.mask) and same(a.depth, b.depth)
    u = lx.afni.unifize(t2, radius=6.0, t2=2, n_threads=1, implementation=ORIGINAL)
    v = lx.afni.unifize(t2, radius=6.0, t2=2, n_threads=3, implementation=ORIGINAL)
    assert same(u.image, v.image)


def test_unifize_scales_white_matter_to_about_1000(t1):
    r = lx.afni.unifize(t1, radius=6.0, implementation=ORIGINAL)
    img = np.asarray(r.image.data)
    assert img.dtype == np.float32
    wm = head() >= 900  # white matter and skull of the phantom, before shading
    assert 800 < np.median(img[wm & (img > 0)]) < 1300
    assert img.max() <= 1500.0  # the squash
    gm = lx.afni.unifize(t1, radius=6.0, gm=True, implementation=ORIGINAL)
    assert not np.array_equal(np.asarray(gm.image.data), img)


def test_epi_scales_every_volume(tmp_path, no_replica):
    base = head(t2=True)
    series = np.stack([base * (1 + 0.01 * t) for t in range(3)], axis=-1).astype(np.float32)
    path = tmp_path / "bold.nii"
    lx.save((series, affine()), path)
    r = lx.afni.unifize(path, epi=True, implementation=ORIGINAL)
    assert np.asarray(r.image.data).shape == (26, 24, 20, 3)
    one = lx.afni.unifize(path, implementation=ORIGINAL)
    assert np.asarray(one.image.data).shape == (26, 24, 20)


def test_in_memory_images(no_replica):
    img = lx.Image(head(), affine())
    r = lx.afni.automask(img, implementation=ORIGINAL)
    assert np.asarray(r.mask.data).sum() > 0
    u = lx.afni.unifize(img, radius=6.0, implementation=ORIGINAL)
    assert np.asarray(u.image.data).shape == (26, 24, 20)


def test_d13_switches(t1, t2):
    # AFNI-41: -nbhrs outside 6..26 is refused by default; reproduced, it is 17.
    with pytest.raises(ValueError, match="AFNI-41"):
        lx.afni.automask(t1, neighbours=40, implementation=ORIGINAL)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-41"):
        r40 = lx.afni.automask(
            t1, neighbours=40, upstream_defects="reproduce", implementation=ORIGINAL
        )
    r17 = lx.afni.automask(t1, upstream_defects="reproduce", implementation=ORIGINAL)
    assert same(r40.mask, r17.mask)
    # AFNI-43: -peels 0 peels once in AFNI; named when the masks differ.
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        fixed = lx.afni.automask(t1, peels=0, implementation=ORIGINAL)
        once = lx.afni.automask(t1, peels=0, upstream_defects="reproduce", implementation=ORIGINAL)
    named = [w for w in caught if "AFNI-43" in str(w.message)]
    differ = not same(fixed.mask, once.mask)
    assert bool(named) == differ
    assert same(once.mask, lx.afni.automask(t1, peels=1, implementation=ORIGINAL).mask)
    # AFNI-45: -T2up 100 reads one past the sorted values; AFNI's 0 zeroes the output.
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-45"):
        fixed = lx.afni.unifize(
            t2, t2=True, t2_percentile=100.0, radius=6.0, implementation=ORIGINAL
        )
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-45"):
        afni = lx.afni.unifize(
            t2,
            t2=True,
            t2_percentile=100.0,
            radius=6.0,
            upstream_defects="reproduce",
            implementation=ORIGINAL,
        )
    assert np.asarray(fixed.image.data).max() > 0
    assert np.asarray(afni.image.data).max() == 0


def test_command_line_refusals(t1, t2, tmp_path):
    out = tmp_path / "x"
    out.mkdir()
    code, err = cli("3dAutomask", "-nbhrs", "40", "-prefix", str(out / "m.nii"), str(t1))
    assert code == 1 and "AFNI-41" in err
    code, err = cli("3dAutomask", "-q", "-peels")
    assert code == 1 and "AFNI-40" in err
    code, err = cli("3dUnifize", "-T2", "-T2", "-T2", "-prefix", str(out / "u.nii"), str(t2))
    assert code == 1 and "AFNI-48" in err
    code, err = cli("3dUnifize", "-prefix", str(out / "u.nii"), "-T2up")
    assert code == 1 and "AFNI-40" in err
    # NIfTI only: AFNI's default outputs are its own format.
    code, err = cli("3dAutomask", str(t1))
    assert code == 1 and "NIfTI only" in err
    code, err = cli("3dUnifize", str(t2))
    assert code == 1 and "NIfTI only" in err


def test_epi_ball_guard(tmp_path, no_replica, monkeypatch):
    """AFNI-75 (``specs/3dUnifize.md`` §9.1): ``-EPI`` on micrometre voxels is refused by default
    before any work, with the memory, the radius and the limit named, nothing written; the
    Python API raises the same message; ``reproduce`` runs it (only the ball's lines that reach
    the grid are kept)."""
    import nibabel as nib

    base = head(t2=True)
    series = np.stack([base * (1 + 0.01 * t) for t in range(3)], axis=-1).astype(np.float32)
    img = nib.Nifti1Image(series, affine())
    img.header["xyzt_units"] = 3 | 8  # micrometres: 3 µm voxels
    path = tmp_path / "um.nii"
    img.to_filename(path)
    monkeypatch.setenv("LARMORX_MEMORY_LIMIT", "4G")
    monkeypatch.setenv("OMP_NUM_THREADS", "2")
    out = tmp_path / "uni.nii"
    code, err = cli("3dUnifize", "-EPI", "-prefix", str(out), str(path))
    assert code == 1 and not out.exists(), err
    assert "upstream defect AFNI-75 (3dUnifize) corrected" in err
    assert "about 2.834e4 GiB" in err and "radius 6100.0 voxels" in err
    assert "the 4.00 GiB of LARMORX_MEMORY_LIMIT (a header whose units are micrometres" in err
    with pytest.raises(ValueError, match=r"radius 6100\.0 voxels.*An error \(AFNI-75\)"):
        lx.afni.unifize(path, epi=True, n_threads=2, implementation=ORIGINAL)
    r = lx.afni.unifize(path, epi=True, upstream_defects="reproduce", implementation=ORIGINAL)
    assert np.asarray(r.image.data).shape == series.shape
