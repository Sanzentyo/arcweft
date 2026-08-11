#!/usr/bin/env python3
"""Standalone validation for the Lang-01.4.2.1 final-contract package.

The validator intentionally uses only the Python standard library for all
required checks.  When ``jsonschema`` is available, it additionally executes
Draft 2020-12 validation against the bundled schema.
"""
from __future__ import annotations

import copy
import hashlib
import json
import math
import re
import struct
import sys
from pathlib import Path
from typing import Any, Callable, Iterable

PIN = "5821a3ca479b5b89ca6ede997b9cf4f42f6280a6"
FORMAT = "arcweft.resource-type-manifest"
DESCRIPTOR_CONTEXT = "arcweft-resource-type-descriptor-v1"
EXPECTED_SCALARS = {
    "unit", "bool", "signed_integer", "unsigned_integer", "float",
    "string", "char", "duration", "ratio", "length", "gain", "pan",
    "locale", "public_id",
}
EXPECTED_VALUE_TYPES = {
    "scalar", "option", "list", "non_empty_list", "ordered_map", "record",
    "enum", "asset_ref", "resource_ref", "retained_identity_ref",
    "constrained_scalar",
}
EXPECTED_CONST_KINDS = {
    "scalar", "option", "list", "ordered_map", "record", "enum",
    "asset_ref", "resource_ref", "retained_identity_ref",
}
EXPECTED_RETAINED = {
    "character", "view", "action", "layer", "signal",
    "presentation_target", "scroll_region",
}
EXPECTED_LAYOUT_UNITS = {
    "px", "sp", "percent", "vw", "vh", "cw", "ch", "em", "glyph_ch",
    "safe_area_top", "safe_area_right", "safe_area_bottom", "safe_area_left",
}
EXPECTED_LIMITS = {
    "bytes": 8_388_608,
    "nesting_depth": 64,
    "lexical_nodes": 65_536,
    "string_bytes": 1_048_576,
    "collection_items": 16_384,
    "object_members": 4_096,
    "semantic_records": 16_384,
    "work_units": 1_048_576,
}
REQUIRED_DOCS = {
    "README.md", "FINAL_CONTRACT.md", "REPOSITORY_INVENTORY.md",
    "WIRE_SCHEMA.md", "DTO_AND_CONVERSION.md", "DIAGNOSTICS_AND_LIMITS.md",
    "OWNERSHIP_AND_DEPENDENCIES.md", "PACKAGE_AND_ARTIFACT_PUBLICATION.md",
    "IMPLEMENTATION_ORDER.md", "RECONCILIATION_MATRIX.md", "TEST_MATRIX.md",
    "VALIDATION.md", "NO_PRODUCTION_CHANGES.md",
}
PACKAGE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]*(?:\.[a-z0-9][a-z0-9-]*)+$")
STABLE_ID_RE = re.compile(r"^[a-z](?:[a-z0-9_-]*[a-z0-9])?(?:\.[a-z](?:[a-z0-9_-]*[a-z0-9])?)*$")
FAMILY_RE = re.compile(r"^[a-z](?:[a-z0-9_-]*[a-z0-9])?$")
DIGEST_RE = re.compile(r"^blake3:[0-9a-f]{64}$")
FLOAT_BITS_RE = re.compile(r"^0x[0-9a-f]{16}$")
SEMVER_RE = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-((?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


class ValidationError(Exception):
    pass


class Reporter:
    def __init__(self) -> None:
        self.passes: list[str] = []

    def check(self, condition: bool, message: str) -> None:
        if not condition:
            raise ValidationError(message)
        self.passes.append(message)

    def note(self, message: str) -> None:
        print(f"NOTE {message}")


# ---- Strict JSON ---------------------------------------------------------

def _duplicate_rejecting_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValidationError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result


def _reject_float(text: str) -> Any:
    raise ValidationError(f"JSON floating-point number is forbidden: {text}")


def _parse_int(text: str) -> int:
    if text == "-0":
        raise ValidationError("non-canonical JSON integer -0")
    return int(text, 10)


def _reject_constant(text: str) -> Any:
    raise ValidationError(f"non-JSON numeric constant is forbidden: {text}")


def _walk_strict(value: Any, path: str = "$") -> None:
    if value is None:
        raise ValidationError(f"explicit null is forbidden at {path}")
    if isinstance(value, str):
        for character in value:
            if 0xD800 <= ord(character) <= 0xDFFF:
                raise ValidationError(f"unpaired UTF-16 surrogate at {path}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _walk_strict(item, f"{path}[{index}]")
    elif isinstance(value, dict):
        for key, item in value.items():
            _walk_strict(key, f"{path}.<key>")
            _walk_strict(item, f"{path}.{key}")
    elif isinstance(value, float):
        raise ValidationError(f"floating-point semantic value at {path}")


def strict_json_bytes(data: bytes, label: str) -> Any:
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise ValidationError(f"{label}: invalid UTF-8: {error}") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_duplicate_rejecting_object,
            parse_float=_reject_float,
            parse_int=_parse_int,
            parse_constant=_reject_constant,
        )
    except ValidationError:
        raise
    except json.JSONDecodeError as error:
        raise ValidationError(f"{label}: invalid JSON: {error}") from error
    _walk_strict(value)
    return value


def strict_load(path: Path) -> Any:
    return strict_json_bytes(path.read_bytes(), str(path))


# ---- BLAKE3, from the public specification -------------------------------

IV = [0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
      0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19]
MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
CHUNK_START = 1
CHUNK_END = 2
PARENT = 4
ROOT = 8
DERIVE_KEY_CONTEXT = 32
DERIVE_KEY_MATERIAL = 64
BLOCK_LEN = 64
CHUNK_LEN = 1024
MASK32 = 0xFFFFFFFF


def _rotr32(value: int, count: int) -> int:
    return ((value >> count) | ((value << (32 - count)) & MASK32)) & MASK32


def _g(state: list[int], a: int, b: int, c: int, d: int, x: int, y: int) -> None:
    state[a] = (state[a] + state[b] + x) & MASK32
    state[d] = _rotr32(state[d] ^ state[a], 16)
    state[c] = (state[c] + state[d]) & MASK32
    state[b] = _rotr32(state[b] ^ state[c], 12)
    state[a] = (state[a] + state[b] + y) & MASK32
    state[d] = _rotr32(state[d] ^ state[a], 8)
    state[c] = (state[c] + state[d]) & MASK32
    state[b] = _rotr32(state[b] ^ state[c], 7)


def _round(state: list[int], words: list[int]) -> None:
    _g(state, 0, 4, 8, 12, words[0], words[1])
    _g(state, 1, 5, 9, 13, words[2], words[3])
    _g(state, 2, 6, 10, 14, words[4], words[5])
    _g(state, 3, 7, 11, 15, words[6], words[7])
    _g(state, 0, 5, 10, 15, words[8], words[9])
    _g(state, 1, 6, 11, 12, words[10], words[11])
    _g(state, 2, 7, 8, 13, words[12], words[13])
    _g(state, 3, 4, 9, 14, words[14], words[15])


def _compress(cv: list[int], block_words: list[int], counter: int,
              block_len: int, flags: int) -> list[int]:
    state = list(cv) + IV[:4] + [counter & MASK32, (counter >> 32) & MASK32,
                                block_len, flags]
    words = list(block_words)
    for round_index in range(7):
        _round(state, words)
        if round_index != 6:
            words = [words[index] for index in MSG_PERMUTATION]
    return [
        *((state[index] ^ state[index + 8]) & MASK32 for index in range(8)),
        *((state[index + 8] ^ cv[index]) & MASK32 for index in range(8)),
    ]


def _block_words(block: bytes) -> list[int]:
    padded = block + bytes(BLOCK_LEN - len(block))
    return [int.from_bytes(padded[offset:offset + 4], "little")
            for offset in range(0, BLOCK_LEN, 4)]


class _Output:
    def __init__(self, input_cv: list[int], block_words: list[int], counter: int,
                 block_len: int, flags: int) -> None:
        self.input_cv = list(input_cv)
        self.block_words = list(block_words)
        self.counter = counter
        self.block_len = block_len
        self.flags = flags

    def chaining_value(self) -> list[int]:
        return _compress(self.input_cv, self.block_words, self.counter,
                         self.block_len, self.flags)[:8]

    def root_bytes(self, length: int) -> bytes:
        output = bytearray()
        output_block_counter = 0
        while len(output) < length:
            words = _compress(self.input_cv, self.block_words,
                              output_block_counter, self.block_len,
                              self.flags | ROOT)
            block = b"".join(word.to_bytes(4, "little") for word in words)
            output.extend(block[:length - len(output)])
            output_block_counter += 1
        return bytes(output)


class _ChunkState:
    def __init__(self, key_words: list[int], chunk_counter: int,
                 flags: int) -> None:
        self.cv = list(key_words)
        self.chunk_counter = chunk_counter
        self.flags = flags
        self.buffer = bytearray()
        self.blocks_compressed = 0

    def length(self) -> int:
        return self.blocks_compressed * BLOCK_LEN + len(self.buffer)

    def _start_flag(self) -> int:
        return CHUNK_START if self.blocks_compressed == 0 else 0

    def update(self, data: bytes) -> None:
        position = 0
        while position < len(data):
            if len(self.buffer) == BLOCK_LEN:
                self.cv = _compress(
                    self.cv, _block_words(bytes(self.buffer)),
                    self.chunk_counter, BLOCK_LEN,
                    self.flags | self._start_flag(),
                )[:8]
                self.blocks_compressed += 1
                self.buffer.clear()
            take = min(BLOCK_LEN - len(self.buffer), len(data) - position)
            self.buffer.extend(data[position:position + take])
            position += take

    def output(self) -> _Output:
        return _Output(
            self.cv, _block_words(bytes(self.buffer)), self.chunk_counter,
            len(self.buffer), self.flags | self._start_flag() | CHUNK_END,
        )


def _parent_output(left_cv: list[int], right_cv: list[int],
                   key_words: list[int], flags: int) -> _Output:
    return _Output(key_words, left_cv + right_cv, 0, BLOCK_LEN, flags | PARENT)


def _blake3_internal(data: bytes, key_words: list[int], flags: int,
                     output_len: int = 32) -> bytes:
    state = _ChunkState(key_words, 0, flags)
    cv_stack: list[list[int]] = []
    position = 0
    while position < len(data):
        if state.length() == CHUNK_LEN:
            new_cv = state.output().chaining_value()
            total_chunks = state.chunk_counter + 1
            while total_chunks & 1 == 0:
                new_cv = _parent_output(cv_stack.pop(), new_cv,
                                        key_words, flags).chaining_value()
                total_chunks >>= 1
            cv_stack.append(new_cv)
            state = _ChunkState(key_words, state.chunk_counter + 1, flags)
        take = min(CHUNK_LEN - state.length(), len(data) - position)
        state.update(data[position:position + take])
        position += take
    output = state.output()
    while cv_stack:
        output = _parent_output(cv_stack.pop(), output.chaining_value(),
                                key_words, flags)
    return output.root_bytes(output_len)


def blake3_hash(data: bytes) -> bytes:
    return _blake3_internal(data, IV, 0)


def blake3_derive(context: str, data: bytes) -> bytes:
    context_key = _blake3_internal(context.encode("utf-8"), IV,
                                   DERIVE_KEY_CONTEXT)
    key_words = [int.from_bytes(context_key[offset:offset + 4], "little")
                 for offset in range(0, 32, 4)]
    return _blake3_internal(data, key_words, DERIVE_KEY_MATERIAL)


def digest_text(raw: bytes) -> str:
    return "blake3:" + raw.hex()


# ---- Exact descriptor transcript and canonical normalization -------------

def uleb128(value: int) -> bytes:
    if value < 0:
        raise ValidationError("ULEB128 input must be non-negative")
    output = bytearray()
    while True:
        low = value & 0x7F
        value >>= 7
        output.append(low if value == 0 else low | 0x80)
        if value == 0:
            return bytes(output)


def encode_string(value: str) -> bytes:
    encoded = value.encode("utf-8")
    return uleb128(len(encoded)) + encoded


EXPOSURE_TAG = {"hidden": 0, "catalog": 1, "catalog_and_runtime": 2}
HOT_RELOAD_TAG = {"restart_required": 0, "replace_definition": 1,
                  "update_live_handle": 2}


def descriptor_transcript(descriptor: dict[str, Any]) -> bytes:
    nominal = descriptor["type_id"]
    capabilities = descriptor["capabilities"]
    lowering = descriptor["lowering"]
    output = bytearray()
    output += encode_string(nominal["package"])
    output += encode_string(nominal["module"])
    output += encode_string(nominal["name"])
    output += encode_string(descriptor["public_id_family"])
    output += encode_string(descriptor["family_group"])
    output += encode_string(descriptor["body_schema"])
    handle = capabilities.get("runtime_handle_kind")
    if handle is None:
        output.append(0)
    else:
        output.append(1)
        output += encode_string(handle)
    output.append(EXPOSURE_TAG[capabilities["agent_exposure"]])
    output.append(1 if capabilities["save_definition_reference"] else 0)
    output.append(HOT_RELOAD_TAG[capabilities["hot_reload"]])
    output += encode_string(lowering["codec_id"])
    output += int(lowering["codec_version"]).to_bytes(4, "little")
    output += encode_string(lowering["section_id"])
    output += int(lowering["section_version"]).to_bytes(4, "little")
    return bytes(output)


def descriptor_digest(descriptor: dict[str, Any]) -> str:
    return digest_text(blake3_derive(DESCRIPTOR_CONTEXT,
                                    descriptor_transcript(descriptor)))


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(",", ":")).encode("utf-8")


def canonical_locale(value: str) -> str:
    if (not value or len(value.encode("utf-8")) > 64 or not value.isascii()
            or any(ord(ch) < 32 or ord(ch) == 127 for ch in value)):
        raise ValidationError(f"invalid locale {value!r}")
    parts = value.split("-")
    if any(not part or len(part) > 8 or not part.isalnum() for part in parts):
        raise ValidationError(f"invalid locale {value!r}")
    if not 2 <= len(parts[0]) <= 8 or not parts[0].isalpha():
        raise ValidationError(f"invalid locale language {value!r}")
    seen: set[str] = set()
    result: list[str] = []
    for index, part in enumerate(parts):
        lower = part.lower()
        if index != 0:
            if lower in seen:
                raise ValidationError(f"duplicate locale subtag in {value!r}")
            seen.add(lower)
        if index == 0:
            result.append(lower)
        elif len(part) == 4 and part.isalpha():
            result.append(lower[0].upper() + lower[1:])
        elif ((len(part) == 2 and part.isalpha())
              or (len(part) == 3 and part.isdigit())):
            result.append(part.upper())
        else:
            result.append(lower)
    return "-".join(result)


def normalize_scalar_value(value: dict[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(value)
    if result["kind"] == "locale":
        result["value"] = canonical_locale(result["value"])
    return result


def normalize_const(value: dict[str, Any]) -> dict[str, Any]:
    kind = value["kind"]
    result: dict[str, Any] = {"kind": kind}
    if kind == "scalar":
        result["value"] = normalize_scalar_value(value["value"])
    elif kind == "option":
        if "value" in value:
            result["value"] = normalize_const(value["value"])
    elif kind == "list":
        result["value"] = [normalize_const(item) for item in value["value"]]
    elif kind == "ordered_map":
        entries = [
            {"key": normalize_const(entry["key"]),
             "value": normalize_const(entry["value"])}
            for entry in value["value"]
        ]
        entries.sort(key=lambda entry: canonical_json_bytes(entry["key"]))
        result["value"] = entries
    elif kind == "record":
        content = value["value"]
        result["value"] = {
            "schema_id": content["schema_id"],
            "fields": sorted(
                [{"field_id": field["field_id"],
                  "value": normalize_const(field["value"])}
                 for field in content["fields"]],
                key=lambda field: field["field_id"],
            ),
        }
    elif kind == "enum":
        content = value["value"]
        normalized_content: dict[str, Any] = {
            "schema_id": content["schema_id"],
            "variant_id": content["variant_id"],
        }
        if "payload" in content:
            normalized_content["payload"] = normalize_const(content["payload"])
        result["value"] = normalized_content
    elif kind in {"asset_ref", "resource_ref", "retained_identity_ref"}:
        result["value"] = copy.deepcopy(value["value"])
    else:
        raise ValidationError(f"unknown constant kind {kind}")
    return result


def normalize_value_type(value: dict[str, Any]) -> dict[str, Any]:
    kind = value["kind"]
    result: dict[str, Any] = {"kind": kind}
    if kind == "scalar":
        result["value"] = value["value"]
    elif kind in {"option", "list", "non_empty_list"}:
        result["value"] = normalize_value_type(value["value"])
    elif kind == "ordered_map":
        result["value"] = {
            "key": normalize_value_type(value["value"]["key"]),
            "value": normalize_value_type(value["value"]["value"]),
        }
    elif kind in {"record", "enum"}:
        result["value"] = value["value"]
    elif kind == "asset_ref":
        result["value"] = {"payload_kind": value["value"]["payload_kind"]}
    elif kind == "resource_ref":
        result["value"] = {"type_id": copy.deepcopy(value["value"]["type_id"])}
    elif kind == "retained_identity_ref":
        result["value"] = value["value"]
    elif kind == "constrained_scalar":
        content = value["value"]
        normalized: dict[str, Any] = {"scalar": content["scalar"]}
        for edge in ("lower", "upper"):
            if edge in content:
                normalized[edge] = {
                    "kind": content[edge]["kind"],
                    "value": normalize_scalar_value(content[edge]["value"]),
                }
        result["value"] = normalized
    else:
        raise ValidationError(f"unknown value-type kind {kind}")
    return result


def nominal_key(value: dict[str, str]) -> tuple[bytes, bytes, bytes]:
    return tuple(value[field].encode("utf-8")
                 for field in ("package", "module", "name"))  # type: ignore[return-value]


def normalize_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    schemas: list[dict[str, Any]] = []
    for schema in manifest["schemas"]:
        kind = schema["kind"]
        content = schema["value"]
        normalized_content: dict[str, Any] = {
            "schema_id": content["schema_id"],
            "nominal_type": copy.deepcopy(content["nominal_type"]),
            "version": content["version"],
        }
        if kind == "record":
            fields = []
            for field in content["fields"]:
                normalized_field: dict[str, Any] = {
                    "field_id": field["field_id"],
                    "name": field["name"],
                    "value_type": normalize_value_type(field["value_type"]),
                    "presence": field["presence"],
                }
                if "default" in field:
                    normalized_field["default"] = normalize_const(field["default"])
                if field.get("docs", ""):
                    normalized_field["docs"] = field["docs"]
                fields.append(normalized_field)
            normalized_content["fields"] = sorted(
                fields,
                key=lambda item: (item["field_id"], item["name"].encode("utf-8")),
            )
        elif kind == "enum":
            variants = []
            for variant in content["variants"]:
                normalized_variant: dict[str, Any] = {
                    "variant_id": variant["variant_id"], "name": variant["name"]}
                if "payload" in variant:
                    normalized_variant["payload"] = normalize_value_type(variant["payload"])
                if variant.get("docs", ""):
                    normalized_variant["docs"] = variant["docs"]
                variants.append(normalized_variant)
            normalized_content["variants"] = sorted(
                variants,
                key=lambda item: (item["variant_id"], item["name"].encode("utf-8")),
            )
        else:
            raise ValidationError(f"unknown resource schema kind {kind}")
        schemas.append({"kind": kind, "value": normalized_content})
    schemas.sort(key=lambda schema: schema["value"]["schema_id"].encode("utf-8"))

    resource_types = []
    for descriptor in manifest["resource_types"]:
        normalized: dict[str, Any] = {
            "type_id": copy.deepcopy(descriptor["type_id"]),
            "public_id_family": descriptor["public_id_family"],
            "family_group": descriptor["family_group"],
            "body_schema": descriptor["body_schema"],
            "capabilities": copy.deepcopy(descriptor["capabilities"]),
            "lowering": copy.deepcopy(descriptor["lowering"]),
        }
        if descriptor.get("docs", {}).get("summary", ""):
            normalized["docs"] = {"summary": descriptor["docs"]["summary"]}
        normalized["descriptor_digest"] = descriptor_digest(normalized)
        resource_types.append(normalized)
    resource_types.sort(key=lambda descriptor: nominal_key(descriptor["type_id"]))

    codecs = [{"codec_id": codec["codec_id"],
               "versions": sorted(codec["versions"])}
              for codec in manifest["codecs"]]
    codecs.sort(key=lambda codec: codec["codec_id"].encode("utf-8"))
    return {
        "format": FORMAT,
        "schema": 1,
        "package": copy.deepcopy(manifest["package"]),
        "schemas": schemas,
        "resource_types": resource_types,
        "codecs": codecs,
    }


# ---- Structural and semantic checks -------------------------------------

def exact_keys(value: Any, required: set[str], optional: set[str], path: str) -> None:
    if not isinstance(value, dict):
        raise ValidationError(f"{path} must be an object")
    keys = set(value)
    missing = required - keys
    unknown = keys - required - optional
    if missing:
        raise ValidationError(f"{path} missing keys: {sorted(missing)}")
    if unknown:
        raise ValidationError(f"{path} unknown keys: {sorted(unknown)}")


def nonzero_u32(value: Any, path: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not (1 <= value <= 0xFFFFFFFF):
        raise ValidationError(f"{path} must be a nonzero u32")
    return value


def i64(value: Any, path: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not (-(1 << 63) <= value < (1 << 63)):
        raise ValidationError(f"{path} must be an i64")
    return value


def u64(value: Any, path: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not (0 <= value < (1 << 64)):
        raise ValidationError(f"{path} must be a u64")
    return value


def stable_id(value: Any, path: str) -> str:
    if not isinstance(value, str) or not STABLE_ID_RE.fullmatch(value):
        raise ValidationError(f"{path} is not a canonical stable dotted ID")
    return value


def valid_arcweft_identifier(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    first, *rest = value
    return (first == "_" or first.isalpha()) and all(
        character == "_" or character.isalpha() or character.isascii() and character.isdigit()
        for character in rest
    )


def valid_module_path(value: Any) -> bool:
    return isinstance(value, str) and bool(value) and all(
        valid_arcweft_identifier(segment) for segment in value.split(".")
    )


def validate_nominal(value: Any, path: str) -> tuple[str, str, str]:
    exact_keys(value, {"package", "module", "name"}, set(), path)
    package = value["package"]
    module = value["module"]
    name = value["name"]
    if not isinstance(package, str) or not PACKAGE_ID_RE.fullmatch(package):
        raise ValidationError(f"{path}.package is invalid")
    if not isinstance(module, str) or not valid_module_path(module):
        raise ValidationError(f"{path}.module is invalid")
    if not isinstance(name, str) or not valid_arcweft_identifier(name):
        raise ValidationError(f"{path}.name is invalid")
    return package, module, name


def validate_entity_id_text(value: Any, path: str) -> str:
    if (not isinstance(value, str) or not value
            or any(character.isspace() or (ord(character) < 32 or ord(character) == 127) for character in value)):
        raise ValidationError(f"{path} is invalid EntityId")
    return value


def validate_public_id_text(value: Any, path: str) -> str:
    if (not isinstance(value, str) or not value or value.startswith(("#", "@"))
            or any(character.isspace() or (ord(character) < 32 or ord(character) == 127) for character in value)):
        raise ValidationError(f"{path} is invalid PublicId")
    first = re.split(r"[.:]", value, maxsplit=1)[0]
    if first in {"arcweft", "__arcweft", "builtin", "core", "std"}:
        raise ValidationError(f"{path} uses reserved PublicId prefix")
    return value


def validate_scalar(value: Any, path: str) -> str:
    exact_keys(value, {"kind"}, {"value"}, path)
    kind = value["kind"]
    if kind not in EXPECTED_SCALARS:
        raise ValidationError(f"{path}.kind unknown scalar {kind!r}")
    has = "value" in value
    if kind == "unit":
        if has:
            raise ValidationError(f"{path}: unit must not carry value")
        return kind
    if not has:
        raise ValidationError(f"{path}: {kind} requires value")
    payload = value["value"]
    if kind == "bool":
        if not isinstance(payload, bool):
            raise ValidationError(f"{path}.value must be bool")
    elif kind == "signed_integer":
        i64(payload, f"{path}.value")
    elif kind in {"unsigned_integer", "duration"}:
        u64(payload, f"{path}.value")
    elif kind == "float":
        if not isinstance(payload, str) or not FLOAT_BITS_RE.fullmatch(payload):
            raise ValidationError(f"{path}.value must be canonical f64 bits")
        bits = int(payload[2:], 16)
        number = struct.unpack(">d", bits.to_bytes(8, "big"))[0]
        if not math.isfinite(number):
            raise ValidationError(f"{path}.value is non-finite")
        if bits == 0x8000000000000000:
            raise ValidationError(f"{path}.value is non-canonical negative zero")
    elif kind == "string":
        if not isinstance(payload, str):
            raise ValidationError(f"{path}.value must be string")
        if len(payload.encode("utf-8")) > EXPECTED_LIMITS["string_bytes"]:
            raise ValidationError(f"{path}.value exceeds string budget")
    elif kind == "char":
        if not isinstance(payload, str) or len(payload) != 1:
            raise ValidationError(f"{path}.value must contain one Unicode scalar")
    elif kind == "ratio":
        if not isinstance(payload, int) or isinstance(payload, bool) or not (0 <= payload <= 1_000_000):
            raise ValidationError(f"{path}.value must be 0..=1000000")
    elif kind == "length":
        exact_keys(payload, {"milli_units", "unit"}, set(), f"{path}.value")
        i64(payload["milli_units"], f"{path}.value.milli_units")
        if payload["unit"] not in EXPECTED_LAYOUT_UNITS:
            raise ValidationError(f"{path}.value.unit unknown")
    elif kind == "gain":
        if not isinstance(payload, int) or isinstance(payload, bool) or not (-120_000 <= payload <= 24_000):
            raise ValidationError(f"{path}.value must be -120000..=24000")
    elif kind == "pan":
        if not isinstance(payload, int) or isinstance(payload, bool) or not (-1_000 <= payload <= 1_000):
            raise ValidationError(f"{path}.value must be -1000..=1000")
    elif kind == "locale":
        if not isinstance(payload, str):
            raise ValidationError(f"{path}.value must be locale string")
        canonical_locale(payload)
    elif kind == "public_id":
        validate_public_id_text(payload, f"{path}.value")
    return kind


def validate_value_type(value: Any, path: str, depth: int = 0) -> str:
    if depth > EXPECTED_LIMITS["nesting_depth"]:
        raise ValidationError(f"{path}: value-type nesting over limit")
    exact_keys(value, {"kind", "value"}, set(), path)
    kind = value["kind"]
    if kind not in EXPECTED_VALUE_TYPES:
        raise ValidationError(f"{path}: unknown value-type kind {kind!r}")
    payload = value["value"]
    if kind == "scalar":
        if payload not in EXPECTED_SCALARS:
            raise ValidationError(f"{path}.value unknown scalar type")
    elif kind in {"option", "list", "non_empty_list"}:
        validate_value_type(payload, f"{path}.value", depth + 1)
    elif kind == "ordered_map":
        exact_keys(payload, {"key", "value"}, set(), f"{path}.value")
        validate_value_type(payload["key"], f"{path}.value.key", depth + 1)
        validate_value_type(payload["value"], f"{path}.value.value", depth + 1)
    elif kind in {"record", "enum"}:
        stable_id(payload, f"{path}.value")
    elif kind == "asset_ref":
        exact_keys(payload, {"payload_kind"}, set(), f"{path}.value")
        stable_id(payload["payload_kind"], f"{path}.value.payload_kind")
    elif kind == "resource_ref":
        exact_keys(payload, {"type_id"}, set(), f"{path}.value")
        validate_nominal(payload["type_id"], f"{path}.value.type_id")
    elif kind == "retained_identity_ref":
        if payload not in EXPECTED_RETAINED:
            raise ValidationError(f"{path}.value unknown retained category")
    elif kind == "constrained_scalar":
        exact_keys(payload, {"scalar"}, {"lower", "upper"}, f"{path}.value")
        scalar = payload["scalar"]
        if scalar not in EXPECTED_SCALARS:
            raise ValidationError(f"{path}.value.scalar unknown")
        for edge in ("lower", "upper"):
            if edge in payload:
                bound = payload[edge]
                exact_keys(bound, {"kind", "value"}, set(), f"{path}.value.{edge}")
                if bound["kind"] not in {"inclusive", "exclusive"}:
                    raise ValidationError(f"{path}.value.{edge}.kind unknown")
                actual = validate_scalar(bound["value"], f"{path}.value.{edge}.value")
                if actual != scalar:
                    raise ValidationError(f"{path}.value.{edge} scalar mismatch")
    return kind


def validate_retained(value: Any, path: str) -> str:
    exact_keys(value, {"kind", "value"}, set(), path)
    kind = value["kind"]
    if kind not in EXPECTED_RETAINED:
        raise ValidationError(f"{path}: unknown retained kind {kind!r}")
    payload = value["value"]
    if kind in {"character", "view", "action", "layer", "signal"}:
        exact_keys(payload, {"entity_id"}, set(), f"{path}.value")
        validate_entity_id_text(payload["entity_id"], f"{path}.value.entity_id")
    elif kind == "presentation_target":
        exact_keys(payload, {"scope", "target_id"}, set(), f"{path}.value")
        scope = payload["scope"]
        exact_keys(scope, {"kind"}, {"value"}, f"{path}.value.scope")
        if scope["kind"] == "global":
            if "value" in scope:
                raise ValidationError(f"{path}: global scope must not carry value")
        elif scope["kind"] == "view":
            exact_keys(scope.get("value"), {"owner_view_entity_id"}, set(),
                       f"{path}.value.scope.value")
            validate_entity_id_text(scope["value"]["owner_view_entity_id"],
                                    f"{path}.value.scope.value.owner_view_entity_id")
        else:
            raise ValidationError(f"{path}: unknown presentation scope")
        validate_public_id_text(payload["target_id"], f"{path}.value.target_id")
    elif kind == "scroll_region":
        exact_keys(payload, {"owner_view_entity_id", "region_id"}, set(),
                   f"{path}.value")
        validate_entity_id_text(payload["owner_view_entity_id"],
                                f"{path}.value.owner_view_entity_id")
        validate_public_id_text(payload["region_id"], f"{path}.value.region_id")
    return kind


def validate_const(value: Any, path: str, depth: int = 0) -> str:
    if depth > EXPECTED_LIMITS["nesting_depth"]:
        raise ValidationError(f"{path}: constant nesting over limit")
    exact_keys(value, {"kind"}, {"value"}, path)
    kind = value["kind"]
    if kind not in EXPECTED_CONST_KINDS:
        raise ValidationError(f"{path}: unknown constant kind {kind!r}")
    if kind == "option":
        if "value" in value:
            validate_const(value["value"], f"{path}.value", depth + 1)
        return kind
    if "value" not in value:
        raise ValidationError(f"{path}: {kind} constant requires value")
    payload = value["value"]
    if kind == "scalar":
        validate_scalar(payload, f"{path}.value")
    elif kind == "list":
        if not isinstance(payload, list):
            raise ValidationError(f"{path}.value must be array")
        if len(payload) > EXPECTED_LIMITS["collection_items"]:
            raise ValidationError(f"{path}.value exceeds collection budget")
        for index, item in enumerate(payload):
            validate_const(item, f"{path}.value[{index}]", depth + 1)
    elif kind == "ordered_map":
        if not isinstance(payload, list):
            raise ValidationError(f"{path}.value must be entry array")
        seen: set[bytes] = set()
        for index, entry in enumerate(payload):
            exact_keys(entry, {"key", "value"}, set(), f"{path}.value[{index}]")
            validate_const(entry["key"], f"{path}.value[{index}].key", depth + 1)
            validate_const(entry["value"], f"{path}.value[{index}].value", depth + 1)
            key = canonical_json_bytes(normalize_const(entry["key"]))
            if key in seen:
                raise ValidationError(f"{path}: duplicate normalized map key")
            seen.add(key)
    elif kind == "record":
        exact_keys(payload, {"schema_id", "fields"}, set(), f"{path}.value")
        stable_id(payload["schema_id"], f"{path}.value.schema_id")
        seen_fields: set[int] = set()
        for index, field in enumerate(payload["fields"]):
            exact_keys(field, {"field_id", "value"}, set(), f"{path}.value.fields[{index}]")
            field_id = nonzero_u32(field["field_id"], f"{path}.value.fields[{index}].field_id")
            if field_id in seen_fields:
                raise ValidationError(f"{path}: duplicate record field ID")
            seen_fields.add(field_id)
            validate_const(field["value"], f"{path}.value.fields[{index}].value", depth + 1)
    elif kind == "enum":
        exact_keys(payload, {"schema_id", "variant_id"}, {"payload"}, f"{path}.value")
        stable_id(payload["schema_id"], f"{path}.value.schema_id")
        nonzero_u32(payload["variant_id"], f"{path}.value.variant_id")
        if "payload" in payload:
            validate_const(payload["payload"], f"{path}.value.payload", depth + 1)
    elif kind == "asset_ref":
        exact_keys(payload, {"public_id", "payload_kind"}, set(), f"{path}.value")
        validate_public_id_text(payload["public_id"], f"{path}.value.public_id")
        stable_id(payload["payload_kind"], f"{path}.value.payload_kind")
    elif kind == "resource_ref":
        exact_keys(payload, {"entity_id", "public_id", "type_id"}, set(), f"{path}.value")
        validate_entity_id_text(payload["entity_id"], f"{path}.value.entity_id")
        validate_public_id_text(payload["public_id"], f"{path}.value.public_id")
        validate_nominal(payload["type_id"], f"{path}.value.type_id")
    elif kind == "retained_identity_ref":
        validate_retained(payload, f"{path}.value")
    return kind


def validate_manifest(manifest: Any, label: str) -> None:
    exact_keys(manifest,
               {"format", "schema", "package", "schemas", "resource_types", "codecs"},
               set(), label)
    if manifest["format"] != FORMAT:
        raise ValidationError(f"{label}.format mismatch")
    if manifest["schema"] != 1 or isinstance(manifest["schema"], bool):
        raise ValidationError(f"{label}.schema must be integer 1")
    package = manifest["package"]
    exact_keys(package, {"id", "version"}, set(), f"{label}.package")
    if not isinstance(package["id"], str) or not PACKAGE_ID_RE.fullmatch(package["id"]):
        raise ValidationError(f"{label}.package.id invalid")
    if not isinstance(package["version"], str) or not SEMVER_RE.fullmatch(package["version"]):
        raise ValidationError(f"{label}.package.version invalid semver")
    for collection in ("schemas", "resource_types", "codecs"):
        if not isinstance(manifest[collection], list):
            raise ValidationError(f"{label}.{collection} must be array")
        if len(manifest[collection]) > EXPECTED_LIMITS["collection_items"]:
            raise ValidationError(f"{label}.{collection} exceeds budget")

    schema_by_id: dict[str, dict[str, Any]] = {}
    nominal_schemas: set[tuple[str, str, str]] = set()
    for index, schema in enumerate(manifest["schemas"]):
        path = f"{label}.schemas[{index}]"
        exact_keys(schema, {"kind", "value"}, set(), path)
        if schema["kind"] not in {"record", "enum"}:
            raise ValidationError(f"{path}.kind unknown")
        content = schema["value"]
        required = {"schema_id", "nominal_type", "version",
                    "fields" if schema["kind"] == "record" else "variants"}
        exact_keys(content, required, set(), f"{path}.value")
        schema_id = stable_id(content["schema_id"], f"{path}.value.schema_id")
        if schema_id in schema_by_id:
            raise ValidationError(f"{label}: duplicate schema {schema_id}")
        schema_by_id[schema_id] = schema
        nominal = validate_nominal(content["nominal_type"], f"{path}.value.nominal_type")
        if nominal[0] != package["id"]:
            raise ValidationError(f"{path}.value.nominal_type package differs from document package")
        if nominal in nominal_schemas:
            raise ValidationError(f"{label}: duplicate nominal schema {nominal}")
        nominal_schemas.add(nominal)
        nonzero_u32(content["version"], f"{path}.value.version")
        if schema["kind"] == "record":
            seen_ids: set[int] = set()
            seen_names: set[str] = set()
            for field_index, field in enumerate(content["fields"]):
                fp = f"{path}.value.fields[{field_index}]"
                exact_keys(field, {"field_id", "name", "value_type", "presence"},
                           {"default", "docs"}, fp)
                field_id = nonzero_u32(field["field_id"], f"{fp}.field_id")
                if field_id in seen_ids:
                    raise ValidationError(f"{path}: duplicate field ID")
                seen_ids.add(field_id)
                if not isinstance(field["name"], str) or not valid_arcweft_identifier(field["name"]):
                    raise ValidationError(f"{fp}.name invalid")
                if field["name"] in seen_names:
                    raise ValidationError(f"{path}: duplicate field name")
                seen_names.add(field["name"])
                validate_value_type(field["value_type"], f"{fp}.value_type")
                if field["presence"] not in {"required", "optional"}:
                    raise ValidationError(f"{fp}.presence unknown")
                if field["presence"] == "required" and "default" in field:
                    raise ValidationError(f"{fp}: required field must not have default")
                if "default" in field:
                    validate_const(field["default"], f"{fp}.default")
                if "docs" in field and not isinstance(field["docs"], str):
                    raise ValidationError(f"{fp}.docs must be string")
        else:
            seen_ids = set()
            seen_names = set()
            for variant_index, variant in enumerate(content["variants"]):
                vp = f"{path}.value.variants[{variant_index}]"
                exact_keys(variant, {"variant_id", "name"}, {"payload", "docs"}, vp)
                variant_id = nonzero_u32(variant["variant_id"], f"{vp}.variant_id")
                if variant_id in seen_ids:
                    raise ValidationError(f"{path}: duplicate variant ID")
                seen_ids.add(variant_id)
                if not isinstance(variant["name"], str) or not valid_arcweft_identifier(variant["name"]):
                    raise ValidationError(f"{vp}.name invalid")
                if variant["name"] in seen_names:
                    raise ValidationError(f"{path}: duplicate variant name")
                seen_names.add(variant["name"])
                if "payload" in variant:
                    validate_value_type(variant["payload"], f"{vp}.payload")

    codec_by_id: dict[str, set[int]] = {}
    for index, codec in enumerate(manifest["codecs"]):
        path = f"{label}.codecs[{index}]"
        exact_keys(codec, {"codec_id", "versions"}, set(), path)
        codec_id = stable_id(codec["codec_id"], f"{path}.codec_id")
        if codec_id in codec_by_id:
            raise ValidationError(f"{label}: duplicate codec {codec_id}")
        if not isinstance(codec["versions"], list) or not codec["versions"]:
            raise ValidationError(f"{path}.versions must be non-empty array")
        versions = {nonzero_u32(version, f"{path}.versions") for version in codec["versions"]}
        if len(versions) != len(codec["versions"]):
            raise ValidationError(f"{path}: duplicate codec version")
        codec_by_id[codec_id] = versions

    type_ids: set[tuple[str, str, str]] = set()
    for index, descriptor in enumerate(manifest["resource_types"]):
        path = f"{label}.resource_types[{index}]"
        exact_keys(descriptor,
                   {"type_id", "public_id_family", "family_group", "body_schema",
                    "capabilities", "lowering", "descriptor_digest"},
                   {"docs"}, path)
        type_id = validate_nominal(descriptor["type_id"], f"{path}.type_id")
        if type_id in type_ids:
            raise ValidationError(f"{label}: duplicate resource type {type_id}")
        type_ids.add(type_id)
        if type_id[0] != package["id"]:
            raise ValidationError(f"{path}.type_id package differs from document package")
        if (not isinstance(descriptor["public_id_family"], str)
                or "." in descriptor["public_id_family"]
                or not FAMILY_RE.fullmatch(descriptor["public_id_family"])):
            raise ValidationError(f"{path}.public_id_family invalid")
        stable_id(descriptor["family_group"], f"{path}.family_group")
        body_id = stable_id(descriptor["body_schema"], f"{path}.body_schema")
        body = schema_by_id.get(body_id)
        if body is None or body["kind"] != "record":
            raise ValidationError(f"{path}.body_schema must name a record schema")
        body_nominal = validate_nominal(body["value"]["nominal_type"], f"{path}.body_schema.nominal")
        if body_nominal != type_id:
            raise ValidationError(f"{path}: body schema nominal type mismatch")
        capabilities = descriptor["capabilities"]
        exact_keys(capabilities,
                   {"agent_exposure", "save_definition_reference", "hot_reload"},
                   {"runtime_handle_kind"}, f"{path}.capabilities")
        if capabilities["agent_exposure"] not in EXPOSURE_TAG:
            raise ValidationError(f"{path}.capabilities.agent_exposure unknown")
        if capabilities["hot_reload"] not in HOT_RELOAD_TAG:
            raise ValidationError(f"{path}.capabilities.hot_reload unknown")
        if not isinstance(capabilities["save_definition_reference"], bool):
            raise ValidationError(f"{path}.capabilities.save_definition_reference not bool")
        handle = capabilities.get("runtime_handle_kind")
        if handle is not None:
            stable_id(handle, f"{path}.capabilities.runtime_handle_kind")
        if (capabilities["agent_exposure"] == "catalog_and_runtime" or
                capabilities["hot_reload"] == "update_live_handle") and handle is None:
            raise ValidationError(f"{path}: runtime capability requires handle kind")
        lowering = descriptor["lowering"]
        exact_keys(lowering,
                   {"codec_id", "codec_version", "section_id", "section_version"},
                   set(), f"{path}.lowering")
        codec_id = stable_id(lowering["codec_id"], f"{path}.lowering.codec_id")
        codec_version = nonzero_u32(lowering["codec_version"], f"{path}.lowering.codec_version")
        stable_id(lowering["section_id"], f"{path}.lowering.section_id")
        nonzero_u32(lowering["section_version"], f"{path}.lowering.section_version")
        if codec_id not in codec_by_id or codec_version not in codec_by_id[codec_id]:
            raise ValidationError(f"{path}: selected codec/version unsupported")
        if not isinstance(descriptor["descriptor_digest"], str) or not DIGEST_RE.fullmatch(descriptor["descriptor_digest"]):
            raise ValidationError(f"{path}.descriptor_digest malformed")
        if descriptor_digest(descriptor) != descriptor["descriptor_digest"]:
            raise ValidationError(f"{path}.descriptor_digest mismatch")
        if "docs" in descriptor:
            exact_keys(descriptor["docs"], {"summary"}, set(), f"{path}.docs")


def collect_coverage(value: Any, result: dict[str, set[str]]) -> None:
    if isinstance(value, list):
        for item in value:
            collect_coverage(item, result)
    elif isinstance(value, dict):
        kind = value.get("kind")
        if kind in EXPECTED_VALUE_TYPES and "value" in value:
            # Distinguish scalar values from value-type scalar by payload shape.
            if kind != "scalar" or isinstance(value["value"], str):
                result["value_types"].add(kind)
                if kind == "scalar" and value["value"] in EXPECTED_SCALARS:
                    result["scalar_types"].add(value["value"])
                if kind == "retained_identity_ref" and isinstance(value["value"], str):
                    result["retained_types"].add(value["value"])
        if kind in EXPECTED_CONST_KINDS:
            # Constants are identifiable because scalar's payload is an object,
            # option may omit value, and composite payloads are not type shapes.
            if kind != "scalar" or isinstance(value.get("value"), dict):
                result["const_kinds"].add(kind)
                if kind == "scalar" and isinstance(value.get("value"), dict):
                    scalar_kind = value["value"].get("kind")
                    if scalar_kind in EXPECTED_SCALARS:
                        result["scalar_values"].add(scalar_kind)
                if kind == "retained_identity_ref" and isinstance(value.get("value"), dict):
                    retained_kind = value["value"].get("kind")
                    if retained_kind in EXPECTED_RETAINED:
                        result["retained_values"].add(retained_kind)
                        if retained_kind == "presentation_target":
                            scope = value["value"].get("value", {}).get("scope", {}).get("kind")
                            if scope in {"global", "view"}:
                                result["presentation_scopes"].add(scope)
        for child in value.values():
            collect_coverage(child, result)


def contains_exact_pattern(value: Any, expected: str) -> bool:
    if isinstance(value, dict):
        return value.get("pattern") == expected or any(
            contains_exact_pattern(child, expected) for child in value.values()
        )
    if isinstance(value, list):
        return any(contains_exact_pattern(child, expected) for child in value)
    return False


def contains_exact_enum(value: Any, expected: set[str]) -> bool:
    if isinstance(value, dict):
        enum = value.get("enum")
        if isinstance(enum, list) and set(enum) == expected and len(enum) == len(expected):
            return True
        return any(contains_exact_enum(child, expected) for child in value.values())
    if isinstance(value, list):
        return any(contains_exact_enum(child, expected) for child in value)
    return False


def validate_manifest_hashes(root: Path, reporter: Reporter) -> None:
    manifest_path = root / "MANIFEST.sha256"
    reporter.check(manifest_path.is_file(), "MANIFEST.sha256 exists")
    expected: dict[str, str] = {}
    for number, line in enumerate(manifest_path.read_text("utf-8").splitlines(), 1):
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if not match:
            raise ValidationError(f"MANIFEST.sha256 line {number} malformed")
        digest, relative = match.groups()
        if relative in expected:
            raise ValidationError(f"MANIFEST.sha256 duplicates {relative}")
        expected[relative] = digest
    actual_files = {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() and path.name != "MANIFEST.sha256"
    }
    reporter.check(set(expected) == actual_files,
                   "MANIFEST.sha256 lists every package file exactly once")
    for relative, digest in sorted(expected.items()):
        actual = hashlib.sha256((root / relative).read_bytes()).hexdigest()
        if actual != digest:
            raise ValidationError(f"SHA-256 mismatch for {relative}")
    reporter.check(True, f"verified {len(expected)} package SHA-256 entries")


def expect_validation_failure(action: Callable[[], Any], label: str, reporter: Reporter) -> None:
    try:
        action()
    except (ValidationError, UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError, ValueError, OverflowError):
        reporter.check(True, label)
        return
    raise ValidationError(f"expected validation failure did not occur: {label}")


def run_validator_self_tests(minimal: dict[str, Any], reporter: Reporter) -> None:
    expect_validation_failure(
        lambda: strict_json_bytes(b'{"a":1,"a":2}', "duplicate-self-test"),
        "strict JSON rejects duplicate keys", reporter)
    expect_validation_failure(
        lambda: strict_json_bytes(b'{"a":1.25}', "float-token-self-test"),
        "strict JSON rejects floating number tokens", reporter)
    expect_validation_failure(
        lambda: strict_json_bytes(b'{"a":null}', "null-self-test"),
        "strict JSON rejects explicit null", reporter)
    expect_validation_failure(
        lambda: strict_json_bytes(b'\xff', "utf8-self-test"),
        "strict JSON rejects invalid UTF-8", reporter)
    expect_validation_failure(
        lambda: validate_scalar({"kind": "float", "value": "0x8000000000000000"}, "negative-zero"),
        "finite-float decoder rejects negative zero bits", reporter)
    expect_validation_failure(
        lambda: validate_scalar({"kind": "float", "value": "0x7ff0000000000000"}, "infinity"),
        "finite-float decoder rejects infinity bits", reporter)
    expect_validation_failure(
        lambda: validate_scalar({"kind": "signed_integer", "value": 1 << 63}, "i64-overflow"),
        "signed integer overflow is rejected", reporter)
    expect_validation_failure(
        lambda: validate_scalar({"kind": "unsigned_integer", "value": 1 << 64}, "u64-overflow"),
        "unsigned integer overflow is rejected", reporter)
    validate_scalar({"kind": "char", "value": "😀"}, "non-bmp-char")
    reporter.check(True, "char accepts exactly one non-BMP Unicode scalar")
    expect_validation_failure(
        lambda: validate_scalar({"kind": "char", "value": "ab"}, "two-char"),
        "char rejects two Unicode scalars", reporter)
    validate_scalar({"kind": "gain", "value": -120_000}, "gain-min")
    validate_scalar({"kind": "gain", "value": 24_000}, "gain-max")
    validate_scalar({"kind": "pan", "value": -1_000}, "pan-min")
    validate_scalar({"kind": "pan", "value": 1_000}, "pan-max")
    reporter.check(True, "gain and pan inclusive repository bounds are admitted")
    expect_validation_failure(
        lambda: validate_scalar({"kind": "gain", "value": 24_001}, "gain-over"),
        "one-over gain bound is rejected", reporter)
    expect_validation_failure(
        lambda: validate_scalar({"kind": "pan", "value": 1_001}, "pan-over"),
        "one-over pan bound is rejected", reporter)
    validate_scalar({"kind": "string", "value": "x" * EXPECTED_LIMITS["string_bytes"]},
                    "string-inclusive")
    reporter.check(True, "inclusive string-byte budget is admitted")
    expect_validation_failure(
        lambda: validate_scalar({"kind": "string",
                                 "value": "x" * (EXPECTED_LIMITS["string_bytes"] + 1)},
                                "string-one-over"),
        "one-over string-byte budget is rejected", reporter)
    unit = {"kind": "scalar", "value": {"kind": "unit"}}
    validate_const({"kind": "list",
                    "value": [unit] * EXPECTED_LIMITS["collection_items"]},
                   "collection-inclusive")
    reporter.check(True, "inclusive collection budget is admitted")
    expect_validation_failure(
        lambda: validate_const({"kind": "list",
                                "value": [unit] * (EXPECTED_LIMITS["collection_items"] + 1)},
                               "collection-one-over"),
        "one-over collection budget is rejected", reporter)
    nested: dict[str, Any] = unit
    for _ in range(EXPECTED_LIMITS["nesting_depth"]):
        nested = {"kind": "option", "value": nested}
    validate_const(nested, "nesting-inclusive")
    reporter.check(True, "inclusive semantic nesting budget is admitted")
    nested = {"kind": "option", "value": nested}
    expect_validation_failure(
        lambda: validate_const(nested, "nesting-one-over"),
        "one-over semantic nesting budget is rejected", reporter)
    expect_validation_failure(
        lambda: stable_id("bad-", "stable-terminal"),
        "stable dotted ID rejects trailing punctuation", reporter)
    expect_validation_failure(
        lambda: validate_public_id_text("std.hidden", "reserved-public-id"),
        "PublicId rejects reserved prefixes", reporter)
    tampered = copy.deepcopy(minimal)
    tampered["resource_types"][0]["descriptor_digest"] = "blake3:" + "00" * 32
    expect_validation_failure(
        lambda: validate_manifest(tampered, "tampered-descriptor"),
        "forged descriptor digest is rejected", reporter)
    unsupported = copy.deepcopy(minimal)
    unsupported["schema"] = 2
    expect_validation_failure(
        lambda: validate_manifest(unsupported, "unsupported-schema"),
        "unsupported schema version is rejected without fallback", reporter)


def validate_optional_jsonschema(root: Path, manifests: list[tuple[str, Any]],
                                 reporter: Reporter) -> None:
    try:
        import jsonschema  # type: ignore
    except ImportError:
        reporter.note("jsonschema not installed; built-in checks remain authoritative")
        return
    schema = strict_load(root / "schema/resource-type-manifest-v1.schema.json")
    validator = jsonschema.Draft202012Validator(schema)
    for label, manifest in manifests:
        errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
        if errors:
            rendered = "; ".join(error.message for error in errors[:5])
            raise ValidationError(f"Draft 2020-12 validation failed for {label}: {rendered}")
    reporter.check(True, "Draft 2020-12 schema validates all four examples")


def validate_package(root: Path) -> Reporter:
    reporter = Reporter()
    reporter.check(root.is_dir(), "package root exists")
    for relative in REQUIRED_DOCS:
        reporter.check((root / relative).is_file(), f"required document exists: {relative}")

    status = strict_load(root / "STATUS.json")
    reporter.check(status["FINAL_CONTRACT"] is True, "FINAL_CONTRACT=true")
    reporter.check(status["FALLBACK"] is False, "FALLBACK=false")
    reporter.check(status["BLOCKED"] is False, "BLOCKED=false")
    reporter.check(status["OPEN_QUESTIONS"] == 0, "OPEN_QUESTIONS=0")
    reporter.check(status["IMPLEMENTATION_READY"] is True, "IMPLEMENTATION_READY=true")
    reporter.check(status["contract_agent_validated"] is True,
                   "contract_agent_validated=true")
    reporter.check(status["repository_contract_validation_succeeded"] is True,
                   "repository_contract_validation_succeeded=true")
    reporter.check(status["production_code_modified"] is False,
                   "production_code_modified=false")
    reporter.check(status["repository_build_commands_executed"] is False,
                   "repository build execution is not falsely claimed")
    reporter.check(status["pinned_revision"] == PIN,
                   "status uses the pinned main revision")
    reporter.check(status["validation_scope"]["not_claimed"],
                   "unexecuted Cargo/build scope is explicit")

    contract = strict_load(root / "CONTRACT.json")
    reporter.check(contract["revision"] == PIN, "CONTRACT.json revision matches pin")
    reporter.check(contract["limits"] == EXPECTED_LIMITS, "all eight exact limits match")
    reporter.check(contract["wire"]["aliases"] == [], "no compatibility aliases")
    reporter.check(contract["wire"]["fallback"] is False, "wire dispatch has no fallback")
    reporter.check(contract["bundle"] == {"code": 22, "kind": "ResourceTypeManifests",
                                          "required_when_present": True, "schema": 1},
                   "typed AWFB section decision is exact")
    reporter.check(contract["retained_tokens"] == [
        "character", "view", "action", "layer", "signal",
        "presentation_target", "scroll_region"],
        "retained tokens preserve repository enum order")

    pin = strict_load(root / "PINNED_REVISION.json")
    reporter.check(pin["resolved_commit"] == PIN and pin["origin_main_commit"] == PIN,
                   "main and origin/main pin are equal")
    reporter.check(pin["equal"] is True, "PINNED_REVISION.equal=true")
    reporter.check(all(item["read_to_end"] for item in pin["applicable_agents"]),
                   "all applicable AGENTS.md files were read to end")

    inventory = strict_load(root / "INPUT_INVENTORY.json")
    reporter.check(inventory["rust_skill"]["read_to_end"] is True,
                   "uploaded Rust Skill is inventoried as read to end")
    reporter.check(inventory["uploaded_request"]["read_to_end"] is True,
                   "uploaded request is inventoried as read to end")
    reporter.check(inventory["project_premise"]["read_to_end"] is True,
                   "project premise is inventoried as read to end")

    evidence = strict_load(root / "REPOSITORY_EVIDENCE.json")
    reporter.check(evidence["revision"] == PIN, "repository evidence uses the pin")
    reporter.check(len(evidence["files"]) >= 25, "repository evidence inventory is substantive")
    for entry in evidence["files"]:
        if not re.fullmatch(r"[0-9a-f]{40}", entry["blob_sha"]):
            raise ValidationError(f"invalid blob SHA in repository evidence: {entry}")
        if not entry["facts"]:
            raise ValidationError(f"repository evidence entry has no facts: {entry['path']}")
    reporter.check(True, "all repository evidence entries carry pinned blob SHAs and facts")

    examples: list[tuple[str, Any]] = []
    for name in ("minimal", "full"):
        input_path = root / f"examples/{name}.input.json"
        canonical_path = root / f"examples/{name}.canonical.json"
        authored = strict_load(input_path)
        canonical = strict_load(canonical_path)
        validate_manifest(authored, f"{name}.input")
        validate_manifest(canonical, f"{name}.canonical")
        normalized = normalize_manifest(authored)
        reporter.check(normalized == canonical,
                       f"{name}: decode/normalize semantic result equals canonical example")
        regenerated = canonical_json_bytes(normalized)
        reporter.check(regenerated == canonical_path.read_bytes(),
                       f"{name}: byte-for-byte canonical regeneration")
        reporter.check(canonical_json_bytes(canonical) == canonical_path.read_bytes(),
                       f"{name}: canonical file is compact UTF-8 with sorted keys")
        examples.extend([(f"{name}.input", authored), (f"{name}.canonical", canonical)])

    run_validator_self_tests(strict_load(root / "examples/minimal.canonical.json"), reporter)

    vectors = strict_load(root / "vectors/canonical-digests.json")
    self_tests = vectors["blake3_self_tests"]
    reporter.check(blake3_hash(b"").hex() == self_tests["empty"],
                   "BLAKE3 official empty vector")
    reporter.check(blake3_hash(b"abc").hex() == self_tests["abc"],
                   "BLAKE3 official abc vector")
    for vector in vectors["vectors"]:
        data = (root / f"examples/{vector['example']}.canonical.json").read_bytes()
        reporter.check(len(data) == vector["bytes"],
                       f"{vector['example']}: canonical byte length vector")
        reporter.check(digest_text(blake3_hash(data)) == vector["raw_digest"],
                       f"{vector['example']}: canonical raw digest vector")

    descriptor_vectors = strict_load(root / "vectors/descriptor-digests.json")
    descriptors: dict[tuple[str, str, str], dict[str, Any]] = {}
    for name in ("minimal", "full"):
        manifest = strict_load(root / f"examples/{name}.canonical.json")
        for descriptor in manifest["resource_types"]:
            key = tuple(descriptor["type_id"][field]
                        for field in ("package", "module", "name"))
            descriptors[key] = descriptor
    for vector in descriptor_vectors["vectors"]:
        key = tuple(vector["type_id"][field]
                    for field in ("package", "module", "name"))
        descriptor = descriptors.get(key)
        if descriptor is None:
            raise ValidationError(f"descriptor vector names unknown type {key}")
        reporter.check(vector["context"] == DESCRIPTOR_CONTEXT,
                       f"descriptor {key}: derive-key context")
        reporter.check(descriptor_transcript(descriptor).hex() == vector["transcript_hex"],
                       f"descriptor {key}: transcript bytes")
        reporter.check(descriptor_digest(descriptor) == vector["digest"],
                       f"descriptor {key}: semantic digest")
        reporter.check(descriptor["descriptor_digest"] == vector["digest"],
                       f"descriptor {key}: manifest claim recomputes")

    full = strict_load(root / "examples/full.input.json")
    coverage = {
        "scalar_types": set(), "scalar_values": set(), "value_types": set(),
        "const_kinds": set(), "retained_types": set(), "retained_values": set(),
        "presentation_scopes": set(),
    }
    collect_coverage(full, coverage)
    reporter.check(coverage["scalar_types"] == EXPECTED_SCALARS,
                   "full example covers every current scalar type")
    reporter.check(coverage["scalar_values"] == EXPECTED_SCALARS,
                   "full example covers every current scalar constant shape")
    reporter.check(coverage["value_types"] == EXPECTED_VALUE_TYPES,
                   "full example covers every current ResourceValueType variant")
    reporter.check(coverage["const_kinds"] == EXPECTED_CONST_KINDS,
                   "full example covers every current ResourceConstValue variant")
    reporter.check(coverage["retained_types"] == EXPECTED_RETAINED,
                   "full example covers every retained-reference type category")
    reporter.check(coverage["retained_values"] == EXPECTED_RETAINED,
                   "full example covers every resolved retained value category")
    reporter.check(coverage["presentation_scopes"] == {"global", "view"},
                   "full example covers global and View-scoped presentation targets")

    schema = strict_load(root / "schema/resource-type-manifest-v1.schema.json")
    reporter.check(contains_exact_enum(schema, EXPECTED_SCALARS),
                   "JSON Schema freezes the exact scalar token set")
    reporter.check(contains_exact_enum(schema, EXPECTED_RETAINED),
                   "JSON Schema freezes the exact retained token set")
    reporter.check(contains_exact_enum(schema, EXPECTED_LAYOUT_UNITS),
                   "JSON Schema freezes every current LayoutUnit token")
    reporter.check(contains_exact_pattern(schema, STABLE_ID_RE.pattern),
                   "JSON Schema uses the exact stable dotted-ID terminal rule")
    reporter.check(contains_exact_pattern(schema, FAMILY_RE.pattern),
                   "JSON Schema uses the exact public-family terminal rule")

    negative = strict_load(root / "vectors/negative-cases.json")
    cases = negative["cases"]
    reporter.check([case["id"] for case in cases] ==
                   [f"NEG-{index:03d}" for index in range(1, 32)],
                   "negative vector matrix is contiguous NEG-001..NEG-031")
    required_codes = {
        "resource_manifest.missing_format", "resource_manifest.malformed_format",
        "resource_manifest.unsupported_format", "resource_manifest.missing_schema_version",
        "resource_manifest.malformed_schema_version", "resource_manifest.unsupported_schema_version",
        "resource_manifest.duplicate_key", "resource_manifest.null_not_allowed",
        "resource_manifest.unknown_field", "resource_manifest.wrong_tag_content",
        "resource_manifest.descriptor_digest_mismatch", "resource_manifest.non_finite_float",
        "resource_manifest.non_canonical_float", "resource_manifest.integer_overflow",
        "resource_manifest.invalid_utf8", "resource_manifest.unknown_tag",
        "resource_manifest.invalid_id", "resource_manifest.registry_validation",
        "resource_manifest.duplicate_record", "resource_manifest.package_mismatch",
        "resource_manifest.version_conflict", "resource_manifest.byte_limit",
        "resource_manifest.depth_limit", "resource_manifest.string_limit",
        "resource_manifest.collection_limit", "resource_manifest.record_limit",
        "resource_manifest.work_limit",
    }
    reporter.check(required_codes <= {case["expected_code"] for case in cases},
                   "negative vectors cover every required diagnostic family")
    reporter.check(any("range_assertion" in case for case in cases),
                   "negative vectors include nested primary/related range assertion")

    validate_optional_jsonschema(root, examples, reporter)

    for relative in REQUIRED_DOCS:
        text = (root / relative).read_text("utf-8")
        for marker in ("TBD", "TODO", "CHANGEME", "<fill-me>"):
            if marker in text:
                raise ValidationError(f"placeholder {marker!r} in {relative}")
    final_text = (root / "FINAL_CONTRACT.md").read_text("utf-8")
    for required in (
        "OPEN_QUESTIONS=0", "IMPLEMENTATION_READY=true", "FALLBACK=false",
        "decode_resource_type_manifest", "encode_resource_type_manifest_v1",
        "ResourceTypeManifestFileV1", "PackageCoordinateFile",
        "ResourceTypeManifests", "atomic",
    ):
        reporter.check(required in final_text,
                       f"FINAL_CONTRACT.md closes required term: {required}")

    allowed_top = {
        "CONTRACT.json", "DIAGNOSTICS_AND_LIMITS.md", "DTO_AND_CONVERSION.md",
        "FINAL_CONTRACT.md", "IMPLEMENTATION_ORDER.md", "INPUT_INVENTORY.json",
        "MANIFEST.sha256", "NO_PRODUCTION_CHANGES.md", "OWNERSHIP_AND_DEPENDENCIES.md",
        "PACKAGE_AND_ARTIFACT_PUBLICATION.md", "PINNED_REVISION.json", "README.md",
        "RECONCILIATION_MATRIX.md", "REPOSITORY_EVIDENCE.json", "REPOSITORY_INVENTORY.md",
        "STATUS.json", "TEST_MATRIX.md", "VALIDATION.md", "WIRE_SCHEMA.md",
        "examples", "logs", "schema", "tools", "vectors",
    }
    reporter.check({path.name for path in root.iterdir()} <= allowed_top,
                   "archive contains only contract artifacts and validator material")
    reporter.check(not any(path.suffix == ".rs" for path in root.rglob("*")),
                   "archive contains no Rust production source")

    validate_manifest_hashes(root, reporter)
    return reporter


def main(argv: list[str]) -> int:
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path(__file__).resolve().parents[1]
    try:
        reporter = validate_package(root)
    except (ValidationError, KeyError, TypeError, ValueError, OverflowError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    for message in reporter.passes:
        print(f"PASS {message}")
    print(f"RESULT PASS checks={len(reporter.passes)} root={root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
