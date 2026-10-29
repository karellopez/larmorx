# SPDX-License-Identifier: Apache-2.0
"""FSL's own step (6.0.7.17 → 6.0.7.23) of chosen cases of an FSL suite, with every check's
detail (the records keep only the step's verdict for passing cases).

Usage: python -P step_details.py <suite: fsl-bet | fsl-mcflirt-replica> <case id regex>...
"""

from __future__ import annotations

import re
import sys


def main() -> int:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.harness import CheckList, Outcome

    suite, patterns = sys.argv[1], sys.argv[2:]
    if suite == "fsl-bet":
        from larmorx_validation.parity import fsl_bet as mod

        cases = mod.cases_of("standard")
    else:
        from larmorx_validation.parity import fsl_mcflirt_replica as mod

        cases = mod.cases("standard")
    for case in cases:
        if not any(re.search(p, case.id) for p in patterns):
            continue
        checks = CheckList()
        try:
            mod.run_case(case, checks)
        except Outcome as o:
            print(f"[{case.id}] outcome {o}")
        for c in checks.checks:
            if c.name == fsl_versions.STEP_CHECK:
                print(f"[{case.id}] {c.value}: {c.detail}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
