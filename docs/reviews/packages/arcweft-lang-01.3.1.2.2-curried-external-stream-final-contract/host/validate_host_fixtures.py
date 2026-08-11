#!/usr/bin/env python3
"""Validate correction-owned host JSON fields with the Python standard library."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

HEX_32 = re.compile(r"[0-9a-f]{64}\Z")
DECIMAL_U64 = re.compile(r"0|[1-9][0-9]*\Z")
ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT / "fixtures"


class ValidationError(ValueError):
    pass


def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValidationError(f"duplicate field: {key}")
        result[key] = value
    return result


def parse(path: Path) -> tuple[dict[str, Any], str]:
    raw = path.read_text(encoding="utf-8").strip()
    value = json.loads(raw, object_pairs_hook=reject_duplicates)
    if not isinstance(value, dict):
        raise ValidationError("top-level JSON must be an object")
    return value, raw


def exact_keys(value: dict[str, Any], expected: tuple[str, ...], where: str) -> None:
    actual = tuple(value.keys())
    if set(actual) != set(expected):
        missing = sorted(set(expected) - set(actual))
        unknown = sorted(set(actual) - set(expected))
        raise ValidationError(f"{where}: missing={missing}, unknown={unknown}")


def validate_hex(value: Any, where: str) -> None:
    if not isinstance(value, str) or HEX_32.fullmatch(value) is None:
        raise ValidationError(f"{where}: expected 64 lowercase hex characters")


def validate_decimal_u64(value: Any, where: str) -> None:
    if not isinstance(value, str) or DECIMAL_U64.fullmatch(value) is None:
        raise ValidationError(f"{where}: expected canonical decimal u64 string")
    if int(value) > 2**64 - 1:
        raise ValidationError(f"{where}: u64 overflow")


def validate_u16(value: Any, where: str) -> None:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 65535:
        raise ValidationError(f"{where}: expected JSON u16 number")


def validate_runtime_payload(value: Any, where: str) -> None:
    if not isinstance(value, dict) or not isinstance(value.get("kind"), str):
        raise ValidationError(f"{where}: expected typed runtime payload object")
    integer_kinds = {
        "i64",
        "i128",
        "isize",
        "u64",
        "u128",
        "usize",
    }
    if value["kind"] in integer_kinds:
        payload = value.get("value")
        if not isinstance(payload, str):
            raise ValidationError(f"{where}.value: wide integer must be a string")
        unsigned = value["kind"].startswith("u")
        pattern = DECIMAL_U64 if unsigned else re.compile(r"0|-?[1-9][0-9]*\Z")
        if pattern.fullmatch(payload) is None or payload == "-0":
            raise ValidationError(f"{where}.value: noncanonical integer string")


def validate_checked(value: Any, where: str, *, named: bool = False) -> None:
    if not isinstance(value, dict):
        raise ValidationError(f"{where}: expected object")
    expected = ("name", "type_layout", "digest", "value") if named else (
        "type_layout",
        "digest",
        "value",
    )
    exact_keys(value, expected, where)
    if named and (not isinstance(value["name"], str) or not value["name"]):
        raise ValidationError(f"{where}.name: expected non-empty string")
    validate_hex(value["type_layout"], f"{where}.type_layout")
    validate_hex(value["digest"], f"{where}.digest")
    validate_runtime_payload(value["value"], f"{where}.value")


def validate_argument_value(value: Any, where: str) -> None:
    if not isinstance(value, dict) or not isinstance(value.get("kind"), str):
        raise ValidationError(f"{where}: expected tagged object")
    kind = value["kind"]
    if kind == "explicit":
        exact_keys(value, ("kind", "type_layout", "digest", "value"), where)
        validate_checked(
            {key: value[key] for key in ("type_layout", "digest", "value")}, where
        )
    elif kind == "defaulted":
        exact_keys(
            value,
            ("kind", "default", "type_layout", "digest", "value"),
            where,
        )
        validate_hex(value["default"], f"{where}.default")
        validate_checked(
            {key: value[key] for key in ("type_layout", "digest", "value")}, where
        )
    elif kind == "omitted_optional":
        exact_keys(value, ("kind",), where)
    elif kind == "rest_positional":
        exact_keys(value, ("kind", "item_type_layout", "items"), where)
        validate_hex(value["item_type_layout"], f"{where}.item_type_layout")
        if not isinstance(value["items"], list):
            raise ValidationError(f"{where}.items: expected array")
        for index, item in enumerate(value["items"]):
            validate_checked(item, f"{where}.items[{index}]")
    elif kind == "rest_named":
        exact_keys(value, ("kind", "value_type_layout", "entries"), where)
        validate_hex(value["value_type_layout"], f"{where}.value_type_layout")
        if not isinstance(value["entries"], list):
            raise ValidationError(f"{where}.entries: expected array")
        previous: bytes | None = None
        for index, entry in enumerate(value["entries"]):
            validate_checked(entry, f"{where}.entries[{index}]", named=True)
            current = entry["name"].encode("utf-8")
            if previous is not None and previous >= current:
                raise ValidationError(f"{where}.entries: duplicate or out of order")
            previous = current
    else:
        raise ValidationError(f"{where}.kind: unknown tag {kind!r}")


def validate_request(value: dict[str, Any]) -> None:
    exact_keys(
        value,
        (
            "kind",
            "definition",
            "declaration",
            "generation",
            "instance",
            "signature",
            "capability",
            "operation",
            "arguments",
            "policy",
        ),
        "request",
    )
    if value["kind"] != "open_stream":
        raise ValidationError("request.kind: expected open_stream")
    for field in ("definition", "declaration", "signature"):
        validate_hex(value[field], f"request.{field}")
    validate_decimal_u64(value["generation"], "request.generation")
    validate_decimal_u64(value["instance"], "request.instance")
    for field in ("capability", "operation"):
        if not isinstance(value[field], str) or not value[field]:
            raise ValidationError(f"request.{field}: expected non-empty string")
    if not isinstance(value["policy"], dict):
        raise ValidationError("request.policy: parent policy must be an object")

    arguments = value["arguments"]
    if not isinstance(arguments, dict):
        raise ValidationError("request.arguments: expected object")
    exact_keys(
        arguments,
        ("completed_groups", "coordinates", "values"),
        "request.arguments",
    )
    validate_u16(arguments["completed_groups"], "request.arguments.completed_groups")
    if not 1 <= arguments["completed_groups"] <= 16:
        raise ValidationError("request.arguments.completed_groups: expected 1..=16")
    coordinates = arguments["coordinates"]
    values = arguments["values"]
    if not isinstance(coordinates, list) or not isinstance(values, list):
        raise ValidationError("request.arguments: vectors must be arrays")
    if len(coordinates) != len(values):
        raise ValidationError("request.arguments: coordinate/value length mismatch")
    if len(coordinates) > 128:
        raise ValidationError("request.arguments: more than 128 cells")

    previous: tuple[int, int] | None = None
    for index, coordinate in enumerate(coordinates):
        if not isinstance(coordinate, dict):
            raise ValidationError(f"coordinates[{index}]: expected object")
        exact_keys(coordinate, ("group", "parameter"), f"coordinates[{index}]")
        validate_u16(coordinate["group"], f"coordinates[{index}].group")
        validate_u16(coordinate["parameter"], f"coordinates[{index}].parameter")
        if coordinate["group"] >= arguments["completed_groups"]:
            raise ValidationError(f"coordinates[{index}]: group not completed")
        current = (coordinate["group"], coordinate["parameter"])
        if previous is not None and previous >= current:
            raise ValidationError("coordinates: duplicate or out of order")
        previous = current
    for index, item in enumerate(values):
        validate_argument_value(item, f"values[{index}]")


def canonical_bytes(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def expect_rejected(path: Path, expected_fragment: str) -> None:
    try:
        value, _ = parse(path)
        validate_request(value)
    except (ValidationError, json.JSONDecodeError) as error:
        if expected_fragment not in str(error):
            raise ValidationError(
                f"{path.name}: wrong rejection {error!s}; expected {expected_fragment!r}"
            ) from error
        return
    raise ValidationError(f"{path.name}: fixture was unexpectedly accepted")


def main() -> int:
    valid_names = (
        "open_stream.valid.json",
        "open_stream.native.json",
        "open_stream.web.json",
        "open_stream.agent.json",
    )
    canonical: list[str] = []
    for name in valid_names:
        value, raw = parse(FIXTURES / name)
        validate_request(value)
        encoded = canonical_bytes(value)
        if encoded != raw:
            raise ValidationError(f"{name}: not canonical compact JSON")
        canonical.append(encoded)
    if len(set(canonical)) != 1:
        raise ValidationError("native/Web/Agent fixture bytes differ")

    expect_rejected(FIXTURES / "open_stream.duplicate-field.json", "duplicate field")
    expect_rejected(FIXTURES / "open_stream.unknown-field.json", "flat_arguments")
    print("host-fixtures: PASS")
    print(f"canonical-bytes: {len(canonical[0].encode('utf-8'))}")
    print("valid-fixtures: 4")
    print("negative-fixtures: 2")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValidationError as error:
        print(f"host-fixtures: FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from error
