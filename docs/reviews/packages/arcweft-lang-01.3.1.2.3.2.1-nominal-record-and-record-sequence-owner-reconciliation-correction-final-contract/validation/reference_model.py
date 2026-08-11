from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Iterable

MAX_FIELD_ID = (1 << 32) - 1


class AdmissionError(Exception):
    def __init__(self, variant: str, **data: Any) -> None:
        self.variant = variant
        self.data = data
        super().__init__(f"{variant}:{data}")


@dataclass(frozen=True)
class FieldType:
    kind: str
    nominal: str | None = None
    layout: str | None = None

    def accepts(self, value: "Value") -> bool:
        if self.kind == "choice":
            return any(alt.accepts(value) for alt in value.choice_types)
        if self.kind == "nominal":
            return value.kind == "nominal" and value.nominal == self.nominal and value.layout == self.layout
        return value.kind == self.kind


@dataclass(frozen=True)
class Value:
    kind: str
    payload: Any = None
    nominal: str | None = None
    layout: str | None = None
    choice_types: tuple[FieldType, ...] = ()


@dataclass(frozen=True)
class LayoutField:
    name: str
    ty: FieldType


@dataclass(frozen=True)
class Layout:
    nominal: str
    semantic: str
    layout: str
    fields: tuple[LayoutField, ...]

    def field_id(self, ordinal: int) -> int | None:
        if ordinal < 0 or ordinal >= len(self.fields):
            return None
        return field_id(ordinal)

    def field_by_name(self, name: str) -> tuple[int, LayoutField] | None:
        for ordinal, field in enumerate(self.fields):
            if field.name == name:
                return field_id(ordinal), field
        return None


@dataclass(frozen=True)
class Initializer:
    field: int
    name: str
    expression: Any


@dataclass(frozen=True)
class NominalValue:
    nominal: str
    layout: str
    fields: tuple[Value, ...]


@dataclass(frozen=True)
class RecordColumn:
    field: int
    name: str
    values: tuple[Any, ...]


def field_id(zero_based: int) -> int:
    one_based = zero_based + 1
    if zero_based < 0 or one_based > MAX_FIELD_ID:
        raise AdmissionError("OrdinalOverflow", ordinal=zero_based)
    return one_based


def make_layout(nominal: str, semantic: str, layout: str, fields: Iterable[tuple[str, FieldType]]) -> Layout:
    fields = tuple(fields)
    if len(fields) > MAX_FIELD_ID:
        raise AdmissionError("TooManyFields", actual=len(fields), maximum=MAX_FIELD_ID)
    seen: set[str] = set()
    projected: list[LayoutField] = []
    for ordinal, (name, ty) in enumerate(fields):
        if name in seen:
            raise AdmissionError("DuplicateFieldName", name=name)
        seen.add(name)
        try:
            field_id(ordinal)
        except AdmissionError as error:
            raise AdmissionError("InvalidFieldIdentity", ordinal=ordinal, name=name, source=error.variant)
        projected.append(LayoutField(name, ty))
    return Layout(nominal, semantic, layout, tuple(projected))


def admit_initializers(layout: Layout, authored: Iterable[tuple[str, Any]]) -> tuple[Initializer, ...]:
    authored = tuple(authored)
    if len(authored) > MAX_FIELD_ID:
        raise AdmissionError("TooManyFields", actual=len(authored), maximum=MAX_FIELD_ID)
    seen: set[str] = set()
    result: list[Initializer] = []
    for name, expr in authored:
        if name in seen:
            raise AdmissionError("DuplicateName", name=name)
        seen.add(name)
        found = layout.field_by_name(name)
        if found is None:
            raise AdmissionError("UnknownField", name=name)
        fid, _ = found
        result.append(Initializer(fid, name, expr))
    for ordinal, field in enumerate(layout.fields):
        if field.name not in seen:
            raise AdmissionError("MissingField", field=field_id(ordinal), name=field.name)
    return tuple(result)


def validate_initializers(layout: Layout, initializers: Iterable[Initializer]) -> None:
    initializers = tuple(initializers)
    if len(initializers) > MAX_FIELD_ID:
        raise AdmissionError("TooManyFields", actual=len(initializers), maximum=MAX_FIELD_ID)
    seen: set[str] = set()
    for init in initializers:
        if init.name in seen:
            raise AdmissionError("DuplicateName", name=init.name)
        seen.add(init.name)
        found = layout.field_by_name(init.name)
        if found is None:
            raise AdmissionError("UnknownField", name=init.name)
        expected, _ = found
        if init.field != expected:
            raise AdmissionError("FieldIdentityMismatch", name=init.name, expected=expected, actual=init.field)
    for ordinal, field in enumerate(layout.fields):
        if field.name not in seen:
            raise AdmissionError("MissingField", field=field_id(ordinal), name=field.name)


def evaluate_nominal(layout: Layout, initializers: Iterable[Initializer], log: list[str]) -> NominalValue:
    initializers = tuple(initializers)
    validate_initializers(layout, initializers)
    slots: list[Value | None] = [None] * len(layout.fields)
    for init in initializers:
        log.append(init.name)
        value = init.expression() if callable(init.expression) else init.expression
        slots[init.field - 1] = value
    values = tuple(v for v in slots if v is not None)
    return admit_nominal_value(layout, values)


def admit_nominal_value(layout: Layout, values: Iterable[Value]) -> NominalValue:
    values = tuple(values)
    if len(values) != len(layout.fields):
        raise AdmissionError("FieldCount", expected=len(layout.fields), actual=len(values))
    for ordinal, (field, value) in enumerate(zip(layout.fields, values)):
        fid = field_id(ordinal)
        if not field.ty.accepts(value):
            raise AdmissionError("FieldType", field=fid, name=field.name, expected=field.ty.kind)
    return NominalValue(layout.nominal, layout.layout, values)


def validate_nominal_value(value: NominalValue, layout: Layout) -> None:
    if value.nominal != layout.nominal:
        raise AdmissionError("Type", expected=layout.nominal, actual=value.nominal)
    if value.layout != layout.layout:
        raise AdmissionError("Layout", expected=layout.layout, actual=value.layout)
    admit_nominal_value(layout, value.fields)


def admit_record_columns(rows: int, fields: Iterable[tuple[str, Iterable[Any]]]) -> tuple[RecordColumn, ...]:
    fields = tuple((name, tuple(values)) for name, values in fields)
    if len(fields) > MAX_FIELD_ID:
        raise AdmissionError("TooManyRecordFields", actual=len(fields), maximum=MAX_FIELD_ID)
    seen: set[str] = set()
    result: list[RecordColumn] = []
    for ordinal, (name, values) in enumerate(fields):
        try:
            fid = field_id(ordinal)
        except AdmissionError as error:
            raise AdmissionError("InvalidRecordFieldIdentity", ordinal=ordinal, field=name, source=error.variant)
        if len(values) != rows:
            raise AdmissionError("ColumnLength", ordinal=ordinal, expected=rows, actual=len(values))
        if name in seen:
            raise AdmissionError("DuplicateRecordField", field=name)
        seen.add(name)
        result.append(RecordColumn(fid, name, values))
    return tuple(result)


def visitor_paths(kind: str, fields: Iterable[Any]) -> tuple[tuple[str, int], ...]:
    tag = {
        "anonymous": "RecordField",
        "column": "RecordColumn",
        "nominal": "NominalRecordField",
    }[kind]
    if kind in {"anonymous", "column"}:
        return tuple((tag, field.field) for field in fields)
    return tuple((tag, field_id(i)) for i, _ in enumerate(fields))


def canonical_anonymous(fields: Iterable[tuple[str, Value]]) -> bytes:
    payload = b"|".join(name.encode() + b"=" + value.kind.encode() for name, value in sorted(fields))
    return b"anon:" + payload


def canonical_nominal(value: NominalValue) -> bytes:
    payload = b"|".join(field.kind.encode() for field in value.fields)
    return b"nom:" + value.nominal.encode() + b":" + value.layout.encode() + b":" + payload
