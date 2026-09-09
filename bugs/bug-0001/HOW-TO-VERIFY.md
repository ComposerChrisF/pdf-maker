# bug-0001 — physical duplex verification kit

Generated 2026-09-09 with pdf-maker v0.13.2 (release build), against medpdf 0.13.0.
These files reproduce the **current, suspected-wrong** behavior — that is the point.

## Files

| File | What it is |
|---|---|
| `input-4page.pdf` | The source: 4 portrait letter pages, each with a huge numeral, `TOP — page N` near the top edge, and a grey `bottom — page N` near the bottom edge |
| `booklet-SHORT-edge.pdf` | `--booklet "flip=short_edge"` — 2 landscape sheets (792×612).  Backs **are** rotated 180° by the current code |
| `booklet-LONG-edge.pdf` | `--booklet "flip=long_edge"` — 2 landscape sheets.  Backs are **not** rotated by the current code |

Sheet 1 carries source pages 4 and 1; sheet 2 carries pages 2 and 3 — standard saddle-stitch order, and not what is under test.

## What to print

Two duplex jobs, each with the printer’s binding setting **matched to the file’s name**:

1. `booklet-SHORT-edge.pdf` → printer two-sided setting: **Short-Edge binding**
2. `booklet-LONG-edge.pdf` → printer two-sided setting: **Long-Edge binding**

On macOS the setting is in the print dialog under Two-Sided (or Layout → Two-Sided);
some drivers word it “Flip on short edge” / “Flip on long edge”.  If the dialog offers
“Booklet”, do **not** use it — that applies the printer’s own imposition on top of
pdf-maker’s, and the result tells us nothing.

Plain one-sided printing tells us nothing either.  The whole question is what the
duplexer does when it turns the sheet over.

## What to look for — one question per sheet

**Is the text on the back of the sheet right-side up, the same way round as the front?**

Hold a printed sheet so the front reads normally, then flip it over about the vertical
axis, like turning a page in a book.  The back should read normally too.

That is all.  Ignore which page number is where — the imposition order is not what is
being tested.

## What the analysis predicts

If bug-0001 is real, **both files print with their back sides upside down.**  The two
settings are predicted wrong for opposite reasons, which is why the test is symmetric
and needs no judgement about which file is which:

- A **landscape** sheet flipped about its **short** (vertical) edge keeps top at top, so
  the back needs **no** compensation — but `flip=short_edge` applies a 180° rotation.
- The same sheet flipped about its **long** (horizontal) edge sends top to bottom, so the
  back **needs** the 180° — but `flip=long_edge` applies none.

So: **backs upside down on both = bug confirmed**, and the fix is the orientation-aware
rule in the report.  **Backs upright on both = the code is right** and the report should
be closed as not-a-bug, with the reasoning corrected.

One printing upright and the other upside down would mean the analysis is wrong in some
third way — worth saying which one, because that changes the fix.

## Please record the result

Note the outcome, plus the printer model and the exact wording of the setting you chose,
in `bugs/bug-0001-booklet-duplex-flip-inverted.md`.  Driver vocabulary is inconsistent
enough that the wording is part of the evidence.
