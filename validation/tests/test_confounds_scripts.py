# SPDX-License-Identifier: Apache-2.0
"""The confounds suites' parser of the check scripts' summary lines
(``larmorx_validation.parity.confounds_scripts.parse``)."""

from __future__ import annotations

from larmorx_validation.parity.confounds_scripts import UNDEFINED_REPORTED, parse

OUT = """\
np.linalg.svd svd path 3: 1360 / 1360 identical; 234 cases crashed in numpy's OpenBLAS
np.linalg.svd svd path 3 [undefined in the original]: 83 / 92 identical; 234 cases crashed in numpy's OpenBLAS
np.linalg.svd svd path 5: 1071 / 1072 identical; 234 cases crashed in numpy's OpenBLAS
  crashes: 234, of which 233 in the undefined class
ALL IDENTICAL
"""


def test_the_undefined_class_is_reported_and_the_defined_cases_are_checks() -> None:
    """numpy's results in its undefined class (NP-09) follow memory it never wrote: reported,
    as the scripts report them; a defined case that differs, and a crash outside the class, fail."""
    items = {i["name"]: i for i in parse(OUT).items}
    undefined = items["np.linalg.svd svd path 3 [undefined in the original]"]
    assert undefined["passed"] and undefined["value"] == "83/92"
    assert undefined["threshold"] == UNDEFINED_REPORTED
    assert items["np.linalg.svd svd path 3"]["passed"]
    assert items["np.linalg.svd svd path 3"]["threshold"] is None
    assert not items["np.linalg.svd svd path 5"]["passed"]
    assert not items["crashes of numpy's OpenBLAS in the undefined class (NP-09)"]["passed"]
