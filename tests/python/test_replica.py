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

import nibabel as nib
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
                                 if k.upper().startswith(("AFNI_", "NIML_"))
                                 or k.upper() == "IDCODE_PREFIX"}}) + "\\n")
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
    assert run["defects"] == "fix"  # D13's choice reaches the replica as its variable
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


def test_the_replica_receives_the_upstream_defects_choice(scripts, tmp_path):
    """``upstream_defects`` reaches the replica as ``LARMORX_UPSTREAM_DEFECTS``, and its
    notice lines come back as :class:`larmorx.UpstreamDefectWarning`."""
    line = (
        "*+ WARNING: upstream defect AFNI-01 (3dTshift) reproduced: the negative scale factor "
        "-0.5 (scl_slope) is applied on output only [LARMORX_UPSTREAM_DEFECTS=fix corrects it]\n"
    )
    program = stub(scripts, stderr=line)
    _, path = series(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-01") as record:
        lx.afni.tshift(path, slice_times="alt+z", upstream_defects="reproduce")
    w = next(r.message for r in record if isinstance(r.message, lx.UpstreamDefectWarning))
    assert (w.entry, w.tool, w.mode) == ("AFNI-01", "3dTshift", "reproduce")
    assert w.detail.endswith("is applied on output only")
    assert calls(program)[-1]["defects"] == "reproduce"


def test_afni_global_options_and_variables_reach_the_replica(scripts, monkeypatch, tmp_path):
    """On the command line, AFNI's global options (``-overwrite``, ``-Dname=value``) pass
    through larmorx's dispatch unchanged and in place, wherever they stand, and AFNI's
    environment variables reach the replica, which takes them out and applies them itself.
    ``lx.afni.tshift`` never passes the process environment's AFNI variables on (CLAUDE.md
    rule 6): only its ``afni_settings`` and the ones its own use of the replica needs."""
    program = stub(scripts)
    _, path = series(tmp_path)
    out = tmp_path / "out.nii"
    out.write_text("taken")
    monkeypatch.setenv("AFNI_NIFTI_VIEW", "tlrc")
    monkeypatch.setenv("AFNI_MESSAGE_PREFIX", "run1")
    args = [
        "-DAFNI_NIFTI_PRIORITY=Q",
        "-tpattern",
        "-overwrite",
        "alt+z",
        "-prefix",
        str(out),
        str(path),
    ]
    code, _, err = cli(*args)
    assert code == 0, err
    run = calls(program)[-1]
    assert run["args"] == ["afni", "3dTshift", *args]
    assert run["afni"]["AFNI_NIFTI_VIEW"] == "tlrc"
    assert run["afni"]["AFNI_MESSAGE_PREFIX"] == "run1"

    lx.afni.tshift(path, slice_times="alt+z")
    fixed = {
        "AFNI_PATH_SPACES_OK": "YES",  # a path is one file
        "AFNI_MESSAGE_COLORIZE": "NO",  # plain messages, read as warnings
        "AFNI_NO_OBLIQUE_WARNING": "YES",  # AFNI's advice, not a warning here
    }
    assert calls(program)[-1]["afni"] == fixed  # AFNI_NIFTI_VIEW, AFNI_MESSAGE_PREFIX: not


def test_afni_settings_are_explicit_never_the_environment(scripts, monkeypatch, tmp_path):
    """``afni_settings`` reaches the replica; AFNI's variables of the process environment never
    do, whatever their case (CLAUDE.md rule 6): the same call gives the same result whatever
    the shell has set."""
    program = stub(scripts)
    _, path = series(tmp_path)
    fixed = {"AFNI_PATH_SPACES_OK": "YES", "AFNI_MESSAGE_COLORIZE": "NO"}
    fixed["AFNI_NO_OBLIQUE_WARNING"] = "YES"
    lx.afni.tshift(path, slice_times="alt+z")
    assert calls(program)[-1]["afni"] == fixed
    for name, value in [
        ("AFNI_NIFTI_SCALE", "NO"),
        ("AFNI_NIFTI_PRIORITY", "Q"),
        ("afni_nifti_view", "tlrc"),  # Windows reads names whatever their case
        ("AFNI_PATH_SPACES_OK", "NO"),
        ("AFNI_MESSAGE_PREFIX", "run1"),
        ("NIML_DEBUG", "1"),
        ("IDCODE_PREFIX", "XYZ"),
    ]:
        monkeypatch.setenv(name, value)
    lx.afni.tshift(path, slice_times="alt+z")
    assert calls(program)[-1]["afni"] == fixed  # no effect

    settings = {"AFNI_NIFTI_SCALE": "NO", "AFNI_NIFTI_VIEW": "tlrc", "AFNI_DECONFLICT": "YES"}
    lx.afni.tshift(path, slice_times="alt+z", afni_settings=settings)
    assert calls(program)[-1]["afni"] == {**settings, **fixed}  # these, and only these


def test_afni_settings_are_checked(no_replica, tmp_path):
    """Names 3dTshift does not read, the settings ``tshift`` makes itself, empty values and
    other types are refused, whichever implementation would run."""
    _, path = series(tmp_path)
    for settings, error, match in [
        ({"AFNI_NIFTI_SCAL": "NO"}, ValueError, "does not read 'AFNI_NIFTI_SCAL'"),
        ({"afni_nifti_scale": "NO"}, ValueError, "case-sensitive"),
        ({"PATH": "/bin"}, ValueError, "does not read 'PATH'"),
        ({"AFNI_PATH_SPACES_OK": "YES"}, ValueError, "sets AFNI_PATH_SPACES_OK itself"),
        ({"AFNI_MESSAGE_PREFIX": "run1"}, ValueError, "sets AFNI_MESSAGE_PREFIX itself"),
        ({"AFNI_NIFTI_SCALE": ""}, ValueError, "non-empty value"),
        ({"AFNI_NIFTI_SCALE": 0}, TypeError, "must be str"),
        ([("AFNI_NIFTI_SCALE", "NO")], TypeError, "mapping"),
    ]:
        with pytest.raises(error, match=match):
            lx.afni.tshift(path, slice_times="alt+z", afni_settings=settings)


def test_the_original_applies_afni_settings(no_replica, monkeypatch, tmp_path):
    """The clean-room original reads AFNI's variables from ``afni_settings`` (completing
    3dTshift, part 2b), never from the environment: NIfTI-2 output, the tlrc view's xform code,
    and ``AFNI_NIFTI_SCALE=NO`` keeping int16 data with their intercept in the header (AFNI-23
    corrected); the variables that change nothing a call returns are accepted."""
    _, path = series(tmp_path)
    reference = lx.afni.tshift(path, slice_times="alt+z", method="linear")
    assert reference.header.version == 1
    # AFNI variables in the environment change nothing.
    monkeypatch.setenv("AFNI_NIFTI_WRITE_TYPE", "2")
    monkeypatch.setenv("AFNI_NIFTI_VIEW", "tlrc")
    monkeypatch.setenv("AFNI_NIFTI_SCALE", "NO")
    img = lx.afni.tshift(
        path,
        slice_times="alt+z",
        method="linear",
        afni_settings={"AFNI_DECONFLICT": "YES", "AFNI_NIFTI_TYPE_WARN": "YES"},
    )
    np.testing.assert_array_equal(img.data, reference.data)
    assert img.header.to_bytes() == reference.header.to_bytes()
    # NIfTI-2: the same values in the wider fields.
    two = lx.afni.tshift(
        path,
        slice_times="alt+z",
        method="linear",
        implementation="original",
        afni_settings={"AFNI_NIFTI_WRITE_TYPE": "2"},
    )
    assert two.header.version == 2
    np.testing.assert_array_equal(two.data, reference.data)
    np.testing.assert_array_equal(two.affine, reference.affine)
    # The tlrc view: sform code 2 is written as code 3 (TLRC).
    aligned = tmp_path / "aligned.nii"
    img = nib.Nifti1Image(np.asarray(nib.load(path).dataobj), AFFINE)
    img.header.set_sform(AFFINE, 2)
    img.header.set_qform(AFFINE, 0)
    img.to_filename(aligned)
    codes = lx.afni.tshift(aligned, slice_times="alt+z", afni_settings={"AFNI_NIFTI_VIEW": "tlrc"})
    assert (codes.header.qform_code, codes.header.sform_code) == (3, 3)
    # AFNI_NIFTI_SCALE=NO: int16 kept, the intercept in the header (AFNI-23 corrected); AFNI's
    # output, off by the intercept, with upstream_defects="reproduce".
    values = np.asfortranarray(np.arange(2 * 2 * 3 * 8, dtype=np.int16).reshape(2, 2, 3, 8))
    values = values * 7 + 1000
    hdr = nib.Nifti1Image(values, AFFINE).header
    hdr.set_data_dtype(np.int16)
    hdr["vox_offset"] = 352
    hdr["pixdim"][4] = 2.0
    hdr.set_slope_inter(0.25, 100.0)
    scaled = tmp_path / "scaled.nii"
    scaled.write_bytes(hdr.binaryblock + b"\0" * 4 + values.tobytes(order="F"))
    plain = lx.afni.tshift(scaled, slice_times="alt+z", method="linear")
    settings = {"AFNI_NIFTI_SCALE": "NO"}
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-23"):
        kept = lx.afni.tshift(scaled, slice_times="alt+z", method="linear", afni_settings=settings)
    assert (kept.header.scl_slope, kept.header.scl_inter) == (0.25, 100.0)
    np.testing.assert_allclose(kept.data, plain.data, atol=0.125)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-23"):
        afni = lx.afni.tshift(
            scaled,
            slice_times="alt+z",
            method="linear",
            afni_settings=settings,
            upstream_defects="reproduce",
        )
    np.testing.assert_allclose(afni.data, plain.data - 100.0, atol=0.125)


def test_a_replica_output_with_an_intercept_is_scaled(scripts, tmp_path):
    """The replica writes the input's intercept back when it corrects AFNI-23: the result's
    values are those ``lx.load`` gives, slope and intercept applied."""
    stub(scripts)  # its output is a copy of its input
    values = np.asfortranarray(np.arange(2 * 2 * 3 * 8, dtype=np.int16).reshape(2, 2, 3, 8))
    values += 1000
    hdr = nib.Nifti1Image(values, AFFINE).header
    hdr.set_data_dtype(np.int16)
    hdr["vox_offset"] = 352
    hdr.set_slope_inter(0.25, 100.0)
    path = tmp_path / "scaled.nii"
    path.write_bytes(hdr.binaryblock + b"\0" * 4 + values.tobytes(order="F"))
    img = lx.afni.tshift(path, slice_times="alt+z")
    np.testing.assert_array_equal(img.data, lx.load(path).data)
    assert img.data[0, 0, 0, 0] == 1000 * 0.25 + 100


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
