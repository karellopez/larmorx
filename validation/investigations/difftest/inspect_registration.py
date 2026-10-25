# SPDX-License-Identifier: Apache-2.0
"""An antsRegistration finding of the differential tests by hand: its shrunk inputs rebuilt, ANTs
run (as built, or with CORE-MATH preloaded with ``--crmath``) and larmorx in ``reproduce`` mode,
both with ``ANTS_RANDOM_SEED=42`` at one thread; their standard error and the first differing
line of standard output printed.

    python -P validation/investigations/difftest/inspect_registration.py <finding.json> <dir> [--crmath]
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

from larmorx_validation.difftest import tools
from larmorx_validation.parity import ants_registration as reg


def main() -> int:
    finding = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(tempfile.mkdtemp())
    crmath = "--crmath" in sys.argv
    work.mkdir(parents=True, exist_ok=True)
    case = tools.get("ants-registration").case(finding.get("shrunk") or finding["recipe"], "port")
    c = case.payload
    a_dir, l_dir = work / "ants", work / "larmorx"
    a_dir.mkdir(exist_ok=True)
    l_dir.mkdir(exist_ok=True)
    a = reg._run_ants(
        reg._resolve(c, a_dir, work), c.seed, correctly_rounded=crmath, never="inspection"
    )
    lx = reg._run_larmorx(reg._resolve(c, l_dir, work), c.seed)
    print("args:", " ".join(c.args))
    print(f"ANTs exit {a[0]}; larmorx exit {lx[0]}")
    print("ANTs stderr:\n  " + "\n  ".join(a[2].splitlines()[-12:]))
    print("larmorx stderr:\n  " + "\n  ".join(lx[2].splitlines()[-12:]))
    oa, ol = a[1].splitlines(), lx[1].splitlines()
    for i, (x, y) in enumerate(zip(oa, ol, strict=False)):
        if x.replace(str(a_dir), "{out}") != y.replace(str(l_dir), "{out}"):
            print(f"stdout line {i + 1}:\n  ANTs    {x}\n  larmorx {y}")
            break
    return 0


if __name__ == "__main__":
    sys.exit(main())
