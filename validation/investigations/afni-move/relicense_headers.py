# SPDX-License-Identifier: Apache-2.0
"""The one-off rewrite of the AFNI replicas' licence headers when they moved into the main package
(2026-10-29): the SPDX expression from what each file translates (`Apache-2.0 AND CC-BY-4.0` where
the header names the Medical College of Wisconsin's code, else `Apache-2.0`), GPL-2's release
statement replaced by CC BY 4.0's attribution (licence, URI, disclaimer, source and tag), AFNI's
request to cite, and the modification statement. Already applied; kept to show how the headers
were made (`--check` prints what it would write).

Usage: python -P relicense_headers.py <crate dir> [--check]
"""

import re
import sys
import textwrap
from pathlib import Path

GPL = re.compile(r"released under the GNU GPL version 2 or any later version")
CCBY = (
    "released by AFNI 26.3.05 under the Creative Commons Attribution 4.0 International licence "
    "(CC BY 4.0)"
)
CCBY_PARA = (
    "CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, "
    "without warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05 "
    "(first translated from AFNI_25.2.09)."
)
CITE = "AFNI asks that work based on it cite its authors (LICENSE.txt)."
OLD_RUST = "Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original."
NEW_RUST = (
    "Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0; "
    "modified from the original: translated to Rust, with the changes PROVENANCE.md lists."
)
MARKERS = ("Translated to Rust", "CC BY 4.0:", "Rust translation Copyright", "AFNI asks")


def header_end(lines: list[str], prefix: str) -> int:
    i = 1
    while i < len(lines) and lines[i].startswith(prefix) and not lines[i].startswith(prefix + "!"):
        i += 1
    return i


def rewrite(path: Path) -> str | None:
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    prefix = "//" if path.suffix == ".rs" else "#"
    first = lines[0]
    if "SPDX-License-Identifier: GPL-3.0-or-later" not in first and not (
        lines[0].startswith("#!") and "GPL-3.0-or-later" in lines[1]
    ):
        return None
    start = 1
    if first.startswith("#!"):
        start = 2
    end = header_end(lines[start - 1 :], prefix) + start - 1
    body = " ".join(
        line[len(prefix) :].strip() for line in lines[start:end] if line.strip() != prefix
    )
    mcw = "Medical College of Wisconsin" in body and "Major portions" in body
    afni = "AFNI" in body and "Translated" in body
    if mcw:
        spdx = "Apache-2.0 AND CC-BY-4.0"
    else:
        spdx = "Apache-2.0"
    body = re.sub(r"\s+", " ", body)
    body = GPL.sub(CCBY, body)
    body = body.replace(" This translation is part of the GPL-3.0-or-later larmorx-gpl package.", "")
    body = body.replace(
        "Written for larmorx-gpl (not a translation):",
        "Written for larmorx-gpl (not a translation) and relicensed to Apache-2.0 by its author "
        "on 2026-10-29, when the AFNI replicas moved into larmorx:",
    )
    if OLD_RUST in body:
        extra = ""
        if mcw:
            extra += " " + CCBY_PARA
        if afni:
            extra += " " + CITE
        body = body.replace(OLD_RUST, extra.strip() + " " + NEW_RUST)
    elif body.startswith("Copyright 2026 Karel Lopez Vilaret.") and "relicensed" not in body:
        body = body.replace(
            "Copyright 2026 Karel Lopez Vilaret.",
            "Copyright 2026 Karel Lopez Vilaret. Written for larmorx-gpl and relicensed to "
            "Apache-2.0 by its author on 2026-10-29, when the AFNI replicas moved into larmorx.",
            1,
        )
    for m in MARKERS:
        body = body.replace(" " + m, "\n" + m)
    paragraphs = [p.strip() for p in body.split("\n") if p.strip()]
    out = []
    for p in paragraphs:
        out += [f"{prefix} {w}" for w in textwrap.wrap(p, 98 - len(prefix), break_on_hyphens=False)]
    head = lines[: start - 1] + [f"{prefix} SPDX-License-Identifier: {spdx}"]
    return "\n".join(head + out + lines[end:])


def main() -> int:
    crate = Path(sys.argv[1])
    check = "--check" in sys.argv
    for path in sorted([*crate.rglob("*.rs"), *crate.rglob("*.py"), *crate.rglob("*.sh")]):
        if "target" in path.parts:
            continue
        new = rewrite(path)
        if new is None:
            continue
        if check:
            print(f"=== {path}")
            print("\n".join(new.split("\n")[:16]))
        else:
            path.write_text(new, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
