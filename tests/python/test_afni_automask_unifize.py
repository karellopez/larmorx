# SPDX-License-Identifier: Apache-2.0
"""``lx.afni.automask`` and ``lx.afni.unifize`` with the replica, built into larmorx since
2026-10-29: ``"auto"`` and ``"replica"`` run it in this process, ``"original"`` the clean-room
original; with a stand-in for the built-in replica (``builtin_stub``) the wrappers pass AFNI's
options, settings and D13's choice to it and read its outputs back. The replica itself is
checked by its own golden tests and by the parity suites against AFNI; the originals by
``test_afni_masks_original.py``, the golden tests and the parity suites."""

from __future__ import annotations

import contextlib
import io
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

REPLICA = lx.Implementation("replica", "larmorx", lx.__version__, "Apache-2.0 AND CC-BY-4.0")


def volume(tmp_path: Path) -> Path:
    data = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    path = tmp_path / "with space" / "t1.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


def cli(tool: str, *args: str, implementation: str | None = None) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "afni", tool, *args], implementation=implementation)
    return code, out.getvalue(), err.getvalue()


def test_auto_runs_the_built_in_replica(no_replica, tmp_path):
    """No program to install: the replica is part of larmorx, and gives what the command line's
    replica writes; ``"original"`` runs the clean-room original."""
    path = volume(tmp_path)
    out = tmp_path / "out"
    out.mkdir()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for implementation in ("auto", "replica"):
            r = lx.afni.automask(path, implementation=implementation)
            assert r.mask.implementation == REPLICA
            u = lx.afni.unifize(path, implementation=implementation)
            assert u.image.implementation == REPLICA
        assert lx.afni.automask(path, implementation="original").mask.implementation.kind == (
            "original"
        )
        assert lx.afni.unifize(path, implementation="original").image.implementation.kind == (
            "original"
        )
    code, _, err = cli("3dAutomask", "-q", "-prefix", str(out / "m.nii"), str(path))
    assert code == 0, err
    np.testing.assert_array_equal(lx.load(out / "m.nii").data, r.mask.data)
    code, _, err = cli("3dUnifize", "-quiet", "-prefix", str(out / "u.nii"), str(path))
    assert code == 0, err
    np.testing.assert_array_equal(lx.load(out / "u.nii").data, u.image.data)
    for implementation in ("original", "replica"):
        code, _, err = cli(
            "3dAutomask", "-q", "-prefix", str(out / f"m_{implementation}.nii"), str(path),
            implementation=implementation,
        )  # fmt: skip
        assert code == 0, err
    # The help lists them, whichever implementation runs.
    for tool in ("3dTshift", "3dAutomask", "3dUnifize"):
        for implementation in ("original", "replica"):
            code, _, _ = cli(tool, implementation=implementation)
            assert code == 0


def test_arguments_are_checked_before_anything_runs(builtin_stub, tmp_path):
    path = volume(tmp_path)
    for kwargs in (
        {"clfrac": 0.05},
        {"peels": -1},
        {"nn": 4},
        {"dilate": -1},
        {"si": 0.0},
        {"n_threads": -1},
    ):
        with pytest.raises(ValueError):
            lx.afni.automask(path, **kwargs)
    for kwargs in (
        {"t2": 3},
        {"t2_percentile": 89.0},
        {"radius": 4.0},
        {"percentiles": (20.0, 80.0)},
        {"percentiles": (70.0, 95.0)},
        {"percentiles": (70.0, 70.0)},
        {"clfrac": 0.95},
    ):
        with pytest.raises(ValueError):
            lx.afni.unifize(path, **kwargs)
    with pytest.raises(ValueError, match="does not read 'AFNI_1D_ZERO_TEXT'"):
        lx.afni.automask(path, afni_settings={"AFNI_1D_ZERO_TEXT": "YES"})
    with pytest.raises(ValueError, match="sets AFNI_PATH_SPACES_OK itself"):
        lx.afni.unifize(path, afni_settings={"AFNI_PATH_SPACES_OK": "YES"})
    with pytest.raises(ValueError, match="upstream_defects"):
        lx.afni.automask(path, upstream_defects="exact")
    assert builtin_stub.calls == []


def test_automask_runs_the_replica_with_the_options(builtin_stub, monkeypatch, tmp_path):
    monkeypatch.setenv("AFNI_NIFTI_PRIORITY", "Q")  # must not reach the replica
    path = volume(tmp_path)
    r = lx.afni.automask(
        path,
        clfrac=0.3,
        gradual=False,
        peels=2,
        nn=1,
        exterior_clip=True,
        dilate=1,
        si=130.0,
        apply=True,
        depth=True,
        n_threads=2,
        upstream_defects="reproduce",
        afni_settings={"AFNI_NIFTI_VIEW": "tlrc"},
    )
    (call,) = builtin_stub.calls
    args = call["args"]
    assert args[:3] == ["afni", "3dAutomask", "-q"]
    for option in ("-nograd", "-NN1", "-eclip"):
        assert option in args
    assert args[args.index("-clfrac") + 1] == "0.3" and args[args.index("-peels") + 1] == "2"
    assert args[args.index("-dilate") + 1] == "1" and args[args.index("-SI") + 1] == "130.0"
    assert args[-1] == str(path.resolve()) or Path(args[-1]) == path.resolve()
    # Outputs named relative to the call's own directory, which the replica is given.
    assert args[args.index("-prefix") + 1] == "mask.nii" and Path(call["cwd"]).is_absolute()
    assert call["threads"] == "2" and call["defects"] == "reproduce"
    assert call["afni"]["AFNI_NIFTI_VIEW"] == "tlrc"
    assert "AFNI_NIFTI_PRIORITY" not in call["afni"]
    assert call["afni"]["AFNI_PATH_SPACES_OK"] == "YES"
    expected = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    for img in (r.mask, r.masked, r.depth):
        assert img is not None
        np.testing.assert_array_equal(img.data, expected)
        assert img.implementation == REPLICA
    plain = lx.afni.automask(path)
    assert plain.masked is None and plain.depth is None


def test_unifize_runs_the_replica_and_passes_its_warnings(builtin_stub, tmp_path):
    builtin_stub.stderr = (
        "*+ WARNING: upstream defect AFNI-45 (3dUnifize) corrected: -T2up 100 points one past "
        "the sorted values [LARMORX_UPSTREAM_DEFECTS=reproduce gives 3dUnifize's result]\n"
        "*+ WARNING: Only processing sub-brick #0 (out of 3)\n"
    )
    path = volume(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-45"):
        r = lx.afni.unifize(
            path,
            **lx.afni.fmriprep_unifize_options(),
            t2_percentile=100.0,
            save_scale=True,
            save_automask=True,
        )
    (call,) = builtin_stub.calls
    args = call["args"]
    assert args[:3] == ["afni", "3dUnifize", "-quiet"]
    i = args.index("-rbt")
    assert args[i + 1 : i + 4] == ["18.3", "65.0", "90.0"]
    assert args.count("-T2") == 1 and args[args.index("-T2up") + 1] == "100.0"
    assert args[args.index("-clfrac") + 1] == "0.2"
    assert args[-2] == "-input"
    assert r.scale is not None and r.automasked is not None
    builtin_stub.stderr = ""
    two = lx.afni.unifize(path, t2=2, gm=True, duplo=False, epi=True, squash=False)
    args = builtin_stub.calls[-1]["args"]
    assert args.count("-T2") == 2
    for option in ("-GM", "-noduplo", "-EPI", "-nosquash"):
        assert option in args
    assert two.image.data.shape == (6, 5, 4)


def test_a_failing_replica_raises(builtin_stub, tmp_path):
    builtin_stub.exit = 1
    builtin_stub.stderr = "** FATAL ERROR: Mask creation fails for unknown reasons!\n"
    with pytest.raises(lx.ReplicaError, match="Mask creation fails"):
        lx.afni.automask(volume(tmp_path))
