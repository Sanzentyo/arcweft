#!/usr/bin/env python3
"""Build a deterministic ZIP for the sealed final-contract directory."""

from __future__ import annotations

import argparse
from pathlib import Path
import stat
import sys
import zipfile

sys.dont_write_bytecode = True

from verify_contract import validate


FIXED_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


def package_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"symlink is forbidden: {path}")
        if path.is_file():
            files.append(path)
    return sorted(files, key=lambda path: path.relative_to(root).as_posix())


def build_zip(root: Path, output: Path) -> None:
    root = root.resolve()
    output = output.resolve()
    validate(root, run_reference_tests=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp")
    temporary.unlink(missing_ok=True)

    with zipfile.ZipFile(
        temporary,
        mode="w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=9,
        strict_timestamps=True,
    ) as archive:
        for path in package_files(root):
            relative = path.relative_to(root).as_posix()
            archive_name = f"{root.name}/{relative}"
            info = zipfile.ZipInfo(archive_name, date_time=FIXED_TIMESTAMP)
            info.create_system = 3
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            info.flag_bits |= 0x800
            archive.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)

    temporary.replace(output)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build_zip(args.root, args.output)
    print(args.output.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
