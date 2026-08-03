# Review package intake

`docs/reviews/` is the repository review inbox. A returned contract ZIP may be
dropped directly into this directory; a ZIP at this level is therefore always
unprocessed intake, not an implementation-ready package.

A ZIP attached directly to the active Codex task is the same intake class. It
must be inspected from the attachment, then retained under `packages/` when
repository retention is intentional; its verified path/hash must otherwise be
recorded in the package-specific implementation intake note. An attachment or
filename is never implementation-readiness evidence by itself.

Repository evidence uses the full Git commit SHA. Jujutsu identities are not
part of current intake or readiness evidence, even when an older request asks
for them. Returned sidecars belong inside the ZIP rather than beside it.

## Intake procedure

At task start and at every reviewable push cut point:

1. enumerate `docs/reviews/**/*.zip`, including the inbox root and
   `packages/`;
2. compute each archive's SHA-256 and compare it with
   `docs/implementation/*reviews-zip-intake*.md` and package-specific intake
   notes;
3. inspect the archive's request, final contract, manifest, implementation
   status, validation evidence, and explicit non-goals;
4. classify it as implementation-ready, active, blocked by a named request,
   superseded/duplicate, or invalid as delivered;
5. move an inspected inbox ZIP to `docs/reviews/packages/` and record its hash,
   state, dependencies, and next action in the intake ledger.

The archive filename is not evidence that a package is final or implementable.
One ZIP may have at most one implementation worker. Independent design
requests may run in parallel, while production integration follows the
dependency order recorded in the intake ledger.

## Directory ownership

- `requests/`: independently throwable design requests. Do not present a new
  named request to the user until its Markdown file exists here.
- `packages/`: returned ZIPs that have been inspected and entered in the
  implementation intake ledger.
- `designs/`: accepted review/design material that is useful outside an
  individual returned package.
- repository-root `*.zip`: temporary inbox only; intake and move it promptly.
