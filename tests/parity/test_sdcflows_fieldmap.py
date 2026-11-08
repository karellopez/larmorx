# SPDX-License-Identifier: Apache-2.0
"""lx.sdcflows against sdcflows in-process and fMRIPrep's own files (see validation/)."""

import os

import pytest

pytest.importorskip("scipy")
pytest.importorskip("sklearn")
pytest.importorskip("nitransforms")
pytest.importorskip("sdcflows")
pytest.importorskip("threadpoolctl")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import sdcflows_fieldmap  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = sdcflows_fieldmap.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_sdcflows_fieldmap_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
