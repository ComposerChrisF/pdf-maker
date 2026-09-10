# TODO

## Priority right now

**`plan-0003` (`--tile`) has LANDED (2026-09-09, v0.21.0), and so has the documentation pass
that waited on it (`bug-0012`, v0.21.2).**  The banner-printing arc is closed: `--tile` ships,
README.md and CLAUDE.md describe the tool again, `--watermark` and `--dry-run` gained the
`--help` text they were missing, and `~/.claude/skills/pdf-tools/SKILL.md` was refreshed from
v0.13.2 to v0.21.x.  Nothing in the queue below is blocked on anything else now.

**`bug-0017` and `bug-0002` LANDED 2026-09-10 (v0.22.0)** — the false imposition `ERROR` is
gone, and `--draw-line`’s `width` now honors `units=`.  Both carry a test verified to fail when
its own fix is reverted; bug-0002’s README row was updated in the same commit, discharging half
the follow-up obligation below.

**Phase B is CLOSED (2026-09-10, v0.23.0)**, and **`bug-0003` closed the same day (v0.24.0)** —
so **`bugs/` is empty**.  Every bug report this repo carried has been fixed and deleted.

**How bug-0003 closed, since it went through three repos.**  Honoring duplicate pages needed
medpdf, so on Chris’s instruction this session handed the work to the medpdf session directly
rather than waiting.  While preparing the hand-off it found the half nobody had: medpdf’s
`copy_page_with_cache` appends to `/Kids` and increments `/Count` itself, so a repeated page
came back as the **same** object listed twice — filed there as medpdf `bug-0040`, with a repro,
and the medpdf `plan-0006` blocked on it.  medpdf 0.15.0 landed both together; this repo raised
its floor and needed **no code change of its own** — `page_spec::expand` and `merge_pages`
already did the right thing once the two medpdf halves were correct.

**The floor of `medpdf = "0.15"` is load-bearing for correctness**, and the reason is now in
`Cargo.toml` and `CLAUDE.md`: below it, a repeated page number does not merely lose the feature,
it produces a malformed page tree.  Do not relax it.

**One known limitation shipped with it**, deliberately and documented: a duplicated page that
carries annotations shares those annotation objects, and each one’s `/P` names the first copy.
Reproduced here against medpdf 0.15.0 and filed upstream as medpdf `bug-0041` (the medpdf
session flagged the residual and asked whether this repo could reach it; it can).  pdf-maker
never edits annotations, so it hands the pair on rather than corrupting anything.  If that fix
lands, the README’s duplicates paragraph and the CHANGELOG’s known-limitation note come out.

**What is next:** no bugs.  The two open plans below (`plan-0001` `--lossy-text`,
`plan-0002` `--recompress-images`) are the whole queue, and `plan-0001` is the small one —
it is also the one the current error message begs for, since it advises “enable lossy text
substitution” and names no flag.

**Historical note — the medpdf half of the critical path (2026-09-09).**  medpdf 0.13.0 (commit
`6208ff4`) fixed their bug-0023, bug-0024 and bug-0039; this repo is on it.  What that adoption
cost and uncovered:

- It **broke `--booklet flip=short_edge`** — the back-page arithmetic hand-compensated the old
  contract and began double-compensating, throwing every back page clean off the sheet.  Found
  by the pdf-orchestrator and medpdf sessions sweeping for the pattern, fixed here, and pinned
  by a test that fails when reverted.
- It **closed bug-0007** (orphaned streams) — verified gone from imposed output; the one
  remaining `--validate` warning is the `/ObjStm` false positive that is pdf-dump’s to fix.
- It **opened bug-0018** — imposition sized cells from the pre-rotation MediaBox, so a
  `/Rotate 90` source was placed upright but mis-scaled.  Fixed in v0.19.0, ahead of `--tile`.

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

**No bug reports live in `bugs/`.**  Eleven were fixed on 2026-09-09 and seven more on
2026-09-10, each deleted per the bug-reports lifecycle; the IDs stay findable in git history
(`git log --all --grep=bug-NNNN`), and are never reused.  (bug-0016 was filed
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
- [x] **bug-0011 — DONE 2026-09-10 (v0.23.0).**  The one-line clap `requires`, as the report
  prescribed, plus the dependency stated in the flag’s help.  Pinned by a CLI test asserting
  exit 2 and that the message names `--pad-to`.
- [x] **bug-0010 — DONE 2026-09-10 (v0.23.0)**, all three parts.  `src_page=0` and `page=0` are
  parse errors (exit 2, before any I/O).  The pad file’s `page` is validated **up front**, so it
  no longer depends on the document’s length modulo `--pad-to` — the latent case where a bad
  argument waits for a differently-sized input.  Both page numbers now route through
  `page_spec::expand` (the CLAUDE.md invariant), so the error names the flag, the file, the page
  and the file’s real count instead of medpdf’s anonymous `Page 99 not found in source
  document`.  The overlay check sits right after the load that already happens, so it costs no
  second read and still fires when `target_pages` resolves to no work.  Four CLI tests, each
  verified failing on revert.

- [x] **bug-0013 — DONE 2026-09-10 (v0.23.0).**  Both probes answer three ways now
  (present / `NotFound` / cannot-be-accessed, naming the underlying error), via
  `symlink_metadata` — chosen over `metadata` so a broken symlink reports as itself rather than
  borrowing its target’s absence.  Pinned by a unit test that locks a temp directory to `0o000`,
  restores the permissions before asserting so a failure still leaves a removable tempdir, and
  **skips loudly when run as root** rather than passing for the wrong reason.
- [x] **bug-0017 — DONE 2026-09-10 (v0.22.0).**  `init_document` now uses `Stream::new`, which
  sets `/Length` from the content; the struct literal had opted out of it.  No other
  struct-literal `Stream` exists in the crate — checked, not assumed.  Pinned by a CLI test that
  asserts **stderr carries no `ERROR`** after an `--nup` run, deliberately not an exit-code
  assertion: the run exited 0 both before and after, so a status check would have passed with
  the bug still in.  Verified failing on revert, with the exact `'2 0 R' is missing the Length
  entry` line the report predicted.

- [x] **bug-0015 — DONE 2026-09-10 (v0.23.0).**  `to_rfc3339_opts(SecondsFormat::Secs, false)`,
  as the report prescribed.  The test parses each of the three values back with
  `DateTime::parse_from_rfc3339` rather than matching a shape — a hand-written pattern can
  accept a string no conformant reader would, which is the failure being fixed.

### Phase C — ruling-dependent code (all rulings now in hand)

- [x] **bug-0003 — DONE 2026-09-10 (v0.24.0).**  medpdf 0.15.0 landed both halves (its
  `plan-0006` and its `bug-0040`, the latter filed from here); this repo raised the floor and
  needed no code change.  Three CLI tests: duplicates honored, the out-of-range invariant
  intact, and the copies independent — the last drawing on the second copy and asserting the
  first is untouched, because a page count alone would have passed against the malformed shape
  too.  Revert-checked against medpdf 0.14 extracted read-only from git history rather than by
  moving the sibling session’s checkout: two tests fail there (`'1,1'` yields 1 page), and the
  out-of-range guard passes on both sides **by design** — it pins something that must not
  change.
- [x] **bug-0008 — done**, see Phase A.  Both tests that pinned the old value were updated;
  `test_nup_spec_custom_paper` gained an explicit `orientation=portrait` so it tests unit
  conversion only, instead of silently testing conversion and orientation at once.
- [x] **bug-0009 item 4 — DONE 2026-09-09**, with the rest of bug-0009 in v0.18.0; this entry
  was stale bookkeeping, corrected 2026-09-09.  Verified at the CLI: `max_dpi=none` parses,
  `max_dpi=0` is a usage error naming the remedy.  The sentinel never reaches the CLI surface —
  `none` maps to 0.0 internally, so no caller has to know a magic value exists.
- [x] **bug-0016 — done**, see Phase A.
- [x] **bug-0002 — DONE 2026-09-10 (v0.22.0).**  `width` converts through `unit.to_points`
  beside the four coordinates.  The test that pinned the old value was rewritten rather than
  deleted (the bug-0009 item-4 precedent), and two new tests pin the rule from both ends: a
  parse-level one that also asserts a `--draw-rect` `h` and a `--draw-line` `width` of the same
  stated thickness now agree, and a CLI-level one asserting the emitted `72 w` operator —
  because a parse-level test cannot see a later stage re-interpreting the value.  Behavior
  change noted in the CHANGELOG, the rule stated in `--help`, and the README row rewritten.
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
