# SPDX-License-Identifier: Apache-2.0
"""The modules the documentation revision renamed (``*_original`` to ``*_clean_room``, 2026-11-12)
still import under their old names, as the investigations that use them do; the suites, the
evidence pages and random testing name the new modules."""

from __future__ import annotations

import importlib

import pytest

PACKAGES = (
    "larmorx_validation.parity",
    "larmorx_validation.evidence",
    "larmorx_validation.difftest.tools",
)


def _renamed():
    for name in PACKAGES:
        pkg = importlib.import_module(name)
        for old, new in pkg._RENAMED.items():
            yield name, old, new


@pytest.mark.parametrize(("package", "old", "new"), list(_renamed()))
def test_old_name_imports_the_renamed_module(package, old, new):
    assert old.endswith("_original") and new == old.removesuffix("_original") + "_clean_room"
    module = importlib.import_module(f"{package}.{new}")
    namespace: dict = {}
    exec(f"from {package} import {old} as m", namespace)
    assert namespace["m"] is module


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
