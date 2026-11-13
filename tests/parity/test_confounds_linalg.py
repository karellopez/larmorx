# SPDX-License-Identifier: Apache-2.0
"""The parity suite ``confounds-linalg`` (the confounds' part's check scripts, each run as a
separate process in the library environment its case names; see
validation/src/larmorx_validation/parity/confounds_scripts.py)."""

import os

import pytest

parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import confounds_linalg  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = confounds_linalg.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_confounds_linalg_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
