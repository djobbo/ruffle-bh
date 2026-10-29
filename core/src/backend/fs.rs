//! AIR filesystem support (`flash.filesystem.File` / `FileStream`).
//!
//! Flash Player does not expose a general-purpose filesystem, but Adobe AIR does.
//! Ruffle runs AIR content (`renderMode=direct`, `extendedDesktop`) and needs a
//! way to satisfy `File` / `FileStream` reads. Hosts provide an implementation of
//! [`FileSystemBackend`]: the web player serves files over HTTP, the desktop
//! player forwards to the host filesystem.
//!
//! Paths are UTF-8 and use forward slashes. They are absolute from the host's
//! point of view (for the web build, the "root" is a virtual directory such as
//! `/depot`).

use std::io;

/// One entry returned by [`FileSystemBackend::list_directory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

/// Host-side filesystem used by AIR `File`/`FileStream` APIs.
///
/// All methods are synchronous: `FileStream.open()` in AIR is synchronous and
/// Brawlhalla relies on that (it reads every asset on the main thread before
/// handing the bytes to `Loader.loadBytes`).
pub trait FileSystemBackend: Send + Sync {
    /// Read an entire file. Returns `ErrorKind::NotFound` when the path does not exist.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;

    /// Write an entire file, replacing it if it exists.
    fn write(&self, _path: &str, _data: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "writing is not supported by this filesystem",
        ))
    }

    /// Whether a path exists (file or directory).
    fn exists(&self, path: &str) -> bool {
        self.read(path).is_ok()
    }

    fn is_directory(&self, _path: &str) -> bool {
        false
    }

    /// Size in bytes. `None` for directories or missing files.
    fn size(&self, path: &str) -> Option<u64> {
        self.read(path).ok().map(|bytes| bytes.len() as u64)
    }

    fn list_directory(&self, _path: &str) -> io::Result<Vec<FileEntry>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory listing is not supported by this filesystem",
        ))
    }

    fn create_directory(&self, _path: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "creating directories is not supported by this filesystem",
        ))
    }

    fn delete(&self, _path: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "deleting is not supported by this filesystem",
        ))
    }

    /// `File.applicationDirectory.nativePath`
    fn application_directory(&self) -> String {
        String::new()
    }

    /// `File.applicationStorageDirectory.nativePath`
    fn application_storage_directory(&self) -> String {
        String::new()
    }

    /// `File.userDirectory.nativePath`
    fn user_directory(&self) -> String {
        String::new()
    }

    /// `File.documentsDirectory.nativePath`
    fn documents_directory(&self) -> String {
        String::new()
    }

    /// `File.desktopDirectory.nativePath`
    fn desktop_directory(&self) -> String {
        String::new()
    }
}

/// Filesystem that reports every path as missing. Used when no host filesystem
/// is configured (e.g. a plain browser embed that only runs a single SWF).
#[derive(Debug, Default)]
pub struct NullFileSystem;

impl FileSystemBackend for NullFileSystem {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("file not found: {path}"),
        ))
    }
}

/// Desktop filesystem backed by the OS. Used when running AIR content
/// (Brawlhalla) with Ruffle desktop so `File`/`FileStream` can read the depot.
#[derive(Debug, Clone)]
pub struct DiskFileSystem {
    root: std::path::PathBuf,
}

impl DiskFileSystem {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> std::path::PathBuf {
        let normalized = path.replace('\\', "/");
        let root = self.root.to_string_lossy().replace('\\', "/");
        let relative = normalized
            .strip_prefix(&root)
            .unwrap_or(&normalized)
            .trim_start_matches('/');
        let mut resolved = self.root.clone();
        for segment in relative.split('/') {
            if !segment.is_empty() && segment != "." && segment != ".." {
                resolved.push(segment);
            }
        }
        resolved
    }
}

impl FileSystemBackend for DiskFileSystem {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.resolve(path))
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        let path = self.resolve(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    fn is_directory(&self, path: &str) -> bool {
        self.resolve(path).is_dir()
    }

    fn size(&self, path: &str) -> Option<u64> {
        std::fs::metadata(self.resolve(path))
            .ok()
            .map(|metadata| metadata.len())
    }

    fn list_directory(&self, path: &str) -> io::Result<Vec<FileEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(self.resolve(path))? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_directory: metadata.is_dir(),
                size: metadata.len(),
            });
        }
        Ok(entries)
    }

    fn create_directory(&self, path: &str) -> io::Result<()> {
        std::fs::create_dir_all(self.resolve(path))
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let path = self.resolve(path);
        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    }

    fn application_directory(&self) -> String {
        self.root.to_string_lossy().replace('\\', "/")
    }

    fn application_storage_directory(&self) -> String {
        self.root
            .join("storage")
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn user_directory(&self) -> String {
        std::env::var("RUFFLE_USER_DIR")
            .map(|dir| dir.replace('\\', "/"))
            .unwrap_or_else(|_| self.application_storage_directory())
    }

    fn documents_directory(&self) -> String {
        self.application_storage_directory()
    }

    fn desktop_directory(&self) -> String {
        self.application_storage_directory()
    }
}

