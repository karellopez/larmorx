# SPDX-License-Identifier: Apache-2.0
"""The head-motion correction's random-testing descriptor (``difftest/tools/mri_hmc.py``): the
mcflirt runs whose empty ``.mat`` directory the oracle cache cannot replay run live."""

from __future__ import annotations

import os

from larmorx_validation.difftest.tools import mri_hmc


def test_empty_directory_runs_are_not_served_from_the_cache() -> None:
    # -rmsrel and -rmsabs without -mats make mcflirt create an empty .mat directory (FSL-13); the
    # cache keeps files only, so such runs go live (the catch-up's trial of 2026-11-05)
    assert mri_hmc.makes_empty_directory(["-in", "x", "-rmsrel"])
    assert mri_hmc.makes_empty_directory(["-in", "x", "-rmsabs", "-plots"])
    assert not mri_hmc.makes_empty_directory(["-in", "x", "-rmsrel", "-mats"])
    assert not mri_hmc.makes_empty_directory(["-in", "x", "-plots"])
    saved = os.environ.get("LARMORX_ORACLE_CACHE_MODE")
    os.environ["LARMORX_ORACLE_CACHE_MODE"] = "use"
    try:
        with mri_hmc._cache_off_for_empty_directory(["-rmsrel"]):
            assert os.environ["LARMORX_ORACLE_CACHE_MODE"] == "off"
        assert os.environ["LARMORX_ORACLE_CACHE_MODE"] == "use"
        with mri_hmc._cache_off_for_empty_directory(["-rmsrel", "-mats"]):
            assert os.environ["LARMORX_ORACLE_CACHE_MODE"] == "use"
    finally:
        if saved is None:
            os.environ.pop("LARMORX_ORACLE_CACHE_MODE", None)
        else:
            os.environ["LARMORX_ORACLE_CACHE_MODE"] = saved
