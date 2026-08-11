#!/usr/bin/env python3
"""Executable reference checks for selected closed Lang-01.3.1.2.3.2 rules.

This is a design model, not production Arcweft code.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import IntEnum
import json
from pathlib import Path
import struct
import sys

U64_MAX = (1 << 64) - 1
checks = 0


def check(condition: bool, label: str) -> None:
    global checks
    if not condition:
        raise AssertionError(label)
    checks += 1


@dataclass(frozen=True)
class Cursor:
    next_value: int | None

    @staticmethod
    def initial() -> "Cursor":
        return Cursor(1)

    def take(self) -> tuple[int, "Cursor"]:
        if self.next_value is None:
            raise OverflowError("exhausted")
        value = self.next_value
        return value, Cursor(None if value == U64_MAX else value + 1)

    def last_issued(self) -> int | None:
        if self.next_value is None:
            return U64_MAX
        if self.next_value == 1:
            return None
        return self.next_value - 1


def test_cursor() -> None:
    cursor = Cursor.initial()
    one, cursor = cursor.take()
    two, cursor = cursor.take()
    check(one == 1, "cursor first")
    check(two == 2, "cursor second")
    max_value, exhausted = Cursor(U64_MAX).take()
    check(max_value == U64_MAX, "cursor max succeeds")
    check(exhausted.next_value is None, "cursor max exhausts")
    check(Cursor.initial().last_issued() is None, "fresh cursor no high water")
    check(Cursor(42).last_issued() == 41, "next cursor high water")
    check(exhausted.last_issued() == U64_MAX, "exhausted high water")
    try:
        exhausted.take()
    except OverflowError:
        check(True, "exhausted rejects")
    else:
        check(False, "exhausted rejects")


def validate_next(cursor: Cursor, used: list[int]) -> bool:
    maximum = max(used, default=0)
    if cursor.next_value is None:
        # Exhausted is itself the persisted high-water authority; the ID that
        # consumed U64_MAX may have retired from currently represented storage.
        return True
    return cursor.next_value > maximum


def test_cursor_restore() -> None:
    check(validate_next(Cursor(1), []), "empty next1")
    check(validate_next(Cursor(42), [1, 7, 41]), "gap continuation")
    check(not validate_next(Cursor(41), [41]), "equal stale")
    check(not validate_next(Cursor(20), [41]), "below stale")
    check(validate_next(Cursor(None), [41]), "retired-max exhausted remains valid")
    check(validate_next(Cursor(None), [U64_MAX]), "represented-max exhausted valid")


def validate_execution_envelope(execution: int, cursor: Cursor) -> bool:
    if cursor.next_value is None:
        return execution == U64_MAX
    return cursor.next_value > execution


def test_execution_cursor_envelope() -> None:
    check(validate_execution_envelope(5, Cursor(8)), "execution gap cursor")
    check(validate_execution_envelope(U64_MAX, Cursor(None)), "execution max exhausted")
    check(not validate_execution_envelope(5, Cursor(None)), "execution false exhausted")


def anonymous_record_ids(names: list[str]) -> list[int]:
    seen: set[str] = set()
    for name in names:
        if name in seen:
            raise ValueError(name)
        seen.add(name)
    return list(range(1, len(names) + 1))


def nominal_layout_ids(layout: list[str], authored: list[str]) -> list[int]:
    if len(authored) != len(set(authored)):
        raise ValueError("duplicate")
    if set(layout) != set(authored):
        raise ValueError("missing/unknown")
    return list(range(1, len(layout) + 1))


def test_record_ids() -> None:
    check(anonymous_record_ids(["z", "a"]) == [1, 2], "anonymous authored order")
    try:
        anonymous_record_ids(["a", "b", "a"])
    except ValueError as error:
        check(str(error) == "a", "anonymous duplicate")
    else:
        check(False, "anonymous duplicate")
    check(nominal_layout_ids(["a", "z"], ["z", "a"]) == [1, 2], "nominal layout")
    for authored in (["a"], ["a", "q"], ["a", "a"]):
        try:
            nominal_layout_ids(["a", "z"], authored)
        except ValueError:
            check(True, f"nominal rejects {authored}")
        else:
            check(False, f"nominal rejects {authored}")


class PathTag(IntEnum):
    TUPLE = 0
    SEQUENCE = 1
    TUPLE_COLUMN = 2
    RECORD = 3
    RECORD_COLUMN = 4
    NOMINAL = 5
    CAPTURE = 6
    VARIANT = 7
    ITERATOR = 8
    WITNESS = 9


@dataclass(frozen=True, order=True)
class Segment:
    tag: PathTag
    payload: int = 0


PathT = tuple[Segment, ...]


def test_path_order() -> None:
    values: list[PathT] = [
        (),
        (Segment(PathTag.TUPLE, 0),),
        (Segment(PathTag.TUPLE, 0), Segment(PathTag.VARIANT)),
        (Segment(PathTag.TUPLE, 1),),
        (Segment(PathTag.SEQUENCE, 0),),
        (Segment(PathTag.RECORD, 1),),
        (Segment(PathTag.RECORD, 2),),
        (Segment(PathTag.NOMINAL, 1),),
        (Segment(PathTag.CAPTURE, 1),),
        (Segment(PathTag.VARIANT),),
        (Segment(PathTag.ITERATOR, 4),),
        (Segment(PathTag.WITNESS),),
    ]
    check(sorted(reversed(values)) == values, "path total order")
    check(values[1] < values[2], "prefix first")
    remainder = [(Segment(PathTag.ITERATOR, i),) for i in range(4, 7)]
    check([p[0].payload for p in remainder] == [4, 5, 6], "absolute iterator remainder")


class SlotTag(IntEnum):
    ENV = 0
    CLOSURE = 1
    AWBC_REG = 2
    AWBC_LOCAL = 3
    MAILBOX = 4
    CHILD = 5
    TRANSFER = 6
    CLEANUP = 7


@dataclass(frozen=True, order=True)
class Slot:
    tag: SlotTag
    fields: tuple[int, ...]


def test_slot_order() -> None:
    slots = [Slot(tag, (1, 2, 3, 4)) for tag in SlotTag]
    check(sorted(reversed(slots)) == slots, "slot tag order")
    check(Slot(SlotTag.ENV, (1, 99)) < Slot(SlotTag.ENV, (2, 1)), "execution first")
    check(Slot(SlotTag.CLOSURE, (1, 1, 9)) < Slot(SlotTag.CLOSURE, (1, 2, 1)), "occurrence first")


class PrepareRank(IntEnum):
    IDENTITY = 0
    STALE = 1
    SOURCE = 2
    DESTINATION = 3
    TYPE_PATH = 4
    DUPLICATE_OWNER = 5
    AFFINE_COPY = 6
    EXHAUSTION = 7
    BUDGET = 8
    ALLOCATION = 9


@dataclass(frozen=True, order=True)
class PrepareError:
    rank: PrepareRank
    slot: Slot
    path: PathT = ()
    owner: tuple[int, int] = (0, 0)
    step: int = 0


def test_error_order() -> None:
    late_stale = PrepareError(PrepareRank.STALE, Slot(SlotTag.CLEANUP, (1, 9)))
    early_copy = PrepareError(PrepareRank.AFFINE_COPY, Slot(SlotTag.ENV, (1, 1)))
    check(min([early_copy, late_stale]) == late_stale, "rank before slot")
    stale_2 = PrepareError(PrepareRank.STALE, Slot(SlotTag.ENV, (1, 2)))
    stale_1 = PrepareError(PrepareRank.STALE, Slot(SlotTag.ENV, (1, 1)))
    check(min([stale_2, stale_1]) == stale_1, "slot within rank")


class Role(IntEnum):
    COPY_SOURCE = 0
    COPY_DESTINATION = 1
    MOVE_SOURCE = 2
    MOVE_DESTINATION = 3
    DROP_SOURCE = 4


def normalize_participants(
    steps: list[tuple[Role, str, int, str]],
) -> tuple[str, ...]:
    seen: dict[str, tuple[Role, int, str]] = {}
    for role, slot, revision, accepted_type in steps:
        prior = seen.get(slot)
        if prior is None:
            seen[slot] = (role, revision, accepted_type)
            continue
        if (
            role == Role.COPY_SOURCE
            and prior[0] == Role.COPY_SOURCE
            and prior[1] == revision
            and prior[2] == accepted_type
        ):
            continue
        raise ValueError(slot)
    return tuple(sorted(seen))


def test_participant_normalization() -> None:
    compatible = [
        (Role.COPY_SOURCE, "s", 7, "T"),
        (Role.COPY_DESTINATION, "d1", 1, "T"),
        (Role.COPY_SOURCE, "s", 7, "T"),
        (Role.COPY_DESTINATION, "d2", 1, "T"),
    ]
    check(normalize_participants(compatible) == ("d1", "d2", "s"), "compatible multi-copy source")
    for conflicting in [
        compatible + [(Role.COPY_DESTINATION, "d1", 1, "T")],
        [(Role.COPY_SOURCE, "s", 7, "T"), (Role.MOVE_SOURCE, "s", 7, "T")],
        [(Role.COPY_SOURCE, "s", 7, "T"), (Role.COPY_SOURCE, "s", 8, "T")],
        [(Role.COPY_SOURCE, "s", 7, "T"), (Role.COPY_SOURCE, "s", 7, "U")],
    ]:
        try:
            normalize_participants(conflicting)
        except ValueError:
            check(True, "conflicting participant rejects")
        else:
            check(False, "conflicting participant rejects")


class StateTag(IntEnum):
    VACANT = 0
    LIVE = 1
    MOVED = 2
    DROPPED = 3


@dataclass(frozen=True)
class Cell:
    revision: int
    state: StateTag
    value: str | None
    reservation: int | None = None


@dataclass(frozen=True)
class PreparedMove:
    tx: int
    source_revision: int
    dest_revision: int
    source_value: str


def prepare_move(source: Cell, dest: Cell, tx: int) -> tuple[Cell, Cell, PreparedMove]:
    if source.state != StateTag.LIVE:
        raise ValueError("source")
    if dest.state != StateTag.VACANT:
        raise ValueError("destination")
    if source.reservation is not None or dest.reservation is not None:
        raise ValueError("reserved")
    prepared = PreparedMove(tx, source.revision, dest.revision, source.value or "")
    return (
        replace(source, reservation=tx),
        replace(dest, reservation=tx),
        prepared,
    )


def commit_move(source: Cell, dest: Cell, prepared: PreparedMove) -> tuple[Cell, Cell]:
    if source.reservation != prepared.tx or dest.reservation != prepared.tx:
        raise ValueError("reservation")
    if source.revision != prepared.source_revision or dest.revision != prepared.dest_revision:
        raise ValueError("revision")
    if source.state != StateTag.LIVE or dest.state != StateTag.VACANT:
        raise ValueError("occupancy")
    # permit boundary: no condition below this line
    value = source.value
    source = Cell(source.revision + 1, StateTag.MOVED, None, None)
    dest = Cell(dest.revision + 1, StateTag.LIVE, value, None)
    return source, dest


def test_transaction() -> None:
    original_source = Cell(1, StateTag.LIVE, "affine#7")
    original_dest = Cell(1, StateTag.VACANT, None)
    source, dest, prepared = prepare_move(original_source, original_dest, 9)
    check(source.value == original_source.value, "prepare preserves source")
    check(source.revision == 1 and dest.revision == 1, "prepare preserves revisions")
    moved, filled = commit_move(source, dest, prepared)
    check(moved.state == StateTag.MOVED and moved.revision == 2, "move tombstone")
    check(filled.state == StateTag.LIVE and filled.value == "affine#7", "exact move value")
    check(filled.revision == 2, "destination revision")
    # Mismatch happens before take and leaves values as at commit entry.
    source2, dest2, prepared2 = prepare_move(original_source, original_dest, 10)
    raced_dest = replace(dest2, revision=2)
    before = (source2, raced_dest)
    try:
        commit_move(source2, raced_dest, prepared2)
    except ValueError as error:
        check(str(error) == "revision", "commit mismatch kind")
        check(before == (source2, raced_dest), "commit mismatch no mutation")
    else:
        check(False, "commit mismatch")


@dataclass
class Domain:
    next_execution: Cursor
    reservation: int | None = None
    active: tuple[int, int] | None = None

    def prepare_new(self) -> int:
        if self.reservation is not None:
            raise ValueError("reservation")
        if self.active is not None:
            raise ValueError("active")
        execution, self.next_execution = self.next_execution.take()
        self.reservation = execution
        return execution

    def activate(self, execution: int) -> None:
        if self.reservation != execution or self.active is not None:
            raise ValueError("activation")
        self.reservation = None
        self.active = (execution, 1)


def test_domain() -> None:
    d = Domain(Cursor.initial())
    e = d.prepare_new()
    check(e == 1 and d.reservation == 1, "domain reserves")
    try:
        d.prepare_new()
    except ValueError:
        check(True, "second reservation rejects")
    else:
        check(False, "second reservation rejects")
    d.activate(e)
    check(d.active == (1, 1) and d.reservation is None, "domain activates one")
    try:
        d.prepare_new()
    except ValueError:
        check(True, "active rejects empty new")
    else:
        check(False, "active rejects empty new")


def test_goldens(root: Path) -> None:
    data = json.loads((root / "CODEC_GOLDENS.json").read_text(encoding="utf-8"))
    items = {item["name"]: item for item in data["goldens"]}
    check(bytes.fromhex(items["execution-1"]["binary_hex"]) == struct.pack("<Q", 1), "golden execution")
    check(bytes.fromhex(items["record-field-1"]["binary_hex"]) == struct.pack("<I", 1), "golden record")
    check(bytes.fromhex(items["cursor-next-1"]["binary_hex"]) == b"\0" + struct.pack("<Q", 1), "golden cursor")
    check(bytes.fromhex(items["cursor-exhausted"]["binary_hex"]) == b"\1", "golden exhausted")
    identity = bytes.fromhex(items["identity-snapshot"]["binary_hex"])
    domain = bytes.fromhex(items["domain-snapshot"]["binary_hex"])
    check(len(identity) == 44, "golden core identity length")
    check(domain == b"\0" + struct.pack("<Q", 3) + struct.pack("<Q", 2) + identity, "golden driver envelope order")
    check(items["domain-snapshot"]["binary_length"] == 61, "golden domain length")
    for item in data["goldens"]:
        raw = bytes.fromhex(item["binary_hex"])
        check(len(raw) == item["binary_length"], f"golden length {item['name']}")


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    test_cursor()
    test_cursor_restore()
    test_execution_cursor_envelope()
    test_record_ids()
    test_path_order()
    test_slot_order()
    test_error_order()
    test_participant_normalization()
    test_transaction()
    test_domain()
    test_goldens(root)
    print(json.dumps({
        "status": "PASS",
        "checks": checks,
        "scope": "design reference model only",
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
