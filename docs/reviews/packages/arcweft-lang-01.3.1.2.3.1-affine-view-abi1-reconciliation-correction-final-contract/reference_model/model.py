from __future__ import annotations
from dataclasses import dataclass
from enum import Enum

class ContractError(Exception):
    pass

class AlreadyActive(ContractError):
    pass

class HolderMismatch(ContractError):
    pass

class CursorError(ContractError):
    pass

class StaticError(ContractError):
    pass

class Transfer(str, Enum):
    COPY = "copy"
    MOVE = "move"

class Role(str, Enum):
    PURE = "pure"
    DIRECT_AWAIT = "direct_await"
    HANDLER = "handler"

@dataclass(frozen=True)
class Lease:
    execution: str
    holder: int
    generation: int

class ActivationDomain:
    def __init__(self) -> None:
        self.active: dict[str, Lease] = {}
        self.next_holder = 0

    def activate_empty(self, execution: str) -> Lease:
        if execution in self.active:
            raise AlreadyActive(execution)
        lease = Lease(execution, self.next_holder, 0)
        self.next_holder += 1
        self.active[execution] = lease
        return lease

    def replace(self, current: Lease, candidate_execution: str | None = None) -> Lease:
        if self.active.get(current.execution) != current:
            raise HolderMismatch(current.execution)
        candidate = current.execution if candidate_execution is None else candidate_execution
        if candidate != current.execution and candidate in self.active:
            raise AlreadyActive(candidate)
        del self.active[current.execution]
        generation = current.generation + 1 if candidate == current.execution else 0
        lease = Lease(candidate, self.next_holder, generation)
        self.next_holder += 1
        self.active[candidate] = lease
        return lease

@dataclass(frozen=True)
class Cursor:
    next_ordinal: int | None

    def validate(self, owner_ordinals: tuple[int, ...]) -> None:
        if self.next_ordinal is None:
            return
        if any(owner >= self.next_ordinal for owner in owner_ordinals):
            raise CursorError((owner_ordinals, self.next_ordinal))

    def mint(self) -> tuple[int, "Cursor"]:
        if self.next_ordinal is None:
            raise CursorError("exhausted")
        owner = self.next_ordinal
        if owner == (2**64 - 1):
            return owner, Cursor(None)
        return owner, Cursor(owner + 1)

@dataclass(frozen=True)
class Requirement:
    subject: tuple[int, int]
    digest: str

@dataclass(frozen=True)
class Certificate:
    subject: tuple[int, int]
    origin: str
    requirement: str | None


def validate_requirement(requirements: list[Requirement], certificates: list[Certificate]) -> None:
    by_subject = {r.subject: r for r in requirements}
    if len(by_subject) != len(requirements):
        raise StaticError("duplicate requirement")
    cert_by_subject = {c.subject: c for c in certificates}
    if len(cert_by_subject) != len(certificates):
        raise StaticError("duplicate certificate")
    for subject, req in by_subject.items():
        cert = cert_by_subject.get(subject)
        if cert is None or cert.origin != "authored_required" or cert.requirement != req.digest:
            raise StaticError("missing or mismatched authored certificate")
    for subject, cert in cert_by_subject.items():
        if cert.origin == "authored_required" and subject not in by_subject:
            raise StaticError("authored origin without requirement")
        if cert.origin == "automatic" and cert.requirement is not None:
            raise StaticError("automatic requirement field")

@dataclass(frozen=True)
class Span:
    subject: str
    start: int
    end: int


def validate_spans(spans: list[Span]) -> None:
    ordered = sorted(spans, key=lambda s: (s.start, -s.end, s.subject))
    stack: list[Span] = []
    for span in ordered:
        if not span.start < span.end:
            raise StaticError("empty span")
        while stack and span.start >= stack[-1].end:
            stack.pop()
        if stack and span.end > stack[-1].end:
            raise StaticError("partial overlap")
        stack.append(span)


def selected_subject(spans: list[Span], pc: int) -> str | None:
    containing = [s for s in spans if s.start <= pc < s.end]
    if not containing:
        return None
    return min(containing, key=lambda s: (s.start, -s.end)).subject


def validate_view_input(role: Role, source: str, transfer: Transfer, unrestricted: bool) -> None:
    if role in {Role.PURE, Role.DIRECT_AWAIT}:
        if transfer is not Transfer.COPY or not unrestricted:
            raise ContractError("retained input must copy unrestricted")
        return
    if role is Role.HANDLER and source == "handler_input":
        if transfer is not Transfer.MOVE:
            raise ContractError("handler input must move")
        return
    if transfer is not Transfer.COPY or not unrestricted:
        raise ContractError("handler captures must copy unrestricted")
