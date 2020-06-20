use globset::Glob;
use std::fs;
use std::io::{Error, Read};
use std::path::Path;
use wish_tree::*;

#[test]
fn basics() {
    // Given
    let dir = dir! {
        "dist" => dir! {
            "README.md" => "README.md",
            "sources" => "tests/foo/b",
            "doc" => dir("tests/foo").include("**/*.txt"),
            "empty-dir" => dir!(),
            "empty-file" => text("")
        },
        "notes.txt" => text("Some notes"),
    };
    // When
    // Then
    let foo_dir = "target/foo-test";
    let foo_zip_file = "target/foo-test.zip";
    let foo_tar_gz_file = "target/foo-test.tar.gz";
    fs::remove_dir_all(foo_dir);
    fs::remove_file(foo_zip_file);
    fs::remove_file(foo_tar_gz_file);
    dir.render_to_fs(foo_dir);
    dir.render_to_zip(foo_zip_file);
    dir.render_to_tar_gz(foo_tar_gz_file);
}
