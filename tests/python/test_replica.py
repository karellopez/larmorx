# SPDX-License-Identifier: Apache-2.0
"""Choosing between the clean-room original and the replica (``implementation=``,
``LARMORX_IMPLEMENTATION``), with a stand-in for the ``larmorx-gpl`` program written by the
test: a small Python script, launched by a shell script (or a ``.cmd`` file on Windows)."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
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
    f.write(json.dumps({"args": args, "threads": os.environ.get("OMP_NUM_THREADS")}) + "\\n")
if args == ["--version"]:
    print("larmorx-gpl " + config["version"])
    sys.exit(0)
sys.stdout.write(config["stdout"])
sys.stderr.write(config["stderr"])
if config["exit"] == 0 and "-prefix" in args:
    shutil.copyfile(args[-1], args[args.index("-prefix") + 1])  # a copy of the input
sys.exit(config["exit"])
"""


def stub(directory: Path, version="9.9.9", exit=0, stdout="", stderr="") -> Path:
    """A ``larmorx-gpl`` program in ``directory`` that reports ``version``, records its calls
    and writes its input to its ``-prefix``."""
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    config = {"version": version, "exit": exit, "stdout": stdout, "stderr": stderr}
    (directory / "config.json").write_text(json.dumps(config), encoding="utf-8")
    script = directory / "stub.py"
    if os.name == "nt":
        program = directory / "larmorx-gpl.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))  # bytes: no newline translation
    else:
        program = directory / "larmorx-gpl"
        program.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n', "utf-8")
        program.chmod(0o755)
    return program


def calls(program: Path) -> list[dict]:
    lines = (program.parent / "calls.jsonl").read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in lines]


AFFINE = np.diag([3.0, 3.0, 3.5, 1.0])


def series(tmp_path: Path) -> tuple[np.ndarray, Path]:
    """A 4D float32 series of 3 slices and 8 time points, saved in a directory with a space."""
    data = np.asfortranarray(np.arange(2 * 2 * 3 * 8, dtype=np.float32).reshape(2, 2, 3, 8))
    path = tmp_path / "with space" / "bold.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((data, AFFINE), path)
    return data, path


def cli(*args: str, implementation: str | None = None) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "afni", "3dTshift", *args], implementation=implementation)
    return code, out.getvalue(), err.getvalue()


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    """The Python environment's scripts directory, as larmorx sees it: empty to start with."""
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def test_auto_runs_the_original_when_no_replica_is_found(scripts, tmp_path):
    _, path = series(tmp_path)
    img = lx.afni.tshift(path, slice_times="alt+z", method="linear")
    assert img.implementation == lx.Implementation(
        "original", "larmorx", lx.__version__, "Apache-2.0"
    )
    out = tmp_path / "out.nii"
    code, _, err = cli("-verbose", "-tpattern", "alt+z", "-prefix", str(out), str(path))
    assert code == 0 and "implementation: original" in err


def test_asking_for_a_missing_replica_names_the_install_command(scripts, monkeypatch, tmp_path):
    _, path = series(tmp_path)
    install = 'pip install "larmorx[exact]"'
    with pytest.raises(lx.ReplicaNotFoundError, match=r'pip install "larmorx\[exact\]"'):
        lx.afni.tshift(path, slice_times="alt+z", implementation="replica")
    code, _, err = cli("-tpattern", "alt+z", str(path), implementation="replica")
    assert code == 127 and install in err
    with pytest.raises(ValueError, match="implementation must be"):
        lx.afni.tshift(path, slice_times="alt+z", implementation="exact")
    monkeypatch.setenv("LARMORX_IMPLEMENTATION", "exact")
    assert cli("-tpattern", "alt+z", str(path))[0] == 2
    monkeypatch.setenv("LARMORX_GPL_BIN", str(tmp_path / "missing"))
    with pytest.raises(ValueError, match="LARMORX_GPL_BIN is set to"):
        lx.afni.tshift(path, slice_times="alt+z")


def test_the_search_order(scripts, monkeypatch, tmp_path):
    explicit = stub(tmp_path / "env", version="1.0.0")
    in_scripts = stub(scripts)
    on_path = stub(tmp_path / "path")
    monkeypatch.setenv("PATH", os.pathsep.join([str(tmp_path / "empty"), str(on_path.parent)]))
    monkeypatch.setenv("LARMORX_GPL_BIN", str(explicit))
    selected = _replica.select("afni", "3dTshift", "auto")
    assert selected.program == str(explicit)
    assert selected.implementation().version == "1.0.0"
    monkeypatch.delenv("LARMORX_GPL_BIN")
    assert _replica.find("afni", "3dTshift") == str(in_scripts)
    monkeypatch.setattr(_replica, "search_dirs", list)
    assert _replica.find("afni", "3dTshift") == str(on_path)
    assert _replica.select("afni", "3dTshift", "original") is None


def test_the_replica_runs_as_a_separate_program(scripts, tmp_path):
    program = stub(scripts, stderr="*+ WARNING: from the replica\n")
    data, path = series(tmp_path)
    with pytest.warns(UserWarning, match="from the replica"):
        img = lx.afni.tshift(
            path,
            slice_times=[0.0, 1.5, 0.75],
            tr=2.0,
            tzero=0.5,
            ignore=1,
            restore="intercept",
            n_threads=3,
            implementation="auto",
        )
    assert img.implementation == lx.Implementation(
        "replica", "larmorx-gpl", "9.9.9", "GPL-3.0-or-later", str(program)
    )
    np.testing.assert_array_equal(img.data, data)  # the stub copied its input
    version, run = calls(program)
    assert version["args"] == ["--version"]
    assert run["threads"] == "3"
    assert run["args"] == [
        "afni", "3dTshift", "-Fourier", "-rlt+", "-ignore", "1", "-TR", "2.0s",
        "-tzero", "0.5", "-tpattern", "@slice_times.1D", "-prefix", "out.nii", str(path),
    ]  # fmt: skip

    # In memory: the image is written for the replica as it is.
    stub(scripts)  # no warning this time
    with pytest.warns(UserWarning, match="-no_detrend"):
        img = lx.afni.tshift(
            (data, AFFINE), slice_times="seq-z", tr=2.0, method="fourier", detrend=False
        )
    np.testing.assert_array_equal(img.data, data)
    args = calls(program)[-1]["args"]
    assert args[2:5] == ["-linear", "-no_detrend", "-Fourier"]  # AFNI's order rule
    assert args[-1].endswith("input.nii")


def test_a_failing_replica_raises_and_never_falls_back(scripts, tmp_path):
    stub(scripts, exit=1, stderr="** FATAL ERROR: boom\n")
    _, path = series(tmp_path)
    with pytest.raises(lx.ReplicaError, match="boom") as e:
        lx.afni.tshift(path, slice_times="alt+z")
    assert e.value.returncode == 1


def test_the_command_line_passes_the_replica_through(scripts, monkeypatch):
    stub(scripts, exit=3, stdout="from the stub\n")
    code, out, _ = cli("-x", "in.nii")
    assert (code, out.rstrip()) == (3, "from the stub")  # "\r\n" on Windows, passed through
    code, _, err = cli("-x", "in.nii", implementation="original")
    assert code == 1 and "Unknown option: -x" in err
    monkeypatch.setenv("LARMORX_IMPLEMENTATION", "original")
    assert cli("-x", "in.nii")[0] == 1
