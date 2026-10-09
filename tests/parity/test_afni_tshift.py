"""3dTshift against AFNI itself, through the AFNI 25.2.09 binary (see validation/)."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import afni_tshift  # noqa: E402

if afni_tshift.afni_binary() is None:
    pytest.skip("the AFNI 3dTshift oracle is not available", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = afni_tshift.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_afni_tshift_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
