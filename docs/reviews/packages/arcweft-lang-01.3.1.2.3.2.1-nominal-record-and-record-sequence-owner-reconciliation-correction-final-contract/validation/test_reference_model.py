from __future__ import annotations

from reference_model import (
    AdmissionError,
    FieldType,
    Initializer,
    Value,
    admit_initializers,
    admit_nominal_value,
    admit_record_columns,
    canonical_anonymous,
    canonical_nominal,
    evaluate_nominal,
    make_layout,
    validate_initializers,
    validate_nominal_value,
    visitor_paths,
)


def expect(variant: str, call) -> None:
    try:
        call()
    except AdmissionError as error:
        assert error.variant == variant, (error.variant, variant, error.data)
    else:
        raise AssertionError(f"expected {variant}")


def run() -> int:
    checks = 0
    i64 = FieldType("i64")
    text = FieldType("string")
    layout = make_layout("Pkg::R", "sem-r", "lay-r", [("a", i64), ("z", text)])
    checks += 1
    assert layout.field_id(0) == 1 and layout.field_id(1) == 2 and layout.field_id(2) is None
    checks += 1
    expect("DuplicateFieldName", lambda: make_layout("R", "s", "l", [("a", i64), ("a", i64)]))
    checks += 1

    init = admit_initializers(layout, [("z", Value("string", "Z")), ("a", Value("i64", 1))])
    assert [(x.name, x.field) for x in init] == [("z", 2), ("a", 1)]
    checks += 1
    expect("DuplicateName", lambda: admit_initializers(layout, [("a", 1), ("a", 2), ("z", 3)]))
    checks += 1
    expect("UnknownField", lambda: admit_initializers(layout, [("q", 1), ("a", 2)]))
    checks += 1
    expect("MissingField", lambda: admit_initializers(layout, [("a", 1)]))
    checks += 1
    expect("FieldIdentityMismatch", lambda: validate_initializers(layout, [Initializer(1, "z", 1), Initializer(1, "a", 2)]))
    checks += 1

    log: list[str] = []
    value = evaluate_nominal(layout, init, log)
    assert log == ["z", "a"]
    assert [v.kind for v in value.fields] == ["i64", "string"]
    checks += 1
    expect("FieldCount", lambda: admit_nominal_value(layout, [Value("i64", 1)]))
    checks += 1
    expect("FieldType", lambda: admit_nominal_value(layout, [Value("string", "bad"), Value("string", "ok")]))
    checks += 1
    validate_nominal_value(value, layout)
    checks += 1
    expect("Type", lambda: validate_nominal_value(type(value)("Other", value.layout, value.fields), layout))
    checks += 1
    expect("Layout", lambda: validate_nominal_value(type(value)(value.nominal, "wrong", value.fields), layout))
    checks += 1

    nested = FieldType("nominal", nominal="Pkg::R", layout="lay-r")
    assert nested.accepts(Value("nominal", nominal="Pkg::R", layout="lay-r"))
    assert not nested.accepts(Value("nominal", nominal="Pkg::R", layout="other"))
    checks += 1

    cols = admit_record_columns(2, [("z", [1, 2]), ("a", [3, 4])])
    assert [(c.name, c.field) for c in cols] == [("z", 1), ("a", 2)]
    checks += 1
    expect("DuplicateRecordField", lambda: admit_record_columns(2, [("a", [1, 2]), ("a", [3, 4])]))
    checks += 1
    expect("ColumnLength", lambda: admit_record_columns(2, [("a", [1, 2]), ("a", [3])]))
    checks += 1

    class F:
        def __init__(self, field: int): self.field = field
    assert visitor_paths("anonymous", [F(1), F(2)]) == (("RecordField", 1), ("RecordField", 2))
    checks += 1
    assert visitor_paths("column", cols) == (("RecordColumn", 1), ("RecordColumn", 2))
    checks += 1
    assert visitor_paths("nominal", value.fields) == (("NominalRecordField", 1), ("NominalRecordField", 2))
    checks += 1

    anon = canonical_anonymous([("a", Value("i64", 1)), ("z", Value("string", "Z"))])
    nom = canonical_nominal(value)
    assert anon != nom
    checks += 1
    assert canonical_nominal(value) == nom
    checks += 1

    print(f"PASS reference_model_checks={checks}")
    return checks


if __name__ == "__main__":
    run()
