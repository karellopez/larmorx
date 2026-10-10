# SPDX-License-Identifier: Apache-2.0
"""ANTs' image programs (ImageMath, ThresholdImage, MultiplyImages, ...) against ANTs 2.6.5's
own binaries (see validation/). Skipped where the oracle is not built."""

import importlib
import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("nibabel")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import ants_programs  # noqa: E402

if ants_programs.ants_bin() is None:
    pytest.skip("the ANTs 2.6.5 oracle is not available", allow_module_level=True)

#: The suites of ANTs image programs; add new ones here.
SUITES = (
    "ants_image_math",
    "ants_threshold_image",
    "ants_multiply_images",
    "ants_smooth_image",
    "ants_resample_image_by_spacing",
    "ants_resample_image",
)
TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")

PARAMS = []
for module in SUITES:
    suite = importlib.import_module(f"larmorx_validation.parity.{module}").suite()
    PARAMS += [
        pytest.param(suite, case, id=f"{suite.name}:{case.id}") for case in suite.cases(TIER)
    ]


@pytest.mark.parametrize(("suite", "case"), PARAMS)
def test_ants_program_parity(suite, case):
    result = parity.run_one(suite, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
