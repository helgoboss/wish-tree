use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};

/// A group of files starting in a base directory.
#[derive(Clone, Debug)]
pub struct FileSet {
    base_dir: PathBuf,
    includes: GlobSet,
}

impl FileSet {
    pub(crate) fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub(crate) fn matches(&self, path: impl AsRef<Path>) -> bool {
        self.includes.is_match(path)
    }
}

/// A builder for constructing file sets.
#[derive(Clone, Debug)]
pub struct FileSetBuilder {
    base_dir: PathBuf,
    includes: GlobSetBuilder,
}

impl FileSetBuilder {
    fn new(base_dir: PathBuf) -> FileSetBuilder {
        Self {
            base_dir,
            includes: GlobSetBuilder::new(),
        }
    }

    /// Defines an include pattern.
    ///
    /// See the [globset documentation](https://docs.rs/globset/0.4.5/globset/#syntax) for details
    /// about the pattern syntax.
    pub fn include(&mut self, value: impl AsRef<str>) -> &mut Self {
        self.includes.add(Glob::new(value.as_ref()).unwrap());
        self
    }

    pub(crate) fn build(&self) -> FileSet {
        FileSet {
            base_dir: self.base_dir.clone(),
            includes: self.includes.build().unwrap(),
        }
    }
}

/// Mounts a set of files from the given base directory.
pub fn dir(base_dir: impl Into<PathBuf>) -> FileSetBuilder {
    FileSetBuilder::new(base_dir.into())
}
