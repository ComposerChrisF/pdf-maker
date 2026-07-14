//! CLI integration tests for pdf-maker.
//!
//! These tests invoke the pdf-maker binary as a subprocess to verify
//! end-to-end behavior: argument parsing, pipeline execution, and output validity.
//!
//! Tests that verify PDF content use `pdf-dump --json` for inspection.

use lopdf::{Document, Object, Stream, StringFormat, dictionary};
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
            "Resources" => dictionary! {},
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page);

        let pd = doc.get_object_mut(pages_id).unwrap().as_dict_mut().unwrap();
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

/// Check whether pdf-dump reports the PDF as encrypted. Passes the user
/// password so pdf-dump (0.23+) can decrypt and exit 0; without a password it
/// exits non-zero (code 3) on an encrypted file and reports only partial data.
fn pdf_dump_is_encrypted(path: &Path, password: &str) -> Option<bool> {
    let output = Command::new("pdf-dump")
        .args([
            path.to_str().unwrap(),
            "--password",
            password,
            "--detail",
            "security",
            "--json",
        ])
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
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
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
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "2-4",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
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
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "userpass",
            "--owner-password",
            "ownerpass",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted =
        pdf_dump_is_encrypted(output.path(), "userpass").expect("pdf-dump must be on PATH");
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
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "pass",
            "--owner-password",
            "owner",
            "--encryption-algorithm",
            "aes256",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path(), "pass").expect("pdf-dump must be on PATH");
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
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "pass",
            "--owner-password",
            "owner",
            "--permissions",
            "print,copy",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path(), "pass").expect("pdf-dump must be on PATH");
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
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--pad-to",
            "4",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
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
            "-o",
            output.path().to_str().unwrap(),
            "--blank-page",
            "letter",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
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
        .args([
            "-o",
            output.path().to_str().unwrap(),
            "/nonexistent/file.pdf",
            "all",
        ])
        .output()
        .unwrap();

    // Assert: a missing caller-asserted input is exit 1, naming the path.
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("/nonexistent/file.pdf"),
        "stderr must name the missing input: {stderr}"
    );
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
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--broad-compatibility",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

// --- Drawing / overlay / imposition coverage (N6) ---

/// Minimal 1×1 red RGB PNG for --draw-image tests.
const RED_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x00, 0x03, 0x00, 0x01, 0xE3, 0xCE, 0xC5, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn write_test_png(path: &Path) {
    std::fs::write(path, RED_PIXEL_PNG).unwrap();
}

#[test]
fn cli_blank_page_custom_dimensions() {
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            "--blank-page",
            "w=8.5,h=11,units=in,count=3",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_watermark_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--watermark",
            "text=DRAFT,font=@Helvetica,size=24,x=1,y=1,units=in,color=red,alpha=0.4,h_align=center,v_align=center,pages=all",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_draw_rect_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-rect",
            "x=72,y=72,w=200,h=100,color=blue,alpha=0.5,pages=all",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_draw_line_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-line",
            "x1=72,y1=720,x2=540,y2=720,width=1.5,color=#444,pages=1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

#[test]
fn cli_draw_image_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let img = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
    write_test_png(img.path());
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-image",
            &format!(
                "file={},x=1,y=1,w=2,units=in,fit=contain,pages=all",
                img.path().display()
            ),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_overlay_applied() {
    let base = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(base.path(), 3);
    let overlay = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(overlay.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            base.path().to_str().unwrap(),
            "all",
            "--overlay",
            &format!(
                "file={},src_page=1,target_pages=1-3",
                overlay.path().display()
            ),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_nup_2x2() {
    // 8 input pages with cells_per_sheet = 4 → 2 output sheets.
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 8);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--nup",
            "cols=2,rows=2",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

// --- Out-of-range page specs must ERROR, never be silently dropped ---
//
// Regression tests for the silent-wrong-output bug: `parse_page_spec` filtered
// pages beyond the document, so `1,99` against a 2-page PDF produced a 1-page
// PDF at exit 0.  Every case below asserts the exit CODE and, where a file could
// still be produced, the page count.

/// Run pdf-maker against an `n`-page input with the given page spec, returning
/// the exit code and the page count of any output actually produced.
fn merge_with_spec(source_pages: u32, spec: &str) -> (Option<i32>, Option<u32>) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, source_pages);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            spec,
        ])
        .output()
        .unwrap();

    let pages = if output.exists() {
        pdf_dump_page_count(&output)
    } else {
        None
    };
    (result.status.code(), pages)
}

#[test]
fn cli_out_of_range_page_mixed_with_valid_page_errors() {
    // The headline bug: this used to exit 0 with a 1-page PDF.
    let (code, pages) = merge_with_spec(2, "1,99");
    assert_eq!(code, Some(1), "out-of-range page must be a tool error");
    assert_eq!(pages, None, "no output file may be produced");
}

#[test]
fn cli_out_of_range_page_names_the_page_and_the_page_count() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("99"),
        "must name the offending page: {stderr}"
    );
    assert!(
        stderr.contains("2 page(s)"),
        "must name the document's real page count: {stderr}"
    );
}

#[test]
fn cli_wholly_out_of_range_spec_errors() {
    let (code, pages) = merge_with_spec(2, "99");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_range_past_the_end_errors_instead_of_clamping() {
    // "1-100" against a 2-page doc used to yield 2 pages at exit 0.
    let (code, pages) = merge_with_spec(2, "1-100");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_ended_range_past_the_end_errors() {
    // "5-" against a 2-page doc used to select nothing.
    let (code, pages) = merge_with_spec(2, "5-");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_start_range_past_the_end_errors() {
    // A bare "-9" cannot reach the page-spec parser: clap eats a leading dash as
    // a flag and exits 2, which is correctly clap's territory.  "1,-9" carries
    // the same open-start range through argv, and must error on page 9.
    let (code, pages) = merge_with_spec(2, "1,-9");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_ended_range_within_the_document_still_works() {
    let (code, pages) = merge_with_spec(3, "2-");
    assert_eq!(code, Some(0));
    assert_eq!(pages, Some(2));
}

#[test]
fn cli_all_still_works() {
    let (code, pages) = merge_with_spec(3, "all");
    assert_eq!(code, Some(0));
    assert_eq!(pages, Some(3));
}

#[test]
fn cli_out_of_range_page_in_a_second_input_errors() {
    // The whole second input used to be dropped silently, leaving a 1-page PDF.
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1",
            input.to_str().unwrap(),
            "5-",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists(), "no output file may be produced");
}

#[test]
fn cli_watermark_targeting_a_nonexistent_page_errors() {
    // Used to exit 0 having applied no watermark at all.
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--watermark",
            "text=DRAFT,font=@Helvetica,x=1,y=1,pages=99",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn cli_draw_rect_targeting_a_nonexistent_page_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--draw-rect",
            "x=1,y=1,w=10,h=10,pages=7",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn cli_overlay_targeting_a_nonexistent_page_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--overlay",
            &format!("file={},src_page=1,target_pages=99", input.display()),
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

// --- Path contract (~/.claude/rules/cli-exit-codes.md) ---

#[test]
fn cli_missing_output_directory_errors_and_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 1);
    let missing_dir = dir.path().join("no-such-dir");
    let output = missing_dir.join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("no-such-dir"),
        "stderr must name the missing output directory: {stderr}"
    );
    assert!(
        !missing_dir.exists(),
        "pdf-maker must never create the output directory"
    );
}

#[test]
fn cli_missing_overlay_file_errors_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 1);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--overlay",
            "file=/nonexistent/overlay.pdf,src_page=1",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("/nonexistent/overlay.pdf"), "{stderr}");
    assert!(!output.exists());
}

// --- --dry-run ---

#[test]
fn cli_dry_run_writes_no_file_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 3);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    assert!(!output.exists(), "--dry-run must create NO file");
}

#[test]
fn cli_dry_run_still_catches_an_out_of_range_page() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

// --- --json ---

#[test]
fn cli_json_summary_reports_the_page_count() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 3);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "2-3",
            "--json",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["page_count"], 2);
    assert_eq!(json["written"], true);
    assert_eq!(json["dry_run"], false);
    assert_eq!(json["inputs"][0]["pages"], serde_json::json!([2, 3]));
    assert!(output.exists());
}

#[test]
fn cli_json_dry_run_reports_written_false() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--json",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["written"], false);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["page_count"], 2);
    assert_eq!(json["bytes"], serde_json::Value::Null);
    assert!(!output.exists());
}

#[test]
fn cli_json_error_object_on_an_out_of_range_page() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
            "--json",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["exit_code"], 1);
    let error = json["error"].as_str().expect("error is a string");
    assert!(error.contains("99"), "{error}");
}

#[test]
fn cli_booklet_default() {
    // 4 input pages → 2 booklet sheets (front + back).
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 4);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--booklet",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}
