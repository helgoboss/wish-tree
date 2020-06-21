use crate::{FileSet, FileSetBuilder};
use core::iter;
use flate2::write::GzEncoder;
use globset::Glob;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::iter::once;
use std::path::{Path, PathBuf};
use tar::{EntryType, Header};
use walkdir::WalkDir;

/// Describes a single file, a single directory or a complete directory tree that should get mounted
/// into a user-defined directory structure.
#[derive(Clone, Debug)]
pub enum MountSource {
    /// Copies the file or directory at the given path in the file system one to one.  
    CopyFromPath(PathBuf),
    /// Creates a directory with user-defined entries.
    CustomDir(Vec<CustomDirEntry>),
    /// Generates a file with the given text content.
    TextContent(String),
    /// Copies a partial directory tree from the file system based on include patterns.
    FileSet(FileSet),
}

impl MountSource {
    /// Creates this directory structure on the file system in the specified target directory.
    pub fn render_to_fs(&self, target_dir: impl AsRef<Path>) {
        for mut w in self.walk_virtual_files() {
            let absolute_path = target_dir.as_ref().join(&w.path);
            if w.is_dir {
                fs::create_dir_all(absolute_path);
            } else {
                fs::create_dir_all(absolute_path.parent().unwrap());
                let mut file = File::create(absolute_path).unwrap();
                io::copy(&mut *w.reader, &mut file);
            }
        }
    }

    /// Creates this directory structure as a ZIP file.
    pub fn render_to_zip(&self, zip_file: impl AsRef<Path>) {
        let zip_file = File::create(zip_file).unwrap();
        let mut zip = zip::ZipWriter::new(zip_file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        let mut buffer = Vec::new();
        for mut w in self.walk_virtual_files() {
            if w.is_dir {
                if w.path.as_os_str().is_empty() {
                    // Ignore root.
                    continue;
                }
                // The "dir" case is important for empty directories only. See comment below.
                zip.add_directory_from_path(&w.path, options).unwrap();
            } else {
                // When file sets are used, it's possible that the walker visits files whose
                // parent directory has not been visited. That's not an issue when creating the ZIP
                // archive. The directory will be created automatically.
                zip.start_file_from_path(&w.path, options).unwrap();
                w.reader.read_to_end(&mut buffer).unwrap();
                zip.write_all(&*buffer).unwrap();
                buffer.clear();
            }
        }
    }

    /// Creates this directory structure as a gzipped tarball.
    pub fn render_to_tar_gz(&self, archive_path: impl AsRef<Path>) {
        let archive_file = File::create(archive_path).unwrap();
        let enc = GzEncoder::new(archive_file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        let mut buffer = Vec::new();
        for mut w in self.walk_virtual_files() {
            if w.is_dir {
                if w.path.as_os_str().is_empty() {
                    // Ignore root.
                    continue;
                }
                // The "dir" case is important for empty directories only. See comment below.
                let mut header = Header::new_gnu();
                header.set_entry_type(EntryType::Directory);
                header.set_size(0);
                tar.append_data(&mut header, w.path, io::empty()).unwrap();
            } else {
                // When file sets are used, it's possible that the walker visits files whose
                // parent directory has not been visited. That's not an issue when creating the tar
                // archive. The directory will be created automatically.
                w.reader.read_to_end(&mut buffer).unwrap();
                let mut header = Header::new_gnu();
                header.set_entry_type(EntryType::Regular);
                header.set_size(buffer.len() as _);
                tar.append_data(&mut header, w.path, buffer.as_slice())
                    .unwrap();
                buffer.clear();
            }
        }
        tar.finish().unwrap();
    }

    /// Returns an iterator that recursively walks over all defined mounts (depth-first).
    ///
    /// Starts with this mount source mounted at root ("").
    fn walk_mounts<'a>(&'a self) -> impl Iterator<Item = Mount<'a>> + 'a {
        self.walk_recursive(PathBuf::from(""))
    }

    /// Returns an iterator that recursively expands all desired mounts into concrete directories
    /// and files (depth-first).
    ///
    /// Starts with this mount source mounted at root ("").
    fn walk_virtual_files<'a>(&'a self) -> impl Iterator<Item = VirtualFile> + 'a {
        self.walk_mounts()
            .map(|m| m.source.resolve_virtual_files(m.point))
            .flatten()
    }

    /// Returns an iterator that expands just this mount source, depending on its type.
    fn resolve_virtual_files<'a>(
        &'a self,
        mount_point: PathBuf,
    ) -> Box<dyn Iterator<Item = VirtualFile> + 'a> {
        match self {
            MountSource::CopyFromPath(p) => {
                if p.is_dir() {
                    let iter = walkdir(p)
                        .map(move |e| create_virtual_file_from_dir_entry(e, &mount_point, p));
                    Box::new(iter)
                } else {
                    let wish = VirtualFile::file(mount_point, File::open(p).unwrap());
                    Box::new(iter::once(wish))
                }
            }
            MountSource::CustomDir(_) => Box::new(iter::once(VirtualFile::dir(mount_point))),
            MountSource::TextContent(text) => {
                let wish = VirtualFile::file(mount_point, io::Cursor::new(text.clone()));
                Box::new(iter::once(wish))
            }
            MountSource::FileSet(set) => {
                let base_dir = set.base_dir();
                let iter = walkdir(base_dir)
                    .filter(move |e| set.matches(e.path()))
                    .map(move |e| create_virtual_file_from_dir_entry(e, &mount_point, base_dir));
                Box::new(iter)
            }
        }
    }

    fn walk_recursive<'a>(
        &'a self,
        mount_point: PathBuf,
    ) -> Box<dyn Iterator<Item = Mount<'a>> + 'a> {
        let current_iter = once(Mount::new(mount_point.clone(), self));
        if let MountSource::CustomDir(entries) = self {
            let entry_iter = entries
                .iter()
                .map(move |entry| {
                    entry
                        .mount_source
                        .walk_recursive(mount_point.join(&entry.name))
                })
                .flatten();
            Box::new(current_iter.chain(entry_iter))
        } else {
            Box::new(current_iter)
        }
    }
}

fn walkdir(base_dir: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    WalkDir::new(base_dir).into_iter().filter_map(|e| e.ok())
}

fn create_virtual_file_from_dir_entry(
    entry: walkdir::DirEntry,
    mount_point: &Path,
    base_dir: &Path,
) -> VirtualFile {
    let full_path = mount_point.join(entry.path().strip_prefix(base_dir).unwrap());
    if entry.file_type().is_dir() {
        VirtualFile::dir(full_path)
    } else {
        VirtualFile::file(full_path, File::open(entry.path()).unwrap())
    }
}

pub struct Mount<'a> {
    point: PathBuf,
    source: &'a MountSource,
}

impl<'a> Mount<'a> {
    fn new(full_path: PathBuf, wish: &'a MountSource) -> Mount<'a> {
        Self {
            point: full_path,
            source: wish,
        }
    }

    pub fn full_path(&self) -> &Path {
        &self.point
    }

    pub fn wish(&self) -> &MountSource {
        self.source
    }
}

/// Description of a target file or directory including its path (relative to a base directory) and
/// content.
struct VirtualFile {
    path: PathBuf,
    is_dir: bool,
    reader: Box<dyn Read>,
}

impl VirtualFile {
    fn file(full_path: PathBuf, reader: impl Read + 'static) -> VirtualFile {
        VirtualFile {
            path: full_path,
            is_dir: false,
            reader: Box::new(reader),
        }
    }

    fn dir(full_path: PathBuf) -> VirtualFile {
        VirtualFile {
            path: full_path,
            is_dir: true,
            reader: Box::new(io::empty()),
        }
    }

    fn full_path(&self) -> &Path {
        &self.path
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn reader(self) -> impl Read {
        self.reader
    }
}

/// A user-defined directory entry.
#[derive(Clone, Debug)]
pub struct CustomDirEntry {
    name: PathBuf,
    mount_source: MountSource,
}

impl CustomDirEntry {
    /// Creates a user-defined directory entry with the given name and mount source.
    pub fn new(name: PathBuf, mount_source: MountSource) -> CustomDirEntry {
        CustomDirEntry { name, mount_source }
    }
}

/// Generates a file with the given text content.
pub fn text(text: impl Into<String>) -> MountSource {
    MountSource::TextContent(text.into())
}

impl<T: Into<PathBuf>> From<T> for MountSource {
    fn from(value: T) -> Self {
        MountSource::CopyFromPath(value.into())
    }
}

impl From<FileSet> for MountSource {
    fn from(value: FileSet) -> Self {
        MountSource::FileSet(value)
    }
}

impl From<&mut FileSetBuilder> for MountSource {
    fn from(value: &mut FileSetBuilder) -> Self {
        MountSource::FileSet(value.build())
    }
}

/// Builds a custom directory listing.
#[macro_export]
macro_rules! dir {
    // Allow comma even if nothing is coming behind it
    ($($key:expr => $value:expr,)+) => { dir!($($key => $value),+) };
    // Main logic
    ($($key:expr => $value:expr),*) => {
        {
            let mut entries: Vec<crate::CustomDirEntry> = Vec::new();
            $(
                entries.push(CustomDirEntry::new($key.into(), $value.into()));
            )*
            crate::MountSource::CustomDir(entries)
        }
    };
}
