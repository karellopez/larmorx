# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.fast``: with a stand-in for the ``larmorx-nc`` program (written by the test, as in
``test_fsl_bet.py``) it passes FAST's options, its channels (channel 1 named last, as FAST reads
them), FSL's settings and D13's choice, and reads the outputs back; without the program it
refuses clearly (FAST has no clean-room original yet). The replica itself is checked by its own
golden tests (``crates-nc/larmorx-nc-cli/tests/golden.rs``) and by the parity suite against
FSL."""

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
                        "cwd": os.getcwd(),
                        "fsl": {k: v for k, v in os.environ.items()
                                if k.upper().startswith("FSL")}}) + "\\n")
if args == ["--version"]:
    print("larmorx-nc 9.9.9")
    sys.exit(0)
sys.stderr.write((here / "stderr.txt").read_text(encoding="utf-8"))
rest = args[2:]
out = rest[rest.index("-o") + 1]
n = int(rest[rest.index("-n") + 1])
s = int(rest[rest.index("-S") + 1])
first = rest[-1]
names = ["_seg"]
if "-g" in rest:
    names += [f"_seg_{i}" for i in range(n)]
if "-p" in rest:
    names += [f"_prob_{i}" for i in range(n)]
if "--nopve" not in rest:
    names += [f"_pve_{i}" for i in range(n)] + ["_pveseg", "_mixeltype"]
for flag, base in (("-B", "_restore"), ("-b", "_bias")):
    if flag in rest:
        names += [base] if s == 1 else [f"{base}_{i + 1}" for i in range(s)]
for name in names:
    shutil.copyfile(first, out + name + ".nii")
print("stdout of fast")
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


def volume(tmp_path: Path, name: str = "t1", offset: int = 0) -> Path:
    data = (np.arange(6 * 5 * 4, dtype=np.int16) + offset).reshape(6, 5, 4)
    path = tmp_path / "with space" / f"{name}.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


def test_without_the_replica(scripts, tmp_path):
    path = volume(tmp_path)
    with pytest.raises(lx.ReplicaNotFoundError, match="no clean-room original of fsl fast"):
        lx.fsl.fast(path)
    with pytest.raises(NotImplementedError, match="larmorx-nc"):
        lx.fsl.fast(path, implementation="original")


def test_fmripreps_call(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR")  # must not reach the replica
    monkeypatch.setenv("FSLDIR", "/nowhere")
    path = volume(tmp_path)
    r = lx.fsl.fast(
        path, bias_iterations=0, no_bias=True, probabilities=True, segments=True, n_threads=2
    )
    (call,) = calls(program)
    args = call["args"]
    assert args[:2] == ["fsl", "fast"]
    assert Path(args[-1]) == path.resolve()
    assert args[args.index("-S") + 1] == "1" and args[args.index("-I") + 1] == "0"
    for option in ("-N", "-p", "-g"):
        assert option in args
    for option in ("-B", "-b", "--nopve", "-v", "-a", "-A", "-P", "-s"):
        assert option not in args
    assert call["threads"] == "2" and call["defects"] == "fix"
    # FSL's settings are the call's: NIfTI-1 outputs, FSLDIR the call's own (no priors asked)
    assert call["fsl"]["FSLOUTPUTTYPE"] == "NIFTI"
    assert Path(call["fsl"]["FSLDIR"]).resolve() == Path(call["cwd"]).resolve()
    expected = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    assert r.segments is not None and len(r.segments) == 3
    assert r.probabilities is not None and len(r.probabilities) == 3
    assert r.partial_volumes is not None and len(r.partial_volumes) == 3
    for img in (r.segmentation, *r.segments, *r.probabilities, r.pve_segmentation):
        np.testing.assert_array_equal(img.data, expected)
        assert img.implementation.kind == "replica"
        assert img.implementation.version == "9.9.9"
    assert r.restored is None and r.bias_fields is None
    assert r.stdout == "stdout of fast\n"


def test_channels_options_and_priors(scripts, tmp_path):
    program = stub(scripts)
    t1, t2 = volume(tmp_path, "t1"), volume(tmp_path, "t2", 1000)
    r = lx.fsl.fast(
        [t1, t2],
        n_classes=2,
        image_type="T2",
        bias_lowpass=15,
        smoothness=0.2,
        mixel_smoothness=0.5,
        initial_smoothness=0.05,
        fixed_iterations=2,
        init_iterations=5,
        pve=False,
        restored=True,
        bias_field=True,
        manual_means=[[100, 400], [900, 300]],
        priors_matrix=np.eye(4),
        priors=[t1, t2, t1],
        priors_throughout=True,
        verbose=True,
        upstream_defects="reproduce",
    )
    args = calls(program)[-1]["args"]
    # channel 1 is the last word of FAST's command line
    assert [Path(a) for a in args[-2:]] == [t2.resolve(), t1.resolve()]
    assert args[args.index("-S") + 1] == "2" and args[args.index("-t") + 1] == "2"
    assert args[args.index("-n") + 1] == "2" and args[args.index("-l") + 1] == "15.0"
    assert args[args.index("-H") + 1] == "0.2" and args[args.index("-R") + 1] == "0.5"
    assert args[args.index("-O") + 1] == "2" and args[args.index("-W") + 1] == "5"
    for option in ("--nopve", "-B", "-b", "-P", "-v"):
        assert option in args
    a = args.index("-A")
    assert [Path(p) for p in args[a + 1 : a + 4]] == [t1.resolve(), t2.resolve(), t1.resolve()]
    means = Path(args[args.index("-s") + 1])
    assert not means.exists()  # written in the call's temporary directory
    assert calls(program)[-1]["defects"] == "reproduce"
    assert r.partial_volumes is None and r.mixeltype is None
    assert r.restored is not None and len(r.restored) == 2
    assert r.bias_fields is not None and len(r.bias_fields) == 2


def test_arguments_are_checked_before_anything_runs(scripts, tmp_path):
    program = stub(scripts)
    path = volume(tmp_path)
    for kwargs, match in (
        ({"n_classes": 0}, "n_classes"),
        ({"bias_lowpass": 0}, "bias_lowpass"),
        ({"smoothness": float("nan")}, "smoothness"),
        ({"image_type": "T3"}, "image_type"),
        ({"priors_throughout": True}, "priors_matrix"),
        ({"priors_matrix": np.eye(4)}, "fsldir"),
        ({"priors_matrix": np.eye(4), "priors": [path], "fsldir": "x"}, "three"),
        ({"priors_matrix": np.eye(4), "fsldir": "x", "n_classes": 4}, "2 or 3"),
        ({"manual_means": [1, 2]}, "manual_means"),
        ({"manual_means": [1, 2, -3]}, "positive"),
    ):
        with pytest.raises(ValueError, match=match):
            lx.fsl.fast(path, **kwargs)
    with pytest.raises(ValueError, match="at most 8"):
        lx.fsl.fast([path] * 9)
    with pytest.raises(ValueError, match="at least one"):
        lx.fsl.fast([])
    assert not (program.parent / "calls.jsonl").exists() or calls(program) == []


def test_standard_priors_use_fsldir_and_notices_become_warnings(scripts, tmp_path):
    line = (
        "larmorx-nc: upstream defect FSL-96 (fast) corrected: -p's probability maps outside the "
        "mask (8 voxels), which FAST never writes: written as 0 "
        "[LARMORX_UPSTREAM_DEFECTS=reproduce reproduces it]\n"
    )
    program = stub(scripts, stderr=line)
    path = volume(tmp_path)
    fsldir = tmp_path / "fsl"
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-96"):
        lx.fsl.fast(path, priors_matrix=np.eye(4), fsldir=fsldir, probabilities=True)
    call = calls(program)[-1]
    assert call["fsl"]["FSLDIR"] == str(fsldir.resolve())
    assert "-a" in call["args"] and "-A" not in call["args"]
