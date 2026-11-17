# Plan: Take medpdf and medpdf-image From GitHub, Pinned in `Cargo.lock`

## Problem

`medpdf` and `medpdf-image` are `version + path` dependencies on `../medpdf/…`.  A clone of pdf-maker cannot build unless a medpdf checkout sits beside it, and a local build compiles whatever that checkout holds, committed or not.  So the build depends on the state of a neighbouring directory rather than on anything this repository records.

## Proposed Change

Replace `path = "../medpdf/medpdf"` and `path = "../medpdf/medpdf-image"` with `git = "https://github.com/ComposerChrisF/medpdf"` in `Cargo.toml`.  Both crates live in that one repository, and Cargo finds each by name.  Keep `version = "0.15"` and `version = "0.4"`, and the features.  Then `cargo update -p medpdf -p medpdf-image` pins each to a commit in `Cargo.lock` (`source = "git+https://github.com/ComposerChrisF/medpdf#<commit>"`).

- **Newer medpdf:** push it there first, then `cargo update -p <crate>` here.  That is a dependency change, so it is a PATCH commit.
- **Building against a local, unpushed medpdf:** use a `[patch."https://github.com/ComposerChrisF/medpdf"]` override, on the command line or in a `.cargo/config.toml` that is never committed.  While that override is active, Cargo rewrites the lock without the pin.  So drop the override before committing, and confirm with `cargo metadata --locked --format-version 1`.

## Implementation Notes

- **crates.io:** `cargo publish` strips `git =` exactly as it stripped `path =`, so the version keys still govern a crates.io release.  pdf-maker’s next publish therefore still waits on medpdf 0.15 and medpdf-image 0.4 reaching crates.io (medpdf is at 0.9.2 there today).  `PUBLISHING.md` records that prerequisite.
- **Proof:** a fresh clone with no medpdf beside it must resolve with `cargo metadata --locked`.
- **Docs this makes untrue:** `CLAUDE.md` § Repo Structure and `PUBLISHING.md` both describe the path dependencies; update them in the same commit.

The procedure is the coding hub’s note on shared crates from GitHub, which covers the same move across the portfolio.

## Why Not a Workaround

Keeping the path dependency and documenting “clone medpdf beside it” leaves the build tied to an unrecorded neighbour.  A pin in `Cargo.lock` is the one place Cargo itself enforces which medpdf commit a build uses.
