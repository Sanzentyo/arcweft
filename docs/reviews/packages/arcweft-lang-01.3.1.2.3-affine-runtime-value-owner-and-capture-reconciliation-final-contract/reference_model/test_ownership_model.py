from __future__ import annotations

from copy import deepcopy
import unittest

from ownership_model import (
    CaptureError,
    CaptureMode,
    CaptureSpec,
    ConsumingIterator,
    DuplicateError,
    OwnerEvidence,
    Ownership,
    RuntimeWorld,
    SequenceOperationError,
    Slot,
    SlotState,
    SnapshotError,
    SnapshotImage,
    Value,
    WorldState,
    assert_snapshot_copy_is_dormant,
    build_snapshot,
    capture_transaction,
    preserving_index,
    preserving_slice,
    repeat_from_slot,
    token_copy_rejected,
    validate_snapshot,
)


class OwnershipLawTests(unittest.TestCase):
    def test_structural_classification(self) -> None:
        unrestricted = Value.record((
            ("left", Value.scalar_value(1)),
            ("right", Value.sequence((Value.scalar_value(2),))),
        ))
        affine = Value.record((
            ("left", Value.scalar_value(1)),
            ("right", Value.sequence((Value.affine_leaf("owner-1"),))),
        ))
        self.assertIs(unrestricted.ownership(), Ownership.UNRESTRICTED)
        self.assertIs(affine.ownership(), Ownership.AFFINE)

    def test_unrestricted_duplicate_is_deep_and_source_is_unchanged(self) -> None:
        source = Value.sequence((Value.scalar_value("a"), Value.tuple_value((Value.scalar_value(2),))))
        duplicate = source.try_duplicate_unrestricted()
        self.assertIsNot(source, duplicate)
        self.assertIsNot(source.edges[0][1], duplicate.edges[0][1])
        self.assertEqual(source.structural_shape(), duplicate.structural_shape())

    def test_nested_affine_duplicate_rejects_first_canonical_path(self) -> None:
        source = Value.record((
            ("a", Value.scalar_value(1)),
            ("b", Value.sequence((Value.scalar_value(2), Value.affine_leaf("owner-b")))),
            ("c", Value.affine_leaf("owner-c")),
        ))
        with self.assertRaises(DuplicateError) as caught:
            source.try_duplicate_unrestricted()
        self.assertEqual(caught.exception.path, "$value.b.1")
        self.assertEqual(caught.exception.owner_id, "owner-b")
        self.assertTrue(source.edges[1][1].edges[1][1].token.active)

    def test_token_copy_and_deepcopy_are_rejected(self) -> None:
        token = Value.affine_leaf("owner-1").token
        assert token is not None
        self.assertTrue(token_copy_rejected(token))

    def test_slot_copy_retains_source_and_move_consumes_source(self) -> None:
        copy_source = Slot.live(Value.scalar_value(7))
        copied = copy_source.duplicate()
        self.assertIs(copy_source.state, SlotState.LIVE)
        self.assertEqual(copied.scalar, 7)

        move_source = Slot.live(Value.affine_leaf("owner-move"))
        moved = move_source.take()
        self.assertIs(move_source.state, SlotState.MOVED)
        self.assertEqual(moved.token.owner_id, "owner-move")

    def test_capture_copy_and_move_commit_together(self) -> None:
        slots = [
            Slot.live(Value.scalar_value("copy")),
            Slot.live(Value.affine_leaf("owner-cap")),
            Slot.live(Value.scalar_value("unrelated")),
        ]
        result = capture_transaction(slots, (
            CaptureSpec(0, 0, CaptureMode.COPY),
            CaptureSpec(1, 1, CaptureMode.MOVE),
        ))
        self.assertIs(result.closure.ownership(), Ownership.AFFINE)
        self.assertIs(slots[0].state, SlotState.LIVE)
        self.assertIs(slots[1].state, SlotState.MOVED)
        self.assertIs(slots[2].state, SlotState.LIVE)
        self.assertEqual(len(result.closure.edges), 2)

    def test_capture_failure_after_copy_stage_is_non_mutating(self) -> None:
        slots = [
            Slot.live(Value.scalar_value("copy")),
            Slot.live(Value.affine_leaf("owner-cap")),
        ]
        before = tuple(slot.state for slot in slots)
        with self.assertRaises(CaptureError):
            capture_transaction(
                slots,
                (
                    CaptureSpec(0, 0, CaptureMode.COPY),
                    CaptureSpec(1, 1, CaptureMode.MOVE),
                ),
                inject_failure_after_copy_stage=True,
            )
        self.assertEqual(tuple(slot.state for slot in slots), before)
        self.assertTrue(slots[1].require_live().token.active)

    def test_capture_copy_of_affine_rejects_without_moving_other_source(self) -> None:
        slots = [
            Slot.live(Value.affine_leaf("owner-copy")),
            Slot.live(Value.affine_leaf("owner-move")),
        ]
        with self.assertRaises(DuplicateError):
            capture_transaction(slots, (
                CaptureSpec(0, 0, CaptureMode.COPY),
                CaptureSpec(1, 1, CaptureMode.MOVE),
            ))
        self.assertEqual([slot.state for slot in slots], [SlotState.LIVE, SlotState.LIVE])

    def test_repeat_zero_drops_affine_once(self) -> None:
        slot = Slot.live(Value.affine_leaf("owner-zero"))
        token = slot.require_live().token
        result = repeat_from_slot(slot, 0)
        self.assertEqual(len(result.edges), 0)
        self.assertIs(slot.state, SlotState.DROPPED)
        self.assertFalse(token.active)

    def test_repeat_one_moves_affine(self) -> None:
        slot = Slot.live(Value.affine_leaf("owner-one"))
        result = repeat_from_slot(slot, 1)
        self.assertIs(slot.state, SlotState.MOVED)
        self.assertIs(result.ownership(), Ownership.AFFINE)
        self.assertEqual(result.edges[0][1].token.owner_id, "owner-one")

    def test_repeat_two_rejects_affine_without_mutation(self) -> None:
        slot = Slot.live(Value.affine_leaf("owner-two"))
        with self.assertRaises(SequenceOperationError):
            repeat_from_slot(slot, 2)
        self.assertIs(slot.state, SlotState.LIVE)
        self.assertTrue(slot.require_live().token.active)

    def test_dynamic_repeat_rejects_affine_even_when_observed_count_is_zero_or_one(self) -> None:
        for count in (0, 1):
            slot = Slot.live(Value.affine_leaf(f"owner-dynamic-{count}"))
            with self.assertRaises(SequenceOperationError):
                repeat_from_slot(slot, count, statically_exact_count=False)
            self.assertIs(slot.state, SlotState.LIVE)

    def test_unrestricted_repeat_materializes_n_minus_one_copies_and_original(self) -> None:
        slot = Slot.live(Value.scalar_value("x"))
        result = repeat_from_slot(slot, 4)
        self.assertIs(slot.state, SlotState.MOVED)
        self.assertEqual([child.scalar for _, child in result.edges], ["x", "x", "x", "x"])
        self.assertEqual(len({id(child) for _, child in result.edges}), 4)

    def test_preserving_index_and_slice_require_full_sequence_unrestricted(self) -> None:
        source = Value.sequence((Value.scalar_value(1), Value.scalar_value(2)))
        indexed = preserving_index(source, 1)
        sliced = preserving_slice(source, 0, 2)
        self.assertEqual(indexed.scalar, 2)
        self.assertEqual([child.scalar for _, child in sliced.edges], [1, 2])
        self.assertEqual([child.scalar for _, child in source.edges], [1, 2])

        affine_source = Value.sequence((Value.scalar_value(1), Value.affine_leaf("owner-index")))
        with self.assertRaises(SequenceOperationError):
            preserving_index(affine_source, 0)
        with self.assertRaises(SequenceOperationError):
            preserving_slice(affine_source, 0, 0)

    def test_consuming_iterator_moves_each_affine_element_exactly_once(self) -> None:
        source = Slot.live(Value.sequence((
            Value.affine_leaf("owner-i0"),
            Value.affine_leaf("owner-i1"),
        )))
        iterator = ConsumingIterator.from_slot(source)
        self.assertIs(source.state, SlotState.MOVED)
        first = iterator.next_value()
        second = iterator.next_value()
        self.assertEqual(first.token.owner_id, "owner-i0")
        self.assertEqual(second.token.owner_id, "owner-i1")
        self.assertIsNone(iterator.next_value())
        self.assertIsNone(iterator.next_value())

    def test_iterator_drop_retires_remaining_owners_once(self) -> None:
        source = Slot.live(Value.sequence((
            Value.affine_leaf("owner-used"),
            Value.affine_leaf("owner-remain"),
        )))
        iterator = ConsumingIterator.from_slot(source)
        used = iterator.next_value()
        remaining_token = iterator.items[0].token
        iterator.drop_remaining()
        self.assertTrue(used.token.active)
        self.assertFalse(remaining_token.active)
        self.assertEqual(len(iterator.items), 0)

    def test_snapshot_is_dormant_copyable_evidence_not_token_authority(self) -> None:
        values = [Value.record((
            ("handle", Value.affine_leaf("owner-snap")),
            ("payload", Value.scalar_value("ok")),
        ))]
        image = build_snapshot(values)
        self.assertEqual(image.schema, 2)
        self.assertEqual(image.owners, (OwnerEvidence("$root[0].handle", "owner-snap"),))
        assert_snapshot_copy_is_dormant(image)
        copied = deepcopy(image)
        self.assertEqual(copied.canonical_bytes(), image.canonical_bytes())
        self.assertNotIn(b"OwnerToken", image.canonical_bytes())

    def test_snapshot_duplicate_owner_is_rejected(self) -> None:
        shared_id_values = [
            Value.affine_leaf("owner-dupe"),
            Value.affine_leaf("owner-dupe"),
        ]
        with self.assertRaises(SnapshotError):
            build_snapshot(shared_id_values)

        tampered = SnapshotImage(
            schema=2,
            roots=(),
            owners=(
                OwnerEvidence("$root[0]", "owner-dupe"),
                OwnerEvidence("$root[1]", "owner-dupe"),
            ),
        )
        with self.assertRaises(SnapshotError):
            validate_snapshot(tampered)

    def test_restore_only_into_empty_or_replace_frozen(self) -> None:
        old_value = Value.affine_leaf("owner-old")
        old_token = old_value.token
        world = RuntimeWorld.from_values((old_value,))
        new_image = build_snapshot((Value.affine_leaf("owner-new"),))

        with self.assertRaises(SnapshotError):
            world.install_into_empty(new_image)
        with self.assertRaises(SnapshotError):
            world.replace_frozen(new_image)

        world.freeze()
        world.replace_frozen(new_image)
        self.assertIs(world.state, WorldState.ACTIVE)
        self.assertFalse(old_token.active)
        self.assertEqual(set(world.active_owners), {"owner-new"})
        self.assertTrue(world.active_owners["owner-new"].active)

        empty = RuntimeWorld()
        empty.install_into_empty(new_image)
        self.assertIs(empty.state, WorldState.ACTIVE)
        self.assertEqual(set(empty.active_owners), {"owner-new"})

    def test_failed_replace_validation_leaves_frozen_world_unchanged(self) -> None:
        old_value = Value.affine_leaf("owner-old")
        old_token = old_value.token
        world = RuntimeWorld.from_values((old_value,))
        world.freeze()
        tampered = SnapshotImage(
            schema=2,
            roots=(),
            owners=(
                OwnerEvidence("$root[1]", "owner-x"),
                OwnerEvidence("$root[0]", "owner-y"),
            ),
        )
        with self.assertRaises(SnapshotError):
            world.replace_frozen(tampered)
        self.assertIs(world.state, WorldState.FROZEN)
        self.assertTrue(old_token.active)
        self.assertEqual(set(world.active_owners), {"owner-old"})


if __name__ == "__main__":
    unittest.main(verbosity=2)
