# SPDX-License-Identifier: Apache-2.0
"""The GPL replica of 3dUnifize (``larmorx-gpl``, run as a separate process) against AFNI
itself, through the AFNI 25.2.09 binary (see validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import afni_replica as ar  # noqa: E402
from larmorx_validation.parity import afni_unifize  # noqa: E402

if ar.afni_binary("3dUnifize") is None:
    pytest.skip("the AFNI 3dUnifize oracle is not available", allow_module_level=True)
if ar.replica_binary() is None:
    pytest.skip("the larmorx-gpl binary is not built (crates-gpl)", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = afni_unifize.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_afni_unifize_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
