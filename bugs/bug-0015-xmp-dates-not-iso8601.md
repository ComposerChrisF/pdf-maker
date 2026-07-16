# bug-0015: XMP metadata dates are not ISO 8601 — space-separated with microseconds instead of the required `T` form

**Severity:** Low (metadata conformance; PDF validators and asset-management tools flag or ignore the dates)
**Type:** Code bug (`src/main.rs::format_xmp_metadata`).  The spec is not the problem.

## Description

`format_xmp_metadata` interpolates `chrono::Local::now()` with the plain `Display` format into `xmp:CreateDate`, `xmp:ModifyDate`, and `xmp:MetadataDate`.  Chrono’s `Display` yields `2026-07-16 07:02:57.068787 -10:00` — a space between date and time, six-digit fractional seconds, and a spaced offset.  The XMP specification requires ISO 8601 date-time of the form `YYYY-MM-DDThh:mm:ss±hh:mm` (the `T` separator, no interior spaces).  Every PDF pdf-maker writes carries three malformed date properties.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o out.pdf --blank-page letter
pdf-dump out.pdf --object 2 --decode | grep -o 'xmp:CreateDate="[^"]*"'
# → xmp:CreateDate="2026-07-16 07:02:57.068787 -10:00"
```

Rust test sketch: write a minimal output, extract the metadata stream (lopdf: trailer → Root → Metadata), and assert each `xmp:*Date` attribute matches `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$` — currently fails on the space.  Unit-level alternative: make the timestamp a parameter of `format_xmp_metadata` and assert on the produced string.

## Suggested fix

```rust
let now = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
```

which yields e.g. `2026-07-16T07:02:57-10:00`, and interpolate that string.  (RFC 3339 is the ISO 8601 profile XMP expects; seconds precision is ample for document metadata.)

## Why this fix addresses the bug

`to_rfc3339_opts` emits exactly the `T`-separated, offset-suffixed form the XMP Date value type requires, so the three properties become machine-parseable by conformant consumers; nothing else in the packet changes.
