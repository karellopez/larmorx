# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.bet`` and ``lx.fsl.betsurf``: with a stand-in for the ``larmorx-nc`` program
(written by the test, as in ``test_replica.py``) they pass FSL's options, settings and D13's
choice to it and read its outputs back; without it they refuse clearly (there is no clean-room
original yet). The replica itself is checked by its own golden tests
(``crates-nc/larmorx-nc-cli/tests/golden.rs``) and by the parity suite against FSL."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica
from larmorx.cli import run as cli_run

STUB = """\
import json, os, shutil, sys
from pathlib import Path

here = Path(__file__).parent
config = json.loads((here / "config.json").read_text(encoding="utf-8"))
args = sys.argv[1:]
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps({"args": args, "threads": os.environ.get("OMP_NUM_THREADS"),
                        "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"),
                        "path": os.environ.get("PATH"), "cwd": os.getcwd(),
                        "fsl": {k: v for k, v in os.environ.items()
                                if k.upper().startswith("FSL")}}) + "\\n")
if args == ["--version"]:
    print("larmorx-nc " + config["version"])
    sys.exit(0)
sys.stderr.write(config["stderr"])
if config["exit"] != 0:
    sys.exit(config["exit"])
mesh = "# vtk DataFile Version 3.0\\nsurface file\\nASCII\\nDATASET POLYDATA\\nPOINTS 3  float\\n"
mesh += "0.000000 0.000000 0.000000\\n1.000000 0.000000 0.000000\\n0.000000 1.500000 0.000000\\n"
mesh += "POLYGONS 1 4\\n3 0 1 2 \\n"
tool, rest = args[1], args[2:]
if tool in ("bet2", "bet"):
    source, out = rest[0], rest[1]
    names = [""]
    for option, name in (("-m", "_mask"), ("-o", "_overlay"), ("-s", "_skull"), ("-F", "_mask")):
        if option in rest:
            names.append(name)
    for name in names:
        shutil.copyfile(source, out + name + ".nii")
    if "-e" in rest:
        Path(out + "_mesh.vtk").write_text(mesh)
else:
    files = [a for a in rest if not a.startswith("-") and not a.isdigit()]
    t1, out = files[0], files[-1]
    for s in ("inskull", "outskull", "outskin"):
        Path(out + "_" + s + "_mesh.vtk").write_text(mesh)
        shutil.copyfile(t1, out + "_" + s + "_mask.nii")
        shutil.copyfile(t1, out + "_" + s + "_mesh.nii")
    shutil.copyfile(t1, out + "_skull_mask.nii")
sys.exit(0)
"""


def stub(directory: Path, exit: int = 0, stderr: str = "") -> Path:
    """A ``larmorx-nc`` program in ``directory`` that records its calls and writes its input
    to each output."""
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    config = {"version": "9.9.9", "exit": exit, "stderr": stderr}
    (directory / "config.json").write_text(json.dumps(config), encoding="utf-8")
    script = directory / "stub.py"
    if os.name == "nt":
        program = directory / "larmorx-nc.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))
    else:
        program = directory / "larmorx-nc"
        program.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n', "utf-8")
        program.chmod(0o755)
    return program


def calls(program: Path) -> list[dict]:
    lines = (program.parent / "calls.jsonl").read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in lines if json.loads(line)["args"] != ["--version"]]


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    """The Python environment's scripts directory, as larmorx sees it: empty to start with."""
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def volume(tmp_path: Path) -> Path:
    data = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    path = tmp_path / "with space" / "t1.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


def cli(*args: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "fsl", *args])
    return code, out.getvalue(), err.getvalue()


def test_without_the_replica_they_refuse(scripts, tmp_path):
    path = volume(tmp_path)
    install = "pip install larmorx-nc"
    for implementation in ("auto", "replica"):
        with pytest.raises(lx.ReplicaNotFoundError, match=install):
            lx.fsl.bet(path, implementation=implementation)
        with pytest.raises(lx.ReplicaNotFoundError, match=install):
            lx.fsl.betsurf(path, path, np.eye(4), implementation=implementation)
    with pytest.raises(NotImplementedError, match="no clean-room original of fsl bet2"):
        lx.fsl.bet(path, implementation="original")
    with pytest.raises(NotImplementedError, match="no clean-room original of fsl bet "):
        lx.fsl.bet(path, variation="robust", implementation="original")
    # the command line refuses the same way
    code, _, err = cli("bet", str(path), "out")
    assert code != 0 and "pip install larmorx-nc" in err, err


def test_arguments_are_checked_before_anything_runs(scripts, tmp_path):
    path = volume(tmp_path)
    for kwargs in (
        {"frac": 1.5},
        {"frac": -0.1},
        {"gradient": 2.0},
        {"centre": (1.0, 2.0)},
        {"variation": "everything"},
        {"variation": "robust", "smoothness": 2.0},
        {"variation": "small_fov", "head_ct": True},
        {"variation": "fmri", "mesh": True},
        {"n_threads": -1},
    ):
        with pytest.raises(ValueError):
            lx.fsl.bet(path, **kwargs)
    with pytest.raises(TypeError):
        lx.fsl.bet(path, radius="large")
    for variation in ("eyes", "bias", "surfaces", "surfaces_t2"):
        with pytest.raises(NotImplementedError, match="does not replicate yet"):
            lx.fsl.bet(path, variation=variation)
    with pytest.raises(ValueError, match="upstream_defects"):
        lx.fsl.bet(path, upstream_defects="exact")
    with pytest.raises(ValueError, match="precision"):
        lx.fsl.betsurf(path, path, np.eye(4), precision=-1)
    with pytest.raises(ValueError, match="4 x 4"):
        stub(scripts)
        lx.fsl.betsurf(path, path, np.eye(3))


def test_bet_runs_bet2_with_the_options(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR")  # must not reach the replica
    monkeypatch.setenv("FSL_LOAD_NIFTI_EXTENSIONS", "1")
    path = volume(tmp_path)
    r = lx.fsl.bet(
        path,
        frac=0.2,
        gradient=-0.1,
        radius=80,
        centre=(1, 2, 3),
        smoothness=0.5,
        threshold=True,
        head_ct=True,
        mask=True,
        outline=True,
        skull=True,
        mesh=True,
        n_threads=2,
        upstream_defects="reproduce",
    )
    (call,) = calls(program)
    args = call["args"]
    assert args[:2] == ["fsl", "bet2"]
    assert Path(args[2]) == path.resolve() and args[3] == "out"
    for option in ("-m", "-o", "-s", "-t", "-e", "--ct"):
        assert option in args
    assert "-n" not in args
    assert args[args.index("-f") + 1] == "0.2" and args[args.index("-g") + 1] == "-0.1"
    assert args[args.index("-r") + 1] == "80.0" and args[args.index("-w") + 1] == "0.5"
    i = args.index("-c")
    assert args[i + 1 : i + 4] == ["1.0", "2.0", "3.0"]
    assert call["threads"] == "2" and call["defects"] == "reproduce"
    # FSL's settings are the call's: NIfTI-1 outputs, nothing from the environment
    assert call["fsl"] == {"FSLOUTPUTTYPE": "NIFTI"}
    # and no FSL program can be found on the PATH (bet -Z's imglob: FSL-45)
    assert Path(call["path"]).resolve() == Path(call["cwd"]).resolve()
    expected = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    for img in (r.brain, r.mask, r.outline, r.skull):
        assert img is not None
        np.testing.assert_array_equal(img.data, expected)
        assert img.implementation.kind == "replica"
        assert img.implementation.package == "larmorx-nc"
        assert img.implementation.version == "9.9.9"
    assert r.mesh is not None
    np.testing.assert_array_equal(r.mesh.triangles, [[0, 1, 2]])
    assert r.mesh.vertices[2, 1] == 1.5
    plain = lx.fsl.bet(path)
    assert plain.mask is None and plain.mesh is None and plain.brain is not None
    args = calls(program)[-1]["args"]
    assert args[4:] == []  # the defaults pass no option


def test_bet_variations_run_the_script(scripts, tmp_path):
    program = stub(scripts)
    path = volume(tmp_path)
    for variation, flag in (("robust", "-R"), ("small_fov", "-Z"), ("fmri", "-F")):
        r = lx.fsl.bet(path, variation=variation, frac=0.3)
        args = calls(program)[-1]["args"]
        assert args[:2] == ["fsl", "bet"] and flag in args, args
        assert args[args.index("-f") + 1] == "0.3"
        assert r.brain is not None
        assert (r.mask is not None) == (variation == "fmri")
    r = lx.fsl.bet(path, variation="fmri", brain=False)
    assert r.brain is None and r.mask is not None


def test_bet_turns_notices_and_warnings_into_python_warnings(tmp_path, scripts):
    line = (
        "larmorx-nc: upstream defect FSL-40 (bet2) reproduced: the output name loses '.hdr' "
        "[LARMORX_UPSTREAM_DEFECTS=fix corrects it]\n"
        "Warning: An input intended to be a single 3D volume has multiple timepoints.\n"
    )
    stub(scripts, stderr=line)
    path = volume(tmp_path)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        lx.fsl.bet(path, upstream_defects="reproduce")
    kinds = [type(x.message).__name__ for x in w]
    assert "UpstreamDefectWarning" in kinds, kinds
    assert any("multiple timepoints" in str(x.message) for x in w)


def test_a_failing_replica_raises(tmp_path, scripts):
    stub(scripts, exit=1, stderr="bet2: stopped: FSL's bet2 aborts here (larmorx)\n")
    path = volume(tmp_path)
    with pytest.raises(lx.ReplicaError, match="aborts here") as e:
        lx.fsl.bet(path)
    assert e.value.returncode == 1


def test_betsurf_passes_its_inputs(scripts, tmp_path):
    program = stub(scripts)
    path = volume(tmp_path)
    surface = lx.fsl.Surface(
        np.array([[0.0, 0.0, 0.0], [1.25, 0.0, 0.0], [0.0, 1.0, 0.1]]), np.array([[0, 1, 2]])
    )
    matrix = np.array([[1.0, 0, 0, 2.5], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]])
    r = lx.fsl.betsurf(path, surface, matrix, skull_mask=True, outlines=True, precision=1)
    args = calls(program)[-1]["args"]
    assert args[:3] == ["fsl", "betsurf", "--t1only"]
    for option in ("-m", "-s", "-o"):
        assert option in args
    assert args[args.index("-p") + 1] == "1"
    t1, mesh, mat, out = args[-4:]
    assert Path(t1) == path.resolve() and out == "out"
    assert mesh.endswith("brain_mesh.vtk") and mat.endswith("t1_to_std.mat")
    assert r.inskull.vertices.shape == (3, 3) and r.skull_mask is not None
    assert r.outskin_outline is not None and r.outskull_mask is not None
    # a T2 drops --t1only and comes second
    r = lx.fsl.betsurf(path, surface, matrix, t2=path, masks=False)
    args = calls(program)[-1]["args"]
    assert "--t1only" not in args and "-m" not in args
    assert Path(args[-5]) == path.resolve() and Path(args[-4]) == path.resolve()
    assert r.inskull_mask is None


def test_read_vtk_and_the_surface_round_trip(tmp_path):
    from larmorx.fsl.bet import _write_vtk

    s = lx.fsl.Surface(
        np.array([[0.5, 1.0, -2.0], [3.0, 4.0, 5.0], [6.0, 7.0, 8.125]]), [[0, 1, 2]]
    )
    p = tmp_path / "s.vtk"
    _write_vtk(s, str(p))
    back = lx.fsl.read_vtk(p)
    np.testing.assert_array_equal(back.vertices, s.vertices)
    np.testing.assert_array_equal(back.triangles, [[0, 1, 2]])
    (tmp_path / "bad.vtk").write_text("not a mesh")
    with pytest.raises(ValueError, match="not a VTK"):
        lx.fsl.read_vtk(tmp_path / "bad.vtk")
