# SPDX-License-Identifier: Apache-2.0
"""Copies the BET suite's generated phantoms (``head-nan``, ``head-large``, ...) into a
directory for a probe and prints their paths: ``phantoms.py <dir> <name>...``."""

import shutil
import sys
from pathlib import Path

from larmorx_validation.parity import fsl_bet

p = fsl_bet._phantoms()
dest = Path(sys.argv[1])
dest.mkdir(exist_ok=True)
for name in sys.argv[2:]:
    shutil.copy(p[name], dest / Path(p[name]).name)
    print(dest / Path(p[name]).name)
