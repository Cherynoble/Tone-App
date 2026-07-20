//! Catalog/file-layer boundary for the upcoming library implementation.
//!
//! Design notes for Task 3:
//! - Originals are opened read-only by the file layer.
//! - Originals are never overwritten.
//! - Catalog DB lives at `<root>/.editor-catalog/catalog.db`.
//! - Thumbnail cache lives at `<root>/.editor-catalog/thumbs/`.
//! - Edits are persisted in SQLite, not sidecar files next to originals.

use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub const CATALOG_DIR: &str = ".editor-catalog";
pub const CATALOG_DB_FILE: &str = "catalog.db";
pub const THUMB_CACHE_DIR_NAME: &str = "thumbs";
pub const CATALOG_DB: &str = ".editor-catalog/catalog.db";
pub const THUMB_CACHE_DIR: &str = ".editor-catalog/thumbs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPaths {
    pub root: PathBuf,
    pub catalog_dir: PathBuf,
    pub catalog_db: PathBuf,
    pub thumb_cache_dir: PathBuf,
}

impl CatalogPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let catalog_dir = root.join(CATALOG_DIR);
        let catalog_db = catalog_dir.join(CATALOG_DB_FILE);
        let thumb_cache_dir = catalog_dir.join(THUMB_CACHE_DIR_NAME);
        Self {
            root,
            catalog_dir,
            catalog_db,
            thumb_cache_dir,
        }
    }
}

pub fn open_original_read_only(path: impl AsRef<Path>) -> io::Result<File> {
    OpenOptions::new().read(true).write(false).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn derives_catalog_paths_under_editor_catalog_root() {
        let paths = CatalogPaths::new("/library/root");
        assert_eq!(
            paths.catalog_dir,
            PathBuf::from("/library/root/.editor-catalog")
        );
        assert_eq!(
            paths.catalog_db,
            PathBuf::from("/library/root/.editor-catalog/catalog.db")
        );
        assert_eq!(
            paths.thumb_cache_dir,
            PathBuf::from("/library/root/.editor-catalog/thumbs")
        );
    }

    #[test]
    fn opens_originals_without_write_access() {
        let path =
            std::env::temp_dir().join(format!("tone-catalog-readonly-{}.txt", std::process::id()));
        std::fs::write(&path, b"original").expect("write temp original");

        let mut file = open_original_read_only(&path).expect("open read-only original");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("read original");
        assert_eq!(contents, "original");
        assert!(file.write_all(b"edit").is_err());

        let _ = std::fs::remove_file(path);
    }
}
