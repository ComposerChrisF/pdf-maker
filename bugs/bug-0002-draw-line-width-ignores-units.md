# bug-0002: `--draw-line` converts coordinates by `units=` but leaves `width` in points

**Severity:** Low-Medium (silent geometry surprise; no data loss)
**Type:** Code bug **plus** an undocumented-semantics gap.  The spec is silent — neither `--help` nor the README says which units `width` uses — so the fix has a small decision attached, but the inconsistency itself is a code-level defect either way.

## Description

In `src/spec_types/drawing.rs`, `DrawLineSpec::from_str` converts `x1`, `y1`, `x2`, `y2` through `unit.to_points(...)` but stores `width` raw.  With `units=in`, the endpoints are interpreted as inches while the stroke width is interpreted as points.  Compare `DrawRectSpec`, where **all four** geometry keys (`x`, `y`, `w`, `h`) are converted — so a hairline drawn as a thin rect honors `units=`, but the same hairline drawn as a line does not.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf --blank-page "w=612,h=792,count=2"
pdf-maker -o out.pdf two.pdf all --draw-line "x1=1,y1=1,x2=4,y2=1,width=1,units=in,pages=1"
pdf-dump out.pdf --operators --page 1
```

Observed operators: `72 72 m`, `288 72 l` (coordinates converted) but `1 w` (width left as 1 pt, not 72 pt).

Rust test sketch: parse `DrawLineSpec::from_str("x1=1,y1=1,x2=4,y2=1,width=1,units=in")` and assert `spec.width == 72.0` (currently `1.0`); plus a CLI-level assertion on the emitted `w` operator.

## Suggested fix

Recommended: convert `width` like every other geometry key —

```rust
width: unit.to_points(width),
```

— and document in `--help` and the README that all `--draw-line` geometry, including `width`, is in `units=`.  The alternative (documenting that `width` is always points) is defensible but leaves the tool internally inconsistent with `--draw-rect`, whose `h` serves the same “thickness” role and does convert.

Behavior-change note for the fixing agent: any existing caller passing `width=` together with a non-`pt` `units=` will see the stroke thicken.  A survey of Chris’s configs is cheap: the `pdf-tools.md` examples use `width=1.5` with default `pt` units, which is unaffected.

## Why this fix addresses the bug

The defect is an inconsistency between sibling keys of one spec (and between `--draw-line` and `--draw-rect`).  Converting `width` at the same point where the other keys are converted removes the inconsistency at its single source; the doc line closes the spec gap so the semantics cannot silently diverge again.
