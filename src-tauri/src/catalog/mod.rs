//! Catalog/file-layer boundary for the upcoming library implementation.
//!
//! Design notes for Task 3:
//! - Originals are opened read-only by the file layer.
//! - Originals are never overwritten.
//! - Catalog DB lives at `<root>/.editor-catalog/catalog.db`.
//! - Thumbnail cache lives at `<root>/.editor-catalog/thumbs/`.
//! - Edits are persisted in SQLite, not sidecar files next to originals.

pub const CATALOG_DIR: &str = ".editor-catalog";
pub const CATALOG_DB: &str = ".editor-catalog/catalog.db";
pub const THUMB_CACHE_DIR: &str = ".editor-catalog/thumbs";
