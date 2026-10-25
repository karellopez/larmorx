# SPDX-License-Identifier: Apache-2.0
"""``lx.mri.hmc(implementation=...)`` and ``larmorx fsl mcflirt`` with the replica of
``larmorx-nc``: a stand-in program written by the test (a small Python script that records its
arguments and writes mcflirt's output files), and, where it is built, the replica itself."""

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
import json, os, shutil, sys
from pathlib import Path

here = Path(__file__).parent
args = sys.argv[1:]
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps({"args": args, "threads": os.environ.get("OMP_NUM_THREADS"),
                        "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"),
                        "fsl": {k: v for k, v in os.environ.items()
                                if k.upper().startswith("FSL")}}) + "\\n")
if args == ["--version"]:
    print("larmorx-nc 9.9.9")
    sys.exit(0)
assert args[:2] == ["fsl", "mcflirt"], args
a = args[2:]
src, out = a[a.index("-in") + 1], a[a.index("-out") + 1]
n = int((here / "volumes.txt").read_text())
Path(out + ".mat").mkdir()
for t in range(n):
    (Path(out + ".mat") / f"MAT_{t:04d}").write_text(
        f"1  0  0  {0.5 * t:.6f}  \\n0  1  0  0  \\n0  0  1  0  \\n0  0  0  1  \\n")
Path(out + ".par").write_text("".join(f"0  0  0  {0.5 * t:g}  0  0  \\n" for t in range(n)))
Path(out + "_abs.rms").write_text("".join(f"{0.5 * t:g}\\n" for t in range(n)))
Path(out + "_rel.rms").write_text("".join("0.5\\n" for _ in range(n - 1)))
shutil.copyfile(src, out + ".nii")
if "-meanvol" in a:
    shutil.copyfile(src, out + "_mean_reg.nii")
sys.stderr.write("larmorx-nc: upstream defect FSL-01 (mcflirt) corrected: the last volume "
                 "starts from its predecessor [LARMORX_UPSTREAM_DEFECTS=reproduce gives "
                 "mcflirt's result]\\n")
"""


def stub(directory: Path, volumes: int) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    (directory / "volumes.txt").write_text(str(volumes), encoding="utf-8")
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
    return [json.loads(line) for line in lines]


AFFINE = np.diag([-3.0, 3.0, 3.5, 1.0])


def series(tmp_path: Path, n: int = 5) -> Path:
    data = np.zeros((8, 7, 6, n), dtype=np.float32, order="F")
    for t in range(n):
        data[2:6, 2:5, 1:5, t] = 100.0 + t
    path = tmp_path / "with space" / "bold.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, AFFINE), path)
    return path


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def test_the_registry_has_mcflirt():
    r = _replica.registry()[("fsl", "mcflirt")]
    assert (r.package, r.program, r.env, r.licence) == (
        "larmorx-nc",
        "larmorx-nc",
        "LARMORX_NC_BIN",
        "LicenseRef-FSL",
    )
    assert r.original and "larmorx-nc" in r.install


def test_auto_without_the_replica_runs_the_original(scripts, tmp_path):
    res = lx.mri.hmc(series(tmp_path), resample=False)
    assert res.implementation.kind == "original"
    with pytest.raises(lx.ReplicaNotFoundError, match="pip install larmorx-nc"):
        lx.mri.hmc(series(tmp_path), implementation="replica")


def test_the_replica_receives_mcflirts_command_line(scripts, monkeypatch, tmp_path):
    program = stub(scripts, 5)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR")
    monkeypatch.setenv("FSL_LOAD_NIFTI_EXTENSIONS", "1")
    path = series(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-01"):
        res = lx.mri.hmc(path, ref_index=1, stages=4, dof=7, cost="corratio", bins=64,
                         smooth=2.0, rotation=1.5, fudge=True, fov=12, final="sinc",
                         init=np.eye(4), n_threads=2)  # fmt: skip
    assert res.implementation == lx.Implementation(
        "replica", "larmorx-nc", "9.9.9", "LicenseRef-FSL", str(program)
    )
    call = calls(program)[-1]
    a = call["args"]
    assert a[:4] == ["fsl", "mcflirt", "-in", str(path)]
    for opt, value in [("-out", "out"), ("-refvol", "1"), ("-stages", "4"), ("-dof", "7"),
                       ("-cost", "corratio"), ("-bins", "64"), ("-smooth", "2.0"),
                       ("-rotation", "1.5"), ("-fov", "12"), ("-init", "init.mat")]:  # fmt: skip
        assert a[a.index(opt) + 1] == value, opt
    for flag in ("-mats", "-plots", "-rmsrel", "-rmsabs", "-fudge", "-sinc_final"):
        assert flag in a
    assert "-reffile" not in a and "-meanvol" not in a and "-2d" not in a
    # FSL's variables are the wrapper's own, not this process's.
    assert call["fsl"] == {"FSLOUTPUTTYPE": "NIFTI"}
    assert (call["threads"], call["defects"]) == ("2", "fix")
    # mcflirt's files, read back.
    assert res.matrices.shape == (5, 4, 4) and res.matrices[3, 0, 3] == 1.5
    assert res.params.shape == (5, 6) and res.params[4, 3] == 2.0
    assert res.rms_abs.tolist() == [0.0, 0.5, 1.0, 1.5, 2.0]
    assert res.rms_rel.tolist() == [0.5] * 4
    assert res.affines.shape == res.itk.shape == (5, 4, 4)
    assert res.fd.shape == (5,) and res.fd[1] == pytest.approx(0.5)
    assert res.reference_index == 1 and not res.in_plane
    assert res.image is not None and res.image.implementation == res.implementation
    assert res.mean is None


def test_reference_mean_and_options_only_the_original_has(scripts, tmp_path):
    program = stub(scripts, 5)
    path = series(tmp_path)
    ref = lx.load(path).with_data(np.asarray(lx.load(path).data)[..., 0])
    res = lx.mri.hmc(lx.load(path), reference=ref, mean=True, in_plane="always",
                     resample=False, upstream_defects="reproduce")  # fmt: skip
    a = calls(program)[-1]["args"]
    assert "-reffile" in a and "-meanvol" in a and "-2d" in a
    assert calls(program)[-1]["defects"] == "reproduce"
    assert res.reference_index is None and res.image is None and res.mean is not None
    n = len(calls(program))
    # Options mcflirt's command line does not have: the original under "auto", an error asked.
    res = lx.mri.hmc(path, in_plane="never", resample=False)
    assert res.implementation.kind == "original" and len(calls(program)) == n
    with pytest.raises(ValueError, match="no in_plane='never'"):
        lx.mri.hmc(path, in_plane="never", implementation="replica")
    with pytest.raises(ValueError, match="normcorr_count='weights'"):
        lx.mri.hmc(path, normcorr_count="weights", implementation="replica")


def _built_replica() -> Path | None:
    if env := os.environ.get("LARMORX_NC_BIN"):
        return Path(env)
    root = Path(__file__).resolve().parents[2] / "crates-nc" / "target" / "release"
    exe = root / ("larmorx-nc.exe" if os.name == "nt" else "larmorx-nc")
    return exe if exe.is_file() else None


@pytest.mark.skipif(_built_replica() is None, reason="the larmorx-nc replica is not built")
def test_the_replica_itself(monkeypatch, no_replica, tmp_path):
    monkeypatch.setenv("LARMORX_NC_BIN", str(_built_replica()))
    rng = np.random.default_rng(0)
    data = np.zeros((16, 14, 10, 4), dtype=np.float32, order="F")
    for t in range(4):
        data[4 + t % 2 : 11 + t % 2, 3:11, 2:8, t] = 500.0
    data += rng.random(data.shape, dtype=np.float32) * 10
    path = tmp_path / "bold.nii"
    lx.save((data, np.diag([-3.0, 3.0, 3.0, 1.0])), path)
    res = lx.mri.hmc(path, implementation="replica", n_threads=1)
    assert res.implementation.kind == "replica" and res.implementation.package == "larmorx-nc"
    assert res.matrices.shape == (4, 4, 4) and np.isfinite(res.matrices).all()
    # The x shift of one voxel (3 mm) is found in the odd volumes.
    assert abs(abs(res.matrices[1, 0, 3]) - 3.0) < 0.5
    again = lx.mri.hmc(lx.load(path), implementation="replica", n_threads=3)
    assert np.array_equal(again.matrices, res.matrices)
