//! Reading a file into a document: size, binary and UTF-8 checks, frontmatter.
//! Task 2 fills in `load`, `strip_frontmatter` and `LoadError::message`; the types are fixed.

use std::path::Path;
use std::sync::Arc;

pub enum DocState {
    Loading,
    Ready { src: Arc<str>, large: bool, lossy: bool },
    Error(LoadError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError { NotFound, Permission, Binary, TooLarge { bytes: u64 }, Io(String) }

/// Blocking; called only from workers.
pub fn load(_path: &Path) -> DocState { unimplemented!("Task 2") }

/// Drops a leading YAML (`---`) or TOML (`+++`) frontmatter block.
pub fn strip_frontmatter(src: &str) -> &str { src }

impl LoadError {
    pub fn message(&self) -> String { unimplemented!("Task 2") }
}
