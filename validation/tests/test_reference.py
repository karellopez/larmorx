# SPDX-License-Identifier: Apache-2.0
"""The programs' reference pages (larmorx_validation.reference): every API page that holds a
program's section agrees with the code, and every example of every reference page runs.

The examples run as a reader would run them: each program's examples in a new folder where the
inputs its section names are made first (``<!-- inputs: name=kind -->``,
:mod:`larmorx_validation.reference.inputs`), the Python blocks in one namespace, in order, and
each line of a shell block as a command (``larmorx ...`` through ``python -m larmorx``, so the
package under test runs). A shell block holds only ``larmorx`` (or ``lx``) commands, one per
line, so that it runs the same in bash, zsh, PowerShell and the Command Prompt. They need the
built extension (``larmorx._core``); without it they are skipped, and the checks still run.
"""

from __future__ import annotations

import os
import shlex
import subprocess
import sys

import pytest
from larmorx_validation import reference
from larmorx_validation.reference import apipage, inputs

REPO = reference.REPO
PAGES = reference.api_pages(REPO)


def _programs() -> list[apipage.Program]:
    return [p for page in PAGES for p in apipage.read(REPO, page).programs]


def test_some_api_page_has_reference_sections():
    assert PAGES, "no API page holds a program's reference section"


@pytest.mark.parametrize("page", PAGES)
def test_api_page_matches_the_code(page):
    problems = [x for r in reference.references(page, REPO) for x in r.problems]
    assert not problems, "\n".join(problems)


@pytest.mark.parametrize("page", PAGES)
def test_export_is_deterministic(page):
    first = [r.markdown for r in reference.references(page, REPO)]
    assert first == [r.markdown for r in reference.references(page, REPO)]


def _core_available() -> bool:
    try:
        import larmorx._core  # noqa: F401
    except ImportError:
        return False
    return True


def _commands(code: apipage.Code) -> list[list[str]]:
    out = []
    for line in code.text.split("\n"):
        if not line.strip():
            continue
        words = shlex.split(line)
        assert words[0] in ("larmorx", "lx"), (
            f"line {code.line}: a shell example runs larmorx only, one command per line: {line!r}"
        )
        assert not any(w in ("|", "&&", "||", ";", ">", "<") for w in words), (
            f"line {code.line}: no pipes, chains or redirections in a shell example: {line!r}"
        )
        out.append(words[1:])
    return out


@pytest.mark.skipif(not _core_available(), reason="needs the built extension (larmorx._core)")
@pytest.mark.parametrize("program", _programs(), ids=lambda p: f"{p.page.path.stem}:{p.name}")
def test_examples_run(program, tmp_path, monkeypatch):
    section = program.child("examples")
    assert section is not None, f"{program.page.path}: {program.name} has no examples"
    text = section.text()
    inputs.make(tmp_path, apipage.inputs(text))
    monkeypatch.chdir(tmp_path)
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join(p for p in sys.path if p)
    namespace: dict = {"__name__": "__example__"}
    blocks = [c for c in apipage.code_blocks(text) if c.example]
    assert blocks
    for c in blocks:
        where = f"{program.page.path}:{c.line}"
        if c.lang == "python":
            exec(compile(c.text, where, "exec"), namespace)
        elif c.lang in ("bash", "sh", "shell", "zsh", "console", "powershell"):
            for args in _commands(c):
                p = subprocess.run(
                    [sys.executable, "-m", "larmorx", *args],
                    cwd=tmp_path,
                    env=env,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                    timeout=300,
                )
                assert p.returncode == 0, f"{where}: larmorx {shlex.join(args)}\n{p.stderr[-2000:]}"
        else:
            pytest.fail(f"{where}: an example in {c.lang!r}: python or a shell")
