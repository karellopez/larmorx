# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's clean-room original against AFNI 26.3.05 (``specs/3dvolreg.md`` §14.3's
thresholds) and the replica's default (a black box): the suite ``afni-volreg-original``
(``validation/src/larmorx_validation/parity/afni_volreg_clean_room.py``)."""

import os

import pytest

parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import afni_volreg_clean_room  # noqa: E402

if afni_volreg_clean_room.afni_binary() is None:
    pytest.skip("the AFNI 26.3.05 3dvolreg oracle is not available", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = afni_volreg_clean_room.suite()
CASES = SUITE.cases(TIER)


@pytest.mark.parametrize("case", CASES, ids=[c.id for c in CASES])
def test_afni_volreg_clean_room_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
