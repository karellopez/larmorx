# SPDX-License-Identifier: Apache-2.0
"""Head-motion correction against FSL's mcflirt, through the recorded oracle runs (see
validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("nibabel")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import mri_hmc  # noqa: E402

if mri_hmc.oracle_dir() is None:
    pytest.skip("the recorded mcflirt runs are not available", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = mri_hmc.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_mri_hmc_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
