# SPDX-License-Identifier: Apache-2.0
"""``lx.afni.automask`` and ``lx.afni.unifize``: tools with only a replica (no clean-room
original yet). Without the ``larmorx-gpl`` program they refuse clearly; with a stand-in for it
(written by the test, as in ``test_replica.py``) they pass AFNI's options, settings and D13's
choice to it and read its outputs back. The replica itself is checked by its own golden tests
and by the parity suites against AFNI."""

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
    f.write(json.dumps({"args": args, "threads": os.environ.get("OMP_NUM_THREADS"),
                        "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"),
                        "afni": {k: v for k, v in os.environ.items()
                                 if k.upper().startswith(("AFNI_", "NIML_"))}}) + "\\n")
if args == ["--version"]:
    print("larmorx-gpl " + config["version"])
    sys.exit(0)
sys.stderr.write(config["stderr"])
source = args[args.index("-input") + 1] if "-input" in args else args[-1]
for option in ("-prefix", "-apply_prefix", "-depth", "-ssave", "-amsave"):
    if config["exit"] == 0 and option in args:
        shutil.copyfile(source, args[args.index(option) + 1])  # a copy of the input
sys.exit(config["exit"])
"""


def stub(directory: Path, exit: int = 0, stderr: str = "") -> Path:
    """A ``larmorx-gpl`` program in ``directory`` that records its calls and writes its input
    to each output it is given."""
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    config = {"version": "9.9.9", "exit": exit, "stderr": stderr}
    (directory / "config.json").write_text(json.dumps(config), encoding="utf-8")
    script = directory / "stub.py"
    if os.name == "nt":
        program = directory / "larmorx-gpl.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))
    else:
        program = directory / "larmorx-gpl"
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


def cli(tool: str, *args: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "afni", tool, *args])
    return code, out.getvalue(), err.getvalue()


def test_without_the_replica_they_refuse_and_name_the_install_command(scripts, tmp_path):
    path = volume(tmp_path)
    install = r'pip install "larmorx\[exact\]"'
    with pytest.raises(lx.ReplicaNotFoundError, match=install):
        lx.afni.automask(path)
    with pytest.raises(lx.ReplicaNotFoundError, match="no clean-room original of afni 3dUnifize"):
        lx.afni.unifize(path)
    with pytest.raises(NotImplementedError, match=r"specs/3dAutomask\.md"):
        lx.afni.automask(path, implementation="original")
    with pytest.raises(NotImplementedError, match=r"specs/3dUnifize\.md"):
        lx.afni.unifize(path, implementation="original")
    code, _, err = cli("3dAutomask", "-prefix", "m.nii", str(path))
    assert code == 127 and 'pip install "larmorx[exact]"' in err
    code, _, err = cli("3dUnifize", "-prefix", "u.nii", str(path))
    assert code == 127 and "no clean-room original of afni 3dUnifize" in err
    # The help still lists them.
    code, _, _ = cli("3dTshift")
    assert code == 0


def test_arguments_are_checked_before_anything_runs(scripts, tmp_path):
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


def test_automask_runs_the_replica_with_the_options(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
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
    (call,) = calls(program)
    args = call["args"]
    assert args[:3] == ["afni", "3dAutomask", "-q"]
    for option in ("-nograd", "-NN1", "-eclip"):
        assert option in args
    assert args[args.index("-clfrac") + 1] == "0.3" and args[args.index("-peels") + 1] == "2"
    assert args[args.index("-dilate") + 1] == "1" and args[args.index("-SI") + 1] == "130.0"
    assert args[-1] == str(path.resolve()) or Path(args[-1]) == path.resolve()
    assert call["threads"] == "2" and call["defects"] == "reproduce"
    assert call["afni"]["AFNI_NIFTI_VIEW"] == "tlrc"
    assert "AFNI_NIFTI_PRIORITY" not in call["afni"]
    assert call["afni"]["AFNI_PATH_SPACES_OK"] == "YES"
    expected = np.arange(6 * 5 * 4, dtype=np.int16).reshape(6, 5, 4)
    for img in (r.mask, r.masked, r.depth):
        assert img is not None
        np.testing.assert_array_equal(img.data, expected)
        assert img.implementation.kind == "replica"
        assert img.implementation.package == "larmorx-gpl"
    plain = lx.afni.automask(path)
    assert plain.masked is None and plain.depth is None


def test_unifize_runs_the_replica_and_passes_its_warnings(scripts, tmp_path):
    line = (
        "*+ WARNING: upstream defect AFNI-45 (3dUnifize) corrected: -T2up 100 points one past "
        "the sorted values [LARMORX_UPSTREAM_DEFECTS=reproduce gives 3dUnifize's result]\n"
        "*+ WARNING: Only processing sub-brick #0 (out of 3)\n"
    )
    program = stub(scripts, stderr=line)
    path = volume(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-45"):
        r = lx.afni.unifize(
            path,
            **lx.afni.fmriprep_unifize_options(),
            t2_percentile=100.0,
            save_scale=True,
            save_automask=True,
        )
    (call,) = calls(program)
    args = call["args"]
    assert args[:3] == ["afni", "3dUnifize", "-quiet"]
    i = args.index("-rbt")
    assert args[i + 1 : i + 4] == ["18.3", "65.0", "90.0"]
    assert args.count("-T2") == 1 and args[args.index("-T2up") + 1] == "100.0"
    assert args[args.index("-clfrac") + 1] == "0.2"
    assert args[-2] == "-input"
    assert r.scale is not None and r.automasked is not None
    two = lx.afni.unifize(path, t2=2, gm=True, duplo=False, epi=True, squash=False)
    args = calls(program)[-1]["args"]
    assert args.count("-T2") == 2
    for option in ("-GM", "-noduplo", "-EPI", "-nosquash"):
        assert option in args
    assert two.image.data.shape == (6, 5, 4)


def test_a_failing_replica_raises(scripts, tmp_path):
    stub(scripts, exit=1, stderr="** FATAL ERROR: Mask creation fails for unknown reasons!\n")
    with pytest.raises(lx.ReplicaError, match="Mask creation fails"):
        lx.afni.automask(volume(tmp_path))
