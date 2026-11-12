# Access UX (branch `feat/access-ux`): what to know before it is deployed

Built from `43fa3fe50` (the live 43fa3fe50 image). Spec: `2026-11-12-access-ux-spec.md`; screenshots: `ux-2026-11-12/before` and `/after`.

## What it changes on a server

* **One migration**, `m20261116_000000_access_warning_reviews` (one table, `access_warning_reviews`; an older image never reads it).
  Manual down: `crates/migrations/manual-down/m20261116_000000_access_warning_reviews.sql` (copy: `research/2026-11-16-access-warning-reviews-manual-down.sql`);
  it only forgets which warnings were dismissed (they all show again). Its test runs that very file.
* **GraphQL, additive**: `dismissAccessWarnings`, `restoreAccessWarnings`, `accessWarnings(includeDismissed)`, new warning kinds
  (`ACCOUNT_LOCKED`, `LEFTOVER_DEVICES`, `BOOKS_WAITING`), `AccessWarning.lastAt/dismissedAt`, `includeEnded` on links, invites and devices,
  `personId` on devices, `othersOnly` on sign-ins, `AccessSignIn.agentLabel`, `AccessAuditEntry.targetLabel`. Defaults keep the old behaviour.
* **No edge change**: nothing under `deploy/public-edge/` is touched, so no nginx reload. The web app is in the image.

## Deploy (AGENTS.md routine)

1. Build on l5 `bash scripts/fork/build-local.sh stump:fork-<rev>`, load it on g3.
2. Jobs = 0, `config.backup-<UTC>-db` (+ copy the manual-down SQL next to it), compose backup, set the `x-stump-image` line (it pins `stump-fork` and `stump-intake-probe`), comment with the rollback image.
3. Try the migration first on a copy of the live database in an isolated container (up, then the manual-down file, `pragma integrity_check`).
4. `docker compose up -d --no-build stump-fork stump-intake-probe`; `docker logs` clean, probe `ready`, home/tailnet 200, public 404.
5. Look at Settings → Public access (the attention list, a person's sheet, *Invite someone*) and the badge.

Rollback: the previous image (the new table is ignored), or the manual-down SQL then the previous image.

## Merging with the other branches

* `feat/public-login` and `feat/app-ux` also touch `packages/i18n/src/locales/{en-US,es-ES}.json` (merge by hand: both add keys) and the generated client:
  after merging, **regenerate** `crates/graphql/schema.graphql` (`scripts/fork/schema.sh`) and the client (`yarn codegen` in `packages/graphql`); never merge those two by hand.
* `AppSidebar.tsx`: one line (the *Add books* label uses `sidebar.buttons.addBooks`).
* `IntakeScene.tsx`, `IntakeItemCard.tsx`, `IntakeNotices.tsx`: the page-level words moved to `intakeScene.*` keys.

## Not done (the spec asked, time did not allow)

* *Add books* and the cards of *Book approvals*: the words **inside** a book's card (the review form, tags, folders, the "send back" dialog, the drop zone) are still English only; the page-level words, the sidebar label and the new lines are translated. Compact rows with one action each were not built.
* The *Share* button is shown first on phones only where the browser can share (as the spec says), but the QR/Share layout was not measured on a real phone.
