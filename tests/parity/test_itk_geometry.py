# SPDX-License-Identifier: Apache-2.0
"""NIfTI geometry as ITK reads it, against ANTsPy (see validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("ants")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import itk_geometry  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = itk_geometry.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_itk_geometry_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
