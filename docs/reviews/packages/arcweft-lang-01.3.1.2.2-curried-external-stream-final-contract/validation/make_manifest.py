#!/usr/bin/env python3
"""Create deterministic package metadata and SHA-256 sidecar."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def files_excluding(*names: str) -> list[Path]:
    excluded = set(names)
    return sorted(
        path
        for path in ROOT.rglob("*")
        if path.is_file()
        and path.name not in excluded
        and "__pycache__" not in path.parts
    )


def main() -> int:
    metadata_path = ROOT / "manifest.json"
    checksum_path = ROOT / "MANIFEST.sha256"
    metadata_path.unlink(missing_ok=True)
    checksum_path.unlink(missing_ok=True)

    payload = []
    for path in files_excluding("manifest.json", "MANIFEST.sha256"):
        payload.append(
            {
                "path": path.relative_to(ROOT).as_posix(),
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
        )
    metadata = {
        "contract_id": "Lang-01.3.1.2.2",
        "contract_status": "FINAL",
        "open_questions": 0,
        "fallback": False,
        "production_code_changed": False,
        "repository_commit": "5821a3ca479b5b89ca6ede997b9cf4f42f6280a6",
        "payload_files": payload,
    }
    metadata_path.write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    lines = []
    for path in files_excluding("MANIFEST.sha256"):
        lines.append(f"{sha256(path)}  {path.relative_to(ROOT).as_posix()}")
    checksum_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"manifest: {len(lines)} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
