# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx``: same as the ``larmorx`` command."""

import sys

from larmorx.cli import run

sys.exit(run(["larmorx", *sys.argv[1:]]))
