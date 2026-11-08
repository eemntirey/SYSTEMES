//! Simple read-only filesystem format for userland bring-up.
//!
//! On-disk layout (all multi-byte fields little-endian):
//!
//! Superblock (512 bytes, block 0):
//!   offset 0x00: magic   = "SYSFS1" (6 bytes)
//!   offset 0x06: version = 1 (u32)
//!   offset 0x0A: block_size = 512 (u32)
//!   offset 0x0E: inode_count (u32)
//!   offset 0x12: data_blocks (u32)
//!   offset 0x16: root_inode_index (u32)
//!   offset 0x1A..0x200: reserved (zero)
//! Inode table (block 1..1+inode_blocks):
//!   Each inode: 64 bytes
//!     offset 0x00: mode (u16) - bit 0 = directory
//!     offset 0x02: size (u32) - bytes for files, entry count for dirs
//!     offset 0x06: first_block (u32) - index of first data block
//!     offset 0x0A: block_count (u32)
//!     offset 0x0E..0x40: name (46 bytes, null-padded)
//!
//! Data blocks: block_size bytes each, starting after the inode table.

use crate::memory::PAGE_SIZE;

const SYSFS_MAGIC: &[u8; 6] = b"SYSFS1";
const SYSFS_VERSION: u32 = 1;
const SYSFS_BLOCK_SIZE: u32 = 512;
const SYSFS_INODE_SIZE: usize = 64;
const SYSFS_NAME_SIZE: usize = 46;

const INODE_FLAG_DIRECTORY: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimpleFsError {
    BadMagic,
    BadVersion,
    TruncatedImage,
    InodeNotFound,
    NotADirectory,
    NameTooLong,
    BlockOutOfRange,
    InvalidInode,
}

/// Parsed superblock from a SYSFS image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Superblock {
    pub block_size: u32,
    pub inode_count: u32,
    pub data_blocks: u32,
    pub root_inode_index: u32,
}

/// Parsed inode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Inode {
    pub mode: u16,
    pub size: u32,
    pub first_block: u32,
    pub block_count: u32,
    pub name: [u8; SYSFS_NAME_SIZE],
}

impl Inode {
    pub fn is_directory(&self) -> bool {
        self.mode & INODE_FLAG_DIRECTORY != 0
    }

    pub fn name_str(&self) -> Option<&str> {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(SYSFS_NAME_SIZE);
        core::str::from_utf8(&self.name[..len]).ok()
    }
}

/// A read-only SYSFS filesystem image in memory.
#[derive(Debug, PartialEq, Eq)]
pub struct SimpleFs {
    pub superblock: Superblock,
    data: &'static [u8],
    inode_table_offset: u32,
    data_region_offset: u32,
}
impl SimpleFs {
    /// Parse a SYSFS image from a byte slice.
    ///
    /// # Safety
    ///
    /// The slice must remain valid for the lifetime of the returned `SimpleFs`.
    pub unsafe fn parse(data: &'static [u8]) -> Result<Self, SimpleFsError> {
        if data.len() < SYSFS_BLOCK_SIZE as usize {
            return Err(SimpleFsError::TruncatedImage);
        }
        if &data[0..6] != SYSFS_MAGIC {
            return Err(SimpleFsError::BadMagic);
        }
        let version = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
        if version != SYSFS_VERSION {
            return Err(SimpleFsError::BadVersion);
        }
        let block_size = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
        if block_size == 0 {
            return Err(SimpleFsError::BadVersion);
        }
        let inode_count = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
        let data_blocks = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        let root_inode_index = u32::from_le_bytes([data[22], data[23], data[24], data[25]]);

        // Inode table starts at block 1.
        let inode_table_offset: u32 = block_size;
        let inode_table_size = inode_count.saturating_mul(SYSFS_INODE_SIZE as u32);
        let data_region_offset = inode_table_offset.saturating_add(inode_table_size);

        if data.len() < data_region_offset as usize + (data_blocks as usize) * (block_size as usize) {
            return Err(SimpleFsError::TruncatedImage);
        }

        Ok(SimpleFs {
            data,
            superblock: Superblock {
                block_size,
                inode_count,
                data_blocks,
                root_inode_index,
            },
            inode_table_offset,
            data_region_offset,
        })
    }

    /// Read one inode by index.
    pub fn inode(&self, index: u32) -> Result<Inode, SimpleFsError> {
        if index >= self.superblock.inode_count {
            return Err(SimpleFsError::InodeNotFound);
        }
        let offset = (self.inode_table_offset as u64
            + index as u64 * SYSFS_INODE_SIZE as u64) as usize;
        if offset + SYSFS_INODE_SIZE > self.data.len() {
            return Err(SimpleFsError::TruncatedImage);
        }
        let mode = u16::from_le_bytes([self.data[offset], self.data[offset + 1]]);
        let size = u32::from_le_bytes([
            self.data[offset + 2],
            self.data[offset + 3],
            self.data[offset + 4],
            self.data[offset + 5],
        ]);
        let first_block = u32::from_le_bytes([
            self.data[offset + 6],
            self.data[offset + 7],
            self.data[offset + 8],
            self.data[offset + 9],
        ]);
        let block_count = u32::from_le_bytes([
            self.data[offset + 10],
            self.data[offset + 11],
            self.data[offset + 12],
            self.data[offset + 13],
        ]);
        let mut name = [0u8; SYSFS_NAME_SIZE];
        name.copy_from_slice(&self.data[offset + 14..offset + 14 + SYSFS_NAME_SIZE]);

        Ok(Inode {
            mode,
            size,
            first_block,
            block_count,
            name,
        })
    }

    /// Look up an inode by name within a directory inode.
    ///
    /// This simple implementation scans all inodes (starting at index 1, since
    /// index 0 is reserved) for a matching name. A real directory would store
    /// its entries in data blocks; this is a boot-time simplification.
    pub fn find_in_dir(&self, dir: &Inode, name: &str) -> Result<u32, SimpleFsError> {
        if !dir.is_directory() {
            return Err(SimpleFsError::NotADirectory);
        }
        for i in 1..self.superblock.inode_count {
            let inode = self.inode(i)?;
            if let Some(iname) = inode.name_str() {
                if iname == name {
                    return Ok(i);
                }
            }
        }
        Err(SimpleFsError::InodeNotFound)
    }

    /// Read bytes from a file inode into `buf`. Returns bytes read.
    pub fn read_file(&self, inode: &Inode, buf: &mut [u8]) -> Result<usize, SimpleFsError> {
        if inode.is_directory() {
            return Err(SimpleFsError::NotADirectory);
        }
        let block_size = self.superblock.block_size as usize;
        let total = inode.size as usize;
        let to_read = buf.len().min(total);
        let mut read = 0;
        let mut block_idx = 0;
        while read < to_read && block_idx < inode.block_count as usize {
            let data_block_index = inode.first_block + block_idx as u32;
            let block_offset = (self.data_region_offset as u64
                + data_block_index as u64 * block_size as u64) as usize;
            let chunk = (to_read - read).min(block_size);
            if block_offset + chunk > self.data.len() {
                return Err(SimpleFsError::BlockOutOfRange);
            }
            buf[read..read + chunk]
                .copy_from_slice(&self.data[block_offset..block_offset + chunk]);
            read += chunk;
            block_idx += 1;
        }
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::vec::Vec;

    fn make_superblock() -> Vec<u8> {
        let mut sb = Vec::new();
        sb.resize(SYSFS_BLOCK_SIZE as usize, 0);
        sb[0..6].copy_from_slice(SYSFS_MAGIC);
        sb[6..10].copy_from_slice(&SYSFS_VERSION.to_le_bytes());
        sb[10..14].copy_from_slice(&SYSFS_BLOCK_SIZE.to_le_bytes());
        sb[14..18].copy_from_slice(&2u32.to_le_bytes()); // inode_count
        sb[18..22].copy_from_slice(&1u32.to_le_bytes()); // data_blocks
        sb[22..26].copy_from_slice(&0u32.to_le_bytes()); // root_inode_index
        sb
    }

    fn make_inode(mode: u16, size: u32, first_block: u32, block_count: u32, name: &str) -> Vec<u8> {
        let mut inode = Vec::new();
        inode.resize(SYSFS_INODE_SIZE, 0);
        inode[0..2].copy_from_slice(&mode.to_le_bytes());
        inode[2..6].copy_from_slice(&size.to_le_bytes());
        inode[6..10].copy_from_slice(&first_block.to_le_bytes());
        inode[10..14].copy_from_slice(&block_count.to_le_bytes());
        let name_bytes = name.as_bytes();
        inode[14..14 + name_bytes.len()].copy_from_slice(name_bytes);
        inode
    }

    #[test]
    fn parses_valid_sysfs_image() {
        let mut data = make_superblock();
        // Inode table at block 1 (offset 512), 2 inodes = 128 bytes
        data.resize(512 + 128, 0);
        data[512..512 + 64].copy_from_slice(&make_inode(
            INODE_FLAG_DIRECTORY, 2, 0, 0, "root",
        ));
        data[512 + 64..512 + 128].copy_from_slice(&make_inode(
            0, 5, 2, 1, "hello.txt",
        ));
        // Data block 2 at offset 512 + 128 + 2*512 = 1664
        data.resize(1664 + 512, 0);
        data[1664..1664 + 5].copy_from_slice(b"hello");

        let fs = unsafe {
            SimpleFs::parse(Box::leak(data.into_boxed_slice()))
        }.expect("valid image should parse");
        assert_eq!(fs.superblock.inode_count, 2);
        assert_eq!(fs.superblock.root_inode_index, 0);

        let root = fs.inode(0).expect("root inode");
        assert!(root.is_directory());

        let hello_idx = fs.find_in_dir(&root, "hello.txt").expect("hello.txt found");
        let hello = fs.inode(hello_idx).expect("hello inode");
        assert!(!hello.is_directory());
        assert_eq!(hello.size, 5);

        let mut buf = [0u8; 16];
        let n = fs.read_file(&hello, &mut buf).expect("read hello");
        assert_eq!(&buf[..n], b"hello");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = make_superblock();
        data[0] = 0;
        assert_eq!(unsafe {
            SimpleFs::parse(Box::leak(data.into_boxed_slice()))
        }, Err(SimpleFsError::BadMagic));
    }

    #[test]
    fn rejects_bad_version() {
        let mut data = make_superblock();
        data[6] = 2;
        assert_eq!(unsafe {
            SimpleFs::parse(Box::leak(data.into_boxed_slice()))
        }, Err(SimpleFsError::BadVersion));
    }

    #[test]
    fn find_in_dir_rejects_non_directory() {
        let mut data = make_superblock();
        data.resize(1152, 0);
        data[576..576 + 64].copy_from_slice(&make_inode(0, 5, 2, 1, "file.txt"));
        let fs = unsafe {
            SimpleFs::parse(Box::leak(data.into_boxed_slice()))
        }.expect("parse");
        let file = fs.inode(1).expect("file inode");
        assert_eq!(
            fs.find_in_dir(&file, "anything"),
            Err(SimpleFsError::NotADirectory)
        );
    }
}
