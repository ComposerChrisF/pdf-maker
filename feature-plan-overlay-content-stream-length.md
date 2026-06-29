# Feature Plan: Fix overlay content streams written with a stale `/Length`

## TL;DR

The `--overlay` path produces page content streams whose `/Length` no longer
matches the actual stream body.  `medpdf`’s `modify_content_stream` re-encodes
the operators and assigns the new bytes directly to `Stream::content` without
updating `/Length`.  lopdf’s length-based reader then reads `/Length` bytes,
fails to find `endstream`, drops the body, and returns a bare dictionary — so
the overlaid text silently disappears on the next read.

This was originally mis-attributed to an object-stream (`/Type /ObjStm`)
problem.  That was a red herring: the `ObjStm` in the output holds only legal
dictionary objects (Pages, Catalog, Resources, Page, Font).  The real defect is
a `/Length` synchronization bug on the overlay-generated content streams.

## Symptom

- `pdf-dump <overlaid>.pdf --text` prints `Warning: Content stream N is not a stream object` for several objects and extracts none of the overlaid text.
- pdf-maker cannot round-trip its own overlay output: re-reading and re-saving the file drops the overlay content entirely.

## Reproduction

```
pdf-maker -o base.pdf --blank-page letter
pdf-maker -o stamp.pdf --blank-page letter \
  --watermark "text=FORMXTEXT,font=@Helvetica,size=36,x=2,y=5,units=in,color=black"
pdf-maker -o combined.pdf base.pdf all \
  --overlay "file=stamp.pdf,src_page=1,target_pages=all"

# The overlaid text is unreadable (no FORMXTEXT, "not a stream object" warnings):
pdf-dump combined.pdf --text

# pdf-maker can't even round-trip it — the overlay content is lost:
pdf-maker -o rt.pdf combined.pdf all
grep -c FORMXTEXT rt.pdf            # => 0
```

## Evidence (combined.pdf)

The page `/Contents` is an array of five fragments: `[13 0 R 7 0 R 8 0 R 9 0 R 10 0 R]`.

Object 10 in the raw file is a perfectly normal, valid content stream:

```
10 0 obj
<</Length 54>>stream
q
0 0 0 rg
BT
/F7_o 36 Tf
144 360 Td
(FORMXTEXT) Tj
ET
Q
Q
endstream
endobj
```

…but its declared `/Length` is **54** while the actual body is **58–59 bytes**
(58 excluding the trailing newline before `endstream`).  Because the length is
wrong, `pdf-dump --list` classifies objects 7, 10, and 13 as `Dictionary`
(body lost) while objects 8 and 9 — whose `/Length` happens to match — parse
correctly as `Stream`.  Object 13 declares `/Length 0` yet contains `q\nQ\n`,
the same class of mismatch.

The shortfall equals the bytes the overlay added when re-encoding: the
`/F7` → `/F7_o` resource rename (`+2`) plus the inserted `q` / `Q` wrapper.

## Root cause

`medpdf/medpdf/src/pdf_overlay_helpers.rs`, in `modify_content_stream`
(around line 191):

```rust
content_stream.content = content.encode()?;   // <-- /Length is NOT updated
content_stream.compress()?;                   // tiny streams aren't compressed; /Length stays stale
```

The function decodes each content stream, renames resource operands
(`/F7` → `/F7_o`, etc.), and wraps the operators in a `q` … `Q` pair, then
assigns the re-encoded bytes straight to `Stream::content`.  That raw field
assignment bypasses lopdf’s `Stream::set_content`, which is the method that
keeps `/Length` in sync with the body.  Since the re-encoded bytes differ in
length from the original (renames plus the `q`/`Q` wrapper), the dictionary’s
original `/Length` is now wrong.  The following `compress()` declines to
compress these small streams (the output is uncompressed) and does not repair
`/Length`.  On save, lopdf writes the stale `/Length` verbatim; on load, lopdf’s
length-based parser reads `/Length` bytes, fails to find `endstream`, and falls
back to a bodiless dictionary — silent data loss.

## Proposed fix

Use lopdf’s length-syncing setter instead of the raw field assignment:

```rust
content_stream.set_content(content.encode()?);  // updates /Length
content_stream.compress()?;
```

If the borrow shape makes `set_content` awkward, set `/Length` explicitly after
assignment:

```rust
let bytes = content.encode()?;
let n = bytes.len();
content_stream.content = bytes;
content_stream.dict.set("Length", n as i64);
```

Then audit the other stream-body writes in the overlay / watermark / place-page
paths for the same omission (`grep` for direct `… .content = …` and
`Stream { content, … }` literals).  Prefer `Stream::new` (which sets `/Length`)
and `set_content` everywhere a stream body is created or replaced.

## Affected module

The fix lives in the **medpdf** dependency:
`medpdf/medpdf/src/pdf_overlay_helpers.rs::modify_content_stream`.  pdf-maker
itself needs no change beyond picking up the medpdf fix.

## Distinct from the known `save_modern()` bug

This is **not** the `save_modern()` + encryption `ObjStm` issue guarded by
`tests/lopdf_save_modern_bug.rs` (lopdf issue #479, where the object stream is
created after encryption and therefore left unencrypted).  This bug reproduces
on plain, unencrypted output and is purely a `/Length` mismatch produced by
medpdf’s overlay re-encode.

## Test plan

- Add a medpdf overlay round-trip test: overlay a text-bearing page, save,
  reload, and assert (a) the overlaid text operators survive, and (b) every
  content-stream `/Length` equals its actual body length.
- Add a pdf-maker CLI test mirroring the reproduction above: overlay, then
  confirm the overlaid text is recoverable (`pdf-dump --text` or a medpdf
  reload) rather than silently dropped.

## Why not Python

This is a correctness bug in the Rust PDF engine (medpdf) that every pdf-maker
overlay and imposition operation depends on.  A Python workaround could not fix
the malformed output the Rust tool writes, nor protect the many call sites that
build content streams.  The one-line `set_content` change plus a round-trip
regression test belong in the Rust core.
