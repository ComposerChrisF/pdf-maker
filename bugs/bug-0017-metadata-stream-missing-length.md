# bug-0017: The XMP metadata stream is built without `/Length`, so every imposition run logs a spurious `ERROR`

**Severity:** Medium — a well-formed-output defect that only lopdf’s leniency hides, plus a false `ERROR` line on every successful imposition run
**Type:** Code bug, entirely in this repo (`src/main.rs:208`).  **Not** a medpdf defect, and **not** bug-0007’s mechanism — see “Attribution” below.
**Filed:** 2026-09-09, during the plan-0003 prerequisite review

## Description

`init_document` builds the XMP metadata stream with a **struct literal**:

```rust
Object::Stream(Stream {
    dict: metadata,
    content: format_xmp_metadata(&doc_uuid).into_bytes(),
    allows_compression: true,
    start_position: None,
})
```

`lopdf::Stream::new` sets `/Length` for you — `object.rs:716-717`, `dict.set("Length", content.len())` — and the struct literal bypasses it.  The metadata dictionary is built from `dictionary! { "Type" => "Metadata", "Subtype" => "XML" }` and never gains a `/Length`, so the in-memory object is a stream whose dictionary lacks the one entry the PDF spec requires of every stream.

Normally this is invisible: `save_document` runs `doc.compress()`, which re-encodes each stream and writes a correct `/Length` on the way out.  **The imposition path does not go through that.**  `impose_pages` (`src/imposition.rs:288-290`) round-trips the whole document through memory to work around lopdf’s missing `Clone`:

```rust
doc.save_to(&mut buf)?;
let source_doc = Document::load_mem(&buf)?;
```

That raw `save_to` writes the malformed stream verbatim, and the `load_mem` immediately behind it logs:

```
[ERROR lopdf::reader] stream dictionary of '2 0 R' is missing the Length entry
```

Object 2 is the metadata stream in a freshly initialized document (`init_document` allocates pages, then metadata, then catalog), which is exactly the object named.

## Reproduction (verified 2026-09-09, v0.13.2)

```bash
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o out.pdf four.pdf all --nup "n=4"
# stderr, between the imposition banner and the next phase:
# [ERROR lopdf::reader] stream dictionary of '2 0 R' is missing the Length entry
# exit 0
```

Reproduced identically with `margin=0.25` and `margin=-0.25`, so it is not geometry-dependent, and **absent from a plain merge** (`pdf-maker -o plain.pdf four.pdf all`), which never round-trips.  Note that `four.pdf` _on disk_ is clean — `pdf-dump four.pdf --strict` reports 0 errors — because `compress()` repaired the `/Length` on the way out.  The malformed artifact is the intermediate that only ever exists in `buf`.

Rust test sketch: capture stderr from an `--nup` run through the `tests/cli_tests.rs` harness and assert it contains no `missing the Length entry`.  A tighter unit-level variant: call `init_document()`, serialize with `save_to` alone (no `compress`), reload with `load_mem`, and assert the metadata object’s dictionary has a `/Length` key.

## Suggested fix

Use the constructor:

```rust
Object::Stream(Stream::new(metadata, format_xmp_metadata(&doc_uuid).into_bytes()))
```

`Stream::new` defaults `allows_compression` to `true` and `start_position` to `None`, so the two remaining fields are unchanged.  While there, consider whether any other struct-literal `Stream { .. }` exists in this crate — this is the only one today, and the constructor should be the house form.

## Why this fix addresses the bug

The defect is a bypassed invariant, not a missing repair step: `Stream::new` exists precisely to keep `/Length` in step with the content, and the struct literal opted out of it.  Restoring the constructor makes the object well-formed at construction, so it is correct in every consumer — the `compress()` path that accidentally repaired it, the raw `save_to` path that did not, and any future path that serializes without compressing.

## Attribution — corrected on the day it was filed

This symptom was first written into **bug-0007** (2026-09-09) as “the same defect surfacing twice”, on the reasoning that both the orphaned zero-byte streams and the `/Length` complaint pointed at medpdf’s `create_blank_page`.  **That was wrong**, and the correction matters because it was about to be handed to the medpdf session as their defect:

- `create_blank_page` builds its content stream with `Stream::new(dictionary! {}, vec![])` (`medpdf/src/pdf_blank_page.rs:13`), so it **does** carry `/Length 0`.  Confirmed on disk: objects 5, 8, 11 and 14 of a four-page `--blank-page` file are `/Length 0` streams and reload cleanly.
- The object lopdf names is `2 0 R`, which is the metadata stream, not a page-content stream.
- The trigger is pdf-maker’s own raw `save_to`, which no medpdf entry point performs.

bug-0007’s orphaned-stream finding stands on its own and remains open; only the `/Length` symptom moves here.
