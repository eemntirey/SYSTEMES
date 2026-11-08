//! Per-process descriptor bookkeeping for the single active Ring 3 process.
//!
//! The kernel currently runs one user process at a time. This fixed-size table
//! is owned by that process and maps its public descriptors (3 and above) to
//! VFS handles. It performs no allocation and is host-testable.

use crate::vfs::FileHandle;

pub const MAX_PROCESS_FILES: usize = 16;
pub const FIRST_FILE_DESCRIPTOR: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileTableError {
    BadDescriptor,
    TooManyOpenFiles,
}

#[derive(Clone, Copy)]
struct Entry {
    descriptor: u32,
    handle: FileHandle,
}

pub struct ProcessFileTable {
    entries: [Option<Entry>; MAX_PROCESS_FILES],
    next_descriptor: u32,
}

impl ProcessFileTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_PROCESS_FILES],
            next_descriptor: FIRST_FILE_DESCRIPTOR,
        }
    }

    pub fn insert(&mut self, handle: FileHandle) -> Result<u32, FileTableError> {
        let slot = self.entries.iter().position(Option::is_none)
            .ok_or(FileTableError::TooManyOpenFiles)?;
        for _ in 0..=MAX_PROCESS_FILES {
            let descriptor = self.next_descriptor;
            self.next_descriptor = self.next_descriptor.wrapping_add(1);
            if self.next_descriptor < FIRST_FILE_DESCRIPTOR {
                self.next_descriptor = FIRST_FILE_DESCRIPTOR;
            }
            if descriptor >= FIRST_FILE_DESCRIPTOR
                && !self.entries.iter().flatten().any(|entry| entry.descriptor == descriptor)
            {
                self.entries[slot] = Some(Entry { descriptor, handle });
                return Ok(descriptor);
            }
        }
        Err(FileTableError::TooManyOpenFiles)
    }

    pub fn get(&self, descriptor: u32) -> Result<FileHandle, FileTableError> {
        self.entries.iter().flatten()
            .find(|entry| entry.descriptor == descriptor)
            .map(|entry| entry.handle)
            .ok_or(FileTableError::BadDescriptor)
    }

    pub fn remove(&mut self, descriptor: u32) -> Result<FileHandle, FileTableError> {
        let slot = self.entries.iter().position(
            |entry| entry.is_some_and(|entry| entry.descriptor == descriptor)
        ).ok_or(FileTableError::BadDescriptor)?;
        self.entries[slot].take()
            .map(|entry| entry.handle)
            .ok_or(FileTableError::BadDescriptor)
    }

    /// Detach every descriptor when the owning process exits.
    pub fn take_all(&mut self) -> [Option<FileHandle>; MAX_PROCESS_FILES] {
        let mut handles = [None; MAX_PROCESS_FILES];
        for (index, entry) in self.entries.iter_mut().enumerate() {
            handles[index] = entry.take().map(|entry| entry.handle);
        }
        handles
    }

    pub fn reset(&mut self) {
        self.entries = [None; MAX_PROCESS_FILES];
        self.next_descriptor = FIRST_FILE_DESCRIPTOR;
    }

    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ProcessFileTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{simple_fs::SimpleFs, vfs::Vfs};
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn descriptors_start_at_three_and_round_trip_handles() {
        let mut table = ProcessFileTable::new();
        let first = table.insert(FileHandle(41)).unwrap();
        let second = table.insert(FileHandle(42)).unwrap();
        assert_eq!(first, 3);
        assert_eq!(second, 4);
        assert_eq!(table.get(first), Ok(FileHandle(41)));
        assert_eq!(table.get(second), Ok(FileHandle(42)));
    }

    #[test]
    fn close_rejects_invalid_and_double_closed_descriptors() {
        let mut table = ProcessFileTable::new();
        assert_eq!(table.remove(0), Err(FileTableError::BadDescriptor));
        assert_eq!(table.remove(2), Err(FileTableError::BadDescriptor));
        let fd = table.insert(FileHandle(7)).unwrap();
        assert_eq!(table.remove(fd), Ok(FileHandle(7)));
        assert_eq!(table.remove(fd), Err(FileTableError::BadDescriptor));
    }

    #[test]
    fn process_table_enforces_sixteen_open_files() {
        let mut table = ProcessFileTable::new();
        for handle in 1..=16 {
            assert!(table.insert(FileHandle(handle)).is_ok());
        }
        assert_eq!(
            table.insert(FileHandle(17)),
            Err(FileTableError::TooManyOpenFiles)
        );
        assert_eq!(table.len(), 16);
    }

    #[test]
    fn process_exit_drains_every_descriptor_without_leaking_slots() {
        let mut table = ProcessFileTable::new();
        for handle in 1..=16 {
            table.insert(FileHandle(handle)).unwrap();
        }
        let closed = table.take_all();
        assert_eq!(closed.iter().flatten().count(), 16);
        assert_eq!(table.len(), 0);
        assert_eq!(table.take_all().iter().flatten().count(), 0);
    }

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

    #[test]
    fn process_exit_closes_all_vfs_handles_and_releases_capacity() {
        let image = make_test_image();
        let filesystem = SimpleFs::parse(&image).unwrap();
        let mut vfs = Vfs::new();
        vfs.mount(filesystem).unwrap();
        let mut table = ProcessFileTable::new();

        for _ in 0..MAX_PROCESS_FILES {
            let handle = vfs.open("/hello.txt").unwrap();
            table.insert(handle).unwrap();
        }
        let handles = table.take_all();
        for handle in handles.into_iter().flatten() {
            vfs.close(handle).unwrap();
        }
        assert!(table.is_empty());

        for _ in 0..MAX_PROCESS_FILES {
            assert!(vfs.open("/hello.txt").is_ok());
        }
    }

    #[test]
    fn closing_one_descriptor_does_not_alias_another() {
        let mut table = ProcessFileTable::new();
        let first = table.insert(FileHandle(11)).unwrap();
        let second = table.insert(FileHandle(12)).unwrap();
        assert_eq!(table.remove(first), Ok(FileHandle(11)));
        assert_eq!(table.get(second), Ok(FileHandle(12)));
    }
}
