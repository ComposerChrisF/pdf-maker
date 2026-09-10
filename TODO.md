# TODO

## Priority right now

**`plan-0003` (`--tile`) has LANDED (2026-09-09, v0.21.0), and so has the documentation pass
that waited on it (`bug-0012`, v0.21.2).**  The banner-printing arc is closed: `--tile` ships,
README.md and CLAUDE.md describe the tool again, `--watermark` and `--dry-run` gained the
`--help` text they were missing, and `~/.claude/skills/pdf-tools/SKILL.md` was refreshed from
v0.13.2 to v0.21.x.  Nothing in the queue below is blocked on anything else now.

**What is next, in order:** `bug-0017` (a false `ERROR` on every imposition run — cheap, and it
is training readers to ignore the message that would announce a real regression), then the rest
of Phase B, then Phase C’s two ruled-but-unimplemented items (`bug-0002`, `bug-0003`).

**Two of those carry a README follow-up.**  `bug-0002` (`--draw-line` `width` honors `units=`)
and `bug-0003` (duplicate pages honored) are each documented in README.md as _current behavior
plus the pending change_ — the “Drawing / `--draw-line`” table and the “Page Specifications”
duplicates paragraph.  The commit that lands either fix updates its README line in the same
commit; that obligation moved here when `bug-0012` was closed.

**Historical note — the medpdf half of the critical path (2026-09-09).**  medpdf 0.13.0 (commit
`6208ff4`) fixed their bug-0023, bug-0024 and bug-0039; this repo is on it and the floor is
`medpdf = "0.13"`.  What that adoption cost and uncovered:

- It **broke `--booklet flip=short_edge`** — the back-page arithmetic hand-compensated the old
  contract and began double-compensating, throwing every back page clean off the sheet.  Found
  by the pdf-orchestrator and medpdf sessions sweeping for the pattern, fixed here, and pinned
  by a test that fails when reverted.
- It **closed bug-0007** (orphaned streams) — verified gone from imposed output; the one
  remaining `--validate` warning is the `/ObjStm` false positive that is pdf-dump’s to fix.
- It **opened bug-0018** — imposition sized cells from the pre-rotation MediaBox, so a
  `/Rotate 90` source was placed upright but mis-scaled.  Fixed in v0.19.0, ahead of `--tile`,
  which derives its grid from effective dimensions.

## Open plans

Proposed changes — options, not obligations — in `plans/`, numbered per
`~/.claude/rules/plan-files.md`.  TODO.md is the ordering index for plans as well as bugs;
**the order below _is_ a priority ruling**, unlike the 2026-08-12 note it replaces.

- [ ] **plan-0001 — `--lossy-text`**, exposing medpdf’s existing `WatermarkParams::lossy_text`
  opt-out.  Small: one flag, no medpdf change for the stderr-warning form.  Worth doing early
  because the current error message advises “enable lossy text substitution” and names no
  flag, so the advice is unactionable today.  Carries one decision for Chris (global flag vs.
  per-watermark key; the plan recommends global).
- [ ] **plan-0002 — `--recompress-images`**, exposing the shipped
  `medpdf_image::recompress::recompress_images()`.  Larger, and the scoping design is the hard
  part — pdf-orchestrator shipped the same feature and carries two open bugs on exactly that
  (bug-0037 scope, bug-0035 junk-enables-lossy).  Read those before implementing.

## Bug-fix queue

**Seven** bug reports live in `bugs/` — bug-0002, bug-0003, bug-0010, bug-0011, bug-0013,
bug-0015 and bug-0017.  Eleven were fixed on 2026-09-09 and deleted per the bug-reports
lifecycle.  (bug-0016 was filed
2026-07-23, after the original deep review, and was missing from this index until 2026-09-09;
bug-0017 was filed 2026-09-09 from the plan-0003 prerequisite review.)
IDs are alphabetical by slug per the bug-reports rule; they encode nothing about priority.
**Work them in the phase order below**, not in ID order.  Each report is self-contained:
description, verified repro (Rust-test-ready), suggested fix, and why it works.  Reports marked
_decision_ need Chris’s ruling before any code changes; do not guess.

### Phase 0 — the `--tile` critical path (start here)

Three pdf-maker prerequisites, two medpdf prerequisites, and one long-lead item that travels
with them.  All are on the imposition path; none is waiting on a ruling.

- [x] **bug-0005 — DONE 2026-09-09.**  Both imposition flags now carry a full key table under
  `long_help`, `--blank-page` offers `legal`, and the `flip`-by-orientation table graduated in
  from bug-0001.  Pinned by drift guards that check the help text against the parser’s own
  `NUP_KEYS` / `BOOKLET_KEYS` — **not against a copy of them** — so adding a key without
  documenting it now fails the build.  Verified by deleting a key from the help and watching
  the test name it.  Scope was extended one line beyond the report: an unknown key now lists
  the valid ones, for every spec type, since that is the same fault class and the error is
  where a caller working from stale docs actually looks.
- [x] **bug-0006 — DONE 2026-09-09.**  Geometry now goes through `CellGeometry::compute`, the
  only constructor, so construction _is_ the validation and downstream code can divide without
  re-checking — this is the type `--tile` reuses.  Non-positive cell is exit 1 naming the whole
  arithmetic; `--booklet` got the matching `binding_margin` guard.  The reporting obligation is
  discharged on both surfaces: stderr progress and a new `--json` `imposition_geometry` object.
- [x] **bug-0009 — DONE 2026-09-09**, all four items including `max_dpi`.  `KvParser` gained
  `optional_positive` / `required_positive` / `optional_alpha`, so `TileSpec` is written to the
  settled pattern rather than retrofitted.  The negative-offset ruling is pinned by
  `offsets_may_be_negative`, which fails if someone later applies the extent rule uniformly —
  verified by doing exactly that and watching it fail.
- [x] **medpdf bug-0023, bug-0024 and bug-0039 — DONE.**  Shipped in medpdf 0.13.0
  (`6208ff4`).  `place_page` now anchors the placed bounding box at `(x, y)` for any MediaBox
  origin and any rotation, and honors the source `/Rotate`; the orphaned-stream leak is gone.
  Two new helpers replace the geometry pdf-maker used to model itself:
  `medpdf::get_page_effective_size` (display dimensions, `/Rotate` applied) and
  `medpdf::placed_page_size(doc, page, scale, rotation)` (the real footprint, computed from the
  same transform `place_page` emits).  `get_page_media_box` is now explicitly the _pre-rotation_
  box.
- [x] **medpdf floor raised to `0.13` — DONE.**  Necessary, not cosmetic: an older 0.12.x
  silently restores the previous placement.
- [x] **The `--booklet` regression that adoption caused — FIXED.**  `apply_booklet` passed
  `(cx + w, cy + h)` for the rotated back side to undo the old contract’s rotation excursion;
  against 0.13.0 that double-compensates and puts every back page off the sheet.  Now `(cx, cy)`
  for both branches.  Pinned by `cli_booklet_back_pages_land_on_the_sheet`, which asserts on the
  destination rectangle — **the `cm` scale coefficients are unchanged by this fault**, so the
  obvious sign-of-scale assertion passes broken _and_ fixed.  Verified failing on revert.
- [x] **bug-0018 — DONE 2026-09-09.**  Both sites now measure the page as placed:
  imposition via `medpdf::placed_page_size(doc, page, 1.0, rotation)` and `--pad-to` via
  `get_page_effective_size`.  Pinned by two CLI tests built on a real `/Rotate 90` fixture
  (`medpdf::set_page_rotation` makes one in four lines), each verified to fail when its own
  site is reverted — the imposition one reporting the exact `396x306 in a 306x396 cell`
  overflow the report measured.  The rotation argument is 0 there deliberately, and the
  comment says why: N-up never rotates and booklet’s 180° preserves the footprint, but a 90°
  placement rotation **transposes** it, so `--tile` must re-measure with its own rotation
  rather than reuse that call.

- [x] **plan-0003 — IMPLEMENTED 2026-09-09.**  `--tile` ships in v0.21.0; the plan file is
  deleted per the plan-files lifecycle, with its durable content graduated into `--help`
  (the key table, the overlap rationale, the auto-orientation rule) and the CHANGELOG.
  The README half belongs to bug-0012’s doc pass.

**bug-0008 is _not_ a prerequisite**, contrary to plan-0003’s first draft.  It concerns
`auto_grid` mapping an `n` onto a grid; `--tile` derives its grid from geometry and never calls
`auto_grid`.  It stays in Phase A on its own merits.

### Phase A — RULED 2026-09-09; implementation moves to Phase C

All six rulings received and recorded in their reports.  One item still needs Chris — a
physical print — and one needs his pick between two options; both are called out below.  The
rulings themselves are settled and no longer block anything.

- [x] **bug-0003** — **honor duplicates.**  `"1,1"` yields two copies of page 1.  Needs a medpdf
  API change: duplicates are invisible to pdf-maker today because `parse_page_spec` collapses
  them before pdf-maker ever sees the list.  Handed to the medpdf session — see Phase C.
- [x] **bug-0008 — RULED _and_ SHIPPED.**  `n` restricted to `1, 2, 4, 6, 8, 9, 16`; anything
  else is a usage error naming the set and pointing at `cols=`/`rows=`.  `auto_grid(2)` changed
  to side-by-side landscape, approved 2026-09-09 on condition an override exists — it does, and
  needs no new syntax: **`cols=1,rows=2` reproduces the old `n=2` byte for byte**, verified and
  pinned by `test_nup_stacked_portrait_still_available_explicitly`.  Report deleted.
- [x] **bug-0002** — **convert `width` like the coordinates.**  `--draw-line` `width` respects
  `units=`.  Behavior change; note in CHANGELOG.
- [x] **bug-0009 item 4** — **`max_dpi=none` is the public spelling** for “no downsampling”,
  mapped internally to an enum (or 0) so the sentinel never reaches the CLI surface.  A numeric
  `max_dpi` below 1.0 — including 0 — is an invalid-value error.  The existing
  `test_draw_image_spec_max_dpi_zero`, which asserts today’s acceptance of `0`, gets rewritten
  rather than deleted: it becomes the assertion that `0` is now rejected and `none` accepted.
- [x] **bug-0016 — RULED _and_ SHIPPED.**  Chris ruled it a medpdf feature; medpdf 0.14.0
  delivered plan-0002 Tier 1 the same day, and pdf-maker needed no change beyond the floor.
  Verified on both text paths and the report is deleted.  One residue moved to bug-0012: `\n`
  and `\t` must not be documented as one feature, because only `\n` renders.
- [x] **bug-0001 — CONFIRMED AT THE PRINTER _and_ FIXED.**  Chris printed both files
  2026-09-09: pages 2–3 upside down on **both**, which is precisely the symmetric failure the
  analysis predicted and the strongest possible confirmation — the two settings were wrong for
  opposite reasons.  Compensation is now keyed on the (sheet orientation, flip) pair:
  `long_edge` rotates on landscape, `short_edge` on portrait, `none` never.  Report and fixture
  directory deleted.  The durable half — which `flip` value goes with which paper orientation —
  graduated into bug-0012 as a table for `--help` and the README, since it outlives the bug.
  **Optional loop-closer:** a confirmation pair is at
  `<scratchpad>/duplex-confirm/CONFIRM-landscape-{LONG,SHORT}-edge.pdf` if you want to prove the
  fix on paper rather than in a content stream.  Expect backs upright on both this time.

### Phase B — independent code fixes, in severity order (no ruling needed, off the `--tile` path)

Each fix lands with a test that fails when the fix is reverted.

- [x] **bug-0004 and bug-0014 — DONE 2026-09-09**, ahead of `--tile` because bug-0004 is a
  **security** fault and Chris’s standing order puts corruption and security bugs ahead of
  feature work.  Both flags now declare a clap dependency on a password; permission names are
  validated by a `value_parser`; the odd-positional-count check is reported through clap.  The
  exit-code contract is pinned from both sides — statically-invalid invocations exit 2, an
  out-of-range page still exits 1.
- [ ] **bug-0011** — `--pad-last-page-file` without `--pad-to` silently ignored.  One-line clap
  `requires`.
- [ ] **bug-0010** — `--overlay src_page=` / `--pad-last-page-file page=` validated late or
  never, with errors that name no flag, file, or page count.  Worth doing before `--tile`
  ships if it is cheap: `--tile`’s `pages=` key is another `page_spec::expand` consumer and
  would inherit the better errors.
- [ ] **bug-0013** — unreadable input misreported as “does not exist” (two-state probe in
  `src/paths.rs`).
- [ ] **bug-0017** — the XMP metadata stream is built with a struct literal that bypasses
  `lopdf::Stream::new`, so it carries no `/Length`; every imposition run logs a spurious
  `ERROR ... missing the Length entry` because `impose_pages` round-trips through a raw
  `save_to` that skips the `compress()` which had been repairing it.  One-line fix
  (`src/main.rs:208`).  Worth doing early despite sitting off the critical path: a false
  ERROR on a run that exits 0 trains both a human and an agent to ignore exactly the message
  that would announce a real `/Length` regression — the one this repo already pins tests
  against.  Pairs naturally with bug-0015, which is the other XMP defect.
- [ ] **bug-0015** — XMP dates not ISO 8601.

### Phase C — ruling-dependent code (all rulings now in hand)

- [ ] **bug-0003** implementation — needs a medpdf API change (duplicates are invisible to
  pdf-maker today); coordinate with the sibling `../medpdf` workspace and its release flow
  (`PUBLISHING.md`).
- [x] **bug-0008 — done**, see Phase A.  Both tests that pinned the old value were updated;
  `test_nup_spec_custom_paper` gained an explicit `orientation=portrait` so it tests unit
  conversion only, instead of silently testing conversion and orientation at once.
- [x] **bug-0009 item 4 — DONE 2026-09-09**, with the rest of bug-0009 in v0.18.0; this entry
  was stale bookkeeping, corrected 2026-09-09.  Verified at the CLI: `max_dpi=none` parses,
  `max_dpi=0` is a usage error naming the remedy.  The sentinel never reaches the CLI surface —
  `none` maps to 0.0 internally, so no caller has to know a magic value exists.
- [x] **bug-0016 — done**, see Phase A.
- [ ] **bug-0002** implementation (behavior change; note in CHANGELOG).
- [x] **bug-0001 — done**, see Phase A.

### Phase D — cross-repo

- [ ] File in **pdf-dump’s** repo (not here): its validator falsely flags `/ObjStm` containers
  as “unreachable from trailer” on every modern-format PDF.  Still reproducible: an imposed
  output now validates with exactly one warning, and it is this false positive.

### Phase E — documentation (last, once behavior settles)

- [x] **bug-0012 — DONE 2026-09-09 (v0.21.2).**  The spec-drift pass over README.md and
  CLAUDE.md, written last exactly as planned, so it describes settled behavior including every
  Phase A ruling and `--tile`.  README gained sections for imposition (all three modes, with the
  `flip`-by-orientation table and the tile guards), blank pages, drawing (`--draw-rect`,
  `--draw-line`, `--draw-image` — previously undocumented in full), `--no-subset`, the complete
  watermark parameter table, the full named-color set, the text escapes, the real encryption
  flags with their password gate, a corrected `--json` sample, and a seven-phase pipeline.
  CLAUDE.md gained the `src/spec_types/` tree, the seven-phase pipeline with the
  imposition-slot consequence, and three new contract invariants.
  **Scope went one step past doc-only, deliberately:** two of the report’s items asked for
  `--help` text, not just README — so `--watermark` gained a `long_help` (keys, colors, escapes)
  and `--dry-run` gained one stating the single-branch contract.  The watermark help is
  drift-guarded against `WATERMARK_KEYS` by a new test, the same mechanism bug-0005 built.
  Two items could not settle, and their obligation moved to the Priority section above: the
  `bug-0002` and `bug-0003` behaviors are documented as current-plus-pending, and each fixing
  commit owes its README line.
- [x] **DONE 2026-09-09:** `~/.claude/skills/pdf-tools/SKILL.md` (outside this repo) refreshed
  from v0.13.2 to v0.21.1 — a `--tile` key table, the corrected `--nup` `n` set and side-by-side
  `n=2`, the `flip`-by-orientation table with its pre-v0.16.0 warning, the encryption password
  gate, the extended color set, the derived-geometry `--json` fields, and the precise
  `--dry-run` contract.  `\t` was named specifically, as instructed: the “Text escaping” line no
  longer pairs it with `\n`, and says outright that `\n` renders and `\t` has no tab-stop model.
  Its stale provenance note — “`--help` is wrong about the keys, transcribe from source” — was
  corrected too: pdf-maker’s help is drift-guarded now, so `--help` wins and this file is the
  one that goes stale.  Version stamps updated.

### Sequencing rationale

The `--tile` prerequisites come first because they are the code `--tile` reuses, and writing
them afterwards means writing them twice.  The medpdf pair comes first for a stronger reason:
they change the primitive `--tile` would be built on, both are already ruled, and both rulings
end with “audit pdf-maker imposition” — so landing them after `--tile` means auditing code that
did not exist when the ruling was made.  A sibling-crate release cycle, not the fix, is the
schedule — which is why everything medpdf-side was batched into 0.13.0 and done first.  That
judgement paid: the adoption broke `--booklet` in a way no existing test could see, and finding
that before `--tile` was written cost an afternoon instead of a re-release.  Decisions still precede their dependent code (a wrong guess costs a
re-release); silent-wrong-output fixes still precede hygiene; docs still come last, now
including `--tile`, so they are written once.  Fixed bugs: delete the report in the fixing
commit and name the ID in the message, per the bug-reports rule.  Behavior changes here warrant
a minor version bump via `/commit-rust-cli`, which handles the bump, format, gate, and push.

## Test-coverage gaps

- [ ] **Overlay round-trip at the CLI level.**  `cli_overlay_applied` asserts only the output
  page count, so it would still pass if the overlaid content vanished on reload — which is
  precisely the failure the medpdf `/Length` fix repaired.  medpdf pins its side
  (`overlay_length_regression_tests.rs`, `no_raw_stream_content_assignment.rs`); pdf-maker
  never added the matching CLI test its plan called for.  Assert the overlaid _text_ survives,
  reading with `pdf-dump --text --strict` so a future regression cannot hide behind pdf-dump’s
  lenient `/Length` recovery.
