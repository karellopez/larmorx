# SPDX-License-Identifier: Apache-2.0
"""Relay 2a: what the wheels built from the sdists hold (licence paths, data, metadata)."""

import collections
import email
import sys
import tarfile
import zipfile
from pathlib import Path

for w in sorted(Path(sys.argv[1]).glob("*.whl")):
    z = zipfile.ZipFile(w)
    names = z.namelist()
    lic = [n for n in names if ".dist-info/licenses/" in n]
    tops = collections.Counter(n.split(".dist-info/licenses/")[1].split("/")[0] for n in lic)
    data = [n for n in names if ".data/" in n]
    meta = email.message_from_bytes(
        z.read(next(n for n in names if n.endswith(".dist-info/METADATA")))
    )
    print(
        w.name,
        len(names),
        "files;",
        len(lic),
        "licence files, top:",
        dict(tops.most_common(6)),
        ";",
        len(data),
        "data files",
    )
    for k in (
        "Version",
        "Summary",
        "License-Expression",
        "Requires-Python",
        "Author",
        "Description-Content-Type",
    ):
        print("   ", k, "=", meta.get(k))
    print("    Project-URL", meta.get_all("Project-URL"))
    print(
        "    Classifier",
        len(meta.get_all("Classifier") or []),
        "; Requires-Dist",
        meta.get_all("Requires-Dist"),
    )
for s in sorted(Path(sys.argv[2]).glob("*.tar.gz")):
    with tarfile.open(s) as t:
        n = t.getnames()
    tops = collections.Counter(x.split("/")[1] if "/" in x else x for x in n)
    print(s.name, len(n), "entries;", dict(tops.most_common(12)))
