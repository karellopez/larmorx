# SPDX-License-Identifier: Apache-2.0
"""The old names of the modules the documentation revision renamed (``*_original`` to
``*_clean_room``, 2026-11-12): every import form reaches the module under its new name, as the
committed investigations use them, ``python -m <old name>`` included.

Each package that renamed modules calls :func:`alias` with its ``_RENAMED`` map; the finder then
answers for the old dotted names: the module imported is the renamed one (one module object,
whatever name imports it), and ``python -m`` runs the renamed module's code."""

from __future__ import annotations

import importlib
import importlib.abc
import importlib.machinery
import importlib.util
import sys
from types import ModuleType


class _Alias(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    """A finder and loader for the old names (old dotted name -> new dotted name)."""

    def __init__(self) -> None:
        self.names: dict[str, str] = {}

    def find_spec(self, fullname, path=None, target=None):
        new = self.names.get(fullname)
        if new is None:
            return None
        spec = importlib.util.find_spec(new)
        if spec is None:
            return None
        return importlib.machinery.ModuleSpec(fullname, self, origin=spec.origin)

    def create_module(self, spec) -> ModuleType:
        module = importlib.import_module(self.names[spec.name])
        self._spec = module.__spec__  # restored by exec_module: the module keeps its own spec
        return module

    def exec_module(self, module: ModuleType) -> None:
        module.__spec__ = self._spec  # the renamed module has run already, under its new name

    def get_code(self, fullname: str):
        """``python -m <old name>``: the renamed module's code."""
        new = self.names[fullname]
        spec = importlib.util.find_spec(new)
        return spec.loader.get_code(new)


_FINDER = _Alias()


def alias(package: str, renamed: dict[str, str]) -> None:
    """Make each old name of ``renamed`` (old -> new, within ``package``) import the new one."""
    for old, new in renamed.items():
        _FINDER.names[f"{package}.{old}"] = f"{package}.{new}"
    if _FINDER not in sys.meta_path:
        sys.meta_path.append(_FINDER)
