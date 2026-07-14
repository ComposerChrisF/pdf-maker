//! Path-contract checks, per `~/.claude/rules/cli-exit-codes.md`
//! § Input and Output Paths.
//!
//! Two rules, both enforced before any work begins so a bad path costs nothing:
//!
//! * **A caller-asserted input path must exist.**  Missing is a tool error
//!   (exit 1) naming the path -- never a silent success, and never auto-created.
//! * **The output's parent directory must already exist.**  Writing to `dir/f.pdf`
//!   asserts `dir/` exists, exactly as naming an input asserts the input exists.
//!   pdf-maker creates the terminal file only, never a directory: the
//!   `--create-destination=none` default.

use medpdf::{MedpdfError, Result};
use std::path::Path;

/// A caller-asserted input file (an input PDF, an overlay source, a
/// `--draw-image` file, a `--pad-last-page-file`) must exist and be a file.
pub fn check_input_file(path: &Path, what: &str) -> Result<()> {
    if !path.exists() {
        return Err(MedpdfError::new(format!(
            "{what} does not exist: {}",
            path.display()
        )));
    }
    if path.is_dir() {
        return Err(MedpdfError::new(format!(
            "{what} is a directory, not a file: {}",
            path.display()
        )));
    }
    Ok(())
}

/// The output file's parent directory must already exist; pdf-maker never
/// creates it.  Also refuses an output path that is itself an existing
/// directory.
pub fn check_output_path(output: &Path) -> Result<()> {
    if output.is_dir() {
        return Err(MedpdfError::new(format!(
            "output path is an existing directory, not a file: {}",
            output.display()
        )));
    }
    let parent = match output.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        // A bare filename ("out.pdf") writes into the current directory.
        _ => return Ok(()),
    };
    if !parent.is_dir() {
        return Err(MedpdfError::new(format!(
            "output directory does not exist: {} -- pdf-maker never creates directories; \
             create it first, then re-run",
            parent.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_input_is_named_in_the_error() {
        let err = check_input_file(Path::new("/nonexistent/in.pdf"), "input PDF").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("/nonexistent/in.pdf"), "{msg}");
        assert!(msg.contains("input PDF"), "{msg}");
    }

    #[test]
    fn existing_input_passes() {
        let f = tempfile::NamedTempFile::new().unwrap();
        assert!(check_input_file(f.path(), "input PDF").is_ok());
    }

    #[test]
    fn missing_output_directory_is_named_in_the_error() {
        let err = check_output_path(Path::new("/nonexistent/dir/out.pdf")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("/nonexistent/dir"), "{msg}");
        assert!(msg.contains("never creates directories"), "{msg}");
    }

    #[test]
    fn bare_filename_is_accepted() {
        assert!(check_output_path(Path::new("out.pdf")).is_ok());
    }

    #[test]
    fn existing_output_directory_passes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_output_path(&dir.path().join("out.pdf")).is_ok());
    }
}
