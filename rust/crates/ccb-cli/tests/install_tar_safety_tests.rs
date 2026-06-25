//! Mirrors Python `test/test_install_tar_safety.py`.

use ccb_cli::management_runtime::install::safe_extract_tar;
use flate2::write::GzEncoder;
use flate2::Compression;

fn build_tar_with_link(name: &str, linkname: &str) -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let encoder = GzEncoder::new(&mut buffer, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path(name).unwrap();
        header.set_link_name(linkname).unwrap();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder.append(&header, &[] as &[u8]).unwrap();
        builder.finish().unwrap();
    }
    buffer
}

fn build_tar_with_file(name: &str, content: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let encoder = GzEncoder::new(&mut buffer, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path(name).unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, content).unwrap();
        builder.finish().unwrap();
    }
    buffer
}

#[test]
fn test_safe_extract_tar_rejects_absolute_symlink_targets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let payload = build_tar_with_link("badlink", "/abs/path");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&payload[..]));

    let err = safe_extract_tar(&mut archive, tmp.path())
        .expect_err("expected unsafe symlink target to be rejected");
    let text = err.to_string();
    assert!(text.contains("Unsafe tar link target"), "{text}");
    assert!(text.contains("badlink"), "{text}");
}

#[test]
fn test_safe_extract_tar_rejects_escaping_relative_symlink_targets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let payload = build_tar_with_link("nested/badlink", "../../escape");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&payload[..]));

    let err = safe_extract_tar(&mut archive, tmp.path())
        .expect_err("expected escaping symlink target to be rejected");
    let text = err.to_string();
    assert!(text.contains("Unsafe tar link target"), "{text}");
    assert!(text.contains("nested/badlink"), "{text}");
}

#[test]
fn test_safe_extract_tar_extracts_regular_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let payload = build_tar_with_file("hello.txt", b"hello world");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&payload[..]));

    safe_extract_tar(&mut archive, tmp.path()).expect("regular file extraction should succeed");

    let extracted = tmp.path().join("hello.txt");
    assert!(extracted.exists());
    let content = std::fs::read_to_string(&extracted).unwrap();
    assert_eq!(content, "hello world");
}
