# SPDX-License-Identifier: Apache-2.0
"""tedana's t2smap, the clean-room original (crate larmorx-tedana), against tedana 26.0.3
(``specs/t2smap.md`` §17.3's thresholds) and the replica's default (a black box): the suite
``tedana-t2smap-original`` (``validation/src/larmorx_validation/parity/tedana_t2smap_original.py``)."""

import os

import pytest

parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import tedana_t2smap_original  # noqa: E402

if not tedana_t2smap_original.LAUNCHER.exists():
    pytest.skip("tedana 26.0.3's launcher is not available", allow_module_level=True)
if not tedana_t2smap_original.original_available():
    pytest.skip("the original's binding is not built", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = tedana_t2smap_original.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_tedana_t2smap_original_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
