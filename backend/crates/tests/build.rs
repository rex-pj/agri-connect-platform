//! Fingerprints the migrations' source, so the test template database
//! (`src/lib.rs`, `template`) is rebuilt whenever a migration changes, and
//! reused by every run until then.

use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migration/src");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    // SipHash with fixed keys: the same sources give the same fingerprint.
    let mut hasher = DefaultHasher::new();
    for file in files {
        file.file_name().hash(&mut hasher);
        fs::read(&file).unwrap().hash(&mut hasher);
    }
    println!(
        "cargo:rustc-env=MIGRATIONS_FINGERPRINT={:016x}",
        hasher.finish()
    );
}
