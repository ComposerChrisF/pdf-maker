# Plan: `--recompress-images` — expose medpdf-image’s FlateDecode → JPEG pass

## Problem

Word on Mac’s “Save As PDF” re-encodes JPEG photographs as FlateDecode streams: a 34 KB JPEG comes back as a 233 KB lossless stream despite already having been downsampled.  Merged into many documents — a perusal-score cover merged into 50+ pieces — the waste compounds into megabytes served to every visitor downloading a score.

`medpdf_image::recompress::recompress_images()` fixes this and **has shipped**.  pdf-orchestrator consumes it (the `recompressImages` attribute on `<ImportPdf>` and `<AddPdf>`, v0.10.0).  **pdf-maker cannot reach it at all** — there is no flag, so the tool that exists to merge and manipulate PDFs is the one member of the family that cannot shrink them.

This is the unimplemented residue of the old `Pdf/feature-plan-recompress-images.md`, which was closed in medpdf as `plan-0003` on 2026-08-12 because the library API had shipped.  The three CLI flags it proposed for pdf-maker never landed; that commit’s message and the rollout doc both record the debt, and this plan is where it lands.

## Proposed Change

A spec-style flag matching pdf-maker’s existing conventions, with a bare form for the common case:

```bash
pdf-maker -o out.pdf cover.pdf all body.pdf all --recompress-images
pdf-maker -o out.pdf cover.pdf all body.pdf all \
  --recompress-images "quality=60,min_size=25k,pages=1"
```

Keys, defaulted from `RecompressParams`:

| Key | Default | Meaning |
|-----|---------|---------|
| `quality` | 85 | JPEG quality, 1–100 |
| `min_size` | 50k | Skip FlateDecode streams smaller than this (`k`/`m` suffixes) |
| `pages` | `all` | Which pages of the **output** document to scope to |

Contract:

- **Off by default.**  This is lossy and irreversible; it never happens unless asked for.
- `--dry-run` reports what _would_ be recompressed — count, bytes before, projected bytes after — and writes nothing.
- `--json` reports `scanned`, `recompressed`, `bytes_before`, `bytes_after`, straight from `RecompressStats`.
- Human progress output names the pass and its net saving on stderr, as its own pipeline phase.
- Malformed keys are a **usage error (exit 2)**, and an unrecognized key is never ignored — see the prior art below.

### `pages=` is the whole design question

`recompress_images()` takes an explicit `object_ids` list rather than sweeping the document, and the reason is load-bearing: **not all images are photos.**  Music-publishing PDFs mix photographic content (cover art, composer headshots) with graphic design (logos, ornamental borders, notation fragments).  JPEG is lossy and its artifacts land hardest on sharp edges, text, and flat color — exactly what those graphics are made of.  The caller must choose, because only the caller knows what each image is.

`pages=` is the coarsest honest way to give a CLI user that choice: in the motivating case the photographs are on the cover and the notation is everywhere else, so page scoping separates them cleanly.  It is not sufficient for a page mixing a photo and a logo.  **Do not paper over that with a heuristic** — “recompress it if it looks photographic” is exactly the guess this API was shaped to refuse.  If per-image control is wanted later, it wants an explicit object-ID or image-index selector, and `pdf-dump --images` already lists the candidates.

### Prior art — two mistakes pdf-orchestrator already made here

Both are open bugs in that repo and both are cheap to avoid up front:

- **pdf-orchestrator bug-0037** — `<AddPdf recompressImages>` recompresses the _destination_ page’s images instead of the overlaid source’s, and misses images nested inside Form XObjects.  Scoping is the part that goes wrong.  Decide exactly which object IDs `pages=` resolves to, write it down, and test a merge where only one input should be touched.
- **pdf-orchestrator bug-0035** — junk without `=` silently enables lossy recompression with defaults.  A parse failure must never be the thing that turns on a destructive-by-nature operation.  Reject unknown keys loudly.

## Implementation Notes

- Dependency: pdf-maker does not currently depend on `medpdf-image` for this path — check whether the `recompress` feature needs enabling, and keep it behind the same version-plus-path arrangement as `medpdf` (`PUBLISHING.md`).
- Pipeline placement: a new phase between drawing commands and padding, i.e. after all content exists and before save.  Recompression rewrites image XObjects in the merged document, so it must see the final object set.
- Resolve `pages=` through `page_spec::expand` — never `medpdf::parse_page_spec` (CLAUDE.md invariant), so an out-of-range page is exit 1 rather than a silently narrowed scope.
- Collect image XObject IDs for the scoped pages, including images reached through nested Form XObjects — bug-0037 is precisely the miss of that traversal.
- `min_size` needs a size parser (`25k`, `2m`, bare bytes).  Check whether one already exists in `spec_types` before adding another.
- Document in `--help` that recompression is lossy and irreversible, and that `/ColorSpace` is normalized to DeviceRGB/DeviceGray — the stream is re-encoded from decoded samples, so an ICCBased profile no longer describes its bytes and is dropped.
- medpdf-image already skips an image that is DCTDecode-encoded, not a single-filter FlateDecode stream, under `min_size`, carrying `/SMask`/`/Mask`/`/ImageMask`, not 8 bits per component, not DeviceRGB/DeviceGray/1- or 3-component ICCBased, carrying an unprovable `/DecodeParms` predictor, or that would not come out smaller.  **Do not re-implement any of that filtering** — pass the IDs and let the library skip.
- Tests: a scoped merge recompresses only the named input’s images; `--dry-run` writes nothing while still reporting the projection; a junk key exits 2 without recompressing anything; a PDF of pure line art comes out unchanged.

## Why Not Python

The recompression already exists in Rust, in a crate this repo’s sibling maintains and that two other tools in the family already consume; the only missing piece is a flag.  Reaching for a Python post-processor would mean a second, divergent implementation of the skip rules above — the ones encoding two fixed bugs (`/SMask` transparency, predictor `/DecodeParms`) — and it would run _after_ pdf-maker has written the file, re-parsing and rewriting a document that was just correctly assembled.  The scoping information that makes the operation safe (which images came from which input) exists only inside the merge, and is gone by the time any external script sees the output.
