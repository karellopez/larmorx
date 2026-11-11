# SPDX-License-Identifier: Apache-2.0
"""``lx.afni.volreg`` with the replica built into larmorx and the clean-room original: the
wrapper passes 3dvolreg's options, runs either in this process and reads its outputs back. Both
are checked by their golden tests and against AFNI (the parity suites; the original's black-box
checks in ``validation/investigations/afni-volreg-original/``)."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest

import larmorx as lx


def series(tmp_path: Path) -> Path:
    """A small blob moved by one voxel along i in volume 1 and two in volume 2 (plain
    arithmetic: no transcendental functions)."""
    i, j, k = np.meshgrid(np.arange(24), np.arange(20), np.arange(12), indexing="ij")
    r2 = ((i - 11.5) / 7.0) ** 2 + ((j - 9.5) / 6.0) ** 2 + ((k - 5.5) / 4.0) ** 2
    blob = np.where(r2 < 1.0, 1000.0 * (1.0 - r2) + 3.0 * j, 0.0).astype(np.float32)
    data = np.stack([blob, np.roll(blob, 1, axis=0), np.roll(blob, 2, axis=0)], axis=-1)
    path = tmp_path / "with space" / "bold.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, np.diag([2.0, 2.0, 3.0, 1.0])), path)
    return path


def test_registration_outputs(tmp_path):
    r = lx.afni.volreg(series(tmp_path), maxdisp=True)
    assert r.registered.data.shape == (24, 20, 12, 3)
    assert r.registered.implementation.kind == "replica"
    assert r.parameters.shape == (3, 6)
    assert r.matrices.shape == (3, 12)
    assert r.rms.shape == (3, 2)
    assert np.all(r.parameters[0] == 0)
    # One and two voxels (2 mm each) along i, which runs R to L (dL).
    assert abs(abs(r.parameters[1, 4]) - 2.0) < 0.2
    assert abs(abs(r.parameters[2, 4]) - 4.0) < 0.2
    assert r.max_displacement is not None and r.max_displacement.shape == (3,)
    assert r.rms[1, 1] < r.rms[1, 0]


def test_fmriprep_call_and_displacements(tmp_path):
    opts = lx.afni.fmriprep_volreg_options()
    assert opts == {"interpolation": "Fourier", "twopass": True, "zpad": 4}
    # -nomaxdisp with -twopass: AFNI-92 named (the coarse search's mask kept as by default).
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-92"):
        r = lx.afni.volreg(series(tmp_path), save_displacements=True, maxdisp=False, **opts)
    assert r.max_displacement is None
    assert r.displacements is not None
    assert all(d.data.shape == (24, 20, 12, 3) for d in r.displacements)


def test_the_original_agrees_with_the_replica(tmp_path):
    path = series(tmp_path)
    o = lx.afni.volreg(path, implementation="clean-room")
    r = lx.afni.volreg(path, implementation="replica")
    assert o.registered.implementation.kind == "clean-room"
    assert o.registered.data.shape == r.registered.data.shape
    # specs/3dvolreg.md §14.3: the convergence thresholds (0.02 degrees, 0.01 voxels).
    assert np.abs(o.parameters - r.parameters).max() < 0.03
    assert np.all(o.parameters[0] == 0)
    with pytest.raises(ValueError, match="nonzero voxel"):
        lx.afni.volreg((np.zeros((8, 8, 8, 2), np.float32), np.eye(4)), implementation="clean-room")


def test_bad_arguments(tmp_path):
    path = series(tmp_path)
    with pytest.raises(ValueError, match="interpolation"):
        lx.afni.volreg(path, interpolation="spline")
    with pytest.raises(ValueError, match="twoblur"):
        lx.afni.volreg(path, twopass=True, twoblur=1.0)
    with pytest.raises(ValueError, match="edging"):
        lx.afni.volreg(path, edging="30%")
    with pytest.raises(ValueError, match="AFNI's 3dvolreg does not read"):
        lx.afni.volreg(path, afni_settings={"AFNI_NOT_A_VARIABLE": "1"})
