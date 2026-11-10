# SPDX-License-Identifier: Apache-2.0
"""The parity suite ``niworkflows-helpers`` (the field maps' part B's check scripts, each run as a separate
process; see validation/src/larmorx_validation/parity/sdcflows_scripts.py)."""

import os

import pytest

pytest.importorskip("scipy")
pytest.importorskip("skimage")
pytest.importorskip("nipype")
pytest.importorskip("niworkflows")
pytest.importorskip("sdcflows")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import niworkflows_helpers  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = niworkflows_helpers.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_niworkflows_helpers_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
