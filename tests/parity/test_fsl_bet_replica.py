# SPDX-License-Identifier: Apache-2.0
"""The BET replicas (``larmorx-nc fsl bet|bet2|betsurf``, run as a separate process) against
FSL itself (6.0.7.23, ``~/fsl-6.0.7.23``, or ``--fsl-version``; see validation/)."""

import os

import pytest

parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import fsl_bet  # noqa: E402

if fsl_bet.fsl_dir() is None:
    pytest.skip("FSL is not installed (~/fsl-6.0.7.23, LARMORX_FSLDIR)", allow_module_level=True)
if fsl_bet.replica_binary() is None:
    pytest.skip("the larmorx-nc program is not built (crates-nc)", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = fsl_bet.suite("replica")
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_fsl_bet_replica_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
