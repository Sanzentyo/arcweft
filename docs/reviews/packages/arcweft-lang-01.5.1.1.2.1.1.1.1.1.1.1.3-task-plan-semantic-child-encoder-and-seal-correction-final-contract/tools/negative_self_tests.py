#!/usr/bin/env python3
"""Mutation tests proving the package validator rejects every child blocker."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT_NAME = (
    "arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-"
    "task-plan-semantic-child-encoder-and-seal-correction-final-contract"
)
ARCHIVE = ROOT_NAME + ".zip"
EXCLUDED = {"MANIFEST.json", "MANIFEST.sha256", "CHECKSUMS.sha256"}


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def rebuild_manifest(root: Path) -> None:
    files = sorted(
        path for path in root.rglob("*")
        if path.is_file() and path.relative_to(root).as_posix() not in EXCLUDED
    )
    manifest = {
        "schema_version": 1,
        "root": ROOT_NAME,
        "files": [
            {
                "path": path.relative_to(root).as_posix(),
                "bytes": path.stat().st_size,
                "sha256": digest(path),
            }
            for path in files
        ],
    }
    data = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    (root / "MANIFEST.json").write_bytes(data)
    (root / "MANIFEST.sha256").write_text(
        hashlib.sha256(data).hexdigest() + "\n", encoding="utf-8"
    )
    checksum_files = sorted(
        path for path in root.rglob("*")
        if path.is_file() and path.name != "CHECKSUMS.sha256"
    )
    (root / "CHECKSUMS.sha256").write_text(
        "".join(
            f"{digest(path)}  {path.relative_to(root).as_posix()}\n"
            for path in checksum_files
        ),
        encoding="utf-8",
    )


def run_validator(validator: Path, package: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(validator), str(package)],
        text=True,
        capture_output=True,
    )


def expect_rejected(
    validator: Path,
    source: Path,
    name: str,
    mutate,
    expected: str,
    rebuild: bool = True,
) -> None:
    with tempfile.TemporaryDirectory(prefix=f"arcweft-neg-{name}-") as tmp:
        root = Path(tmp) / ROOT_NAME
        shutil.copytree(source, root)
        mutate(root)
        if rebuild:
            rebuild_manifest(root)
        result = run_validator(validator, root)
        if result.returncode == 0:
            raise AssertionError(f"{name}: validator unexpectedly accepted mutation")
        output = result.stdout + result.stderr
        if expected not in output:
            raise AssertionError(
                f"{name}: expected rejection containing {expected!r}, got:\n{output}"
            )


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise AssertionError(f"mutation anchor missing in {path}: {old!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def unsafe_zip_test(validator: Path, source: Path, casefold: bool) -> None:
    with tempfile.TemporaryDirectory(prefix="arcweft-neg-zip-") as tmp:
        archive = Path(tmp) / ARCHIVE
        with zipfile.ZipFile(archive, "w") as zf:
            zf.writestr(f"{ROOT_NAME}/README.md", "x")
            if casefold:
                zf.writestr(f"{ROOT_NAME}/readme.MD", "y")
            else:
                zf.writestr(f"{ROOT_NAME}/../escape.txt", "y")
        result = run_validator(validator, archive)
        if result.returncode == 0:
            raise AssertionError("unsafe ZIP mutation unexpectedly accepted")
        output = result.stdout + result.stderr
        expected = "case-fold collision" if casefold else "unsafe ZIP member path"
        if expected not in output:
            raise AssertionError(f"unsafe ZIP expected {expected!r}, got:\n{output}")


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} PACKAGE_ROOT", file=sys.stderr)
        return 2
    source = Path(sys.argv[1]).resolve()
    if source.name != ROOT_NAME:
        print(f"package root must be named {ROOT_NAME}", file=sys.stderr)
        return 2
    validator = source / "tools/validate_contract.py"

    cases = [
        (
            "status",
            lambda r: (r / "FINAL_STATUS").write_text("DESIGN_NOT_READY\n"),
            "FINAL_STATUS must be exactly READY_FOR_IMPLEMENTATION",
            True,
        ),
        (
            "open-questions",
            lambda r: (r / "OPEN_QUESTIONS").write_text("one\n"),
            "OPEN_QUESTIONS must be exactly none",
            True,
        ),
        (
            "manifest-tamper",
            lambda r: (r / "README.md").write_text(
                (r / "README.md").read_text(encoding="utf-8") + "tamper\n",
                encoding="utf-8",
            ),
            "manifest size mismatch",
            False,
        ),
        (
            "missing-transcript",
            lambda r: replace_once(
                r / "TRANSCRIPTS.md",
                'domain = "arcweft.task.plan-semantic.v1\\0"',
                'domain removed',
            ),
            "transcript missing authoritative token",
            True,
        ),
        (
            "self-digest",
            lambda r: replace_once(
                r / "schemas/final_contract.rs",
                "pub struct RuntimeTaskPlan {\n",
                "pub struct RuntimeTaskPlan {\n    semantic_digest: TaskPlanSemanticDigest,\n",
            ),
            "RuntimeTaskPlan contains forbidden field semantic_digest:",
            True,
        ),
        (
            "raw-constructor",
            lambda r: (r / "schemas/final_contract.rs").write_text(
                (r / "schemas/final_contract.rs").read_text(encoding="utf-8")
                + "\nimpl TaskPlanSemanticDigest { pub fn from_bytes(v: [u8; 32]) -> Self { Self(v) } }\n",
                encoding="utf-8",
            ),
            "public raw digest constructor is forbidden",
            True,
        ),
        (
            "raw-view-projection",
            lambda r: (r / "schemas/final_contract.rs").write_text(
                (r / "schemas/final_contract.rs").read_text(encoding="utf-8")
                + "\npub struct ViewProgramIdProjection(pub [u8; 32]);\n",
                encoding="utf-8",
            ),
            "raw core View projection is forbidden",
            True,
        ),
        (
            "caller-sink",
            lambda r: (r / "schemas/final_contract.rs").write_text(
                (r / "schemas/final_contract.rs").read_text(encoding="utf-8")
                + "\npub fn caller_sink(caller_sink: &mut dyn std::io::Write) {}\n",
                encoding="utf-8",
            ),
            "caller/general byte sink API is forbidden",
            True,
        ),
        (
            "public-expected-key",
            lambda r: replace_once(
                r / "schemas/final_contract.rs",
                "struct ExpectedTaskPlanKey([u8; 32]);",
                "pub struct ExpectedTaskPlanKey([u8; 32]);",
            ),
            "expected task-plan key must remain private",
            True,
        ),
        (
            "core-view-dependency",
            lambda r: mutate_dependency(r),
            "core->View must be forbidden",
            True,
        ),
        (
            "version-marker",
            lambda r: mutate_version(r),
            "contract schema_version must be exactly 1",
            True,
        ),
        (
            "missing-required-file",
            lambda r: (r / "CYCLE_PROOF.md").unlink(),
            "required files missing",
            True,
        ),
    ]

    for name, mutate, expected, rebuild in cases:
        expect_rejected(validator, source, name, mutate, expected, rebuild)
    unsafe_zip_test(validator, source, casefold=False)
    unsafe_zip_test(validator, source, casefold=True)
    print(f"PASS ({len(cases) + 2} negative cases)")
    return 0


def mutate_dependency(root: Path) -> None:
    path = root / "machine/dependencies.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    for edge in data["edges"]:
        if edge["from"] == "arcweft-core" and edge["to"] == "arcweft-view":
            edge["allowed"] = True
            break
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def mutate_version(root: Path) -> None:
    path = root / "machine/contract.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    data["schema_version"] = 2
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
