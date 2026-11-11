# SPDX-License-Identifier: Apache-2.0
"""The release's container images (`docker/Dockerfile`): build, check, assemble, push.

Two images per release (D5), each for linux/amd64 and linux/arm64, built on each architecture's
own runner from the release's own Linux wheels: `ghcr.io/karellopez/larmorx:<v>` (larmorx with
its `[prep]` dependencies, without the replica packages: commercial use allowed) and `:<v>-all`
(also larmorx-gpl and larmorx-nc, labelled non-commercial). `latest` and `latest-all` follow
final releases only. The release workflow runs the steps in this order:

`stage`  the build context: this architecture's three wheels of the version, the companions'
         sdists, `docker/README.txt`; anything missing, of another version or there twice is an
         error.
`build`  both images with `docker buildx` (the docker-container builder, BuildKit pinned by
         digest; created when missing), each saved as an OCI archive (`<out>/<image>.oci.tar`:
         the bytes that are pushed later) and loaded into Docker for the checks; `images.json`
         records each image's manifest and config digests. Nothing is pushed.
`check`  inside each loaded image: its labels; `smoke.py --installed --image ...` as root and as
         an unprivileged user on a read-only file system (as Apptainer runs it); the default
         command; `pip freeze` (the -all image: larmorx's set plus exactly larmorx-nc and
         larmorx-gpl); the golden tests and `tests/python/test_smoke.py` of the checkout.
`compare` (after both architectures) each image's packages the same on both (a difference is a
         warning: PyPI changed between the two builds).
`index`  (after both architectures) one OCI layout with every blob and, per image, a
         multi-architecture index over the two manifests (`index.json` names them `<v>` and
         `<v>-all`); `digests.json`; the two architectures' `pip freeze` compared.
`push`   (a release only, after PyPI took the files) `skopeo copy --all --preserve-digests` of
         each index to the registry, as `<v>` and `<v>-all` (and `latest`, `latest-all` for a
         final release); each tag read back and its digest compared.

Last line of each step `CONTAINER <step> ok ...` or `CONTAINER <step> FAILED ...`; in GitHub
Actions failures are also error annotations. Exit status 0, 1 (a failed check) or 2 (usage).

Usage:
    python -P scripts/release/container.py stage --version V --arch amd64 --wheels DIR \
        --sdists DIR --out CONTEXT
    python -P scripts/release/container.py build --version V --arch amd64 --context CONTEXT \
        --out OUT --revision SHA --epoch SECONDS
    python -P scripts/release/container.py check --version V --out OUT [--checkout DIR]
    python -P scripts/release/container.py compare --version V --images OUT_AMD64 OUT_ARM64
    python -P scripts/release/container.py index --version V --images OUT_AMD64 OUT_ARM64 \
        --out LAYOUT
    python -P scripts/release/container.py push --layout LAYOUT --repository ghcr.io/karellopez/larmorx \
        [--latest] [--authfile FILE] [--no-tls-verify]
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCKERFILE = ROOT / "docker" / "Dockerfile"
README = ROOT / "docker" / "README.txt"
REPOSITORY = "ghcr.io/karellopez/larmorx"
#: BuildKit for `docker buildx`'s docker-container builder: v0.34.1, its multi-architecture
#: index pinned by digest.
BUILDKIT = (
    "docker.io/moby/buildkit:v0.34.1"
    "@sha256:99368f3e46177c6405b9740e4461d91cee004128daeecca78d4ceac50ec1da5c"
)
BUILDER = "larmorx-release"
#: The architectures: Docker's name -> the wheels' platform-tag suffix.
ARCHES = {"amd64": "x86_64", "arm64": "aarch64"}
#: The images: Dockerfile target -> (tag suffix, OCI licence expression, companions installed).
IMAGES = {
    "larmorx": ("", "Apache-2.0", False),
    "larmorx-all": ("-all", "Apache-2.0 AND GPL-3.0-only AND LicenseRef-FSL", True),
}
COMPANIONS = ("larmorx-nc", "larmorx-gpl")
INDEX = "application/vnd.oci.image.index.v1+json"
MANIFEST = "application/vnd.oci.image.manifest.v1+json"
DOCKER_MANIFEST = "application/vnd.docker.distribution.manifest.v2+json"


class ContainerError(Exception):
    pass


class Checks:
    def __init__(self, step: str) -> None:
        self.step, self.n, self.failed = step, 0, []

    def check(self, ok: bool, what: str, detail: str = "") -> bool:
        self.n += 1
        if ok:
            print(f"ok    {what}", flush=True)
        else:
            message = f"{what}: {detail}" if detail else what
            self.failed.append(message)
            print(f"FAIL  {message}", flush=True)
            if os.environ.get("GITHUB_ACTIONS") == "true":
                print(f"::error title=container {self.step}::{message[:1000]}", flush=True)
        return ok

    def end(self) -> int:
        if self.failed:
            print(f"CONTAINER {self.step} FAILED: {len(self.failed)} of {self.n} checks")
            return 1
        print(f"CONTAINER {self.step} ok: {self.n} checks")
        return 0


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def local_tag(version: str, image: str) -> str:
    """The name the loaded image has in the runner's Docker."""
    return f"larmorx-release/{image}:{version}"


def tag(version: str, image: str) -> str:
    """The registry tag of an image: `<v>` or `<v>-all`."""
    return version + IMAGES[image][0]


def expected_labels(version: str, image: str, revision: str, created: str) -> dict[str, str]:
    """The OCI labels each image carries (the ones this release sets; the base image's others
    are allowed)."""
    site = "https://karellopez.github.io/larmorx_documentation/"
    return {
        "org.opencontainers.image.title": image,
        "org.opencontainers.image.version": version,
        "org.opencontainers.image.revision": revision,
        "org.opencontainers.image.created": created,
        "org.opencontainers.image.source": "https://github.com/karellopez/larmorx",
        "org.opencontainers.image.url": site,
        "org.opencontainers.image.documentation": site,
        "org.opencontainers.image.licenses": IMAGES[image][1],
    }


def created_from_epoch(epoch: int) -> str:
    return datetime.datetime.fromtimestamp(epoch, datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def _run(command: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    print("+ " + " ".join(command), flush=True)
    return subprocess.run(command, text=True, check=False, **kwargs)


# ---------------------------------------------------------------------------------------------
# stage


def stage(version: str, arch: str, wheels: list[Path], sdists: Path, out: Path) -> list[Path]:
    """Copy the build context into ``out``; returns its files."""
    if arch not in ARCHES:
        raise ContainerError(f"unknown architecture {arch!r} (one of {', '.join(ARCHES)})")
    machine = ARCHES[arch]
    found: dict[str, list[Path]] = {d: [] for d in ("larmorx", "larmorx_nc", "larmorx_gpl")}
    for folder in wheels:
        for path in sorted(folder.glob("*.whl")):
            dist, _, rest = path.name.partition(f"-{version}-")
            if dist in found and rest and "linux" in rest and rest.endswith(f"_{machine}.whl"):
                found[dist].append(path)
    for dist, paths in found.items():
        if len(paths) != 1:
            names = ", ".join(p.name for p in paths) or "none"
            raise ContainerError(
                f"{dist} {version}: one Linux {machine} wheel needed, found {len(paths)}: {names}"
            )
    sources = [sdists / f"{d}-{version}.tar.gz" for d in ("larmorx_nc", "larmorx_gpl")]
    for path in sources:
        if not path.is_file():
            raise ContainerError(f"{path.name} not found in {sdists}")
    if out.exists() and any(out.iterdir()):
        raise ContainerError(f"{out} is not empty")
    (out / "wheels").mkdir(parents=True, exist_ok=True)
    (out / "source").mkdir(exist_ok=True)
    copied = []
    for paths in found.values():
        copied.append(Path(shutil.copy2(paths[0], out / "wheels" / paths[0].name)))
    for path in sources:
        copied.append(Path(shutil.copy2(path, out / "source" / path.name)))
    copied.append(Path(shutil.copy2(README, out / "README.txt")))
    return copied


# ---------------------------------------------------------------------------------------------
# OCI archives and layouts


def read_archive(path: Path) -> tuple[dict, dict[str, bytes]]:
    """An OCI archive (a tar of an OCI layout): its index.json and its blobs by digest. Every
    blob's content is checked against its name."""
    index, blobs = None, {}
    with tarfile.open(path) as tar:
        for member in tar:
            name = member.name.removeprefix("./")
            if not member.isfile():
                continue
            data = tar.extractfile(member).read()  # type: ignore[union-attr]
            if name == "index.json":
                index = json.loads(data)
            elif name.startswith("blobs/sha256/"):
                digest = "sha256:" + name.rsplit("/", 1)[1]
                if sha256(data) != digest:
                    raise ContainerError(f"{path.name}: blob {digest} has another content")
                blobs[digest] = data
    if index is None:
        raise ContainerError(f"{path.name}: no index.json (not an OCI archive)")
    return index, blobs


def image_manifests(index: dict, blobs: dict[str, bytes], label: str) -> list[dict]:
    """The image manifests an OCI index reaches (nested indexes followed, attestation manifests
    left out): each with its descriptor and its platform from the config."""
    found = []
    for desc in index.get("manifests", []):
        media = desc.get("mediaType")
        digest = desc["digest"]
        if digest not in blobs:
            raise ContainerError(f"{label}: {digest} is not in the archive")
        if media == INDEX:
            found += image_manifests(json.loads(blobs[digest]), blobs, label)
            continue
        if media not in (MANIFEST, DOCKER_MANIFEST):
            raise ContainerError(f"{label}: unexpected media type {media}")
        annotations = desc.get("annotations", {})
        if annotations.get("vnd.docker.reference.type") == "attestation-manifest":
            continue
        manifest = json.loads(blobs[digest])
        config_digest = manifest["config"]["digest"]
        if config_digest not in blobs:
            raise ContainerError(f"{label}: config {config_digest} is not in the archive")
        for layer in manifest.get("layers", []):
            if layer["digest"] not in blobs:
                raise ContainerError(f"{label}: layer {layer['digest']} is not in the archive")
        config = json.loads(blobs[config_digest])
        platform = {"architecture": config["architecture"], "os": config["os"]}
        if config.get("variant"):
            platform["variant"] = config["variant"]
        found.append(
            {
                "mediaType": media,
                "digest": digest,
                "size": len(blobs[digest]),
                "platform": platform,
                "config": config_digest,
                "labels": config.get("config", {}).get("Labels") or {},
            }
        )
    return found


def _dump(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode("utf-8")


def index_layout(version: str, images: list[Path], out: Path, arches: list[str]) -> dict[str, str]:
    """One OCI layout in ``out`` with a multi-architecture index per image; returns the
    indexes' digests by tag."""
    records = []
    for folder in images:
        record = json.loads((folder / "images.json").read_text(encoding="utf-8"))
        if record["version"] != version:
            raise ContainerError(f"{folder}: images of {record['version']}, not {version}")
        records.append((folder, record))
    got = sorted(r["arch"] for _, r in records)
    if got != sorted(arches):
        raise ContainerError(f"architectures {got}, expected {sorted(arches)}")
    if out.exists() and any(out.iterdir()):
        raise ContainerError(f"{out} is not empty")
    (out / "blobs" / "sha256").mkdir(parents=True, exist_ok=True)

    def put(data: bytes) -> str:
        digest = sha256(data)
        path = out / "blobs" / "sha256" / digest.split(":", 1)[1]
        if not path.exists():
            path.write_bytes(data)
        return digest

    entries, digests = [], {}
    for image in IMAGES:
        children = []
        for folder, record in records:
            info = record["images"][image]
            index, blobs = read_archive(folder / info["archive"])
            manifests = image_manifests(index, blobs, info["archive"])
            if len(manifests) != 1:
                raise ContainerError(f"{info['archive']}: {len(manifests)} image manifests, not 1")
            (manifest,) = manifests
            if manifest["digest"] != info["manifest"]:
                raise ContainerError(
                    f"{info['archive']}: manifest is not the one images.json names"
                )
            if manifest["platform"]["architecture"] != record["arch"]:
                raise ContainerError(
                    f"{info['archive']}: an image for {manifest['platform']['architecture']}"
                    f" in the {record['arch']} folder"
                )
            if manifest["labels"].get("org.opencontainers.image.version") != version:
                raise ContainerError(f"{info['archive']}: its version label is not {version}")
            for data in blobs.values():
                put(data)
            children.append({k: manifest[k] for k in ("mediaType", "digest", "size", "platform")})
        children.sort(key=lambda c: (c["platform"]["architecture"], c["digest"]))
        body = _dump({"schemaVersion": 2, "mediaType": INDEX, "manifests": children})
        digest = put(body)
        name = tag(version, image)
        digests[name] = digest
        entries.append(
            {
                "mediaType": INDEX,
                "digest": digest,
                "size": len(body),
                "annotations": {"org.opencontainers.image.ref.name": name},
            }
        )
    (out / "oci-layout").write_bytes(_dump({"imageLayoutVersion": "1.0.0"}))
    (out / "index.json").write_bytes(
        _dump({"schemaVersion": 2, "mediaType": INDEX, "manifests": entries})
    )
    (out / "digests.json").write_text(json.dumps(digests, indent=2) + "\n", encoding="utf-8")
    return digests


def compare_freezes(version: str, images: list[Path]) -> list[str]:
    """Each image's packages (`pip list --format=freeze`, saved by `check`) the same on every
    architecture: the differences."""
    differences = []
    for image in IMAGES:
        sets = {}
        for folder in images:
            record = json.loads((folder / "images.json").read_text(encoding="utf-8"))
            if record["version"] != version:
                raise ContainerError(f"{folder}: images of {record['version']}, not {version}")
            path = folder / f"pip-freeze-{image}.txt"
            if not path.is_file():
                raise ContainerError(f"{path} not found (the check step writes it)")
            sets[record["arch"]] = set(path.read_text(encoding="utf-8").split())
        if len(sets) != len(images):
            raise ContainerError(f"{len(images)} folders but {len(sets)} architectures")
        union = set().union(*sets.values())
        for arch, lines in sorted(sets.items()):
            if lines != union:
                missing = sorted(union - lines)
                differences.append(f"{image} on {arch} lacks {', '.join(missing)}")
    return differences


# ---------------------------------------------------------------------------------------------
# build, check, push (they run docker and skopeo)


def build(
    version: str, arch: str, context: Path, out: Path, revision: str, epoch: int, builder: str
) -> int:
    checks = Checks("build")
    out.mkdir(parents=True, exist_ok=True)
    have = _run(["docker", "buildx", "inspect", builder], capture_output=True)
    if have.returncode != 0:
        create = [
            "docker", "buildx", "create", "--name", builder, "--driver", "docker-container",
            "--driver-opt", f"image={BUILDKIT}", "--bootstrap",
        ]  # fmt: skip
        if not checks.check(_run(create).returncode == 0, f"the builder {builder}"):
            return checks.end()
    created = created_from_epoch(epoch)
    record = {"version": version, "arch": arch, "revision": revision, "created": created}
    record["images"] = {}
    for image in IMAGES:
        archive = out / f"{image}-{version}-{arch}.oci.tar"
        command = [
            "docker", "buildx", "build", "--builder", builder, "--progress", "plain",
            "--platform", f"linux/{arch}", "--file", str(DOCKERFILE), "--target", image,
            "--build-arg", f"VERSION={version}", "--build-arg", f"REVISION={revision}",
            "--build-arg", f"CREATED={created}", "--build-arg", f"SOURCE_DATE_EPOCH={epoch}",
            "--provenance", "false", "--sbom", "false",
            "--output", f"type=oci,dest={archive}",
            "--output", f"type=docker,name={local_tag(version, image)}",
            str(context),
        ]  # fmt: skip
        if not checks.check(_run(command).returncode == 0, f"{image} built"):
            return checks.end()
        index, blobs = read_archive(archive)
        manifests = image_manifests(index, blobs, archive.name)
        if not checks.check(len(manifests) == 1, f"{archive.name}: one image manifest"):
            return checks.end()
        (manifest,) = manifests
        checks.check(
            manifest["platform"]["architecture"] == arch,
            f"{archive.name}: an image for {arch}",
            str(manifest["platform"]),
        )
        record["images"][image] = {
            "archive": archive.name,
            "manifest": manifest["digest"],
            "config": manifest["config"],
            "bytes": archive.stat().st_size,
        }
        print(f"{image}: manifest {manifest['digest']}, {archive.stat().st_size / 1e6:.1f} MB")
    (out / "images.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return checks.end()


def _docker_run(args: list[str], command: list[str]) -> subprocess.CompletedProcess[str]:
    return _run(["docker", "run", "--rm", *args, *command], capture_output=True)


def check(version: str, out: Path, checkout: Path) -> int:
    checks = Checks("check")
    record = json.loads((out / "images.json").read_text(encoding="utf-8"))
    freezes: dict[str, set[str]] = {}
    for image, (_, _, companions) in IMAGES.items():
        name = local_tag(version, image)
        info = record["images"][image]
        inspect = _run(["docker", "image", "inspect", name], capture_output=True)
        if not checks.check(inspect.returncode == 0, f"{name} loaded", inspect.stderr.strip()):
            continue
        config = json.loads(inspect.stdout)[0]
        # Docker's own store names an image by its config, the containerd store by its manifest
        checks.check(
            config["Id"] in (info["config"], info["manifest"]),
            f"{image}: the loaded image is the archive's ({info['manifest'][:19]})",
            config["Id"],
        )
        labels = config.get("Config", {}).get("Labels") or {}
        want = expected_labels(version, image, record["revision"], record["created"])
        wrong = {k: labels.get(k) for k, v in want.items() if labels.get(k) != v}
        checks.check(not wrong, f"{image}: its labels", str(wrong))
        checks.check(not config.get("Config", {}).get("Entrypoint"), f"{image}: no entrypoint")
        # the smoke checks as root, then as an unprivileged user on a read-only file system
        smoke = ["python", "-P", "/opt/release-checks/smoke.py", "--version", version]
        smoke += ["--installed", "--image", image]
        mount = ["-v", f"{checkout / 'scripts' / 'release'}:/opt/release-checks:ro"]
        unprivileged = ["--read-only", "--tmpfs", "/tmp", "--user", "65534:65534"]
        unprivileged += ["-e", "HOME=/tmp"]
        for who, extra in (("root", []), ("an unprivileged user, read-only", unprivileged)):
            r = _docker_run([*mount, *extra, name], smoke)
            print(r.stdout[-4000:], r.stderr[-2000:], sep="\n")
            checks.check(r.returncode == 0, f"{image}: smoke.py as {who}", r.stdout[-300:])
        r = _docker_run([*unprivileged, name], [])
        checks.check(
            r.returncode == 0 and "Usage: larmorx" in r.stdout,
            f"{image}: the default command (larmorx --help)",
            f"exit status {r.returncode}: {r.stdout[:200]!r} {r.stderr[:200]!r}",
        )
        r = _docker_run([name], ["cat", "/opt/larmorx/pip-freeze.txt"])
        if checks.check(r.returncode == 0, f"{image}: /opt/larmorx/pip-freeze.txt", r.stderr):
            (out / f"pip-freeze-{image}.txt").write_text(r.stdout, encoding="utf-8")
            lines = set(r.stdout.split())
            freezes[image] = lines
            checks.check(f"larmorx=={version}" in lines, f"{image}: larmorx=={version} frozen")
            for companion in COMPANIONS:
                pinned = f"{companion}=={version}" in lines
                checks.check(
                    pinned == companions,
                    f"{image}: {companion} {'installed' if companions else 'absent'}",
                )
        sources = _docker_run([name], ["ls", "/opt/larmorx/source"])
        want_sources = sorted(f"{d}-{version}.tar.gz" for d in ("larmorx_gpl", "larmorx_nc"))
        got_sources = sorted(sources.stdout.split()) if sources.returncode == 0 else []
        checks.check(
            got_sources == (want_sources if companions else []),
            f"{image}: /opt/larmorx/source/ {'holds the two sdists' if companions else 'absent'}",
            str(got_sources),
        )
        # the golden tests and the installed package's smoke tests, the checkout read-only;
        # pytest and the test group installed into this throwaway container only
        mismatches = out / f"golden-mismatches-{image}"
        mismatches.mkdir(exist_ok=True)
        tests = (
            "set -e; python -m pip install -q --upgrade pip; "
            "python -m pip install -q --only-binary :all: --group test; "
            "python -m pytest -p no:cacheprovider -q tests/golden tests/python/test_smoke.py"
        )
        r = _docker_run(
            [
                "-v", f"{checkout}:/src:ro", "-w", "/src",
                "-v", f"{mismatches}:/mismatches", "-e", "LARMORX_GOLDEN_MISMATCHES=/mismatches",
                name,
            ],
            ["sh", "-c", tests],
        )  # fmt: skip
        print(r.stdout[-3000:], r.stderr[-2000:], sep="\n")
        tail = (r.stdout.strip().splitlines() or [""])[-1]
        checks.check(r.returncode == 0, f"{image}: the golden tests and test_smoke.py ({tail})")
    if "larmorx" in freezes and "larmorx-all" in freezes:
        extra = freezes["larmorx-all"] - freezes["larmorx"]
        want = {f"{c}=={version}" for c in COMPANIONS}
        checks.check(
            extra == want and freezes["larmorx"] <= freezes["larmorx-all"],
            "larmorx-all: larmorx's packages and exactly larmorx-nc and larmorx-gpl",
            f"added {sorted(extra)}, removed {sorted(freezes['larmorx'] - freezes['larmorx-all'])}",
        )
    return checks.end()


def push_names(ref: str, latest: bool) -> list[str]:
    """The tags an index of the layout gets: its own, and latest or latest-all."""
    return [ref, "latest-all" if ref.endswith("-all") else "latest"] if latest else [ref]


def push(
    layout: Path, repository: str, latest: bool, authfile: Path | None, tls_verify: bool
) -> int:
    checks = Checks("push")
    if not checks.check(shutil.which("skopeo") is not None, "skopeo"):
        return checks.end()
    digests = json.loads((layout / "digests.json").read_text(encoding="utf-8"))
    auth = ["--authfile", str(authfile)] if authfile else []
    outputs = []
    for ref, digest in digests.items():
        for name in push_names(ref, latest):
            copy = ["skopeo", "copy", "--all", "--preserve-digests", "--retry-times", "3", *auth]
            copy += [f"--dest-tls-verify={str(tls_verify).lower()}"]
            copy += [f"oci:{layout}:{ref}", f"docker://{repository}:{name}"]
            if not checks.check(_run(copy).returncode == 0, f"{repository}:{name} pushed"):
                continue
            inspect = ["skopeo", "inspect", "--raw", *auth]
            inspect += [f"--tls-verify={str(tls_verify).lower()}", f"docker://{repository}:{name}"]
            r = subprocess.run(inspect, capture_output=True, check=False)
            got = sha256(r.stdout) if r.returncode == 0 else r.stderr.decode(errors="replace")
            checks.check(got == digest, f"{repository}:{name} is {digest}", got)
        key = "digest-all" if ref.endswith("-all") else "digest"
        outputs.append(f"{key}={digest}")
        print(f"{repository}:{ref}@{digest}")
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as f:
            f.write("\n".join(outputs) + "\n")
    return checks.end()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="step", required=True)
    p = sub.add_parser("stage", help="the build context")
    p.add_argument("--version", required=True)
    p.add_argument("--arch", required=True, choices=sorted(ARCHES))
    p.add_argument("--wheels", type=Path, nargs="+", required=True)
    p.add_argument("--sdists", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("build", help="both images, as OCI archives, loaded into Docker")
    p.add_argument("--version", required=True)
    p.add_argument("--arch", required=True, choices=sorted(ARCHES))
    p.add_argument("--context", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--revision", required=True)
    p.add_argument("--epoch", type=int, required=True, help="the commit's time (seconds)")
    p.add_argument("--builder", default=BUILDER)
    p = sub.add_parser("check", help="the checks inside the loaded images")
    p.add_argument("--version", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--checkout", type=Path, default=ROOT)
    p = sub.add_parser("index", help="one OCI layout with the multi-architecture indexes")
    p.add_argument("--version", required=True)
    p.add_argument("--images", type=Path, nargs="+", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--arches", default=",".join(ARCHES), help="default: amd64,arm64")
    p = sub.add_parser("compare", help="each image's packages the same on every architecture")
    p.add_argument("--version", required=True)
    p.add_argument("--images", type=Path, nargs="+", required=True)
    p = sub.add_parser("push", help="push the layout's indexes and read them back")
    p.add_argument("--layout", type=Path, required=True)
    p.add_argument("--repository", default=REPOSITORY)
    p.add_argument("--latest", action="store_true", help="also latest and latest-all")
    p.add_argument("--authfile", type=Path)
    p.add_argument("--no-tls-verify", action="store_true", help="a local test registry")
    args = parser.parse_args(argv)
    try:
        if args.step == "stage":
            files = stage(args.version, args.arch, args.wheels, args.sdists, args.out)
            for path in files:
                print(f"{path.relative_to(args.out)}  {path.stat().st_size / 1e6:.2f} MB")
            print(f"CONTAINER stage ok: {len(files)} files in {args.out}")
            return 0
        if args.step == "build":
            return build(
                args.version, args.arch, args.context.resolve(), args.out.resolve(),
                args.revision, args.epoch, args.builder,
            )  # fmt: skip
        if args.step == "check":
            return check(args.version, args.out.resolve(), args.checkout.resolve())
        if args.step == "index":
            arches = [a for a in args.arches.split(",") if a]
            digests = index_layout(args.version, args.images, args.out, arches)
            for name, digest in digests.items():
                print(f"{name}: {digest}")
            for difference in compare_freezes(args.version, args.images):
                print(f"::warning title=container images::{difference}")
            print(f"CONTAINER index ok: {len(digests)} images in {args.out}")
            return 0
        if args.step == "compare":
            differences = compare_freezes(args.version, args.images)
            for difference in differences:
                print(f"::warning title=container images::{difference}")
            print(f"CONTAINER compare ok: {len(differences)} differences between the architectures")
            return 0
        return push(
            args.layout.resolve(), args.repository, args.latest, args.authfile,
            not args.no_tls_verify,
        )  # fmt: skip
    except (ContainerError, OSError, KeyError, ValueError) as e:
        print(f"container {args.step}: {e}", file=sys.stderr)
        if os.environ.get("GITHUB_ACTIONS") == "true":
            print(f"::error title=container {args.step}::{e}")
        print(f"CONTAINER {args.step} FAILED: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
