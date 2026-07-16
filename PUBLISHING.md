# Publishing pdf-maker to crates.io

pdf-maker is part of the **PDF crate family**.  The authoritative procedure, the
dependency order, and the `publish-status` tool live in the medpdf repo:
`../medpdf/PUBLISHING.md`.

**Prerequisite (do not skip):** pdf-maker depends on `medpdf` and `medpdf-image`
via `version + path` deps.  A published crate cannot carry `path =` deps, so
those two must already be on crates.io at the pinned versions **before**
pdf-maker can publish.  Publish the medpdf workspace first (step 1 of the medpdf
procedure), then pdf-maker.

If you are adopting new medpdf / medpdf-image versions, bump the pins here as a
normal code change committed via `/commit-rust-cli` first.

The model is **publish-only**: `verset` / the `/commit-*` skills own the version
number; cargo-release only uploads and tags the already-committed current
version.  Every command is dry-run by default — add `-x` to execute.

```
cargo release publish        # dry-run preview
cargo release publish -x      # upload the current version
cargo release tag -x          # tag pdf-maker-v<version>
cargo release push -x         # push the tag
```

Do **not** run `cargo release <level>` or `cargo release version` — that bumps
the version and collides with the commit-skill workflow.  Config lives in
`release.toml`.
