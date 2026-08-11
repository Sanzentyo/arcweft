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

At task start and at every reviewable push cut point:

1. enumerate `docs/reviews/**/*.zip`, including the inbox root and
   `packages/zips/`;
2. compute each archive's SHA-256 and compare it with
   `docs/implementation/*reviews-zip-intake*.md` and package-specific intake
   notes;
3. inspect the archive's request, final contract, manifest, implementation
   status, validation evidence, and explicit non-goals;
4. classify it as implementation-ready, active, blocked by a named request,
   superseded/duplicate, or invalid as delivered;
5. move an inspected inbox ZIP unchanged to `docs/reviews/packages/zips/`,
   safely extract its contents to
   `docs/reviews/packages/<zip-basename>/`, and record its hash, state,
   dependencies, and next action in the intake ledger.

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
