//! CLI integration tests for pdf-maker.
//!
//! These tests invoke the pdf-maker binary as a subprocess to verify
//! end-to-end behavior: argument parsing, pipeline execution, and output validity.
//!
//! Tests that verify PDF content use `pdf-dump --json` for inspection.

use lopdf::{dictionary, Document, Object, Stream, StringFormat};
use std::path::Path;
use std::process::Command;

/// Create a minimal valid PDF and write it to the given path.
fn create_test_pdf(path: &Path, num_pages: u32) {
    let mut doc = Document::with_version("1.7");

    let pages_id = doc.new_object_id();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => Object::Integer(0),
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let id_bytes = b"test0123456789ab".to_vec();
    doc.trailer.set(
        "ID",
        Object::Array(vec![
            Object::String(id_bytes.clone(), StringFormat::Literal),
            Object::String(id_bytes, StringFormat::Literal),
        ]),
    );

    for i in 0..num_pages {
        let content = Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 100 700 Td (Page {}) Tj ET", i + 1).into_bytes(),
        );
        let content_id = doc.add_object(content);

        let page = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Real(0.0), Object::Real(0.0),
                Object::Real(612.0), Object::Real(792.0),
            ],
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page);

        let pd = doc
            .get_object_mut(pages_id)
            .unwrap()
            .as_dict_mut()
            .unwrap();
        pd.get_mut(b"Kids")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(Object::Reference(page_id));
        pd.set("Count", Object::Integer((i + 1) as i64));
    }

    doc.save(path).unwrap();
}

fn pdf_maker_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pdf-maker"))
}

/// Query pdf-dump for the page count of a PDF file.
fn pdf_dump_page_count(path: &Path) -> Option<u32> {
    let output = Command::new("pdf-dump")
        .args([path.to_str().unwrap(), "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("page_count")?.as_u64().map(|n| n as u32)
}

/// Check if pdf-dump reports the PDF as encrypted.
fn pdf_dump_is_encrypted(path: &Path) -> Option<bool> {
    let output = Command::new("pdf-dump")
        .args([path.to_str().unwrap(), "--detail", "security", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("encrypted")?.as_bool()
}

// --- Basic merge ---

#[test]
fn cli_merge_all_pages() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 3);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args(["-o", output.path().to_str().unwrap(), input.path().to_str().unwrap(), "all"])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path())
        .expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_merge_page_range() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 5);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args(["-o", output.path().to_str().unwrap(), input.path().to_str().unwrap(), "2-4"])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path())
        .expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

// --- Encryption (#5) ---

#[test]
fn cli_encryption_produces_encrypted_pdf() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--user-password", "userpass",
            "--owner-password", "ownerpass",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path())
        .expect("pdf-dump must be on PATH");
    assert!(encrypted, "Output PDF should be encrypted");
}

#[test]
fn cli_encryption_aes256() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--user-password", "pass",
            "--owner-password", "owner",
            "--encryption-algorithm", "aes256",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path())
        .expect("pdf-dump must be on PATH");
    assert!(encrypted);
}

#[test]
fn cli_encryption_with_permissions() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--user-password", "pass",
            "--owner-password", "owner",
            "--permissions", "print,copy",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path())
        .expect("pdf-dump must be on PATH");
    assert!(encrypted);
}

// --- Padding ---

#[test]
fn cli_pad_to_multiple() {
    // Arrange: 3 pages, pad to multiple of 4 → expect 4
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 3);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--pad-to", "4",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path())
        .expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 4);
}

// --- Blank pages ---

#[test]
fn cli_blank_page_named_size() {
    // Arrange
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            "--blank-page", "letter",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path())
        .expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

// --- Error cases ---

#[test]
fn cli_missing_output_flag() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);

    // Act
    let output = pdf_maker_bin()
        .args([input.path().to_str().unwrap(), "all"])
        .output()
        .unwrap();

    // Assert
    assert!(!output.status.success());
}

#[test]
fn cli_nonexistent_input_file() {
    // Arrange
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let result = pdf_maker_bin()
        .args(["-o", output.path().to_str().unwrap(), "/nonexistent/file.pdf", "all"])
        .output()
        .unwrap();

    // Assert
    assert!(!result.status.success());
}

#[test]
fn cli_broad_compatibility() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--broad-compatibility",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path())
        .expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}
