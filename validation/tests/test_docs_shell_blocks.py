# SPDX-License-Identifier: Apache-2.0
"""No comments in the copyable shell blocks of the user documents.

zsh, macOS's default shell, does not take a `#` typed (or pasted) at its prompt as the start of a
comment (its option INTERACTIVE_COMMENTS is off): `pip install larmorx  # the library` installs
the packages `#`, `the` and `library` too, and `export X=1  # a note` fails. Windows' Command
Prompt never reads `#` as a comment either. So the shell blocks people copy from the README, the
PyPI descriptions and the API pages hold commands only; what a command does is said in the prose
(found by 0.1.0rc1's checks on macOS, 2026-11-11). The documentation site checks its own pages
the same way (its `tools/build.py`)."""

from __future__ import annotations

import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

#: The documents a user copies commands from (PyPI shows the three descriptions).
DOCUMENTS = sorted(
    [
        "README.md",
        "RELEASING.md",
        "crates-nc/PYPI.md",
        "crates-gpl/PYPI.md",
        "crates-nc/README.md",
        "crates-gpl/README.md",
        "docs/overview.md",
        *(p.relative_to(ROOT).as_posix() for p in (ROOT / "docs" / "api").glob("*.md")),
    ]
)

#: A fence's language that is a shell, and the blocks without one (they are copied alike).
SHELLS = {"", "bash", "sh", "shell", "zsh", "console", "powershell", "ps1", "pwsh", "bat", "cmd"}
_FENCE = re.compile(r"^\s*(`{3,}|~{3,})\s*([\w+-]*)")


def comment_at(line: str) -> int | None:
    """Where a shell comment starts in ``line``: a ``#`` at the start of a word, outside quotes
    (``a#b``, ``"#"``, ``'#'`` and ``${#x}`` are not comments); ``None`` without one."""
    quote = None
    for i, c in enumerate(line):
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c == "#" and (i == 0 or line[i - 1] in " \t"):
            return i
    return None


def shell_comments(text: str) -> list[tuple[int, str, str]]:
    """Every line of a shell block in ``text`` that holds a comment: (line number, the block's
    language, the line)."""
    found, fence, lang = [], None, ""
    for n, line in enumerate(text.split("\n"), 1):
        m = _FENCE.match(line)
        opens = fence is None
        closes = fence is not None and m is not None and m.group(1)[0] == fence[0]
        if m and (opens or (closes and len(m.group(1)) >= len(fence))):
            if fence is None:
                fence, lang = m.group(1), m.group(2).lower()
            else:
                fence = None
            continue
        if fence is not None and lang in SHELLS and comment_at(line) is not None:
            found.append((n, lang, line.strip()))
    return found


def test_the_finder():
    text = "\n".join(
        [
            "```bash",
            "pip install larmorx  # the library",
            "# a line of its own",
            "echo \"a # in quotes\" 'and # here' a#b ${#x}",
            "```",
            "# a heading, outside every block",
            "```python",
            "x = 1  # Python's comments are Python's",
            "```",
            "```",
            "AFNI_DECONFLICT=YES larmorx afni 3dTshift   # a block without a language",
            "```",
            "~~~powershell",
            "$env:X = 1  # PowerShell's",
            "~~~",
        ]
    )
    assert [(n, lang) for n, lang, _ in shell_comments(text)] == [
        (2, "bash"),
        (3, "bash"),
        (11, ""),
        (14, "powershell"),
    ]


@pytest.mark.parametrize("name", DOCUMENTS)
def test_no_comments_in_copyable_shell_blocks(name):
    found = shell_comments((ROOT / name).read_text(encoding="utf-8"))
    assert not found, (
        f"{name}: a comment in a copyable shell block breaks the command when pasted into zsh "
        "(or Windows' Command Prompt): say it in the prose instead:\n"
        + "\n".join(f"  line {n} [{lang or 'no language'}]: {line}" for n, lang, line in found)
    )
