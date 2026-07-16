# bug-0007: Imposition leaks one orphaned zero-byte stream per output sheet

**Severity:** Low (file bloat and object-graph hygiene; no incorrect rendering)
**Type:** Code bug; root cause likely in the medpdf `create_blank_page` + `place_page` interplay, so the fix may belong in the medpdf repo — investigate before patching pdf-maker.

## Description

Every sheet produced by `--nup` or `--booklet` leaves behind an unreachable zero-byte stream object in the output.  The likely mechanism: `impose_pages` creates each destination sheet with `medpdf::create_blank_page` (which attaches an empty `/Contents` stream), then `place_page` rewrites the page’s `/Contents` while placing source content, orphaning the original empty stream.  `doc.compress()` does not garbage-collect unreachable objects, so the orphans ship.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o out1.pdf four.pdf all --nup "n=4"                      # 1 sheet
pdf-maker -o out2.pdf four.pdf all --booklet "flip=short_edge"      # 2 sheets
pdf-dump out1.pdf --validate   # 1 orphan stream + 1 ObjStm warning
pdf-dump out2.pdf --validate   # 2 orphan streams + 1 ObjStm warning
```

Observed: the orphan count equals the sheet count (`out1`: object 5; `out2`: objects 5 and 17 — each a plain `/Length 0` stream).

**Do not chase the ObjStm warning.**  Every `save_modern` output additionally gets one “Object N 0 is unreachable from trailer” warning for its `/ObjStm` container — that is a **pdf-dump validator false positive** (object-stream containers are referenced from the xref stream, not the object graph) and is pdf-dump’s defect to fix in its own repo, not pdf-maker’s.  The signal here is the _plain_ zero-byte streams, and only in imposed output.

Rust test sketch: impose a 4-page doc onto 2 sheets, reload the output with lopdf, walk references from the trailer, and assert every non-ObjStm object is reachable (currently two `/Length 0` streams are not).

## Suggested fix

First confirm the mechanism (dump the imposed output’s page `/Contents` arrays and check the orphaned IDs are the `create_blank_page` originals).  Then pick the cheapest correct layer:

1. **medpdf:** let `place_page` reuse or remove the empty original stream when it rewrites `/Contents`, or give `create_blank_page` a content-less variant for imposition sheets; or
2. **pdf-maker:** garbage-collect before save (lopdf exposes pruning of unreachable objects; verify the current lopdf 0.42 API) in `save_document`.

Option 2 is broader (it would also clean any future orphan source), but it touches every save path; option 1 fixes the actual leak at its source.  Either is acceptable; document the choice in the fixing commit.

## Why this fix addresses the bug

The orphans exist because an object is replaced without being reclaimed; both options close that window — one by not creating the doomed object (or reclaiming it at replacement), the other by sweeping unreachable objects before serialization.  The test pins the invariant (no unreachable non-container objects) rather than the mechanism, so either fix satisfies it.
