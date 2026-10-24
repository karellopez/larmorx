# SPDX-License-Identifier: Apache-2.0
"""The FSL replica of mcflirt (``larmorx-nc``, run as a separate process) against mcflirt
itself: the recorded oracle runs and runs of FSL 6.0.7.17's binary (see validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("nibabel")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import fsl_mcflirt_replica  # noqa: E402

if fsl_mcflirt_replica.oracle_dir() is None:
    pytest.skip("the recorded mcflirt runs are not available", allow_module_level=True)
if fsl_mcflirt_replica.replica_binary() is None:
    pytest.skip("the larmorx-nc binary is not built (crates-nc)", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = fsl_mcflirt_replica.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_fsl_mcflirt_replica_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
