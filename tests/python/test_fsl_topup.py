# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.topup`` and ``lx.fsl.applytopup``: with a stand-in for the ``larmorx-nc`` program
(written by the test, as in ``test_fsl_fast.py``) they pass topup's and applytopup's options, the
acquisition parameters, FSL's settings and D13's choice, and read the outputs back; a
:class:`TopupResult` given to ``applytopup`` is written back as topup's files; without the
program both run their clean-room originals in this process. The
replicas themselves are checked by their golden tests (``crates-nc/larmorx-nc-cli/tests/
golden.rs``) and by the parity suite against FSL, topup's original by its suite ``fsl-topup``."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica

STUB = """\
import glob, json, os, shutil, sys
from pathlib import Path

here = Path(__file__).parent
args = sys.argv[1:]
record = {"args": args, "threads": os.environ.get("OMP_NUM_THREADS"),
          "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"), "cwd": os.getcwd(),
          "fsl": {k: v for k, v in os.environ.items() if k.upper().startswith("FSL")}}
if args[:2] == ["fsl", "applytopup"]:
    opts = dict(a.split("=", 1) for a in args[2:] if "=" in a)
    base = opts["--topup"]
    record["topup_files"] = sorted(os.path.basename(p) for p in glob.glob(base + "*"))
    if os.path.exists(base + "_movpar.txt"):
        record["movpar"] = Path(base + "_movpar.txt").read_text(encoding="utf-8")
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps(record) + "\\n")
if args == ["--version"]:
    print("larmorx-nc 9.9.9")
    sys.exit(0)
sys.stderr.write((here / "stderr.txt").read_text(encoding="utf-8"))
opts = dict(a.split("=", 1) for a in args[2:] if "=" in a)
if args[1] == "topup":
    image = opts["--imain"]
    rows = Path(opts["--datain"]).read_text(encoding="utf-8").splitlines()
    out = opts["--out"]
    shutil.copyfile(image, out + "_fieldcoef.nii")
    Path(out + "_movpar.txt").write_text("0  0  0  0  0  0  \\n" * len(rows), encoding="utf-8")
    Path(opts["--logout"]).write_text("the log\\n", encoding="utf-8")
    names = []
    for flag, suffixes in (("--fout", [""]), ("--iout", ["", "_mean"]),
                           ("--featout", ["_fieldmap_rads", "_fieldmap_mag"])):
        if flag in opts:
            names += [opts[flag] + s for s in suffixes]
    for flag in ("--dfout", "--jacout"):
        if flag in opts:
            names += [f"{opts[flag]}_{i + 1:02d}" for i in range(len(rows))]
    for name in names:
        shutil.copyfile(image, name + ".nii")
    if "--rbmout" in opts:
        for i in range(len(rows)):
            Path(f"{opts['--rbmout']}_{i + 1:02d}.mat").write_text(
                "1 0 0 0\\n0 1 0 0\\n0 0 1 0\\n0 0 0 1\\n", encoding="utf-8")
    print("stdout of topup")
else:
    first = opts["--imain"].split(",")[0]
    shutil.copyfile(first, opts["--out"] + ".nii")
    print("stdout of applytopup")
"""


def stub(directory: Path, stderr: str = "") -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / "stub.py"
    script.write_text(STUB, encoding="utf-8")
    (directory / "stderr.txt").write_text(stderr, encoding="utf-8")
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
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def pair(tmp_path: Path) -> Path:
    data = np.arange(6 * 5 * 4 * 2, dtype=np.float32).reshape(6, 5, 4, 2)
    path = tmp_path / "with space" / "pair.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


ACQ = [[0, 1, 0, 0.05], [0, -1, 0, 0.05]]


def smooth_pair(tmp_path: Path) -> Path:
    """Two volumes of one blob, shifted by a voxel along y against each other (positive values:
    topup's validity mask needs no zeros), 12 x 10 x 6 voxels of 2 mm."""
    x, y, z = np.meshgrid(np.arange(12), np.arange(10), np.arange(6), indexing="ij")
    vols = [
        100.0 + 50.0 * np.exp(-((x - 5.5) ** 2 + (y - c) ** 2 + (z - 2.5) ** 2) / 8.0)
        for c in (4.0, 5.0)
    ]
    path = tmp_path / "with space" / "blob.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((np.stack(vols, axis=3).astype(np.float32), np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


def test_without_the_replica(scripts, tmp_path):
    """Without ``larmorx-nc``, ``auto`` runs topup's and applytopup's clean-room originals (in
    this process) and ``replica`` refuses."""
    path = smooth_pair(tmp_path)
    kw: dict = dict(
        warpres=8.0,
        subsamp=1,
        fwhm=0.0,
        miter=2,
        lambda_=0.005,
        estmov=False,
        minmet="LM",
        corrected=True,
        jacobians=True,
        displacement_fields=True,
        rigid_body_matrices=True,
        verbose=True,
    )
    r = lx.fsl.topup(path, ACQ, **kw)
    assert r.coefficients.implementation == _replica.ORIGINAL
    assert r.field is not None and np.asarray(r.field.data).shape == (12, 10, 6)
    assert r.corrected is not None and np.asarray(r.corrected.data).shape == (12, 10, 6, 2)
    assert r.jacobians is not None and len(r.jacobians) == 2
    assert r.displacement_fields is not None and len(r.displacement_fields) == 2
    assert r.rigid_body_matrices is not None and np.allclose(r.rigid_body_matrices[0], np.eye(4))
    assert np.array_equal(r.movement_parameters, np.zeros((2, 6)))
    assert "--warpres=8" in r.log and r.stdout.startswith("SSD = ")
    costs = [float(ln.rsplit("=", 1)[1]) for ln in r.stdout.splitlines() if "Cost" in ln]
    assert len(costs) >= 2 and costs[-1] < costs[0]
    o = lx.fsl.topup(path, ACQ, implementation="clean-room", n_threads=3, **kw)
    assert np.array_equal(np.asarray(o.field.data), np.asarray(r.field.data))
    assert o.stdout == r.stdout
    with pytest.raises(lx.ReplicaNotFoundError, match="larmorx-nc"):
        lx.fsl.topup(path, ACQ, implementation="replica")
    # a failure of the original: topup's message in the error
    with pytest.raises(RuntimeError, match=r"clean-room original.*exit code 1"):
        lx.fsl.topup(path, [[0, 1, 0, 0.05]], **kw)
    # applytopup with the result: least-squares restoration of the pair, Jacobian modulation
    pair_imgs = [path, path]
    a = lx.fsl.applytopup(pair_imgs, ACQ, [1, 2], r)
    assert a.corrected.implementation == _replica.ORIGINAL
    assert np.asarray(a.corrected.data).shape == (12, 10, 6, 2)
    j = lx.fsl.applytopup(path, ACQ, [1], r, method="jac", implementation="clean-room")
    assert np.asarray(j.corrected.data).shape == (12, 10, 6, 2)
    assert np.isfinite(np.asarray(j.corrected.data)).all()
    with pytest.raises(lx.ReplicaNotFoundError, match="larmorx-nc"):
        lx.fsl.applytopup(path, ACQ, [1], r, implementation="replica")
    # the undocumented variational-Bayes restoration: its iterations on standard output
    v = lx.fsl.applytopup(pair_imgs, ACQ, [1, 2], r, method="vb2D")
    assert v.stdout.startswith("NCollections = 1\nNScans = 2\nslice = 0\nrelative lambda = 1,")
    assert np.asarray(v.corrected.data).shape == (12, 10, 6, 2)
    assert np.isfinite(np.asarray(v.corrected.data)).all()
    # FSL-144: jac with a phase encoding along z (which topup refuses) is refused by default
    with pytest.raises(RuntimeError, match=r"(?s)applytopup \(the clean-room original\).*FSL-144"):
        lx.fsl.applytopup(path, [[0, 0, 1, 0.05]], [1], r, method="jac")


def test_fmripreps_call(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR")  # must not reach the replica
    monkeypatch.setenv("FSL_NUM_THREADS", "4")
    path = pair(tmp_path)
    config = tmp_path / "with space" / "b02b0.cnf"
    config.write_text("--warpres=20\n", encoding="utf-8")
    r = lx.fsl.topup(
        path,
        ACQ,
        config=config,
        corrected=True,
        jacobians=True,
        rigid_body_matrices=True,
        displacement_fields=True,
    )
    (call,) = calls(program)
    args = call["args"]
    assert args[:2] == ["fsl", "topup"]
    opts = dict(a.split("=", 1) for a in args[2:] if "=" in a)
    assert Path(opts["--imain"]) == path.resolve()
    assert Path(opts["--config"]) == config.resolve()
    rows = Path(call["cwd"], opts["--datain"]).name
    assert rows == "acqparams.txt"
    assert opts["--out"] == "tu" and opts["--nthr"] == "1"
    for flag in ("--fout", "--iout", "--jacout", "--rbmout", "--dfout", "--logout"):
        assert flag in opts
    for flag in ("--warpres", "--miter", "--lambda", "--featout", "--scale", "--regrid"):
        assert flag not in opts  # left to the configuration
    assert "-v" not in args
    assert call["defects"] == "fix"
    assert call["fsl"]["FSLOUTPUTTYPE"] == "NIFTI" and "FSL_NUM_THREADS" not in call["fsl"]
    assert Path(call["fsl"]["FSLDIR"]).resolve() == Path(call["cwd"]).resolve()
    expected = np.arange(6 * 5 * 4 * 2, dtype=np.float32).reshape(6, 5, 4, 2)
    for img in (r.coefficients, r.field, r.corrected, r.corrected_mean):
        assert img is not None
        np.testing.assert_array_equal(img.data, expected)
        assert img.implementation.kind == "replica" and img.implementation.version == "9.9.9"
    assert r.movement_parameters.shape == (2, 6)
    assert r.jacobians is not None and len(r.jacobians) == 2
    assert r.displacement_fields is not None and len(r.displacement_fields) == 2
    assert r.rigid_body_matrices is not None
    np.testing.assert_array_equal(r.rigid_body_matrices[1], np.eye(4))
    assert r.feat_fieldmap_rads is None and r.feat_fieldmap_mag is None
    assert r.log == "the log\n" and r.stdout == "stdout of topup\n"


def test_options_and_applytopup_with_the_result(scripts, tmp_path):
    program = stub(scripts)
    path = pair(tmp_path)
    r = lx.fsl.topup(
        path,
        ACQ,
        warpres=[20, 14],
        subsamp=[2, 1],
        fwhm=(8, 4),
        miter=[3, 2],
        lambda_=[0.005, 0.0001],
        ssqlambda=False,
        regmod="membrane_energy",
        estmov=[True, False],
        minmet=["LM", "SCG"],
        splineorder=2,
        numprec="float",
        interp="linear",
        scale=True,
        regrid=False,
        field=False,
        feat=True,
        verbose=True,
        n_threads=3,
        upstream_defects="reproduce",
    )
    args = calls(program)[-1]["args"]
    opts = dict(a.split("=", 1) for a in args[2:] if "=" in a)
    assert opts["--warpres"] == "20.0,14.0" and opts["--subsamp"] == "2,1"
    assert opts["--fwhm"] == "8.0,4.0" and opts["--miter"] == "3,2"
    assert opts["--lambda"] == "0.005,0.0001" and opts["--estmov"] == "1,0"
    assert opts["--minmet"] == "0,1" and opts["--ssqlambda"] == "0"
    assert opts["--regmod"] == "membrane_energy" and opts["--splineorder"] == "2"
    assert opts["--numprec"] == "float" and opts["--interp"] == "linear"
    assert opts["--scale"] == "1" and opts["--regrid"] == "0" and opts["--nthr"] == "3"
    assert "--fout" not in opts and opts["--featout"] == "feat" and "-v" in args
    assert calls(program)[-1]["defects"] == "reproduce"
    assert r.field is None and r.feat_fieldmap_rads is not None
    # applytopup with the result: its coefficients and movements written back as topup's
    up = lx.load(path)
    a = lx.fsl.applytopup(
        [(up.data[..., 0], up.affine), (up.data[..., 1], up.affine)],
        ACQ,
        [1, 2],
        r,
        method="vb3D",
        interp="trilinear",
        datatype="short",
        verbose=True,
    )
    call = calls(program)[-1]
    args = call["args"]
    assert args[:2] == ["fsl", "applytopup"]
    opts = dict(a.split("=", 1) for a in args[2:] if "=" in a)
    assert opts["--topup"] == "tu" and opts["--inindex"] == "1,2"
    assert opts["--method"] == "vb3D" and opts["--interp"] == "trilinear"
    assert opts["--datatype"] == "short" and opts["--out"] == "hifi" and "-v" in args
    assert len(opts["--imain"].split(",")) == 2
    assert call["topup_files"] == ["tu_fieldcoef.nii", "tu_movpar.txt"]
    assert call["movpar"].splitlines()[1].split() == ["0.0"] * 6
    assert a.corrected.data.shape == (6, 5, 4) and a.stdout == "stdout of applytopup\n"
    # a base name of topup's files, as FSL takes it
    lx.fsl.applytopup(path, ACQ, [1], tmp_path / "tu", method="jac")
    opts = dict(a.split("=", 1) for a in calls(program)[-1]["args"][2:] if "=" in a)
    assert Path(opts["--topup"]) == (tmp_path / "tu").resolve()


def test_arguments_are_checked_before_anything_runs(scripts, tmp_path):
    program = stub(scripts)
    path = pair(tmp_path)
    for kwargs, match in (
        ({"minmet": "BFGS"}, "minmet"),
        ({"regmod": "tv"}, "regmod"),
        ({"numprec": "half"}, "numprec"),
        ({"interp": "sinc"}, "interp"),
        ({"splineorder": 4}, "splineorder"),
        ({"miter": 2.5}, "whole"),
        ({"warpres": float("nan")}, "finite"),
        ({"config": "b02b0.cnf"}, "fsldir"),
    ):
        with pytest.raises(ValueError, match=match):
            lx.fsl.topup(path, ACQ, **kwargs)
    with pytest.raises(ValueError, match="shape"):
        lx.fsl.topup(path, [[0, 1, 0]])
    for kwargs, match in (
        ({"method": "fast"}, "method"),
        ({"interp": "nn"}, "interp"),
        ({"datatype": "uint8"}, "datatype"),
    ):
        with pytest.raises(ValueError, match=match):
            lx.fsl.applytopup(path, ACQ, [1], "tu", **kwargs)
    with pytest.raises(ValueError, match="indices"):
        lx.fsl.applytopup([path, path], ACQ, [1], "tu")
    with pytest.raises(ValueError, match="at least one"):
        lx.fsl.applytopup([], ACQ, [], "tu")
    assert not (program.parent / "calls.jsonl").exists() or calls(program) == []
