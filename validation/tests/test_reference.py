# SPDX-License-Identifier: Apache-2.0
"""The programs' reference pages (larmorx_validation.reference): every API page that holds a
program's section agrees with the code, and every example of every reference page runs.

The examples run as a reader would run them: each program's examples in a new folder where the
inputs its section names are made first (``<!-- inputs: name=kind -->``,
:mod:`larmorx_validation.reference.inputs`), the Python blocks in one namespace, in order, and
each line of a shell block as a command (``larmorx ...`` through ``python -m larmorx``, so the
package under test runs). A shell block holds only ``larmorx`` (or ``lx``) commands, one per
line, so that it runs the same in bash, zsh, PowerShell and the Command Prompt. A Python block
runs statement by statement: a comment after an expression shows its output, ``# -> <repr>``,
and the run must give it; a description goes on a line of its own. They need the built
extension (``larmorx._core``); without it they are skipped, and the checks still run.
"""

from __future__ import annotations

import ast
import io
import os
import re
import shlex
import shutil
import subprocess
import sys
import tokenize
from pathlib import Path

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


#: An output shown after an expression of a Python example.
SHOWN = re.compile(r"^#\s*->\s*(?P<repr>.+?)\s*$")


def _shown(text: str) -> dict[int, str | None]:
    """The comments that follow code on their line, by line (1-based): the output shown
    (``# -> <repr>``), or ``None`` for any other comment."""
    out: dict[int, str | None] = {}
    lines = text.split("\n")
    for tok in tokenize.generate_tokens(io.StringIO(text).readline):
        if tok.type == tokenize.COMMENT:
            row, col = tok.start
            if lines[row - 1][:col].strip():
                m = SHOWN.match(tok.string)
                out[row] = m.group("repr") if m else None
    return out


def run_python(text: str, where: str, namespace: dict) -> None:
    """A Python example, statement by statement: an expression with ``# -> <repr>`` after it
    must give that ``repr``; any other comment after code is refused (a description goes on a
    line of its own)."""
    shown = _shown(text)
    tree = ast.parse(text, where)
    for stmt in tree.body:
        row = stmt.end_lineno or stmt.lineno
        if row in shown and shown[row] is None:
            raise AssertionError(
                f"{where}, line {row}: a comment after code shows an output (# -> <repr>); a "
                "description goes on a line of its own"
            )
        if isinstance(stmt, ast.Expr):
            value = eval(compile(ast.Expression(stmt.value), where, "eval"), namespace)
            if row in shown and repr(value) != shown[row]:
                raise AssertionError(
                    f"{where}, line {row}: the example shows {shown[row]}, the run gives {value!r}"
                )
        else:
            if row in shown:
                raise AssertionError(f"{where}, line {row}: an output shown after a statement")
            exec(compile(ast.Module([stmt], type_ignores=[]), where, "exec"), namespace)


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
            run_python(c.text, where, namespace)
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


API = "docs/api/fsl-bet.md"
BET_RS = "crates/larmorx-fsl/src/cli/bet.rs"

#: Drifts the checks must catch, in bet's API page or in the code it is checked against: (what
#: changes, the file, the text, its replacement, a part of the problem expected).
DRIFTS = [
    (
        "a missing row",
        API,
        "| `-d` | (command line only) | keeps the temporary files |\n",
        "",
        "`-d`",
    ),
    (
        "an option the program does not take",
        API,
        "| `-m` | `mask=True` | |",
        "| `-m`, `--mask` | `mask=True` | |",
        "`--mask`",
    ),
    (
        "an argument the function does not take",
        API,
        "| `-t` | `threshold=True` |",
        "| `-t` | `thresh=True` |",
        "`thresh`",
    ),
    (
        "a stale signature",
        API,
        'upstream_defects="fix")`\n\n`image` is a path',
        'upstream_defects="reproduce")`\n\n`image` is a path',
        "signature",
    ),
    (
        "the parser drops an option its usage text still lists",
        BET_RS,
        '"-o" | "-m" | "-s" | "-n" | "-t" | "-e" =>',
        '"-o" | "-m" | "-s" | "-n" | "-e" =>',
        "`-t` is in the usage text",
    ),
    (
        "a usage-only spelling the parser accepts",
        API,
        'usage-only: ["-h"]',
        'usage-only: ["-h", "-m"]',
        "usage-only names `-m`",
    ),
    (
        "a link to a replica crate",
        API,
        "[licensing](../licensing.md)",
        "[licensing](../../crates-nc/larmorx-nc-bet/PROVENANCE.md)",
        "restricted for clean-room implementers",
    ),
]


def _mirror(tmp: Path, rel: str, old: str, new: str) -> Path:
    """A copy of every file the exporter reads for bet's API page (its sources, the records it
    names, the package's Python), with ``old`` replaced by ``new`` in ``rel``."""
    from larmorx_validation.reference.page import build

    api = apipage.read(REPO, API)
    files = {s for p in api.programs for s in build(REPO, p).sources}
    for p in api.programs:
        for ids in (p.meta.get("records") or {}).values():
            files |= {f"docs/validation/{i}.md" for i in ids}
    for f in files:
        (tmp / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / f, tmp / f)
    shutil.copytree(
        REPO / "python/larmorx",
        tmp / "python/larmorx",
        ignore=shutil.ignore_patterns("*.so", "*.pyd", "__pycache__"),
        dirs_exist_ok=True,
    )
    text = (tmp / rel).read_text(encoding="utf-8")
    assert old in text, f"the drift's text is no longer in {rel}: update DRIFTS"
    (tmp / rel).write_text(text.replace(old, new, 1), encoding="utf-8")
    return tmp


@pytest.mark.parametrize("drift", DRIFTS, ids=[d[0] for d in DRIFTS])
def test_checks_catch_drift(drift, tmp_path):
    from larmorx_validation.reference.page import build

    _, rel, old, new, expected = drift
    repo = _mirror(tmp_path, rel, old, new)
    api = apipage.read(repo, API)
    problems = [x for p in api.programs for x in build(repo, p).problems]
    assert any(expected in x for x in problems), problems


def test_the_mirror_has_no_drift(tmp_path):
    """The copy the drift tests mutate passes the checks unmutated."""
    from larmorx_validation.reference.page import build

    repo = _mirror(tmp_path, API, "", "")
    api = apipage.read(repo, API)
    assert [x for p in api.programs for x in build(repo, p).problems] == []


@pytest.mark.parametrize(
    ("text", "fails"),
    [
        ("x = 2\nx + 1  # -> 3\n", None),
        ("x = 2\nx + 1  # -> 4\n", "the example shows 4, the run gives 3"),
        ("x = 2\nx + 1  # three\n", "a description goes on a line of its own"),
        ("# three, on its own line\nx = 2\nx + 1\n", None),
        ("'a' + 'b'  # -> 'ab'\n", None),
    ],
)
def test_shown_outputs_are_checked(text, fails):
    """An output an example shows after an expression is what the run gives."""
    if fails is None:
        run_python(text, "example", {})
    else:
        with pytest.raises(AssertionError, match=re.escape(fails)):
            run_python(text, "example", {})


def test_next_release_marks_the_options_until_a_release_has_them(tmp_path):
    """LARMORX's own options are marked "from the next release" while CHANGELOG.md's Unreleased
    section names their synopsis and no released one does."""
    from larmorx_validation.reference.page import next_release

    options = ["--n-threads", "--implementation", "--upstream-defects"]
    synopsis = "- `larmorx [--n-threads N] [--implementation ...] <family> <tool> ...`\n\n"
    log = tmp_path / "CHANGELOG.md"
    log.write_text(
        "# Changelog\n\n## [Unreleased]\n\n" + synopsis + "## [0.1.0rc1] - 2026-11-11\n\n- one\n",
        encoding="utf-8",
    )
    assert next_release(tmp_path, options) == "0.1.0rc1"
    log.write_text(
        "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2027-01-01\n\n"
        + synopsis
        + "## [0.1.0rc1] - 2026-11-11\n\n- one\n",
        encoding="utf-8",
    )
    assert next_release(tmp_path, options) is None


def test_programs_page_data_matches_the_code():
    """validation/programs.toml names every command of larmorx once, ports the replica registry
    lists, records, API pages and Python names that exist (the Programs page's data)."""
    from larmorx_validation.reference import programs

    groups, progs, problems = programs.build(reference.REPO, None)
    assert problems == []
    assert {p.group for p in progs} <= set(groups)
    text = programs.render(groups, progs)
    assert text == programs.render(*programs.build(reference.REPO, None)[:2])
    assert all(x.status for p in progs for x in p.ports)


def test_programs_check_catches_a_missing_command(tmp_path, monkeypatch):
    """A command of larmorx without an entry is a problem."""
    from larmorx_validation.reference import programs

    real = programs.commands
    monkeypatch.setattr(programs, "commands", lambda repo: [*real(repo), ("fsl", "fnirt")])
    _, _, problems = programs.build(reference.REPO, None)
    assert any("larmorx fsl fnirt" in p for p in problems)


def test_programs_what_it_does_has_one_source(monkeypatch):
    """Where an API page holds a program's reference block, the Programs page takes what the
    program does from it, and an entry that also gives its own `does` is a problem."""
    from larmorx_validation.reference import programs

    _, progs, _ = programs.build(reference.REPO, None)
    bet = next(p for p in progs if p.name == "bet")
    page = apipage.read(REPO, API)
    first = next(p for p in page.programs if p.name == "bet").description.split("\n\n")[0]
    assert bet.does == programs.first_sentence(first)
    real = programs._api_does
    monkeypatch.setattr(
        programs,
        "_api_does",
        lambda repo, api: (
            {"antsRegistration": "It registers."}
            if api == "docs/api/ants-registration.md"
            else real(repo, api)
        ),
    )
    _, _, problems = programs.build(reference.REPO, None)
    assert any("antsRegistration: `does` given" in p for p in problems), problems


def test_programs_check_catches_an_api_page_without_an_entry(monkeypatch):
    """Every API page but the register's is some entry's `api`."""
    from larmorx_validation.reference import programs

    monkeypatch.setattr(programs, "NOT_PROGRAMS", set())
    _, _, problems = programs.build(reference.REPO, None)
    assert problems == [
        "validation/programs.toml: no entry names docs/api/upstream-defects.md as its `api`"
    ]


@pytest.mark.parametrize(
    ("text", "first"),
    [
        ("`bet` finds the brain.\nIt writes it.", "`bet` finds the brain."),
        ("FSL 6.0.7.23 runs it. Then", "FSL 6.0.7.23 runs it."),
        ("No full stop", "No full stop"),
    ],
)
def test_first_sentence(text, first):
    from larmorx_validation.reference import programs

    assert programs.first_sentence(text) == first
