# Review package intake

`docs/reviews/` is the repository review inbox. A returned contract ZIP may be
dropped directly into this directory; a ZIP at this level is therefore always
unprocessed intake, not an implementation-ready package.

A ZIP attached directly to the active Codex task is the same intake class. It
must be inspected from the attachment, then retained without modification at
`packages/zips/<archive>.zip` when repository retention is intentional. Its
validated, searchable contents are extracted to
`packages/<zip-basename>/`; the retained ZIP remains the byte authority. Its
verified path/hash must otherwise be recorded in the package-specific
implementation intake note. An attachment or filename is never
implementation-readiness evidence by itself.

When every archive member is below one top-level directory and the archive has
no top-level file, extraction removes that redundant wrapper. Otherwise the
archive member paths are retained exactly. In both cases, extracted files must
remain byte-identical to their ZIP members.

Repository evidence uses the full Git commit SHA. Jujutsu identities are not
part of current intake or readiness evidence, even when an older request asks
for them. Returned sidecars belong inside the ZIP rather than beside it.

## Intake procedure

Apply intake when the task concerns a returned ZIP, package readiness, or
package-driven implementation. It is not a prerequisite for unrelated code,
documentation, or instruction edits.

At the start/resume and reviewable push cut of package work, check the review
inbox and archive paths changed since the last recorded intake revision. On the
first intake, or without a trustworthy baseline, enumerate the review archive
inventory once. Include repository-root ZIPs and active attachments. Use the
ledger and immutable Git blob identities to locate new, changed, unclassified,
and task-selected packages; do not rehash/reopen every unchanged historical ZIP.
Dirty, untracked, attached, or externally stored bytes are not proven unchanged
by a filename, timestamp, or committed blob identity: hash those bytes before use.

For each package requiring intake:

1. Compute SHA-256 and byte length; compare with the relevant intake record.
   Inspect member paths before extraction; reject traversal, absolute/escaping
   paths, unsafe links, and colliding destinations without overwriting user files.
2. Verify the member set, internal manifest and member hashes, request copies,
   `FINAL_STATUS`, `OPEN_QUESTIONS`, schemas, matrices, traceability, repository
   evidence, validation claims, and non-goals required by its request.
3. Classify as implementation-ready, active, blocked by a named request,
   superseded/duplicate, or invalid as delivered. Integrity alone does not prove
   readiness: reconcile the complete applicable contract with current consumers.
4. Retain an inspected inbox ZIP unchanged in `packages/zips/` when repository
   retention is intentional, safely extract to `packages/<zip-basename>/`, and
   record the archive hash, Git blob identity when available, inspected revision,
   classification, dependencies, and next action in the package intake note.

A prior integrity check can be reused only for the same verified bytes; a prior
readiness decision also needs unchanged applicable contracts and consumer
assumptions. Re-evaluate affected readiness when those change, even if the ZIP
bytes do not. Missing identity/evidence means verify again, not assume success.
Do not move, extract, or reclassify unrelated historical packages as side work.

The archive filename is not evidence that a package is final or implementable.
One ZIP may have at most one implementation worker. Independent design
requests may run in parallel, while production integration follows the
dependency order recorded in the intake ledger.

## Directory ownership

- `requests/`: independently throwable design requests. Do not present a new
  named request to the user until its Markdown file exists here.
- `packages/zips/`: unchanged returned ZIPs that have been inspected and
  entered in the implementation intake ledger; these are the retained-byte
  authority.
- `packages/<zip-basename>/`: safely extracted, searchable contents of a
  retained package ZIP. These must be regenerated from the paired ZIP rather
  than edited independently.
- `designs/`: accepted review/design material that is useful outside an
  individual returned package. A design directory that retains a ZIP stores it
  unchanged in its `zips/` child directory and safely extracts the searchable
  contents into that design directory itself. A single redundant archive root
  directory is removed only when every member is below it and the archive has
  no top-level file; otherwise member paths are retained exactly. Extracted
  design contents must be regenerated from the paired ZIP rather than edited
  independently.
- repository-root `*.zip`: temporary inbox only; intake and move it promptly.

Extracted package and design files are frozen mirrors, including historical
request copies and ledgers. They may contain repository paths that were valid
when the archive was produced. Preserve those bytes and update maintained
repository navigation around them; do not rewrite a mirrored file merely to
make an old embedded path current.
