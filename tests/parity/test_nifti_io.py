"""NIfTI parity with nibabel on the test-data catalog (see validation/).

Runs the `nifti-io` parity suite case by case. Needs larmorx-testdata, larmorx-validation and
nibabel; the tier comes from LARMORX_PARITY_TIER (default: smoke).
"""

import os

import pytest

td = pytest.importorskip("larmorx_testdata")
pytest.importorskip("nibabel")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import nifti_io  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = nifti_io.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_nifti_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
