# SPDX-License-Identifier: Apache-2.0
"""The field maps' part B defects decided by its defects relay (2026-11-09): every ID a notice
of the package names is in ``larmorx_core::defects``; PIPE-32 (participants sharing a
``B0FieldIdentifier``), PIPE-33 (``IntensityClip`` of a constant image), PIPE-34
(``RobustAverage`` of a volume of zeros), SKI-03 and SKI-04 (``random_walker``'s labels and full
probabilities), SCI-09 (the 1-D rank filters' footprint holes) and SCI-10 (the propagation's
``int8`` mask), each in both modes."""

from __future__ import annotations

import json
import re
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _defects

PACKAGE = Path(lx.__file__).resolve().parent


def entries(fn, *args, **kwargs):
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        out = fn(*args, **kwargs)
    return out, [w.message.entry for w in record if isinstance(w.message, lx.UpstreamDefectWarning)]


def test_every_notice_of_the_package_is_registered():
    """A notice names its ID as ``("ID", "tool", ...`` (``_defects.warn``, the watershed's table)
    or as ``UpstreamDefectWarning("ID", "tool", ...``: each such ID has a register entry with a
    switch or notice (``_core.upstream_defects()``)."""
    registered = {d.id for d in _defects.defects()}
    pattern = re.compile(r'"((?:SDC|PIPE|SKI|SCI|NP|NIB)-\d+)",\s*"')
    used: dict[str, str] = {}
    for f in PACKAGE.rglob("*.py"):
        for m in pattern.finditer(f.read_text(encoding="utf-8")):
            used.setdefault(m.group(1), f.name)
    assert {"SDC-12", "PIPE-32", "SKI-04", "SCI-13"} <= set(used)
    missing = sorted(i for i in used if i not in registered)
    assert not missing, missing


def _tree(root: Path) -> Path:
    import nibabel as nb

    for sub in ("01", "02"):
        for d, pe in (("AP", "j-"), ("PA", "j")):
            p = root / f"sub-{sub}/fmap/sub-{sub}_dir-{d}_epi.nii.gz"
            p.parent.mkdir(parents=True, exist_ok=True)
            nb.Nifti1Image(np.zeros((4, 4, 4), np.int16), np.eye(4)).to_filename(p)
            meta = {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05}
            meta["B0FieldIdentifier"] = "pepolar"
            p.with_name(p.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))
        p = root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz"
        p.parent.mkdir(parents=True, exist_ok=True)
        nb.Nifti1Image(np.zeros((4, 4, 4, 2), np.int16), np.eye(4)).to_filename(p)
        meta = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2}
        meta["B0FieldSource"] = "pepolar"
        p.with_name(p.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))
    (root / "dataset_description.json").write_text('{"Name": "t", "BIDSVersion": "1.8.0"}')
    return root


def test_pipe32_participants_sharing_an_identifier(tmp_path):
    from larmorx.pipelines.larmorprepx import estimators as le

    root = _tree(tmp_path / "ds ~1")
    lay = le.fmriprep_layout(root, ["01", "02"])

    def both(**kw):
        out = []
        for sub in ("01", "02"):
            bold = [[str(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz")]]
            ests, emap = le.map_fieldmap_estimation(
                lay, sub, None, bold, False, False, False, None, **kw
            )
            out.append(([e.bids_id for e in ests], sorted(Path(k).name for k in emap)))
        return out

    lx.sdcflows.clear_registry()
    with pytest.raises(KeyError, match="'pepolar' is already  in mapping"):
        both(upstream_defects="reproduce")
    lx.sdcflows.clear_registry()
    out, ids = entries(both)
    assert out == [
        (["pepolar"], ["sub-01_task-rest_bold.nii.gz"]),
        (["pepolar"], ["sub-02_task-rest_bold.nii.gz"]),
    ]
    assert ids == ["PIPE-32"]
    assert all("sub-01" in s for s in lx.sdcflows.REGISTRY.estimators["pepolar"])
    lx.sdcflows.clear_registry()


def _save(path: Path, data: np.ndarray) -> str:
    import nibabel as nb

    nb.Nifti1Image(data, np.diag([2.0, 2.0, 2.0, 1.0])).to_filename(path)
    return str(path)


def test_pipe33_a_constant_clipped_image(tmp_path):
    from larmorx.niworkflows import intensity_clip

    f = _save(tmp_path / "flat.nii.gz", np.full((12, 12, 12), 7.0, np.float32))
    with pytest.raises(ValueError, match="PIPE-33"):
        intensity_clip(f)
    r, ids = entries(intensity_clip, f, upstream_defects="reproduce")
    assert ids == ["PIPE-33"] and np.asarray(r.image.data).dtype == np.int16
    assert not np.asarray(r.image.data).any()
    r, _ = entries(intensity_clip, f, dtype="float32", upstream_defects="reproduce")
    bits = np.asarray(r.image.data, dtype=np.float32).view(np.uint32)
    assert (bits == 0xFFC00000).all()  # x86-64's default NaN, on every platform
    # an image with contrast is unchanged by the switch
    rng = np.random.default_rng(3)
    g = _save(tmp_path / "noise.nii.gz", rng.uniform(1, 100, (12, 12, 12)).astype(np.float32))
    a, b = intensity_clip(g), intensity_clip(g, upstream_defects="reproduce")
    assert np.asarray(a.image.data).tobytes() == np.asarray(b.image.data).tobytes()


def test_pipe34_a_volume_of_zeros(tmp_path):
    from larmorx.niworkflows import robust_average

    rng = np.random.default_rng(4)
    data = rng.uniform(10, 50, (8, 8, 8, 3)).astype(np.float32)
    data[..., 1] = 0
    f = _save(tmp_path / "series.nii.gz", data)
    with pytest.raises(ValueError, match=r"volume\(s\) \[1\].*PIPE-34"):
        robust_average(f, mc_method=None, out_dir=tmp_path / "a")
    with np.errstate(all="ignore"):
        _, ids = entries(
            robust_average, f, mc_method=None, out_dir=tmp_path / "b", upstream_defects="reproduce"
        )
    assert "PIPE-34" in ids


def test_ski03_ski04_random_walker_labels_and_probabilities():
    rng = np.random.default_rng(0)
    data = rng.random((20, 20))
    lab = np.zeros((20, 20), int)
    lab[2:4, 2:4], lab[15:17, 15:17], lab[0, 19] = 3, 7, -2
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        got, ids = entries(lx.skimage.random_walker, data, lab.copy())
        assert set(np.unique(got)) == {-2, 3, 7} and "SKI-03" in ids
        old, ids = entries(lx.skimage.random_walker, data, lab.copy(), upstream_defects="reproduce")
        assert set(np.unique(old)) == {-1, 1, 2} and "SKI-03" in ids
        mapping = {-1: -2, 1: 3, 2: 7}
        assert (np.vectorize(mapping.get)(old) == got).all()
        consecutive = np.where(lab == 3, 1, np.where(lab == 7, 2, np.where(lab < 0, -1, 0)))
        _, ids = entries(lx.skimage.random_walker, data, consecutive)
        assert "SKI-03" not in ids
        seeds = np.ones((6, 6), int)
        seeds[3:] = 4
        p, ids = entries(lx.skimage.random_walker, data[:6, :6], seeds, return_full_prob=True)
        assert p.shape == (2, 6, 6) and p.dtype == np.float64 and "SKI-04" in ids
        assert (p[0] == (seeds == 1)).all() and (p[1] == (seeds == 4)).all()
        q, _ = entries(
            lx.skimage.random_walker,
            data[:6, :6],
            seeds,
            return_full_prob=True,
            upstream_defects="reproduce",
        )
        assert q.shape == (6, 6, 2) and q.dtype == np.bool_


def test_sci09_a_1d_footprint_with_holes():
    x = np.random.default_rng(1).random(40)
    fp = np.array([1, 0, 1, 1, 0, 1], bool)
    fixed, ids = entries(lx.ndimage.median_filter, x, footprint=fp)
    assert ids == ["SCI-09"]
    nd = lx.ndimage.median_filter(np.stack([x, x]), footprint=fp[None, :])[0]
    assert np.array_equal(fixed, nd)
    old, ids = entries(lx.ndimage.median_filter, x, footprint=fp, upstream_defects="reproduce")
    assert ids == ["SCI-09"] and not np.array_equal(old, fixed)
    _, ids = entries(lx.ndimage.median_filter, x, size=5)
    assert ids == []


def test_sci10_the_propagations_int8_mask():
    mask = np.ones(30)
    mask[10:20] = 0.5
    seed = np.zeros(30, bool)
    seed[0] = True
    fixed, ids = entries(lx.ndimage.binary_propagation, seed, mask=mask)
    assert ids == ["SCI-10"] and fixed.sum() == 30
    assert np.array_equal(fixed, lx.ndimage.binary_propagation(seed, mask=mask != 0))
    old, ids = entries(lx.ndimage.binary_propagation, seed, mask=mask, upstream_defects="reproduce")
    assert ids == ["SCI-10"] and old.sum() == 10
    _, ids = entries(lx.ndimage.binary_propagation, seed, mask=mask > 0)
    assert ids == []
