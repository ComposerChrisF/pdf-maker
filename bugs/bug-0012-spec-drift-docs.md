# bug-0012: Spec drift — imposition is entirely undocumented, and README/CLAUDE.md lag the code in a dozen places

**Severity:** Medium (the README is the published spec; whole shipped features are invisible from it)
**Type:** **Spec bug, explicitly.**  The code is not the problem for any item below — do not “fix” code to match the stale docs.  A few items are blocked on rulings from other reports (noted inline); write those doc lines only after the rulings land.

## Description

Git history tells the story: `--nup` and `--booklet` shipped across `56f7b01` (v0.10.0), `170b034` (v0.11.0, `repeat=`), and `31f7441` (v0.12.0, `back=`), but no README section was ever added — the README’s own `--json` example includes an `"imposition"` field it never explains.  Later doc passes (`8bd7e1d` “fix spec docs”, `425b9f6` page-spec contract) updated other sections and still did not add imposition.  The complete drift list, verified against v0.13.1 code:

### README

1. `--nup` and `--booklet` are wholly undocumented — no section, no keys, no examples, no pipeline mention.  Include the `n`/`cols`/`rows` grammar, paper/margins/gutter/units, `order`, `border`, `repeat` (with `repeat=auto`), booklet `flip` and `back=` (back-matter pinning), and where imposition sits in the pipeline (after merge, before overlays — so `pages=` targets of every drawing flag index the **imposed sheets**, not the source pages).  Blocked in part on bug-0001 (flip semantics) and bug-0008 (`n` semantics).
2. `--blank-page` is undocumented (named sizes letter/a4/legal, `w=/h=/units=/count=`, and the fact that blank pages append **after** all input pages, before imposition).
3. `--no-subset` is undocumented.
4. The Encryption section omits `--encryption-algorithm` and `--permissions` entirely, including the defaults (aes128; all permissions when `--permissions` is absent) and — once bug-0004 lands — the passwords-gate.
5. The watermark `units` row says “`in` (inches) or `mm` (millimeters)”; the code accepts `pt`, `in`, `mm`, `cm` (`CliUnit`), as CLAUDE.md already documents.
6. The watermark parameter table omits `color`, `alpha`, `rotation`, `h_align`, `v_align`, `strikeout`, `underline`, `weight`, `style`, `layer` — all of which `--help` lists.  A README table that presents itself as the parameter list should be complete.
7. Named colors: the code accepts `yellow`, `cyan`, `magenta`, `orange`, `purple` in addition to the documented black/white/red/blue/green/gray — no user-facing doc (README or `--help`) lists the full set.
8. The Processing Pipeline section (five phases) omits the imposition step between Merge and Overlay.
9. Page-spec semantics for duplicates — document whichever ruling bug-0003 produces.
10. `--draw-line` `width` units — document whichever resolution bug-0002 produces.

### CLAUDE.md

11. The repo-structure tree lists `spec_types.rs`; it is now the directory `src/spec_types/` with `drawing.rs`, `layout.rs`, `misc.rs`, `parse.rs`, `mod.rs`.
12. The 5-Phase Processing Pipeline section likewise omits imposition (the code runs merge → imposition → overlays → drawing → subset → padding).

### Outside the repo (pointer only — not fixable in this repo)

`~/.claude/skills/pdf-tools/SKILL.md` describes pdf-maker v0.13.0 and also predates imposition (no `--nup`/`--booklet` at all) and the extended color set.  After the fixes above land and a version ships, that reference needs a matching refresh — flagged here so the drift is not forgotten, per the rule’s own warning that stale restatements are worse than none.

Amended 2026-08-04: the flag reference moved out of `~/.claude/rules/pdf-tools.md` into the `pdf-tools` skill at the path above.  The rule file now holds only the use-the-CLI mandate and the feature-plan fallback, so the refresh belongs in the skill; editing the rule file would not fix this drift.

## Reproduction

Not runtime-reproducible; verify by diffing each claim against the code (`src/main.rs` Args, `src/spec_types/*`) and against `git log --oneline -- README.md` versus the feature commits named above.

## Suggested fix

A documentation-only pass over README.md and CLAUDE.md covering items 1–8, 11, 12 immediately; items 9–10 and the flip/`n` semantics inside item 1 wait for their rulings (bugs 0001, 0002, 0003, 0008).  No code changes.  Sequence note: do this **after** the behavior-changing fixes in this hunt land, so the docs are written once, against the settled behavior.

## Why this fix addresses the bug

The defect is that the spec no longer describes the tool; the fix is to make it describe the tool, in the order that avoids documenting behavior about to change.

## Additional item (added 2026-09-09): state the `--dry-run` contract precisely

`CLAUDE.md` currently says `--dry-run` “writes no file, and still performs the full validation
pass”.  That is very nearly true and slightly overstated, and the doc pass should not copy the
overstatement into the README.

Verified 2026-09-09: the flag branches at exactly one place, the **save**
(`src/main.rs:645`).  Merge, imposition, overlays, drawing commands and padding all run against
the real document, and the encryption parameters — including `parse_permissions` — are resolved
at `:627`, _before_ the branch.  So everything the phrase implies about the pipeline holds.

What a dry run does **not** exercise is `save_document` itself: compression, the actual
application of encryption, and the file write.  A failure that lives in there — the class the
`lopdf_save_modern_bug.rs` sentinel guards — is invisible to `--dry-run`, which will report
success for a document that cannot be written.

So the accurate phrasing is “runs the full **pipeline** and skips only the save”, not “full
validation pass”.  Say which, in both `--help` and the README, and say what that leaves
unchecked.

This distinction had practical value beyond wording: the single-branch structure is what makes
a dry run and a real run agree about geometry by construction, which is not true of every
tool — the pdf-orchestrator session runs a separate simulation against a substituted page size
and has five open parity bugs as a consequence.  Worth one sentence in the README saying the
property is deliberate, so nobody later “optimizes” `--dry-run` into a second code path.

## Additional item (added 2026-09-09): watermark text escapes are undocumented, and `\n` / `\t` are not one feature

Neither `--help` nor `README.md` documents the `--watermark` text escapes **at all** — checked
2026-09-09 — even though `unescape_text` supports `\,`, `\n`, `\t`, `\\`, `\uXXXX` and
`\U{XXXXX}`.  The only user-facing description lives in
`~/.claude/skills/pdf-tools/SKILL.md`, outside this repo, which is the wrong home for a
tool’s own interface.

When the doc pass adds them, **do not present `\n` and `\t` as a pair.**  They decode
identically and behave differently:

- **`\n` renders.**  Since medpdf 0.14.0 (its plan-0002 Tier 1), `add_text_params` splits on
  `\n`, `\r\n` and a lone `\r`, and draws each line on its own baseline — leading from the
  embedded face’s `ascender - descender + line_gap`, or `font_size x 1.2` for a built-in
  Standard-14.  Verified 2026-09-09: `text=Line 1\nLine 2` at size 24 emits `(Line 1) Tj`,
  `0 -28.8 Td`, `(Line 2) Tj`.
- **`\t` does not.**  There is no tab-stop model.  A decoded tab is dropped on the WinAnsi
  path and rejected on the composite path.

Two further behaviors worth stating rather than leaving to be discovered, both decided in
medpdf 0.14.0: a trailing newline yields a trailing **empty line** (`"a\n"` is two lines, so
block height does not depend on invisible whitespace), and leading is not caller-settable —
there is no wrap or truncation, those being medpdf’s Tiers 2-3, which pdf-maker is explicitly
not requesting.

## Additional item (added 2026-09-09): document which `flip` value goes with which paper orientation

Graduated out of bug-0001 when it was fixed, because it outlives the bug: the rule is not
obvious, it is the thing a user gets wrong, and nothing in the tool currently states it.

`--booklet`’s `flip` key compensates for the physical turn the duplexer performs, so the right
value depends on the **sheet orientation**, not on preference:

| Sheet | Use | Why |
|---|---|---|
| **Landscape** (the default, 792×612) | `flip=long_edge` | The long edge is horizontal, so the duplexer sends top to bottom and the backs need the 180° |
| **Portrait** (e.g. `paper_w=612,paper_h=792`) | `flip=short_edge` | The short edge is horizontal; same reasoning, other axis |
| Single-sided output | `flip=none` | No compensation at all |

State it in `--help` and in the README booklet section, as a table rather than prose — the
failure mode is choosing the wrong one, and a table is checkable at a glance where a sentence
is not.  Worth adding the diagnostic too: **if the backs print upside down, the other `flip`
value is the fix**, which saves the reader deriving the geometry.

Confirmed by physical duplex print 2026-09-09; the code was inverted until then (bug-0001), so
any pre-v0.16.0 advice about `flip` found elsewhere is wrong.
