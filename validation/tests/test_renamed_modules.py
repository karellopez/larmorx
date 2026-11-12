# SPDX-License-Identifier: Apache-2.0
"""The modules the documentation revision renamed (``*_original`` to ``*_clean_room``, 2026-11-12)
still import under their old names, as the investigations that use them do; the suites, the
evidence pages and random testing name the new modules."""

from __future__ import annotations

import importlib
import subprocess
import sys
from pathlib import Path

import pytest

#: The parity suites look for the workspace (its ``oracles/`` folder) when they are imported:
#: outside one, their entries are skipped.
WORKSPACE = any((p / "oracles").is_dir() for p in Path(__file__).resolve().parents)

PACKAGES = (
    "larmorx_validation.parity",
    "larmorx_validation.evidence",
    "larmorx_validation.difftest.tools",
)


def _renamed():
    for name in PACKAGES:
        pkg = importlib.import_module(name)
        for old, new in pkg._RENAMED.items():
            skip = pytest.mark.skipif(
                name.endswith(".parity") and not WORKSPACE, reason="no workspace (oracles/)"
            )
            yield pytest.param(name, old, new, marks=skip)


@pytest.mark.parametrize(("package", "old", "new"), list(_renamed()))
def test_old_name_imports_the_renamed_module(package, old, new):
    """Every import form of an old name gives the renamed module, and ``python -m`` its code."""
    assert old.endswith("_original") and new == old.removesuffix("_original") + "_clean_room"
    module = importlib.import_module(f"{package}.{new}")
    namespace: dict = {}
    exec(f"from {package} import {old} as m", namespace)
    assert namespace["m"] is module
    assert importlib.import_module(f"{package}.{old}") is module
    exec(f"import {package}.{old} as m2", namespace)
    assert namespace["m2"] is module
    assert module.__spec__.name == f"{package}.{new}"  # it keeps its own spec
    names = [n for n in dir(module) if not n.startswith("_")][:1]
    if names:
        exec(f"from {package}.{old} import {names[0]} as m3", namespace)
        assert namespace["m3"] is getattr(module, names[0])
    # python -m <old name>: the renamed module's code
    from larmorx_validation import renamed

    code = renamed._FINDER.get_code(f"{package}.{old}")
    assert Path(code.co_filename).resolve() == Path(module.__file__).resolve()


def test_python_m_runs_an_old_name():
    """``python -m`` on an old name runs the renamed module (a random-testing descriptor: it has
    no main block, so running it only defines it)."""
    old = "larmorx_validation.difftest.tools.fsl_topup_original"
    r = subprocess.run([sys.executable, "-m", old], capture_output=True, text=True, check=False)
    assert r.returncode == 0, r.stderr


def test_registries_name_the_new_modules():
    from larmorx_validation import __main__ as cli
    from larmorx_validation import evidence
    from larmorx_validation.difftest import tools

    names = list(cli.PARITY_MODULES.values()) + list(evidence.TOOLS.values())
    names += [module for module, _ in tools.TOOLS.values()]
    assert not [n for n in names if n.endswith("_original")]
    # a suite's name is its record's: the clean rooms' suites keep theirs until each is made again
    assert cli.PARITY_MODULES["afni-volreg-original"] == "afni_volreg_clean_room"
    assert cli.PARITY_MODULES["tedana-t2smap-original"] == "tedana_t2smap_clean_room"


def test_unknown_names_still_fail():
    from larmorx_validation import evidence

    with pytest.raises(AttributeError):
        evidence.no_such_module_original  # noqa: B018
    with pytest.raises(ImportError):
        exec("from larmorx_validation.parity import no_such_original", {})
