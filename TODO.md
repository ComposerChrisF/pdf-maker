# TODO

## Priority right now

**`plan-0003` (`--tile`) is the top priority.**  Everything below is ordered around getting it
landed safely rather than around bug severity, which is a deliberate change from the
2026-07-16 ordering.  Four pdf-maker bug reports sit on the imposition path `--tile` extends —
three are true prerequisites and one is not, contrary to the plan’s first draft — and **two
_medpdf_ bugs sit under it**, on the `place_page` primitive imposition is built from.  All are
gathered into **Phase 0** below and should be worked first.

**The critical path runs through medpdf, not through this repo.**  Corrected 2026-09-09: an
earlier reading of this queue named bug-0007 the longest lead item because its fix _might_
belong in medpdf.  medpdf bug-0023 and bug-0024 certainly do, are already ruled, and are
already sequenced in medpdf’s own `TODO.md` (Step 5) — so the schedule is set by a medpdf
release, and bug-0007 rides along on it rather than driving it.

Two decisions have since been made and are recorded in the reports themselves, so nothing in
Phase 0 is waiting on a ruling any more:

- **The `--tile` default overlap is 0.75in, not 0.5in** — a consumer printer holds about a
  quarter inch unprintable at each edge, so a 0.5in overlap leaves _zero_ real overlap to tape
  against (plan-0003, decision 1).  The `units` default moves to `in` in the same breath, since
  `pt` would have made the default overlap about a hundredth of an inch.
- **Negative `margin` / `gutter` / `binding_margin` stay legal** — an extent may not be
  negative, an offset may.  A negative margin is a _bleed_, verified working on v0.13.2, and it
  can never cause bug-0006’s fault.  Full ruling in bug-0009; consequences in bug-0006 and
  plan-0003.

## Open plans

Proposed changes — options, not obligations — in `plans/`, numbered per
`~/.claude/rules/plan-files.md`.  TODO.md is the ordering index for plans as well as bugs;
**the order below _is_ a priority ruling**, unlike the 2026-08-12 note it replaces.

- [ ] **plan-0003 — `--tile`: split one large page across many sheets, with overlap.**  Filed
  2026-09-09 from the Publisher retirement review.  Publisher goes away 2026-10-01 and takes
  tiled banner printing with it; PowerPoint has no tiled printing at all, and Affinity has it
  but re-introduces the single-vendor-format dependency the migration exists to escape.
  `--tile` is the **inverse of `--nup`** and shares its cell arithmetic, which is why it
  belongs here.  Prerequisites are Phase 0 below.  Not blocking 1 October: the existing banner
  PDF is already tiled; what is lost is re-tiling a changed one.
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

**Seventeen** bug reports live in `bugs/` — bug-0001 through bug-0017.  (bug-0016 was filed
2026-07-23, after the original deep review, and was missing from this index until 2026-09-09;
bug-0017 was filed 2026-09-09 from the plan-0003 prerequisite review.)
IDs are alphabetical by slug per the bug-reports rule; they encode nothing about priority.
**Work them in the phase order below**, not in ID order.  Each report is self-contained:
description, verified repro (Rust-test-ready), suggested fix, and why it works.  Reports marked
_decision_ need Chris’s ruling before any code changes; do not guess.

### Phase 0 — the `--tile` critical path (start here)

Three pdf-maker prerequisites, two medpdf prerequisites, and one long-lead item that travels
with them.  All are on the imposition path; none is waiting on a ruling.

- [ ] **bug-0005** — `--help` advertises booklet keys that do not exist (`orientation`,
  `duplex_flip`) and lists no `--nup` keys at all.  **First**, because it is cheap, needs no
  decision, and `--tile` adds a _third_ imposition mode: writing its help text against a pair
  that documents nothing would compound the fault three ways instead of two.
- [ ] **bug-0006** — oversized margins/gutters give non-positive imposition cells: mirrored,
  shrunken pages at exit 0.  **The hard prerequisite.**  Its fix is to extract the cell
  geometry into one checked computation, which is exactly the code `--tile` reuses; doing it
  after `--tile` means writing the check twice.  Now also carries the reporting obligation from
  the negative-offset ruling: emit the computed cell size and any off-sheet overhang in
  `--json` and on stderr.
- [ ] **bug-0009** — parse-level range validation (alpha in `[0, 1]`, positive extents).  Pair
  with bug-0006 — same validation theme, adjacent code — and land it before `TileSpec` exists,
  so the new spec type is written to the settled pattern rather than retrofitted.  Hold back
  only the `max_dpi` item, which still needs its Phase A ruling.  **The negative-margin item is
  ruled and closed**: no sign check on offsets.
- [ ] **medpdf bug-0024** — `place_page` (x, y) semantics undefined for a non-zero-origin
  MediaBox.  **Hard prerequisite.**  Ruled 2026-07-24: **compensate**, so `(x, y, scale)` alone
  determines where the visible box lands.  `--tile`’s whole job is “put source point P at sheet
  point Q”, repeated once per sheet; write that math against today’s uncompensated behavior and
  the medpdf fix then shifts every tile by `scale x origin` — silently, and only for
  non-zero-origin sources, which is the hardest kind of regression to see.  The ruling’s own
  action item already reads “audit pdf-maker imposition”; doing `--tile` first means auditing it
  twice.  Note `place_page_tests.rs:375` pins the current behavior on the medpdf side.
- [ ] **medpdf bug-0023** — `place_page` ignores the source page’s `/Rotate`.  **Hard
  prerequisite, and it bites `--tile` harder than `--nup`.**  Ruled 2026-07-24: **honor
  `/Rotate`**, swapping effective width/height for 90/270, and implement together with bug-0024
  since they share the transform.  For `--nup` the symptom is a sideways page in a correct
  grid; for `--tile` the grid itself is _derived_ from the source’s effective width and height,
  so a `/Rotate 90` source computes rows and columns swapped — the wrong **number of sheets**,
  not merely wrong content on them.  That is a page-count error baked into the arithmetic,
  which is exactly what plan-0003’s `--dry-run` grid report exists to catch.
- [ ] **Bump the medpdf floor when that release lands.**  `Cargo.toml` asks for
  `medpdf = "0.12"`; once `place_page` compensates and honors `/Rotate`, building against an
  older 0.12.x silently restores the old placement.  Same shape as the v0.11.0 encoding floor
  already recorded in CLAUDE.md’s contract invariants — raise the requirement in the commit
  that consumes the new behavior.
- [ ] **bug-0007 / medpdf bug-0039** — imposition leaks one orphaned zero-byte stream per
  sheet.  Filed on the medpdf side 2026-09-09 as bug-0039 and handed to that session, since
  `create_blank_page` and `place_page` are both theirs; bug-0007 stays open here as the
  consumer-side record until it lands.  **Not a prerequisite, but batched with the two medpdf
  bugs above** so one family release carries all three.  `--tile` multiplies the leak by the
  sheet count — a twelve-sheet banner leaks twelve objects.  Note the mechanism is _not_
  confirmed: `place_page` appears to preserve the destination’s `/Contents` rather than replace
  it, which contradicts bug-0007’s original explanation while the orphan count still tracks the
  sheet count exactly.
- [ ] **plan-0003 implementation** once the five prerequisites land.  Land it **before**
  bug-0012, so the doc sweep documents all three imposition modes once instead of twice.

**bug-0008 is _not_ a prerequisite**, contrary to plan-0003’s first draft.  It concerns
`auto_grid` mapping an `n` onto a grid; `--tile` derives its grid from geometry and never calls
`auto_grid`.  It stays in Phase A on its own merits.

### Phase A — decisions from Chris (blocking their own Phase C work; none blocks `--tile`)

- [ ] **bug-0003** — duplicates in a page spec (`"1,1"` silently yields one page).  Rule: error
  on duplicates (recommended) vs. honor them (a medpdf feature).  Gates Phase C work and one
  README line.
- [ ] **bug-0008** — `--nup n=3` silently produces 4-up.  Rule: restrict `n` to canonical values
  (recommended) vs. honor arbitrary `n` vs. document rounding.  Also rule on the
  stacked-portrait `n=2` auto layout.
- [ ] **bug-0001** — booklet duplex-flip compensation looks inverted (backs rotate on
  `short_edge`, not `long_edge`).  **Needs a physical duplex print to confirm** — the one
  finding a terminal cannot verify.
- [ ] **bug-0002** — `--draw-line` `width` ignores `units=`.  Rule: convert like the coordinates
  (recommended) vs. document points-only.
- [ ] **bug-0009 item 4** — `--draw-image max_dpi=0` is accepted (an existing unit test asserts
  it).  Rule: define 0 as “no downsampling” and document, or reject.
- [ ] **bug-0016** — `--watermark` `\n`/`\t` escapes do not render as multiple lines.  Rule:
  where the fix lives.  medpdf owns the font metrics (leading, ascent/descent) a real
  line-layout needs, so a pdf-maker-side `\n`-split would re-derive them badly; the plausible
  alternative is to stop documenting the escapes.  Cross-repo either way.

### Phase B — independent code fixes, in severity order (no ruling needed, off the `--tile` path)

Each fix lands with a test that fails when the fix is reverted.

- [ ] **bug-0004** — `--permissions` / `--encryption-algorithm` silently ignored without a
  password (writes an unencrypted, unrestricted file at exit 0).  The worst silent-wrong-output
  fault left in the queue.  Pair with **bug-0014** (usage errors exit 1, not 2) — both touch
  permissions parsing and clap wiring.
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

### Phase C — ruling-dependent code (after the matching Phase A decision)

- [ ] **bug-0003** implementation — needs a medpdf API change (duplicates are invisible to
  pdf-maker today); coordinate with the sibling `../medpdf` workspace and its release flow
  (`PUBLISHING.md`).
- [ ] **bug-0016** implementation — medpdf-side if that is the ruling; batch it with any other
  medpdf change so the family releases once.
- [ ] **bug-0008** implementation.
- [ ] **bug-0002** implementation (behavior change; note in CHANGELOG).
- [ ] **bug-0001** implementation (only after physical confirmation).

### Phase D — cross-repo

- [ ] File in **pdf-dump’s** repo (not here): its validator falsely flags `/ObjStm` containers
  as “unreachable from trailer” on every modern-format PDF.  Evidence in bug-0007’s repro.

### Phase E — documentation (last, once behavior settles)

- [ ] **bug-0012** — the big spec-drift pass over README.md and CLAUDE.md (imposition wholly
  undocumented, `--blank-page`/`--no-subset`/encryption flags missing, watermark
  units/params/colors incomplete, pipeline diagrams missing the imposition phase, CLAUDE.md’s
  stale `spec_types.rs` tree entry).  Doc-only; written last so it describes the settled
  behavior, including the Phase A rulings **and `--tile`** — hence Phase 0’s instruction to land
  `--tile` before this.  Note while here: CLAUDE.md’s “5-Phase Processing Pipeline” omits
  imposition entirely, though it runs between merge and overlays (`src/main.rs:601`).
- [ ] After release: refresh `~/.claude/skills/pdf-tools/SKILL.md` (outside this repo).  It was
  brought up to v0.13.2 with the imposition tables on 2026-09-09; `--tile` and the Phase A
  rulings will need another pass.

### Sequencing rationale

The `--tile` prerequisites come first because they are the code `--tile` reuses, and writing
them afterwards means writing them twice.  The medpdf pair comes first for a stronger reason:
they change the primitive `--tile` would be built on, both are already ruled, and both rulings
end with “audit pdf-maker imposition” — so landing them after `--tile` means auditing code that
did not exist when the ruling was made.  A sibling-crate release cycle, not the fix, is the
schedule, so everything medpdf-side (bug-0023, bug-0024, and probably bug-0007) is batched into
one release and started before anything here.  Decisions still precede their dependent code (a wrong guess costs a
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
