# SPDX-License-Identifier: Apache-2.0
"""Fetch a package's sdist from PyPI (its JSON API), verify its SHA-256, save it."""

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

name, version, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
meta = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/{name}/{version}/json", timeout=60))
for f in meta["urls"]:
    if f["packagetype"] == "sdist":
        data = urllib.request.urlopen(f["url"], timeout=300).read()
        digest = hashlib.sha256(data).hexdigest()
        assert digest == f["digests"]["sha256"], (digest, f["digests"]["sha256"])
        (out / f["filename"]).write_bytes(data)
        print(f["filename"], digest, len(data))
