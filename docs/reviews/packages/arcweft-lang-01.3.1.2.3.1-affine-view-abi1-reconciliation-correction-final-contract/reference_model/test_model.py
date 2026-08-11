from __future__ import annotations
import unittest
from model import (
    ActivationDomain, AlreadyActive, Certificate, ContractError, Cursor, CursorError,
    HolderMismatch, Lease, Requirement, Role, Span, StaticError, Transfer,
    selected_subject, validate_requirement, validate_spans, validate_view_input,
)

class CorrectionModelTests(unittest.TestCase):
    def test_domain_rejects_second_driver(self) -> None:
        d = ActivationDomain(); d.activate_empty("e")
        with self.assertRaises(AlreadyActive): d.activate_empty("e")

    def test_replacement_requires_exact_holder(self) -> None:
        d = ActivationDomain(); lease = d.activate_empty("e")
        with self.assertRaises(HolderMismatch): d.replace(Lease("e", 99, 0))
        next_lease = d.replace(lease)
        self.assertEqual(next_lease.generation, 1)

    def test_replacement_may_switch_to_inactive_execution(self) -> None:
        d = ActivationDomain(); lease = d.activate_empty("old")
        next_lease = d.replace(lease, "new")
        self.assertEqual(next_lease.execution, "new")
        self.assertNotIn("old", d.active)

    def test_separate_domains_are_separate_universes(self) -> None:
        self.assertEqual(ActivationDomain().activate_empty("e").execution, ActivationDomain().activate_empty("e").execution)

    def test_cursor_valid_and_first_mint(self) -> None:
        c = Cursor(5); c.validate((0, 4)); owner, c2 = c.mint()
        self.assertEqual((owner, c2.next_ordinal), (5, 6))

    def test_cursor_rejects_reuse(self) -> None:
        with self.assertRaises(CursorError): Cursor(5).validate((5,))

    def test_cursor_exhaustion(self) -> None:
        owner, exhausted = Cursor(2**64 - 1).mint()
        self.assertEqual(owner, 2**64 - 1)
        with self.assertRaises(CursorError): exhausted.mint()

    def test_static_requirement_success(self) -> None:
        validate_requirement([Requirement((0,10), "r")], [Certificate((0,10), "authored_required", "r")])

    def test_static_requirement_missing_certificate(self) -> None:
        with self.assertRaises(StaticError): validate_requirement([Requirement((0,10), "r")], [])

    def test_authored_origin_without_requirement(self) -> None:
        with self.assertRaises(StaticError): validate_requirement([], [Certificate((0,10), "authored_required", "r")])

    def test_automatic_has_no_requirement(self) -> None:
        validate_requirement([], [Certificate((0,10), "automatic", None)])

    def test_strict_nested_spans(self) -> None:
        validate_spans([Span("outer",0,10), Span("inner",2,5), Span("sib",10,12)])
        self.assertEqual(selected_subject([Span("outer",0,10), Span("inner",2,5)], 3), "outer")

    def test_partial_overlap_rejected(self) -> None:
        with self.assertRaises(StaticError): validate_spans([Span("a",0,5), Span("b",3,8)])

    def test_pure_input_copy_unrestricted(self) -> None:
        validate_view_input(Role.PURE, "parameter", Transfer.COPY, True)
        with self.assertRaises(ContractError): validate_view_input(Role.PURE, "parameter", Transfer.MOVE, True)
        with self.assertRaises(ContractError): validate_view_input(Role.PURE, "parameter", Transfer.COPY, False)

    def test_handler_input_moves(self) -> None:
        validate_view_input(Role.HANDLER, "handler_input", Transfer.MOVE, False)
        with self.assertRaises(ContractError): validate_view_input(Role.HANDLER, "handler_input", Transfer.COPY, True)

    def test_handler_capture_copies_unrestricted(self) -> None:
        validate_view_input(Role.HANDLER, "parameter", Transfer.COPY, True)
        with self.assertRaises(ContractError): validate_view_input(Role.HANDLER, "parameter", Transfer.COPY, False)

if __name__ == "__main__": unittest.main()
