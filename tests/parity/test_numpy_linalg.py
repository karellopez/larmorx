# SPDX-License-Identifier: Apache-2.0
"""lx.transforms' 4×4 products and inverses against numpy's OpenBLAS (see validation/)."""

import os

import pytest

pytest.importorskip("threadpoolctl")
pytest.importorskip("nitransforms")
pytest.importorskip("sdcflows")
pytest.importorskip("fmriprep")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import numpy_linalg  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = numpy_linalg.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_numpy_linalg_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
