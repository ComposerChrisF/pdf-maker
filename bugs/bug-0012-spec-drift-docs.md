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
