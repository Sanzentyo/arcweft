"""Executable law model for the Lang-01.3.1.2.3 ownership contract.

This is deliberately a small, dependency-free model.  It is not production code
and does not recreate Arcweft's complete value enum.  It executes the selected
result-changing laws: structural ownership, checked unrestricted duplication,
slot moves, transactional capture, repeat/index/slice behavior, consuming
iteration, and dormant snapshot evidence with exclusive activation.
"""

from __future__ import annotations

from collections import deque
from copy import copy, deepcopy
from dataclasses import dataclass, field
from enum import Enum
import json
from typing import Deque, Iterable, Iterator, Sequence


class Ownership(str, Enum):
    UNRESTRICTED = "unrestricted"
    AFFINE = "affine"

    @staticmethod
    def join(left: "Ownership", right: "Ownership") -> "Ownership":
        if left is Ownership.UNRESTRICTED and right is Ownership.UNRESTRICTED:
            return Ownership.UNRESTRICTED
        return Ownership.AFFINE


class SlotState(str, Enum):
    EMPTY = "empty"
    LIVE = "live"
    MOVED = "moved"
    DROPPED = "dropped"


class CaptureMode(str, Enum):
    COPY = "copy"
    MOVE = "move"


class WorldState(str, Enum):
    EMPTY = "empty"
    ACTIVE = "active"
    FROZEN = "frozen"


class OwnershipModelError(Exception):
    """Base typed failure for the executable model."""


class DuplicateError(OwnershipModelError):
    def __init__(self, path: str, owner_id: str) -> None:
        super().__init__(f"affine owner {owner_id!r} at {path!r} is not duplicable")
        self.path = path
        self.owner_id = owner_id


class SlotError(OwnershipModelError):
    pass


class CaptureError(OwnershipModelError):
    pass


class SequenceOperationError(OwnershipModelError):
    pass


class SnapshotError(OwnershipModelError):
    pass


@dataclass(eq=False)
class OwnerToken:
    """Opaque runnable authority; IDs alone are not authority."""

    owner_id: str
    active: bool = True

    def __copy__(self) -> "OwnerToken":  # pragma: no cover - asserted indirectly
        raise TypeError("OwnerToken is not copyable")

    def __deepcopy__(self, memo: dict[int, object]) -> "OwnerToken":  # pragma: no cover
        raise TypeError("OwnerToken is not deepcopyable")

    def retire(self) -> None:
        if not self.active:
            raise SnapshotError(f"owner {self.owner_id!r} is already retired")
        self.active = False


@dataclass(eq=False)
class Value:
    """A generic structural runtime value.

    ``edges`` are kept in canonical traversal order.  For sequences/tuples the
    labels are decimal indices; for records they are authored canonical field
    labels.  Affinity is never cached as authority: it is recomputed from the
    transitive graph.
    """

    kind: str
    scalar: object | None = None
    edges: tuple[tuple[str, "Value"], ...] = ()
    token: OwnerToken | None = None

    @staticmethod
    def scalar_value(value: object) -> "Value":
        return Value(kind="scalar", scalar=value)

    @staticmethod
    def affine_leaf(owner_id: str, *, kind: str = "stream_handle") -> "Value":
        return Value(kind=kind, token=OwnerToken(owner_id))

    @staticmethod
    def sequence(values: Iterable["Value"]) -> "Value":
        items = tuple(values)
        return Value(
            kind="sequence",
            edges=tuple((str(index), value) for index, value in enumerate(items)),
        )

    @staticmethod
    def tuple_value(values: Iterable["Value"]) -> "Value":
        items = tuple(values)
        return Value(
            kind="tuple",
            edges=tuple((str(index), value) for index, value in enumerate(items)),
        )

    @staticmethod
    def record(fields: Sequence[tuple[str, "Value"]]) -> "Value":
        names = [name for name, _ in fields]
        if len(names) != len(set(names)):
            raise OwnershipModelError("record field labels must be unique")
        return Value(kind="record", edges=tuple(fields))

    @staticmethod
    def closure(captures: Sequence["Value"]) -> "Value":
        return Value(
            kind="closure",
            edges=tuple((f"capture[{index}]", value) for index, value in enumerate(captures)),
        )

    def ownership(self) -> Ownership:
        result = Ownership.AFFINE if self.token is not None else Ownership.UNRESTRICTED
        for _, child in self.edges:
            result = Ownership.join(result, child.ownership())
        return result

    def owner_occurrences(self, path: str = "$value") -> list[tuple[str, OwnerToken]]:
        rows: list[tuple[str, OwnerToken]] = []
        if self.token is not None:
            rows.append((path, self.token))
        for label, child in self.edges:
            rows.extend(child.owner_occurrences(f"{path}.{label}"))
        return rows

    def try_duplicate_unrestricted(self, path: str = "$value") -> "Value":
        if self.token is not None:
            raise DuplicateError(path, self.token.owner_id)
        copied_edges: list[tuple[str, Value]] = []
        for label, child in self.edges:
            copied_edges.append((label, child.try_duplicate_unrestricted(f"{path}.{label}")))
        # Scalars in the model are payload-safe immutable primitives.  Production
        # uses the closed RuntimePayload/constant eligibility boundary instead.
        return Value(kind=self.kind, scalar=self.scalar, edges=tuple(copied_edges))

    def structural_shape(self, path: str = "$value") -> dict[str, object]:
        """Return dormant, non-runnable shape evidence; never include a token."""
        result: dict[str, object] = {
            "path": path,
            "kind": self.kind,
            "scalar": self.scalar,
            "children": [],
        }
        children = result["children"]
        assert isinstance(children, list)
        for label, child in self.edges:
            children.append({"label": label, "value": child.structural_shape(f"{path}.{label}")})
        if self.token is not None:
            result["affine_owner_id"] = self.token.owner_id
        return result


@dataclass
class Slot:
    state: SlotState = SlotState.EMPTY
    value: Value | None = None

    @staticmethod
    def live(value: Value) -> "Slot":
        return Slot(SlotState.LIVE, value)

    def require_live(self) -> Value:
        if self.state is not SlotState.LIVE or self.value is None:
            raise SlotError(f"slot is not live: {self.state.value}")
        return self.value

    def take(self) -> Value:
        value = self.require_live()
        self.value = None
        self.state = SlotState.MOVED
        return value

    def put(self, value: Value) -> None:
        if self.state not in {SlotState.EMPTY, SlotState.MOVED, SlotState.DROPPED}:
            raise SlotError(f"destination is not empty: {self.state.value}")
        self.value = value
        self.state = SlotState.LIVE

    def duplicate(self) -> Value:
        return self.require_live().try_duplicate_unrestricted()

    def drop(self) -> None:
        value = self.require_live()
        for _, token in reversed(value.owner_occurrences()):
            token.retire()
        self.value = None
        self.state = SlotState.DROPPED


@dataclass(frozen=True)
class CaptureSpec:
    source_slot: int
    destination_slot: int
    mode: CaptureMode


@dataclass(frozen=True)
class CaptureResult:
    closure: Value
    source_states: tuple[SlotState, ...]


def capture_transaction(
    slots: Sequence[Slot],
    specs: Sequence[CaptureSpec],
    *,
    inject_failure_after_copy_stage: bool = False,
) -> CaptureResult:
    """Prepare all copies/moves, then commit moves and closure atomically."""

    source_indices = [spec.source_slot for spec in specs]
    destination_indices = [spec.destination_slot for spec in specs]
    if len(source_indices) != len(set(source_indices)):
        raise CaptureError("capture source slots must be unique")
    if len(destination_indices) != len(set(destination_indices)):
        raise CaptureError("capture destination slots must be unique")
    if sorted(destination_indices) != list(range(len(specs))):
        raise CaptureError("capture destination slots must be canonical and dense")

    staged: dict[int, Value] = {}
    # Preflight every source before staging anything observable.
    for spec in sorted(specs, key=lambda row: row.destination_slot):
        if spec.source_slot < 0 or spec.source_slot >= len(slots):
            raise CaptureError(f"source slot {spec.source_slot} is out of bounds")
        slots[spec.source_slot].require_live()

    for spec in sorted(specs, key=lambda row: row.destination_slot):
        source = slots[spec.source_slot].require_live()
        if spec.mode is CaptureMode.COPY:
            staged[spec.destination_slot] = source.try_duplicate_unrestricted(
                f"$capture[{spec.destination_slot}]"
            )

    if inject_failure_after_copy_stage:
        raise CaptureError("injected failure after checked copy staging")

    # All remaining operations are infallible in this model after preflight.
    # Capture the move values first and update all slots in one commit phase.
    moved: dict[int, Value] = {}
    for spec in sorted(specs, key=lambda row: row.destination_slot):
        if spec.mode is CaptureMode.MOVE:
            moved[spec.destination_slot] = slots[spec.source_slot].require_live()

    for spec in sorted(specs, key=lambda row: row.destination_slot):
        if spec.mode is CaptureMode.MOVE:
            slots[spec.source_slot].take()

    captures = [
        staged.get(index, moved[index]) if index not in staged else staged[index]
        for index in range(len(specs))
    ]
    closure = Value.closure(captures)
    return CaptureResult(closure=closure, source_states=tuple(slot.state for slot in slots))


def repeat_from_slot(
    source: Slot,
    count: int,
    *,
    statically_exact_count: bool = True,
) -> Value:
    if count < 0:
        raise SequenceOperationError("repeat count is negative")
    value = source.require_live()

    # Dynamic repeat must be valid for every runtime count and therefore cannot
    # consume an affine value under a hidden 0/1 special case.
    if not statically_exact_count and value.ownership() is Ownership.AFFINE:
        raise SequenceOperationError("dynamic repeat requires an unrestricted value")

    if count == 0:
        source.drop()
        return Value.sequence(())
    if count == 1:
        return Value.sequence((source.take(),))

    if value.ownership() is Ownership.AFFINE:
        raise SequenceOperationError("repeat count >= 2 requires an unrestricted value")

    staged = [value.try_duplicate_unrestricted(f"$repeat[{index}]") for index in range(count - 1)]
    staged.append(source.take())
    return Value.sequence(staged)


def _sequence_children(value: Value) -> tuple[Value, ...]:
    if value.kind != "sequence":
        raise SequenceOperationError(f"expected sequence, got {value.kind!r}")
    return tuple(child for _, child in value.edges)


def preserving_index(value: Value, index: int) -> Value:
    children = _sequence_children(value)
    if value.ownership() is Ownership.AFFINE:
        raise SequenceOperationError("ordinary indexing requires an unrestricted sequence")
    if index < 0 or index >= len(children):
        raise SequenceOperationError("index out of bounds")
    return children[index].try_duplicate_unrestricted(f"$index[{index}]")


def preserving_slice(value: Value, start: int, end: int) -> Value:
    children = _sequence_children(value)
    # The full source sequence must be unrestricted even for an empty range.
    if value.ownership() is Ownership.AFFINE:
        raise SequenceOperationError("ordinary slicing requires an unrestricted sequence")
    if start < 0 or end < start or end > len(children):
        raise SequenceOperationError("slice bounds are invalid")
    return Value.sequence(
        child.try_duplicate_unrestricted(f"$slice[{start + offset}]")
        for offset, child in enumerate(children[start:end])
    )


@dataclass
class ConsumingIterator:
    items: Deque[Value] = field(default_factory=deque)

    @staticmethod
    def from_slot(source: Slot) -> "ConsumingIterator":
        value = source.require_live()
        children = _sequence_children(value)
        source.take()
        # Moving the sequence into the iterator transfers all element owners.
        return ConsumingIterator(deque(children))

    def next_value(self) -> Value | None:
        if not self.items:
            return None
        return self.items.popleft()

    def drop_remaining(self) -> None:
        while self.items:
            value = self.items.pop()
            for _, token in reversed(value.owner_occurrences("$iterator.remaining")):
                token.retire()


@dataclass(frozen=True)
class OwnerEvidence:
    path: str
    owner_id: str


@dataclass(frozen=True)
class SnapshotImage:
    schema: int
    roots: tuple[dict[str, object], ...]
    owners: tuple[OwnerEvidence, ...]

    def canonical_bytes(self) -> bytes:
        payload = {
            "schema": self.schema,
            "roots": self.roots,
            "owners": [
                {"path": owner.path, "owner_id": owner.owner_id} for owner in self.owners
            ],
        }
        return json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")


def build_snapshot(values: Sequence[Value]) -> SnapshotImage:
    owners: list[OwnerEvidence] = []
    roots: list[dict[str, object]] = []
    seen_ids: set[str] = set()
    seen_paths: set[str] = set()

    for root_index, value in enumerate(values):
        root_path = f"$root[{root_index}]"
        roots.append(value.structural_shape(root_path))
        for path, token in value.owner_occurrences(root_path):
            if not token.active:
                raise SnapshotError(f"inactive owner {token.owner_id!r} at {path!r}")
            if token.owner_id in seen_ids:
                raise SnapshotError(f"duplicate runnable owner ID {token.owner_id!r}")
            if path in seen_paths:
                raise SnapshotError(f"duplicate owner path {path!r}")
            seen_ids.add(token.owner_id)
            seen_paths.add(path)
            owners.append(OwnerEvidence(path=path, owner_id=token.owner_id))

    owners.sort(key=lambda row: row.path)
    return SnapshotImage(schema=2, roots=tuple(roots), owners=tuple(owners))


def validate_snapshot(image: SnapshotImage) -> None:
    if image.schema != 2:
        raise SnapshotError(f"unsupported snapshot schema {image.schema}")
    ids: set[str] = set()
    paths: set[str] = set()
    previous: str | None = None
    for row in image.owners:
        if previous is not None and row.path <= previous:
            raise SnapshotError("owner evidence is not in canonical path order")
        previous = row.path
        if row.owner_id in ids:
            raise SnapshotError(f"duplicate owner ID evidence {row.owner_id!r}")
        if row.path in paths:
            raise SnapshotError(f"duplicate owner path evidence {row.path!r}")
        ids.add(row.owner_id)
        paths.add(row.path)

    # Snapshot shapes carry IDs as dormant evidence only.  A token object in the
    # DTO would constitute a second runnable owner channel.
    def reject_tokens(node: object) -> None:
        if isinstance(node, OwnerToken):
            raise SnapshotError("snapshot image contains a runnable OwnerToken")
        if isinstance(node, dict):
            for value in node.values():
                reject_tokens(value)
        elif isinstance(node, (tuple, list)):
            for value in node:
                reject_tokens(value)

    reject_tokens(image.roots)


@dataclass
class RuntimeWorld:
    state: WorldState = WorldState.EMPTY
    active_owners: dict[str, OwnerToken] = field(default_factory=dict)

    @staticmethod
    def from_values(values: Sequence[Value]) -> "RuntimeWorld":
        owners: dict[str, OwnerToken] = {}
        for root_index, value in enumerate(values):
            for path, token in value.owner_occurrences(f"$root[{root_index}]"):
                if token.owner_id in owners:
                    raise SnapshotError(f"duplicate active owner {token.owner_id!r} at {path!r}")
                owners[token.owner_id] = token
        return RuntimeWorld(WorldState.ACTIVE, owners)

    def freeze(self) -> None:
        if self.state is not WorldState.ACTIVE:
            raise SnapshotError(f"only an active world can freeze, got {self.state.value}")
        self.state = WorldState.FROZEN

    def thaw(self) -> None:
        if self.state is not WorldState.FROZEN:
            raise SnapshotError(f"only a frozen world can thaw, got {self.state.value}")
        self.state = WorldState.ACTIVE

    def install_into_empty(self, image: SnapshotImage) -> None:
        if self.state is not WorldState.EMPTY or self.active_owners:
            raise SnapshotError("restore target is not empty")
        validate_snapshot(image)
        self.active_owners = {
            row.owner_id: OwnerToken(row.owner_id) for row in image.owners
        }
        self.state = WorldState.ACTIVE

    def replace_frozen(self, image: SnapshotImage) -> None:
        if self.state is not WorldState.FROZEN:
            raise SnapshotError("replacement requires a frozen target")
        validate_snapshot(image)
        # Prepare dormant replacement tokens without publishing them.
        prepared = {row.owner_id: OwnerToken(row.owner_id, active=False) for row in image.owners}
        # Retire all old owners before any replacement becomes runnable.
        for token in self.active_owners.values():
            token.retire()
        for token in prepared.values():
            token.active = True
        self.active_owners = prepared
        self.state = WorldState.ACTIVE


def assert_snapshot_copy_is_dormant(image: SnapshotImage) -> None:
    copied = deepcopy(image)
    validate_snapshot(copied)
    if copied.canonical_bytes() != image.canonical_bytes():
        raise SnapshotError("copying dormant evidence changed canonical bytes")


def token_copy_rejected(token: OwnerToken) -> bool:
    for operation in (copy, deepcopy):
        try:
            operation(token)
        except TypeError:
            continue
        return False
    return True
