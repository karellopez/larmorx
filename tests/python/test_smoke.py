"""Smoke tests of the installed package: import, version and command lines."""

import importlib.metadata
import shutil
import subprocess
import sys
import sysconfig

import pytest
from packaging.requirements import Requirement

import larmorx
from larmorx import _core
from larmorx.cli import run
from larmorx.pipelines.larmorprepx import cli as prep_cli

VERSION = importlib.metadata.version("larmorx")


def script(name: str) -> str:
    """Path of an installed console script (with ``.exe`` on Windows)."""
    path = shutil.which(name, path=sysconfig.get_path("scripts"))
    assert path is not None, f"console script {name!r} is not installed"
    return path


def run_process(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True, encoding="utf-8", check=False)


def test_version_matches_distribution_metadata():
    assert larmorx.__version__ == VERSION
    assert _core.__version__ == VERSION


@pytest.mark.parametrize("name", ["larmorx", "lx"])
def test_cli_prints_version(name):
    result = run_process(script(name), "--version")
    assert result.returncode == 0, result.stderr
    assert result.stdout == f"larmorx {VERSION}\n"


def test_lx_help_uses_its_own_name():
    result = run_process(script("lx"), "--help")
    assert result.returncode == 0, result.stderr
    assert "Usage: lx <family> <tool>" in result.stdout


def test_cli_unknown_command_is_a_usage_error():
    result = run_process(script("larmorx"), "no-such-family")
    assert result.returncode == 2
    assert result.stdout == ""
    assert "unknown command 'no-such-family'" in result.stderr


def test_python_m_larmorx():
    result = run_process(sys.executable, "-m", "larmorx", "--version")
    assert result.returncode == 0, result.stderr
    assert result.stdout == f"larmorx {VERSION}\n"


def test_cli_run_writes_through_sys_streams(capsys):
    assert run(["larmorx", "--version"]) == 0
    assert capsys.readouterr().out == f"larmorx {VERSION}\n"


def test_larmorprepx_prints_version():
    result = run_process(script("larmorprepx"), "--version")
    assert result.returncode == 0, result.stderr
    assert result.stdout == f"larmorprepx {VERSION}\n"


def test_larmorprepx_explains_missing_prep_extra(monkeypatch, capsys):
    monkeypatch.setattr(prep_cli, "PREP_MODULES", ("larmorx_no_such_module",))
    assert prep_cli.main([]) == 1
    assert 'pip install "larmorx[prep]"' in capsys.readouterr().err


def test_prep_extra_matches_the_checked_modules():
    requires = [Requirement(req) for req in importlib.metadata.requires("larmorx") or []]
    prep = {req.name for req in requires if req.marker and req.marker.evaluate({"extra": "prep"})}
    assert prep == set(prep_cli.PREP_MODULES)
