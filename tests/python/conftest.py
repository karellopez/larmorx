# SPDX-License-Identifier: Apache-2.0
"""Fixtures shared by the Python tests."""

from __future__ import annotations

import pytest

from larmorx import _replica


@pytest.fixture
def no_replica(monkeypatch, tmp_path_factory):
    """No replica program can be found: neither through its environment variable, nor in the
    Python environment's scripts directory, nor on ``PATH``. So ``implementation="auto"`` and
    the command line run the clean-room original, whatever this machine has installed."""
    monkeypatch.delenv("LARMORX_GPL_BIN", raising=False)
    monkeypatch.delenv("LARMORX_IMPLEMENTATION", raising=False)
    monkeypatch.setattr(_replica, "search_dirs", list)
    monkeypatch.setenv("PATH", str(tmp_path_factory.mktemp("empty-path")))
