# SPDX-License-Identifier: Apache-2.0
"""Choosing between the clean-room original and the replica (``implementation=``,
``LARMORX_IMPLEMENTATION``). AFNI's replicas are built into larmorx (since 2026-10-29): they run
in this process, for real here, or through a stand-in that records what the wrappers give them
(``builtin_stub``). The search for a replica in a package of its own follows its program
(``larmorx-nc``, stood in for by a script written by the test); the FSL wrappers' use of it is in
``test_mri_replica.py`` and ``test_fsl_bet.py``."""

from __future__ import annotations

import contextlib
import io
import os
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica
from larmorx.cli import run as cli_run

REPLICA = lx.Implementation("replica", "larmorx", lx.__version__, "Apache-2.0 AND CC-BY-4.0")
ORIGINAL = lx.Implementation("original", "larmorx", lx.__version__, "Apache-2.0")

VERSION_STUB = """\
import sys
if sys.argv[1:] == ["--version"]:
    print("larmorx-nc {version}")
    sys.exit(0)
sys.exit(3)
"""


def program_stub(directory: Path, version: str = "9.9.9") -> Path:
    """A ``larmorx-nc`` program in ``directory`` that reports ``version``."""
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / "stub.py"
    script.write_text(VERSION_STUB.format(version=version), encoding="utf-8")
    if os.name == "nt":
        program = directory / "larmorx-nc.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))  # bytes: no newline translation
    else:
        program = directory / "larmorx-nc"
        program.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n', "utf-8")
        program.chmod(0o755)
    return program


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


def test_the_registry_has_the_afni_replicas_built_in():
    for tool in ("3dTshift", "3dAutomask", "3dUnifize"):
        r = _replica.registry()[("afni", tool)]
        assert r.builtin and r.original
        assert (r.package, r.program, r.env, r.install) == ("larmorx", None, None, None)
        assert r.licence == "Apache-2.0 AND CC-BY-4.0"
        assert _replica.find("afni", tool) is None
        selected = _replica.select("afni", tool, "auto")
        assert selected is not None and selected.program is None
        assert selected.implementation() == REPLICA
        assert _replica.select("afni", tool, "original") is None
    assert not _replica.registry()[("fsl", "mcflirt")].builtin


@pytest.mark.parametrize("method", ["fourier", "linear", "wsinc9"])
def test_auto_runs_the_built_in_replica(no_replica, tmp_path, method):
    """``"auto"`` runs the replica, in this process, and gives what the command line's replica
    writes; ``"original"`` runs the clean-room original, as ``LARMORX_IMPLEMENTATION=original``
    does on the command line."""
    _, path = series(tmp_path)
    img = lx.afni.tshift(path, slice_times="alt+z", method=method, n_threads=2)
    assert img.implementation == REPLICA
    forced = lx.afni.tshift(path, slice_times="alt+z", method=method, implementation="replica")
    np.testing.assert_array_equal(forced.data, img.data)
    out = tmp_path / f"out_{method}.nii"
    flag = {"fourier": "-Fourier", "linear": "-linear", "wsinc9": "-wsinc9"}[method]
    code, _, err = cli("-verbose", flag, "-tpattern", "alt+z", "-prefix", str(out), str(path))
    assert code == 0 and "implementation: replica (larmorx " in err, err
    np.testing.assert_array_equal(lx.load(out).data, img.data)
    original = lx.afni.tshift(path, slice_times="alt+z", method=method, implementation="original")
    assert original.implementation == ORIGINAL
    out = tmp_path / f"orig_{method}.nii"
    code, _, err = cli(
        "-verbose", flag, "-tpattern", "alt+z", "-prefix", str(out), str(path),
        implementation="original",
    )  # fmt: skip
    assert code == 0 and "implementation: original (larmorx " in err, err
    np.testing.assert_array_equal(lx.load(out).data, original.data)


def test_implementation_names_are_checked(no_replica, monkeypatch, tmp_path):
    _, path = series(tmp_path)
    with pytest.raises(ValueError, match="implementation must be"):
        lx.afni.tshift(path, slice_times="alt+z", implementation="exact")
    monkeypatch.setenv("LARMORX_IMPLEMENTATION", "exact")
    assert cli("-tpattern", "alt+z", str(path))[0] == 2
    # The variable of the old GPL package is not read any more.
    monkeypatch.delenv("LARMORX_IMPLEMENTATION")
    monkeypatch.setenv("LARMORX_GPL_BIN", str(tmp_path / "missing"))
    assert lx.afni.tshift(path, slice_times="alt+z").implementation == REPLICA


def test_the_search_order_of_a_separate_replica(scripts, monkeypatch, tmp_path):
    explicit = program_stub(tmp_path / "env", version="1.0.0")
    in_scripts = program_stub(scripts)
    on_path = program_stub(tmp_path / "path")
    monkeypatch.setenv("PATH", os.pathsep.join([str(tmp_path / "empty"), str(on_path.parent)]))
    monkeypatch.setenv("LARMORX_NC_BIN", str(explicit))
    selected = _replica.select("fsl", "mcflirt", "auto")
    assert selected.program == str(explicit)
    assert selected.implementation().version == "1.0.0"
    monkeypatch.delenv("LARMORX_NC_BIN")
    assert _replica.find("fsl", "mcflirt") == str(in_scripts)
    monkeypatch.setattr(_replica, "search_dirs", list)
    assert _replica.find("fsl", "mcflirt") == str(on_path)
    assert _replica.select("fsl", "mcflirt", "original") is None
    monkeypatch.setenv("LARMORX_NC_BIN", str(tmp_path / "missing"))
    with pytest.raises(ValueError, match="LARMORX_NC_BIN is set to"):
        _replica.select("fsl", "mcflirt", "auto")


def test_the_replica_receives_the_arguments(builtin_stub, tmp_path):
    builtin_stub.stderr = "*+ WARNING: from the replica\n"
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
    assert img.implementation == REPLICA
    np.testing.assert_array_equal(img.data, data)  # the stand-in copied its input
    (run,) = builtin_stub.calls
    assert run["threads"] == "3"
    assert run["defects"] == "fix"  # D13's choice reaches the replica as its variable
    assert run["args"] == [
        "afni", "3dTshift", "-Fourier", "-rlt+", "-ignore", "1", "-TR", "2.0s",
        "-tzero", "0.5", "-tpattern", "@slice_times.1D", "-prefix", "out.nii", str(path),
    ]  # fmt: skip
    # The slice times file and the output are in the call's own directory.
    assert Path(run["cwd"]).is_absolute() and not Path(run["cwd"]).exists()

    # In memory: the image is written for the replica as it is.
    builtin_stub.stderr = ""
    with pytest.warns(UserWarning, match="-no_detrend"):
        img = lx.afni.tshift(
            (data, AFFINE), slice_times="seq-z", tr=2.0, method="fourier", detrend=False
        )
    np.testing.assert_array_equal(img.data, data)
    args = builtin_stub.calls[-1]["args"]
    assert args[2:5] == ["-linear", "-no_detrend", "-Fourier"]  # AFNI's order rule
    assert args[-1].endswith("input.nii")


def test_the_replica_receives_the_upstream_defects_choice(builtin_stub, tmp_path):
    """``upstream_defects`` reaches the replica as ``LARMORX_UPSTREAM_DEFECTS``, and its
    notice lines come back as :class:`larmorx.UpstreamDefectWarning`."""
    builtin_stub.stderr = (
        "*+ WARNING: upstream defect AFNI-01 (3dTshift) reproduced: the negative scale factor "
        "-0.5 (scl_slope) is applied on output only [LARMORX_UPSTREAM_DEFECTS=fix corrects it]\n"
    )
    _, path = series(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-01") as record:
        lx.afni.tshift(path, slice_times="alt+z", upstream_defects="reproduce")
    w = next(r.message for r in record if isinstance(r.message, lx.UpstreamDefectWarning))
    assert (w.entry, w.tool, w.mode) == ("AFNI-01", "3dTshift", "reproduce")
    assert w.detail.endswith("is applied on output only")
    assert builtin_stub.calls[-1]["defects"] == "reproduce"


def test_afni_global_options_and_variables_reach_the_replica(no_replica, monkeypatch, tmp_path):
    """On the command line, AFNI's global options (``-overwrite``, ``-Dname=value``) pass
    through larmorx's dispatch unchanged and in place, wherever they stand, and AFNI's
    environment variables reach the replica, which takes them out and applies them itself."""
    _, path = series(tmp_path)
    out = tmp_path / "out.nii"
    out.write_text("taken")
    monkeypatch.setenv("AFNI_NIFTI_WRITE_TYPE", "2")
    monkeypatch.setenv("AFNI_MESSAGE_PREFIX", "run1")
    args = ["-DAFNI_NIFTI_VIEW=tlrc", "-tpattern", "-overwrite", "alt+z", "-prefix"]
    code, _, err = cli(*args, str(out), str(path))
    assert code == 0, err
    written = lx.load(out)
    assert written.header.version == 2  # AFNI_NIFTI_WRITE_TYPE from the environment
    assert "run1" in err  # AFNI_MESSAGE_PREFIX: AFNI's message prefix


def test_afni_settings_are_explicit_never_the_environment(builtin_stub, monkeypatch, tmp_path):
    """``afni_settings`` reaches the replica; AFNI's variables of the process environment never
    do, whatever their case (CLAUDE.md rule 6): the same call gives the same result whatever
    the shell has set."""
    _, path = series(tmp_path)
    fixed = {"AFNI_PATH_SPACES_OK": "YES", "AFNI_MESSAGE_COLORIZE": "NO"}
    fixed["AFNI_NO_OBLIQUE_WARNING"] = "YES"
    lx.afni.tshift(path, slice_times="alt+z")
    assert builtin_stub.calls[-1]["afni"] == fixed
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
    assert builtin_stub.calls[-1]["afni"] == fixed  # no effect

    settings = {"AFNI_NIFTI_SCALE": "NO", "AFNI_NIFTI_VIEW": "tlrc", "AFNI_DECONFLICT": "YES"}
    lx.afni.tshift(path, slice_times="alt+z", afni_settings=settings)
    assert builtin_stub.calls[-1]["afni"] == {**settings, **fixed}  # these, and only these


def test_the_real_replica_ignores_the_environment(no_replica, monkeypatch, tmp_path):
    """The built-in replica runs in this process but never reads its environment: AFNI's
    variables set in the shell change nothing ``lx.afni.tshift`` returns."""
    _, path = series(tmp_path)
    reference = lx.afni.tshift(path, slice_times="alt+z", method="linear")
    monkeypatch.setenv("AFNI_NIFTI_WRITE_TYPE", "2")
    monkeypatch.setenv("AFNI_NIFTI_VIEW", "tlrc")
    monkeypatch.setenv("OMP_NUM_THREADS", "1")
    img = lx.afni.tshift(path, slice_times="alt+z", method="linear")
    assert img.header.to_bytes() == reference.header.to_bytes()
    np.testing.assert_array_equal(img.data, reference.data)
    two = lx.afni.tshift(
        path, slice_times="alt+z", method="linear", afni_settings={"AFNI_NIFTI_WRITE_TYPE": "2"}
    )
    assert two.header.version == 2 and two.implementation == REPLICA


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


@pytest.mark.parametrize("implementation", ["original", "replica"])
def test_both_implementations_apply_afni_settings(
    no_replica, monkeypatch, tmp_path, implementation
):
    """Both implementations read AFNI's variables from ``afni_settings``, never from the
    environment: NIfTI-2 output, the tlrc view's xform code, and ``AFNI_NIFTI_SCALE=NO`` keeping
    int16 data with their intercept in the header (AFNI-23 corrected); the variables that change
    nothing a call returns are accepted."""
    _, path = series(tmp_path)
    run = lambda *a, **k: lx.afni.tshift(*a, implementation=implementation, **k)  # noqa: E731
    reference = run(path, slice_times="alt+z", method="linear")
    assert reference.header.version == 1
    # AFNI variables in the environment change nothing.
    monkeypatch.setenv("AFNI_NIFTI_WRITE_TYPE", "2")
    monkeypatch.setenv("AFNI_NIFTI_VIEW", "tlrc")
    monkeypatch.setenv("AFNI_NIFTI_SCALE", "NO")
    img = run(
        path,
        slice_times="alt+z",
        method="linear",
        afni_settings={"AFNI_DECONFLICT": "YES", "AFNI_NIFTI_TYPE_WARN": "YES"},
    )
    np.testing.assert_array_equal(img.data, reference.data)
    assert img.header.to_bytes() == reference.header.to_bytes()
    # NIfTI-2: the same values in the wider fields.
    two = run(
        path, slice_times="alt+z", method="linear", afni_settings={"AFNI_NIFTI_WRITE_TYPE": "2"}
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
    codes = run(aligned, slice_times="alt+z", afni_settings={"AFNI_NIFTI_VIEW": "tlrc"})
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
    plain = run(scaled, slice_times="alt+z", method="linear")
    settings = {"AFNI_NIFTI_SCALE": "NO"}
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-23"):
        kept = run(scaled, slice_times="alt+z", method="linear", afni_settings=settings)
    assert (kept.header.scl_slope, kept.header.scl_inter) == (0.25, 100.0)
    np.testing.assert_allclose(kept.data, plain.data, atol=0.125)
    with pytest.warns(lx.UpstreamDefectWarning, match="AFNI-23"):
        afni = run(
            scaled,
            slice_times="alt+z",
            method="linear",
            afni_settings=settings,
            upstream_defects="reproduce",
        )
    np.testing.assert_allclose(afni.data, plain.data - 100.0, atol=0.125)


def test_a_replica_output_with_an_intercept_is_scaled(builtin_stub, tmp_path):
    """The replica writes the input's intercept back when it corrects AFNI-23: the result's
    values are those ``lx.load`` gives, slope and intercept applied."""
    values = np.asfortranarray(np.arange(2 * 2 * 3 * 8, dtype=np.int16).reshape(2, 2, 3, 8))
    values += 1000
    hdr = nib.Nifti1Image(values, AFFINE).header
    hdr.set_data_dtype(np.int16)
    hdr["vox_offset"] = 352
    hdr.set_slope_inter(0.25, 100.0)
    path = tmp_path / "scaled.nii"
    path.write_bytes(hdr.binaryblock + b"\0" * 4 + values.tobytes(order="F"))
    img = lx.afni.tshift(path, slice_times="alt+z")  # the stand-in's output is its input
    np.testing.assert_array_equal(img.data, lx.load(path).data)
    assert img.data[0, 0, 0, 0] == 1000 * 0.25 + 100


def test_a_failing_replica_raises_and_never_falls_back(no_replica, builtin_stub, tmp_path):
    builtin_stub.exit = 1
    builtin_stub.stderr = "** FATAL ERROR: boom\n"
    _, path = series(tmp_path)
    with pytest.raises(lx.ReplicaError, match="boom") as e:
        lx.afni.tshift(path, slice_times="alt+z")
    assert e.value.returncode == 1


def test_the_real_replica_reports_afni_errors(no_replica, tmp_path):
    """The checks that need the image are the implementation's: the replica reports them as
    AFNI does, as a :class:`larmorx.ReplicaError`; the original as a ``ValueError``."""
    _, path = series(tmp_path)
    with pytest.raises(lx.ReplicaError, match="slice") as e:
        lx.afni.tshift(path, slice_times=[0.0, 1.0])
    assert e.value.returncode == 1
    with pytest.raises(ValueError, match="slice times for 3 slices"):
        lx.afni.tshift(path, slice_times=[0.0, 1.0], implementation="original")


def test_the_command_line_runs_the_chosen_implementation(no_replica, monkeypatch, tmp_path):
    code, _, err = cli("-x", "in.nii")
    assert code == 1 and "Unknown option: -x" in err  # the replica's AFNI message
    # -verbose names the implementation first (the input does not exist: both then fail).
    missing = str(tmp_path / "missing.nii")
    verbose = ("-verbose", "-prefix", str(tmp_path / "o.nii"), missing)
    code, _, err = cli(*verbose)
    assert code == 1 and err.startswith("++ implementation: replica (larmorx "), err
    code, _, err = cli(*verbose, implementation="original")
    assert code == 1 and err.startswith("++ implementation: original (larmorx "), err
    monkeypatch.setenv("LARMORX_IMPLEMENTATION", "original")
    code, _, err = cli(*verbose)
    assert err.startswith("++ implementation: original (larmorx "), err
