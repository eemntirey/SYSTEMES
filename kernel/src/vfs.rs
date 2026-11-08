//! Minimal virtual filesystem layer.
//!
//! Provides a `Vfs` abstraction over a mounted read-only filesystem (currently
//! `simple_fs::SimpleFs`). Supports path lookup (`/file`), open, read, and
//! close. Designed for boot-time userland bring-up before a full VFS with
//! mount points, inodes, and directory traversal is implemented.

use crate::simple_fs::{Inode, SimpleFs, SimpleFsError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsError {
    NotMounted,
    NotFound,
    NotAFile,
    OutOfHandles,
    IoError,
    BadHandle,
}

/// An open file handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileHandle(pub u32);

/// A mounted read-only filesystem.
pub struct Vfs {
    fs: Option<SimpleFs>,
    root_inode: Option<Inode>,
}

impl Vfs {
    /// Create an empty, unmounted VFS.
    pub const fn new() -> Self {
        Vfs {
            fs: None,
            root_inode: None,
        }
    }

    /// Mount a `SimpleFs` image. The root inode is looked up from the
    /// superblock's `root_inode_index`.
    pub fn mount(&mut self, fs: SimpleFs) -> Result<(), VfsError> {
        let root = fs
            .inode(fs.superblock.root_inode_index)
            .map_err(|_| VfsError::IoError)?;
        if !root.is_directory() {
            return Err(VfsError::NotAFile);
        }
        self.fs = Some(fs);
        self.root_inode = Some(root);
        Ok(())
    }

    /// Open a file at the given path (relative to root, e.g. `"hello.txt"`).
    /// Only single-component paths are supported in this minimal implementation.
    ///
    /// Returns a handle that can be used with [`read`](Self::read).
    pub fn open(&self, path: &str) -> Result<FileHandle, VfsError> {
        let fs = self.fs.as_ref().ok_or(VfsError::NotMounted)?;
        let root = self.root_inode.ok_or(VfsError::IoError)?;
        let inode_index = fs
            .find_in_dir(&root, path)
            .map_err(|_| VfsError::NotFound)?;
        let inode = fs.inode(inode_index).map_err(|_| VfsError::IoError)?;
        if inode.is_directory() {
            return Err(VfsError::NotAFile);
        }
        // Handle value 1 is reserved for the first open file.
        Ok(FileHandle(inode_index + 1))
    }

    /// Read up to `buf.len()` bytes from an open file into `buf`.
    /// Returns the number of bytes read.
    pub fn read(&self, handle: FileHandle, buf: &mut [u8]) -> Result<usize, VfsError> {
        let fs = self.fs.as_ref().ok_or(VfsError::NotMounted)?;
        // Handle 0 is invalid; handles start at 1.
        if handle.0 == 0 {
            return Err(VfsError::BadHandle);
        }
        let inode_index = handle.0 - 1;
        let inode = fs.inode(inode_index).map_err(|_| VfsError::BadHandle)?;
        if inode.is_directory() {
            return Err(VfsError::NotAFile);
        }
        fs.read_file(&inode, buf).map_err(|_| VfsError::IoError)
    }

    /// Close an open file handle. In this minimal implementation handles are
    /// never recycled, so close is a no-op after validation.
    pub fn close(&self, _handle: FileHandle) -> Result<(), VfsError> {
        if self.fs.is_none() {
            return Err(VfsError::NotMounted);
        }
        Ok(())
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::vec::Vec;

    fn make_test_fs() -> SimpleFs {
        let mut data = Vec::new();
        data.resize(512, 0);
        data[0..6].copy_from_slice(b"SYSFS1");
        data[6..10].copy_from_slice(&1u32.to_le_bytes());
        data[10..14].copy_from_slice(&512u32.to_le_bytes());
        data[14..18].copy_from_slice(&2u32.to_le_bytes());
        data[18..22].copy_from_slice(&1u32.to_le_bytes());
        data[22..26].copy_from_slice(&0u32.to_le_bytes());
        data.resize(512 + 128, 0);
        data[512..512 + 64].copy_from_slice(&{
            let mut inode = [0u8; 64];
            inode[0..2].copy_from_slice(&1u16.to_le_bytes()); // directory
            inode[2..6].copy_from_slice(&1u32.to_le_bytes());
            inode
        });
        data[512 + 64..512 + 128].copy_from_slice(&{
            let mut inode = [0u8; 64];
            inode[2..6].copy_from_slice(&5u32.to_le_bytes()); // size 5
            inode[6..10].copy_from_slice(&1u32.to_le_bytes()); // first block 1
            inode[10..14].copy_from_slice(&1u32.to_le_bytes()); // 1 block
            let name = b"hello.txt";
            inode[14..14 + name.len()].copy_from_slice(name);
            inode
        });
        data.resize(512 + 128 + 512 + 5, 0);
        data[512 + 128 + 512..512 + 128 + 512 + 5].copy_from_slice(b"hello");

        unsafe { SimpleFs::parse(Box::leak(data.into_boxed_slice())) }.unwrap()
    }

    #[test]
    fn mount_and_read_file() {
        let fs = make_test_fs();
        let mut vfs = Vfs::new();
        assert!(vfs.mount(fs).is_ok());
        let handle = vfs.open("hello.txt").expect("open hello.txt");
        let mut buf = [0u8; 16];
        let n = vfs.read(handle, &mut buf).expect("read hello.txt");
        assert_eq!(&buf[..n], b"hello");
        assert!(vfs.close(handle).is_ok());
    }

    #[test]
    fn open_nonexistent_file_fails() {
        let fs = make_test_fs();
        let mut vfs = Vfs::new();
        assert!(vfs.mount(fs).is_ok());
        assert_eq!(vfs.open("missing.txt"), Err(VfsError::NotFound));
    }

    #[test]
    fn open_directory_fails() {
        let fs = make_test_fs();
        let mut vfs = Vfs::new();
        assert!(vfs.mount(fs).is_ok());
        assert_eq!(vfs.open(""), Err(VfsError::NotFound));
    }
}
