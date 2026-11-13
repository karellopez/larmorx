# SPDX-License-Identifier: Apache-2.0
"""``larmorx nipype run`` (:mod:`larmorx.nipype`): nipype's ``nipypecli run`` for the interfaces
larmorx has ported, without nipype. The messages, listings, helps, printed inputs and outputs and
exit statuses are those of nipype's command line (checked against it, case by case, by
``validation/investigations/confounds/check_nipype_cli.py``); here, without nipype: the click
layer, a run through the console script, traits' and nipype's errors, the defects of nipype's
command line corrected by default (PIPE-81, PIPE-82) and reproduced on request."""

from __future__ import annotations

import os
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _defects
from larmorx.cli import run as cli_run
from larmorx.nipype import list_interfaces, ported_interfaces, run

NAC = "nipype.algorithms.confounds"


@pytest.fixture(autouse=True)
def nipype_env(monkeypatch, tmp_path):
    for name in ("LARMORX_UPSTREAM_DEFECTS", "OMP_NUM_THREADS", "LARMORX_IMPLEMENTATION"):
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setenv("COLUMNS", "80")
    monkeypatch.chdir(tmp_path)
    return tmp_path


def series(path: Path) -> str:
    """A small 4-D float32 series (plain arithmetic) with a repetition time of 2 s."""
    shape = (6, 5, 4, 8)
    i = np.arange(int(np.prod(shape)))
    data = (500.0 + (i * 7919) % 97 + (i // 120) * 3).astype(np.float32).reshape(shape, order="F")
    lx.save((np.asfortranarray(data), np.diag([2.0, 2.0, 2.5, 1.0])), path)
    return str(path)


def mask(path: Path) -> str:
    data = np.zeros((6, 5, 4), np.uint8)
    data[1:5, 1:4, 1:3] = 1
    lx.save((np.asfortranarray(data), np.diag([2.0, 2.0, 2.5, 1.0])), path)
    return str(path)


def test_the_click_layer(capsys):
    assert run([]) == 2
    out, err = capsys.readouterr()
    assert out == ""
    assert err == (
        "Usage: larmorx nipype run [OPTIONS] [MODULE] [INTERFACE]\n\n"
        "Error: Invalid value for '[MODULE]': got None.\n"
    )
    # click's parser's own errors come without the usage line
    assert run(["--list=x", NAC]) == 2
    assert capsys.readouterr().err == "Error: Option '--list' does not take a value.\n"
    # -h and --help are a flag the command never reads
    assert run(["-h"]) == 2
    assert run([NAC, "--list"]) == 0
    out = capsys.readouterr().out
    assert out.splitlines()[:3] == ["Available Interfaces:", "    ACompCor", "    BaseInterface"]
    assert run([NAC]) == 0
    assert capsys.readouterr().out == out
    assert run([NAC, "Nope"]) == 1
    assert capsys.readouterr().err == (
        "Traceback (most recent call last):\n"
        f"AttributeError: module '{NAC}' has no attribute 'Nope'\n"
    )
    assert run(["nipype.algorithms.misc", "--list"]) == 1
    assert (
        "larmorx does not run the interfaces of 'nipype.algorithms.misc'" in capsys.readouterr().err
    )
    assert run(["niworkflows.interfaces.utility", "KeySelect"]) == 1
    assert capsys.readouterr().err.endswith(
        "ValueError: A list or multiplexed fields must be provided at instantiation time.\n"
    )


def test_listing_and_ported_interfaces():
    assert list_interfaces(NAC) == [
        "ACompCor", "BaseInterface", "CompCor", "ComputeDVARS", "FramewiseDisplacement",
        "NonSteadyStateDetector", "SimpleInterface", "TCompCor", "TSNR",
    ]  # fmt: skip
    ported = ported_interfaces()
    assert set(ported) == set(lx.nipype.MODULES)
    assert "ComputeDVARS" in ported[NAC] and "BaseInterface" not in ported[NAC]
    assert ported["fmriprep.interfaces.confounds"] == [
        "FSLMotionParams", "FSLRMSDeviation", "FilterDropped", "FramewiseDisplacement",
        "GatherConfounds", "RenameACompCor", "aCompCorMasks",
    ]  # fmt: skip
    with pytest.raises(ValueError, match="does not run the interfaces of 'x'"):
        list_interfaces("x")


def test_help(capsys):
    assert run([NAC, "ComputeDVARS"]) == 0
    out = capsys.readouterr().out
    assert out.startswith("usage: larmorx nipype run nipype.algorithms.confounds ComputeDVARS")
    assert "\nRun ComputeDVARS\n" in out and "  in_file " in out
    assert "--variance_tol VARIANCE_TOL" in out  # argparse's layout: Python's own, as nipype's
    assert "nipype default value: 1e-07" in out
    # the help, then the usage again (nipype's print_help, then print_usage)
    assert out.count("usage: larmorx nipype run") == 2


def test_a_run_through_the_console_script(capsys, nipype_env):
    bold, msk = series(nipype_env / "bold.nii.gz"), mask(nipype_env / "mask.nii.gz")
    code = cli_run(["larmorx", "nipype", "run", NAC, "ComputeDVARS", bold, msk, "--save_all"])
    assert code == 0
    out = capsys.readouterr().out
    lines = out.splitlines()
    assert lines[:3] == ["setting function inputs", "", "figdpi = 100"]
    assert f"in_file = {bold}" in lines
    assert "remove_zerovariance = True" in lines and "series_tr = <undefined>" in lines
    (nipype_env / "py").mkdir()
    want = lx.confounds.compute_dvars_node(bold, msk, save_all=True, out_dir=nipype_env / "py")
    assert f"avg_std = {want.avg_std}" in lines
    assert f"out_all = {nipype_env / 'bold_dvars.tsv'}" in lines
    assert "out_nstd = <undefined>" in lines
    for name in ("bold_dvars.tsv", "bold_dvars_std.tsv"):
        assert (nipype_env / name).read_bytes() == (nipype_env / "py" / name).read_bytes()


def test_traits_and_nipype_errors(capsys, nipype_env):
    bold, msk = series(nipype_env / "bold.nii.gz"), mask(nipype_env / "mask.nii.gz")
    assert run([NAC, "ComputeDVARS", "nope.nii", msk]) == 1
    out, err = capsys.readouterr()
    assert out == "setting function inputs\n"
    assert err == (
        "Traceback (most recent call last):\ntraits.trait_errors.TraitError: The 'in_file' "
        "trait of a ComputeDVARSInputSpec instance must be a pathlike object or string "
        "representing an existing file, but a value of 'nope.nii' <class 'str'> was specified.\n"
    )
    assert run([NAC, "ACompCor", bold, "--mask_files", msk, "--mask_index", "0",
                "--merge_method", "union"]) == 1  # fmt: skip
    assert capsys.readouterr().err.endswith(
        'OSError: Input "merge_method" is mutually exclusive with input "mask_index", which is '
        "already set\n"
    )
    assert run([NAC, "ACompCor", bold, "--mask_index", "0", "--repetition_time", "2"]) == 1
    assert capsys.readouterr().err.endswith(
        "ValueError: ACompCor requires a value for input 'mask_files' because 'mask_index' is "
        "set. For a list of required inputs, see ACompCor.help()\n"
    )
    assert run([NAC, "ACompCor", bold, "--mask_files", msk, "--use_regress_poly"]) == 1
    assert "Input use_regress_poly in interface CompCor is deprecated." in capsys.readouterr().err
    assert run([NAC, "ComputeDVARS", bold]) == 2
    assert "error: the following arguments are required: in_mask" in capsys.readouterr().err
    assert run([NAC, "ACompCor", bold, "--failure_mode", "x"]) == 1
    assert "must be 'error' or 'NaN', but a value of 'x' <class 'str'>" in capsys.readouterr().err
    assert run(["niworkflows.interfaces.images", "Conform", bold]) == 1
    assert "larmorx has not ported niworkflows.interfaces.images.Conform" in capsys.readouterr().err


def test_pipe81_compound_inputs(capsys, nipype_env):
    bold, msk = series(nipype_env / "bold.nii.gz"), mask(nipype_env / "mask.nii.gz")
    args = [NAC, "ACompCor", bold, "--mask_files", msk, "--num_components", "2"]
    with pytest.warns(_defects.UpstreamDefectWarning, match="PIPE-81"):
        assert run(args) == 0
    out = capsys.readouterr().out
    assert "num_components = 2\n" in out
    assert (nipype_env / "components_file.txt").read_text().splitlines()[0] == (
        "aCompCor00\taCompCor01"
    )
    # nipype's command line reads the number as text, which the trait refuses
    assert run(args, upstream_defects="reproduce") == 1
    assert capsys.readouterr().err.endswith(
        "must be an integer >= 1 or 'all', but a value of '2' <class 'str'> was specified.\n"
    )
    # niworkflows' TSV2JSON: nipype's command line cannot build its parser
    assert run(["niworkflows.interfaces.utility", "TSV2JSON"], upstream_defects="reproduce") == 1
    assert capsys.readouterr().err == (
        "Traceback (most recent call last):\nUnboundLocalError: cannot access local variable "
        "'trait_type' where it is not associated with a value\n"
    )
    with pytest.warns(_defects.UpstreamDefectWarning, match="PIPE-81"):
        assert run(["niworkflows.interfaces.utility", "TSV2JSON"]) == 0
    assert "Run TSV2JSON" in capsys.readouterr().out


def test_pipe82_texts_read_by_their_traits(capsys, nipype_env):
    bold = series(nipype_env / "bold.nii.gz")
    vfs = [mask(nipype_env / f"{n}.nii.gz") for n in ("gm", "wm", "csf")]
    args = ["fmriprep.interfaces.confounds", "aCompCorMasks", "2,2,2.5", "--in_vfs", *vfs]
    with pytest.warns(_defects.UpstreamDefectWarning, match="PIPE-82"):
        assert run(args) == 0
    out = capsys.readouterr().out
    assert "bold_zooms = (2.0, 2.0, 2.5)" in out
    assert run(args, upstream_defects="reproduce") == 1
    assert "a value of '2,2,2.5' <class 'str'> was specified." in capsys.readouterr().err
    # CompCor's pre_filter False, and a float radius whose default is an integer
    with pytest.warns(_defects.UpstreamDefectWarning, match="PIPE-82"):
        assert run([NAC, "TCompCor", bold, "--pre_filter", "False", "--num_components", "all"]) == 0
    assert "pre_filter = False" in capsys.readouterr().out
    par = nipype_env / "mc.par"
    np.savetxt(par, (np.arange(48).reshape(8, 6) % 5) / 1000, fmt="%.6f")
    fd = [NAC, "FramewiseDisplacement", str(par), "FSL", "--radius", "40.5"]
    with pytest.warns(_defects.UpstreamDefectWarning, match="PIPE-82"):
        assert run(fd) == 0
    assert "radius = 40.5" in capsys.readouterr().out
    assert run(fd, upstream_defects="reproduce") == 2
    assert "error: argument --radius: invalid int value: '40.5'" in capsys.readouterr().err


def test_the_options_before_the_family(capsys, nipype_env, monkeypatch):
    bold, msk = series(nipype_env / "bold.nii.gz"), mask(nipype_env / "mask.nii.gz")
    args = ["nipype", "run", NAC, "ACompCor", bold, "--mask_files", msk, "--num_components", "2"]
    assert cli_run(["larmorx", "--upstream-defects", "reproduce", "--n-threads", "1", *args]) == 1
    assert "TraitError" in capsys.readouterr().err
    monkeypatch.setenv("LARMORX_UPSTREAM_DEFECTS", "reproduce")
    assert cli_run(["lx", *args]) == 1
    assert "TraitError" in capsys.readouterr().err
    with pytest.warns(_defects.UpstreamDefectWarning):
        assert cli_run(["lx", "--upstream-defects", "fix", *args]) == 0
    assert capsys.readouterr().out.startswith("setting function inputs")
    assert run([NAC], env={"LARMORX_UPSTREAM_DEFECTS": "wrong"}) == 2
    assert "must be 'fix' or 'reproduce'" in capsys.readouterr().err
    assert os.environ["LARMORX_UPSTREAM_DEFECTS"] == "reproduce"  # never changed
