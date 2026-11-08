//! Read-only VFS over a mounted SYSFS filesystem.

use crate::simple_fs::{Inode, SimpleFs, SimpleFsError};

const MAX_OPEN_FILES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsError {
    NotMounted,
    NotFound,
    NotAFile,
    InvalidPath,
    NameTooLong,
    OutOfHandles,
    Busy,
    IoError,
    BadHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileHandle(pub u32);

#[derive(Clone, Copy)]
struct OpenFile {
    handle: FileHandle,
    inode_index: u32,
    offset: usize,
}

pub struct Vfs<'a> {
    fs: Option<SimpleFs<'a>>,
    root_inode: Option<Inode>,
    handles: [Option<OpenFile>; MAX_OPEN_FILES],
    next_handle: u32,
}

impl<'a> Vfs<'a> {
    pub const fn new() -> Self {
        Self {
            fs: None,
            root_inode: None,
            handles: [None; MAX_OPEN_FILES],
            next_handle: 1,
        }
    }

    pub fn mount(&mut self, fs: SimpleFs<'a>) -> Result<(), VfsError> {
        if self.handles.iter().any(Option::is_some) {
            return Err(VfsError::Busy);
        }
        let root = fs
            .inode(fs.superblock.root_inode_index)
            .map_err(map_fs_error)?;
        if !root.is_directory() {
            return Err(VfsError::NotAFile);
        }
        self.fs = Some(fs);
        self.root_inode = Some(root);
        Ok(())
    }

    pub fn open(&mut self, path: &str) -> Result<FileHandle, VfsError> {
        let path = path.strip_prefix('/').unwrap_or(path);
        let (inode_index, inode) = {
            let fs = self.fs.as_ref().ok_or(VfsError::NotMounted)?;
            let root = self.root_inode.ok_or(VfsError::IoError)?;
            let inode_index = fs.find_in_dir(&root, path).map_err(map_fs_error)?;
            let inode = fs.inode(inode_index).map_err(map_fs_error)?;
            (inode_index, inode)
        };
        if inode.is_directory() {
            return Err(VfsError::NotAFile);
        }
        let slot = self
            .handles
            .iter()
            .position(Option::is_none)
            .ok_or(VfsError::OutOfHandles)?;
        let handle = self.allocate_handle()?;
        self.handles[slot] = Some(OpenFile {
            handle,
            inode_index,
            offset: 0,
        });
        Ok(handle)
    }

    pub fn read(&mut self, handle: FileHandle, buf: &mut [u8]) -> Result<usize, VfsError> {
        let slot = self
            .handles
            .iter()
            .position(|open| open.is_some_and(|open| open.handle == handle))
            .ok_or(VfsError::BadHandle)?;
        let open = self.handles[slot].ok_or(VfsError::BadHandle)?;
        let fs = self.fs.as_ref().ok_or(VfsError::NotMounted)?;
        let inode = fs.inode(open.inode_index).map_err(map_fs_error)?;
        let count = fs
            .read_file_at(&inode, open.offset, buf)
            .map_err(map_fs_error)?;
        let updated_offset = open.offset.checked_add(count).ok_or(VfsError::IoError)?;
        let open = self.handles[slot].as_mut().ok_or(VfsError::BadHandle)?;
        open.offset = updated_offset;
        Ok(count)
    }

    pub fn close(&mut self, handle: FileHandle) -> Result<(), VfsError> {
        let slot = self
            .handles
            .iter()
            .position(|open| open.is_some_and(|open| open.handle == handle))
            .ok_or(VfsError::BadHandle)?;
        self.handles[slot] = None;
        Ok(())
    }

    fn allocate_handle(&mut self) -> Result<FileHandle, VfsError> {
        for _ in 0..=MAX_OPEN_FILES {
            let candidate = self.next_handle;
            self.next_handle = self.next_handle.wrapping_add(1).max(1);
            if candidate != 0
                && !self
                    .handles
                    .iter()
                    .flatten()
                    .any(|open| open.handle.0 == candidate)
            {
                return Ok(FileHandle(candidate));
            }
        }
        Err(VfsError::OutOfHandles)
    }
}

impl Default for Vfs<'_> {
    fn default() -> Self {
        Self::new()
    }
}

fn map_fs_error(error: SimpleFsError) -> VfsError {
    match error {
        SimpleFsError::InodeNotFound => VfsError::NotFound,
        SimpleFsError::NotADirectory => VfsError::NotAFile,
        SimpleFsError::InvalidPath => VfsError::InvalidPath,
        SimpleFsError::NameTooLong => VfsError::NameTooLong,
        SimpleFsError::BadMagic
        | SimpleFsError::BadVersion
        | SimpleFsError::TruncatedImage
        | SimpleFsError::BlockOutOfRange
        | SimpleFsError::InvalidInode => VfsError::IoError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn make_test_image() -> Vec<u8> {
        let mut data = vec![0; 1536];
        data[0..6].copy_from_slice(b"SYSFS1");
        data[6..10].copy_from_slice(&1u32.to_le_bytes());
        data[10..14].copy_from_slice(&512u32.to_le_bytes());
        data[14..18].copy_from_slice(&2u32.to_le_bytes());
        data[18..22].copy_from_slice(&1u32.to_le_bytes());
        data[22..26].copy_from_slice(&0u32.to_le_bytes());
        data[512..514].copy_from_slice(&1u16.to_le_bytes());
        data[578..582].copy_from_slice(&5u32.to_le_bytes());
        data[582..586].copy_from_slice(&0u32.to_le_bytes());
        data[586..590].copy_from_slice(&1u32.to_le_bytes());
        data[590..599].copy_from_slice(b"hello.txt");
        data[1024..1029].copy_from_slice(b"hello");
        data
    }

    fn mounted_vfs(image: &[u8]) -> Vfs<'_> {
        let fs = SimpleFs::parse(image).expect("valid SYSFS image");
        let mut vfs = Vfs::new();
        vfs.mount(fs).expect("mount SYSFS image");
        vfs
    }

    #[test]
    fn mount_open_read_and_close_track_file_offsets() {
        let image = make_test_image();
        let mut vfs = mounted_vfs(&image);
        let handle = vfs.open("/hello.txt").expect("open file");
        let mut first = [0; 2];
        let mut rest = [0; 8];
        assert_eq!(vfs.read(handle, &mut first), Ok(2));
        assert_eq!(&first, b"he");
        assert_eq!(vfs.read(handle, &mut rest), Ok(3));
        assert_eq!(&rest[..3], b"llo");
        assert_eq!(vfs.read(handle, &mut rest), Ok(0));
        assert_eq!(vfs.close(handle), Ok(()));
        assert_eq!(vfs.read(handle, &mut rest), Err(VfsError::BadHandle));
        assert_eq!(vfs.close(handle), Err(VfsError::BadHandle));
    }

    #[test]
    fn rejects_invalid_paths_and_unmounted_access() {
        let image = make_test_image();
        let mut vfs = Vfs::new();
        assert_eq!(vfs.open("hello.txt"), Err(VfsError::NotMounted));
        vfs.mount(SimpleFs::parse(&image).unwrap()).unwrap();
        assert_eq!(vfs.open("missing.txt"), Err(VfsError::NotFound));
        assert_eq!(vfs.open("../hello.txt"), Err(VfsError::InvalidPath));
        assert_eq!(vfs.open(&"x".repeat(47)), Err(VfsError::NameTooLong));
        assert_eq!(vfs.open(""), Err(VfsError::InvalidPath));
    }

    #[test]
    fn handles_are_independent_and_limited() {
        let image = make_test_image();
        let mut vfs = mounted_vfs(&image);
        let first = vfs.open("hello.txt").unwrap();
        let second = vfs.open("hello.txt").unwrap();
        assert_ne!(first, second);

        for _ in 2..MAX_OPEN_FILES {
            vfs.open("hello.txt").unwrap();
        }
        assert_eq!(vfs.open("hello.txt"), Err(VfsError::OutOfHandles));

        vfs.close(first).unwrap();
        let replacement = vfs.open("hello.txt").unwrap();
        assert_ne!(replacement, first);
        let mut content = [0; 8];
        assert_eq!(vfs.read(second, &mut content), Ok(5));
        assert_eq!(&content[..5], b"hello");
    }
}
