# SPDX-License-Identifier: Apache-2.0
"""List the text blocks of an agent's HANDOFF.md with the instruction above each, to integrate them.

Usage: python -P handoff_blocks.py <HANDOFF.md>
"""

import re
import sys
from pathlib import Path

H = Path(sys.argv[1]).read_text()
for n, m in enumerate(re.finditer(r"```[a-z]*\n(.*?)```", H, re.S)):
    pre = [ln for ln in H[: m.start()].rstrip().split("\n")[-4:] if ln.strip()][-2:]
    body = m.group(1).split("\n")
    print(f"[{n}] line {H[: m.start()].count(chr(10))}: {' / '.join(p[:110] for p in pre)}")
    print(f"     {body[0][:110]} ({len(body)} lines)")
