# SPDX-License-Identifier: Apache-2.0
"""larmorx's own options before the family through the console script (``larmorx.cli``):
``--n-threads``, ``--implementation`` and ``--upstream-defects`` give the same output bytes as
the environment variables they replace, and win over them; the implementation's new name
``clean-room`` and its old spelling ``original``; the tool's messages while it runs. (The
standalone program's own tests: ``crates/larmorx-cli/tests/options.rs``.)"""

from __future__ import annotations

import contextlib
import io
import os
import sys
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _core, _replica
from larmorx.cli import run as cli_run

#: The variables the options replace and that change which program runs: removed for each test.
VARIABLES = (
    "LARMORX_IMPLEMENTATION",
    "LARMORX_UPSTREAM_DEFECTS",
    "OMP_NUM_THREADS",
    "NSLOTS",
    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS",
    "ITK_NUMBER_OF_THREADS_ENV_LIST",
)


@pytest.fixture
def clean_env(monkeypatch, no_replica):
    for name in VARIABLES:
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI")
    return monkeypatch


def image(path: Path, shape: tuple[int, ...]) -> Path:
    """A float32 image with a bright block in its middle (plain arithmetic: the same bytes on
    every platform)."""
    n = int(np.prod(shape))
    i = np.arange(n)
    noise = ((i * 7919) % 97).astype(np.float32)
    data = noise.reshape(shape, order="F")
    inner = tuple(slice(s // 4, s - s // 4) for s in shape[:3])
    data[inner] += 1000.0
    lx.save((np.asfortranarray(data), np.eye(4)), path)
    return path


def negatives(path: Path, shape: tuple[int, ...]) -> Path:
    """Every third voxel negative: ImageMath's ``/`` divides only by positive values (ANTS-08)."""
    i = np.arange(int(np.prod(shape)))
    data = (
        np.where(i % 3 == 0, -2.0, 1.0 + (i * 31) % 7).astype(np.float32).reshape(shape, order="F")
    )
    lx.save((np.asfortranarray(data), np.eye(4)), path)
    return path


def cli(argv: list[str], env: dict[str, str] | None = None) -> tuple[int, str, str]:
    """``larmorx <argv>`` through the console script, with ``env`` set for the call."""
    saved = {k: os.environ.get(k) for k in env or {}}
    os.environ.update(env or {})
    out, err = io.StringIO(), io.StringIO()
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = cli_run(["larmorx", *argv])
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    return code, out.getvalue(), err.getvalue()


def outputs(tmp_path: Path, names: list[str]) -> list[bytes]:
    files = []
    for name in names:
        p = tmp_path / name
        files.append(p.read_bytes())
        p.unlink()
    return files


def same(tmp_path, option: list[str], variable: dict[str, str], argv: list[str], names: list[str]):
    """The run with the option and the run with the variable: the same exit status, messages
    and output bytes."""
    a = cli([*option, *argv])
    fa = outputs(tmp_path, names) if a[0] == 0 else []
    b = cli(argv, variable)
    fb = outputs(tmp_path, names) if b[0] == 0 else []
    assert a == b, (option, variable)
    assert fa == fb, (option, variable)
    return a, fa


def test_ants_options_are_its_variables(clean_env, tmp_path):
    src, den = image(tmp_path / "in.nii", (12, 10, 8)), negatives(tmp_path / "den.nii", (12, 10, 8))
    q = str(tmp_path / "q.nii")
    smooth = ["ants", "SmoothImage", "3", str(src), "1.5", str(tmp_path / "s.nii")]
    for option, variable in (
        (["--n-threads", "3"], {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "3"}),
        (["--n-threads=2"], {"NSLOTS": "2"}),
        (["--n-threads", "0"], {}),
    ):
        (code, _, _), _ = same(tmp_path, option, variable, smooth, ["s.nii"])
        assert code == 0
    divide = ["ants", "ImageMath", "3", q, "/", str(src), str(den)]
    _, fixed = same(tmp_path, ["--upstream-defects", "fix"], {"LARMORX_UPSTREAM_DEFECTS": "fix"}, divide, ["q.nii"])  # fmt: skip
    (_, _, err), reproduced = same(tmp_path, ["--upstream-defects=reproduce"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}, divide, ["q.nii"])  # fmt: skip
    assert fixed != reproduced and "ANTS-08" in err
    # the option wins over its variable, even a wrong one
    for value in ("reproduce", "bogus"):
        code, _, _ = cli(
            ["--upstream-defects", "fix", *divide], {"LARMORX_UPSTREAM_DEFECTS": value}
        )
        assert code == 0 and outputs(tmp_path, ["q.nii"]) == fixed


def test_afni_options_choose_and_reach_both_implementations(clean_env, tmp_path):
    src = image(tmp_path / "in.nii", (16, 16, 12))
    mask = ["afni", "3dAutomask", "-prefix", str(tmp_path / "m.nii"), str(src)]
    results = {}
    for implementation in ("replica", "clean-room", "original"):
        for n in ("1", "3", "0"):
            variable = {"LARMORX_IMPLEMENTATION": implementation}
            if n != "0":
                variable["OMP_NUM_THREADS"] = n
            option = ["--implementation", implementation, "--n-threads", n]
            (code, out, err), files = same(tmp_path, option, variable, mask, ["m.nii"])
            assert code == 0, err
            results.setdefault(implementation, (out, err, files))
            assert results[implementation] == (out, err, files)
    # "original", the old spelling, is clean-room's: silently, on the command line
    assert results["original"] == results["clean-room"]
    # the option wins over its variable
    code, out, err = cli(["--implementation", "clean-room", *mask], {"LARMORX_IMPLEMENTATION": "replica"})  # fmt: skip
    assert code == 0 and (out, err, outputs(tmp_path, ["m.nii"])) == results["clean-room"]


def test_fsl_and_mri_options_are_their_variables(clean_env, tmp_path):
    src = image(tmp_path / "in.nii", (12, 12, 10))
    series = image(tmp_path / "series.nii", (12, 12, 10, 3))
    maths = ["fsl", "fslmaths", str(src), "-s", "1", str(tmp_path / "f")]
    for option, variable in ((["--n-threads", "3"], {"OMP_NUM_THREADS": "3"}), (["--n-threads", "0"], {})):  # fmt: skip
        (code, _, err), _ = same(tmp_path, option, variable, maths, ["f.nii"])
        assert code == 0, err
    hmc = ["mri", "hmc", "-in", str(series), "-out", str(tmp_path / "mc"), "-plots"]
    (code, _, err), _ = same(tmp_path, ["--n-threads", "2", "--upstream-defects", "reproduce"], {"OMP_NUM_THREADS": "2", "LARMORX_UPSTREAM_DEFECTS": "reproduce"}, hmc, ["mc.nii", "mc.par"])  # fmt: skip
    assert code == 0, err
    mcflirt = ["fsl", "mcflirt", "-in", str(series), "-out", str(tmp_path / "mc"), "-plots"]
    same(tmp_path, ["--implementation", "clean-room"], {"LARMORX_IMPLEMENTATION": "clean-room"}, mcflirt, ["mc.nii", "mc.par"])  # fmt: skip


def test_wrong_options_are_usage_errors(clean_env):
    for argv, message in (
        (["--n-threads", "-1", "ants"], "--n-threads must be a whole number of threads"),
        (
            ["--implementation", "fast", "ants"],
            "--implementation must be auto, replica or clean-room, not 'fast'",
        ),
        (["--upstream-defects"], "--upstream-defects needs a value: fix or reproduce"),
        (["ants", "--n-threads", "2", "SmoothImage"], "--n-threads after the family"),
    ):
        code, out, err = cli(argv)
        assert code == 2 and out == "" and err.startswith("error: "), err
        assert message in err, err


def test_sdcflows_gets_the_options_in_its_environment(clean_env, monkeypatch):
    from larmorx.sdcflows import app

    calls: list[tuple[list[str], dict[str, str]]] = []

    def run_app(args, *, upstream_defects=None, env=None):
        calls.append((list(args), dict(env or {})))
        return 0

    monkeypatch.setattr(app, "run_app", run_app)
    monkeypatch.setenv("OMP_NUM_THREADS", "7")
    argv = ["sdcflows", "sdcflows", "bids", "out", "participant", "--omp-nthreads", "2"]
    assert cli(["--n-threads", "3", "--implementation", "clean-room", "--upstream-defects", "reproduce", *argv])[0] == 0  # fmt: skip
    args, env = calls.pop()
    assert args == argv[2:]  # the app's own --omp-nthreads stays its own, and wins there
    assert (env["OMP_NUM_THREADS"], env["LARMORX_IMPLEMENTATION"], env["LARMORX_UPSTREAM_DEFECTS"]) == ("3", "clean-room", "reproduce")  # fmt: skip
    assert cli(["--n-threads", "0", *argv])[0] == 0
    args, env = calls.pop()
    assert "OMP_NUM_THREADS" not in env  # every logical CPU
    assert cli(argv)[0] == 0
    assert calls.pop()[1]["OMP_NUM_THREADS"] == "7"
    # a wrong option is refused before the app runs
    assert cli(["--n-threads", "x", *argv])[0] == 2 and not calls


def test_python_clean_room_and_its_old_spelling(clean_env, tmp_path):
    src = image(tmp_path / "in.nii", (16, 16, 12))
    with warnings.catch_warnings():
        warnings.simplefilter("error", DeprecationWarning)
        new = lx.afni.automask(src, implementation="clean-room")
    with pytest.warns(DeprecationWarning, match="old spelling") as w:
        old = lx.afni.automask(src, implementation="original")
    deprecated = [x for x in w if "old spelling" in str(x.message)]
    assert deprecated[0].filename == __file__  # the caller's line
    assert new.mask.implementation == old.mask.implementation == _replica.ORIGINAL
    assert _replica.ORIGINAL.kind == "clean-room"
    np.testing.assert_array_equal(new.mask.data, old.mask.data)
    assert _replica.IMPLEMENTATIONS == ("auto", "replica", "clean-room")
    with pytest.raises(ValueError, match="'auto', 'replica' or 'clean-room'"):
        lx.afni.automask(src, implementation="clean_room")
    with pytest.warns(DeprecationWarning):
        assert cli_run(["larmorx", "--version"], implementation="original") == 0


def test_the_console_script_writes_as_the_tool_runs(clean_env, tmp_path, capsys):
    """The streamed output is the collected output, stream by stream."""
    src = image(tmp_path / "in.nii", (12, 10, 8))
    argv = ["larmorx", "ants", "ImageMath", "3", str(tmp_path / "t.nii"), "total", str(src)]
    code, out, err = _core.cli_main(argv, [], "clean-room")
    assert code == 0 and out
    assert cli_run(argv, implementation="clean-room") == 0
    streamed = capsys.readouterr()
    assert (streamed.out, streamed.err) == (out, err)


@pytest.mark.skipif(sys.platform == "win32", reason="a POSIX shell stand-in")
def test_a_replica_programs_output_is_passed_on_in_order(monkeypatch, tmp_path):
    """A replica program's standard output and error reach ``sys.stdout`` and ``sys.stderr`` in
    the order it writes them, with the options in its environment."""
    program = tmp_path / "larmorx-nc"
    program.write_text(
        "#!/bin/sh\n"
        'echo "o1 threads=${OMP_NUM_THREADS-unset} defects=${LARMORX_UPSTREAM_DEFECTS-unset}"\n'
        "sleep 0.2\necho e1 >&2\nsleep 0.2\necho o2\nsleep 0.2\nprintf 'e2' >&2\nexit 4\n",
        encoding="utf-8",
    )
    program.chmod(0o755)
    monkeypatch.setenv("LARMORX_NC_BIN", str(program))
    monkeypatch.setenv("OMP_NUM_THREADS", "9")
    order: list[tuple[str, str]] = []

    class Recorder(io.StringIO):
        def __init__(self, name: str):
            super().__init__()
            self.name = name

        def write(self, s: str) -> int:
            order.append((self.name, s))
            return len(s)

    monkeypatch.setattr(sys, "stdout", Recorder("out"))
    monkeypatch.setattr(sys, "stderr", Recorder("err"))
    code = cli_run(["larmorx", "--n-threads", "0", "--upstream-defects", "reproduce", "fsl", "bet", "a", "b"])  # fmt: skip
    monkeypatch.undo()
    assert code == 4
    assert order == [
        ("out", "o1 threads=unset defects=reproduce\n"),
        ("err", "e1\n"),
        ("out", "o2\n"),
        ("err", "e2"),
    ], order


def test_fsl_tools_write_nifti_gz_when_fsloutputtype_is_unset(clean_env, tmp_path):
    """FSL-11 through the console script: with ``FSLOUTPUTTYPE`` unset a clean-room FSL tool
    writes ``NIFTI_GZ`` (the bytes it writes with the variable set) and names FSL-11 on one
    line; ``--upstream-defects reproduce`` gives FSL's message and exit 1, no image. (Every FSL
    tool: ``crates/larmorx-cli/tests/fsl_output_types.rs``.)"""
    src = image(tmp_path / "in.nii", (12, 12, 10))
    series = image(tmp_path / "series.nii", (12, 12, 10, 3))
    calls = {
        "fslmaths": ["fsl", "fslmaths", str(src), "-add", "1", str(tmp_path / "m")],
        "mcflirt": ["mri", "hmc", "-in", str(series), "-out", str(tmp_path / "mc")],
    }
    first = {"fslmaths": tmp_path / "m.nii.gz", "mcflirt": tmp_path / "mc.nii.gz"}
    for tool, argv in calls.items():
        argv = ["--implementation", "clean-room", *argv]
        clean_env.setenv("FSLOUTPUTTYPE", "NIFTI_GZ")
        code, out, err = cli(argv)
        assert code == 0, err
        given = first[tool].read_bytes()
        first[tool].unlink()
        clean_env.delenv("FSLOUTPUTTYPE")
        code_u, out_u, err_u = cli(argv)
        assert (code_u, out_u) == (0, out), err_u
        named = [line for line in err_u.splitlines() if "FSL-11" in line]
        assert named == [
            "larmorx: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the "
            "images are written as NIFTI_GZ (.nii.gz); FSL's "
            f"{tool} stops with an error at its first image (exit 1) "
            "[LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]"
        ], err_u
        assert [line for line in err_u.splitlines() if "FSL-11" not in line] == err.splitlines()
        assert first[tool].read_bytes() == given
        first[tool].unlink()
        code_r, _, err_r = cli(["--upstream-defects", "reproduce", *argv])
        assert code_r == 1 and "ERROR:: Environment variable FSLOUTPUTTYPE is not set!" in err_r
        assert "FSL-11" not in err_r and not first[tool].exists()
