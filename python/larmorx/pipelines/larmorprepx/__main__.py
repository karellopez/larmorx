# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx.pipelines.larmorprepx``: same as the ``larmorprepx`` command."""

import sys

from larmorx.pipelines.larmorprepx.cli import main

sys.exit(main())
