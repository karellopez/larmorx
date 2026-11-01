# SPDX-License-Identifier: Apache-2.0
"""The FLIRT replica (``larmorx-nc fsl flirt|convert_xfm``, run as a separate process) against
FSL 6.0.7.23 itself (``~/fsl-6.0.7.23``; see validation/)."""

import os

import pytest

parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import fsl_flirt_replica, fsl_versions  # noqa: E402

if not fsl_versions.installed(fsl_versions.NEWEST):
    pytest.skip(f"FSL {fsl_versions.NEWEST} is not installed", allow_module_level=True)
if fsl_flirt_replica.replica_binary() is None:
    pytest.skip("the larmorx-nc program is not built (crates-nc)", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = fsl_flirt_replica.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_fsl_flirt_replica_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
