# TODO

## Bug-fix queue from the 2026-07-16 deep review

Fifteen bug reports live in `bugs/` (bug-0001 through bug-0015; IDs are alphabetical by slug per the bug-reports rule — they encode nothing about priority).  **Work them in the phase order below**, not in ID order.  Each report is self-contained: description, verified repro (Rust-test-ready), suggested fix, and why it works.  Reports marked _decision_ need Chris’s ruling before any code changes; do not guess.

### Phase A — decisions from Chris (blocking; no code until ruled)

- [ ] **bug-0003** — duplicates in a page spec (`"1,1"` silently yields one page).  Rule: error on duplicates (recommended) vs. honor them (a medpdf feature).  Gates Phase C work and one README line.
- [ ] **bug-0008** — `--nup n=3` silently produces 4-up.  Rule: restrict `n` to canonical values (recommended) vs. honor arbitrary `n` vs. document rounding.  Also rule on the stacked-portrait `n=2` auto layout.
- [ ] **bug-0001** — booklet duplex-flip compensation looks inverted (backs rotate on `short_edge`, not `long_edge`).  **Needs a physical duplex print to confirm** — the one finding a terminal cannot verify.
- [ ] **bug-0002** — `--draw-line` `width` ignores `units=`.  Rule: convert like the coordinates (recommended) vs. document points-only.
- [ ] **bug-0009 item 4** — `--draw-image max_dpi=0` is accepted (an existing unit test asserts it).  Rule: define 0 as “no downsampling” and document, or reject.

### Phase B — independent code fixes (no ruling needed; start here while Phase A is pending)

Suggested order — high severity and shared code first; each fix lands with a test that fails when the fix is reverted:

- [ ] **bug-0004** — `--permissions` / `--encryption-algorithm` silently ignored without a password (writes an unencrypted, unrestricted file at exit 0).  Pair with **bug-0014** (usage errors exit 1, not 2) — both touch permissions parsing and clap wiring.
- [ ] **bug-0011** — `--pad-last-page-file` without `--pad-to` silently ignored.  One-line clap `requires`.
- [ ] **bug-0005** — `--help` advertises booklet keys that do not exist (`orientation`, `duplex_flip`) and lists no `--nup` keys.  Actively misleading, so it does not wait for Phase E docs.
- [ ] **bug-0006** — oversized margins/gutters give negative imposition cells: mirrored, shrunken pages at exit 0.  Pair with **bug-0009** (parse-level range validation: alpha in [0,1], positive dimensions, non-negative margins) — same validation theme, adjacent code; hold back only the `max_dpi` item pending its Phase A ruling.
- [ ] **bug-0010** — `--overlay src_page=` / `--pad-last-page-file page=` validated late or never, with errors that name no flag, file, or page count.
- [ ] **bug-0013** — unreadable input misreported as “does not exist” (two-state probe in `src/paths.rs`).
- [ ] **bug-0015** — XMP dates not ISO 8601.

### Phase C — ruling-dependent code (after the matching Phase A decision)

- [ ] **bug-0003** implementation — needs a medpdf API change (duplicates are invisible to pdf-maker today); coordinate with the sibling `../medpdf` workspace and its release flow (`PUBLISHING.md`).
- [ ] **bug-0008** implementation.
- [ ] **bug-0002** implementation (behavior change; note in CHANGELOG).
- [ ] **bug-0001** implementation (only after physical confirmation).

### Phase D — investigation / cross-repo

- [ ] **bug-0007** — imposition leaks one orphaned zero-byte stream per sheet.  Confirm the `create_blank_page` + `place_page` mechanism first; the fix may belong in medpdf.
- [ ] File in **pdf-dump’s** repo (not here): its validator falsely flags `/ObjStm` containers as “unreachable from trailer” on every modern-format PDF.  Evidence in bug-0007’s repro.

### Phase E — documentation (last, once behavior settles)

- [ ] **bug-0012** — the big spec-drift pass over README.md and CLAUDE.md (imposition wholly undocumented, `--blank-page`/`--no-subset`/encryption flags missing, watermark units/params/colors incomplete, pipeline diagrams missing the imposition phase, CLAUDE.md’s stale `spec_types.rs` tree entry).  Doc-only; written last so it describes the settled behavior, including the Phase A rulings.
- [ ] After release: refresh `~/.claude/skills/pdf-tools/SKILL.md` (outside this repo — it still describes v0.13.0 and predates imposition).  The reference moved out of `~/.claude/rules/pdf-tools.md` into that skill on 2026-08-04; the rule file no longer carries flag tables, so refreshing it is not the fix.

### Sequencing rationale

Decisions before dependent code (a wrong guess costs a re-release); silent-wrong-output fixes before hygiene; docs after behavior so they are written once.  Fixed bugs: delete the report in the fixing commit and name the ID in the message, per the bug-reports rule.  Behavior changes here warrant a minor version bump (v0.14.0) via `/commit-rust-cli`, which handles the bump, format, gate, and push.

## Open plans

Proposed changes — options, not obligations — in `plans/`, numbered per `~/.claude/rules/plan-files.md`.  TODO.md is the ordering index for plans as well as bugs; **the order below is sequencing, not a priority ruling**, and rejecting a plan is a normal exit.  Both landed 2026-08-12 as the surviving residue of the legacy `feature-plan-*.md` migration.

- [ ] **plan-0001** — `--lossy-text`, exposing medpdf’s existing `WatermarkParams::lossy_text` opt-out.  Small: one flag, no medpdf change for the stderr-warning form.  Worth doing early because the current error message advises “enable lossy text substitution” and names no flag, so the advice is unactionable today.  Carries one decision for Chris (global flag vs. per-watermark key; the plan recommends global).
- [ ] **plan-0002** — `--recompress-images`, exposing the shipped `medpdf_image::recompress::recompress_images()`.  Larger, and the scoping design is the hard part — pdf-orchestrator shipped the same feature and carries two open bugs on exactly that (bug-0037 scope, bug-0035 junk-enables-lossy).  Read those before implementing.

## Test-coverage gaps

- [ ] **Overlay round-trip at the CLI level.**  `cli_overlay_applied` asserts only the output page count, so it would still pass if the overlaid content vanished on reload — which is precisely the failure the medpdf `/Length` fix repaired.  medpdf pins its side (`overlay_length_regression_tests.rs`, `no_raw_stream_content_assignment.rs`); pdf-maker never added the matching CLI test its plan called for.  Assert the overlaid _text_ survives, reading with `pdf-dump --text --strict` so a future regression cannot hide behind pdf-dump’s lenient `/Length` recovery.
