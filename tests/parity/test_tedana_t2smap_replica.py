# SPDX-License-Identifier: Apache-2.0
"""The t2smap replica (larmorx-gpl) against tedana's t2smap (see validation/)."""

import os

import pytest

pytest.importorskip("nibabel")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import tedana_t2smap_replica  # noqa: E402

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = tedana_t2smap_replica.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_tedana_t2smap_replica_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
