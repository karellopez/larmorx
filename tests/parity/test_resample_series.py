# SPDX-License-Identifier: Apache-2.0
"""One-shot BOLD resampling against fMRIPrep, nitransforms and SciPy (see validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("scipy")
pytest.importorskip("nitransforms")
pytest.importorskip("fmriprep")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import resample_series  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = resample_series.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_resample_series_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
