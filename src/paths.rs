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
///
/// The probe answers three ways, not two (`positive-evidence-of-absence.md`):
/// present, provably absent (`NotFound`, and nothing else), or unknown.  It used
/// to use `path.exists()`, a bare bool that folds EACCES, EIO and ELOOP in with
/// `NotFound` — so an input that existed but could not be stat-ed was reported as
/// missing, sending the reader hunting for a typo that was not there (bug-0013).
/// Nothing here gates a destructive action, so the stakes are diagnostic only;
/// the three-answer shape is still the right one, and it is what makes the
/// message true.
pub fn check_input_file(path: &Path, what: &str) -> Result<()> {
    // `symlink_metadata`, not `metadata`: a symlink to a missing target should
    // report as a broken link rather than borrowing its target's absence.
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => Err(MedpdfError::new(format!(
            "{what} is a directory, not a file: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(MedpdfError::new(format!(
            "{what} does not exist: {}",
            path.display()
        ))),
        Err(e) => Err(MedpdfError::new(format!(
            "{what} cannot be accessed: {} ({e})",
            path.display()
        ))),
    }
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
    // The same three answers as `check_input_file`, for the same reason: an
    // unreadable parent directory is not a missing one, and saying so sends the
    // reader to the wrong fix (bug-0013).
    match std::fs::metadata(parent) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(MedpdfError::new(format!(
            "output path's parent is not a directory: {}",
            parent.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(MedpdfError::new(format!(
            "output directory does not exist: {} -- pdf-maker never creates directories; \
             create it first, then re-run",
            parent.display()
        ))),
        Err(e) => Err(MedpdfError::new(format!(
            "output directory cannot be accessed: {} ({e})",
            parent.display()
        ))),
    }
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

    /// bug-0013: an input that exists but cannot be stat-ed must not be reported
    /// as missing. `path.exists()` folded EACCES into `false`, so the message sent
    /// the reader hunting for a typo when the problem was permissions.
    ///
    /// Skipped when running as root, which bypasses the permission check entirely —
    /// a test that silently passes for the wrong reason is worse than no test, so
    /// it says so rather than asserting.
    #[cfg(unix)]
    #[test]
    fn unreadable_input_reports_access_not_absence() {
        use std::os::unix::fs::PermissionsExt;

        if unsafe { libc_geteuid() } == 0 {
            eprintln!("skipping: running as root, which bypasses directory permissions");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in.pdf");
        std::fs::write(&file, b"%PDF-1.7\n").unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o000)).unwrap();

        let err = check_input_file(&file, "input PDF").unwrap_err();
        let msg = err.to_string();

        // Restore before asserting, so a failure still leaves a removable tempdir.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            msg.contains("cannot be accessed"),
            "an unreadable input must report access failure, not absence: {msg}"
        );
        assert!(
            !msg.contains("does not exist"),
            "the file exists; saying otherwise sends the reader to the wrong fix: {msg}"
        );
    }

    #[cfg(unix)]
    unsafe extern "C" {
        #[link_name = "geteuid"]
        fn libc_geteuid() -> u32;
    }

    #[test]
    fn existing_output_directory_passes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_output_path(&dir.path().join("out.pdf")).is_ok());
    }
}
