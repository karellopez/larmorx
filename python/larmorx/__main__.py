"""``python -m larmorx``: same as the ``larmorx`` command."""

import sys

from larmorx.cli import run

sys.exit(run(["larmorx", *sys.argv[1:]]))
