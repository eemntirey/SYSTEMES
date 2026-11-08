//! Read-only SYSFS image parser for a 512-byte-sector disk.
//!
//! The image contains a 512-byte superblock, a block-rounded inode table, and
//! `data_blocks` data blocks. Inodes contain a contiguous extent in that data
//! region; directories currently expose their child inodes by name.

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
    InvalidPath,
    NameTooLong,
    BlockOutOfRange,
    InvalidInode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Superblock {
    pub block_size: u32,
    pub inode_count: u32,
    pub data_blocks: u32,
    pub root_inode_index: u32,
}

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
        let len = self.name.iter().position(|&byte| byte == 0).unwrap_or(SYSFS_NAME_SIZE);
        core::str::from_utf8(&self.name[..len]).ok()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SimpleFs<'a> {
    pub superblock: Superblock,
    data: &'a [u8],
    inode_table_offset: usize,
    data_region_offset: usize,
}

impl<'a> SimpleFs<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, SimpleFsError> {
        let required_size = Self::required_image_size(data)?;
        if data.len() < required_size {
            return Err(SimpleFsError::TruncatedImage);
        }
        let inode_count = read_u32(data, 14).ok_or(SimpleFsError::TruncatedImage)?;
        let data_blocks = read_u32(data, 18).ok_or(SimpleFsError::TruncatedImage)?;
        let root_inode_index = read_u32(data, 22).ok_or(SimpleFsError::TruncatedImage)?;
        let inode_table_blocks = inode_table_blocks(inode_count)?;
        let data_region_offset = (SYSFS_BLOCK_SIZE as usize)
            .checked_add(
                inode_table_blocks
                    .checked_mul(SYSFS_BLOCK_SIZE as usize)
                    .ok_or(SimpleFsError::InvalidInode)?,
            )
            .ok_or(SimpleFsError::InvalidInode)?;

        let fs = Self {
            data,
            superblock: Superblock {
                block_size: SYSFS_BLOCK_SIZE,
                inode_count,
                data_blocks,
                root_inode_index,
            },
            inode_table_offset: SYSFS_BLOCK_SIZE as usize,
            data_region_offset,
        };
        if !fs.inode(root_inode_index)?.is_directory() {
            return Err(SimpleFsError::InvalidInode);
        }
        Ok(fs)
    }

    pub fn required_image_size(header: &[u8]) -> Result<usize, SimpleFsError> {
        if header.len() < SYSFS_BLOCK_SIZE as usize {
            return Err(SimpleFsError::TruncatedImage);
        }
        if header.get(0..SYSFS_MAGIC.len()) != Some(SYSFS_MAGIC.as_slice()) {
            return Err(SimpleFsError::BadMagic);
        }
        if read_u32(header, 6) != Some(SYSFS_VERSION)
            || read_u32(header, 10) != Some(SYSFS_BLOCK_SIZE)
        {
            return Err(SimpleFsError::BadVersion);
        }
        let inode_count = read_u32(header, 14).ok_or(SimpleFsError::TruncatedImage)?;
        let data_blocks = read_u32(header, 18).ok_or(SimpleFsError::TruncatedImage)?;
        let root_inode_index = read_u32(header, 22).ok_or(SimpleFsError::TruncatedImage)?;
        if inode_count == 0 || root_inode_index >= inode_count {
            return Err(SimpleFsError::InvalidInode);
        }
        let data_offset = (SYSFS_BLOCK_SIZE as usize)
            .checked_add(
                inode_table_blocks(inode_count)?
                    .checked_mul(SYSFS_BLOCK_SIZE as usize)
                    .ok_or(SimpleFsError::InvalidInode)?,
            )
            .ok_or(SimpleFsError::InvalidInode)?;
        let data_size = usize::try_from(data_blocks)
            .ok()
            .and_then(|blocks| blocks.checked_mul(SYSFS_BLOCK_SIZE as usize))
            .ok_or(SimpleFsError::InvalidInode)?;
        data_offset
            .checked_add(data_size)
            .ok_or(SimpleFsError::InvalidInode)
    }

    pub fn inode(&self, index: u32) -> Result<Inode, SimpleFsError> {
        if index >= self.superblock.inode_count {
            return Err(SimpleFsError::InodeNotFound);
        }
        let offset = usize::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(SYSFS_INODE_SIZE))
            .and_then(|index| self.inode_table_offset.checked_add(index))
            .ok_or(SimpleFsError::InvalidInode)?;
        let record_end = offset
            .checked_add(SYSFS_INODE_SIZE)
            .ok_or(SimpleFsError::InvalidInode)?;
        let record = self.data.get(offset..record_end).ok_or(SimpleFsError::TruncatedImage)?;

        let mode = read_u16(record, 0).ok_or(SimpleFsError::InvalidInode)?;
        let size = read_u32(record, 2).ok_or(SimpleFsError::InvalidInode)?;
        let first_block = read_u32(record, 6).ok_or(SimpleFsError::InvalidInode)?;
        let block_count = read_u32(record, 10).ok_or(SimpleFsError::InvalidInode)?;
        if mode & !INODE_FLAG_DIRECTORY != 0 {
            return Err(SimpleFsError::InvalidInode);
        }
        let name: [u8; SYSFS_NAME_SIZE] = record[14..14 + SYSFS_NAME_SIZE]
            .try_into()
            .map_err(|_| SimpleFsError::InvalidInode)?;
        let inode = Inode {
            mode,
            size,
            first_block,
            block_count,
            name,
        };
        self.validate_inode_extent(&inode)?;
        Ok(inode)
    }

    pub fn find_in_dir(&self, dir: &Inode, name: &str) -> Result<u32, SimpleFsError> {
        if !dir.is_directory() {
            return Err(SimpleFsError::NotADirectory);
        }
        if name.is_empty() || name.len() > SYSFS_NAME_SIZE {
            return Err(if name.len() > SYSFS_NAME_SIZE {
                SimpleFsError::NameTooLong
            } else {
                SimpleFsError::InvalidPath
            });
        }
        if name.contains('/') || name == "." || name == ".." || name.as_bytes().contains(&0) {
            return Err(SimpleFsError::InvalidPath);
        }

        for index in 0..self.superblock.inode_count {
            if index == self.superblock.root_inode_index {
                continue;
            }
            let inode = self.inode(index)?;
            if inode.name_str() == Some(name) {
                return Ok(index);
            }
        }
        Err(SimpleFsError::InodeNotFound)
    }

    pub fn read_file(&self, inode: &Inode, buf: &mut [u8]) -> Result<usize, SimpleFsError> {
        self.read_file_at(inode, 0, buf)
    }

    pub fn read_file_at(
        &self,
        inode: &Inode,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize, SimpleFsError> {
        if inode.is_directory() {
            return Err(SimpleFsError::NotADirectory);
        }
        self.validate_inode_extent(inode)?;

        let file_size = inode.size as usize;
        let start = offset.min(file_size);
        let mut copied = 0;
        while copied < buf.len() && start + copied < file_size {
            let file_offset = start + copied;
            let block_index = file_offset / SYSFS_BLOCK_SIZE as usize;
            if block_index >= inode.block_count as usize {
                return Err(SimpleFsError::BlockOutOfRange);
            }
            let data_block = (inode.first_block as usize)
                .checked_add(block_index)
                .ok_or(SimpleFsError::BlockOutOfRange)?;
            let within_block = file_offset % SYSFS_BLOCK_SIZE as usize;
            let block_offset = self
                .data_region_offset
                .checked_add(
                    data_block
                        .checked_mul(SYSFS_BLOCK_SIZE as usize)
                        .ok_or(SimpleFsError::BlockOutOfRange)?,
                )
                .and_then(|offset| offset.checked_add(within_block))
                .ok_or(SimpleFsError::BlockOutOfRange)?;
            let count = (buf.len() - copied)
                .min(SYSFS_BLOCK_SIZE as usize - within_block)
                .min(file_size - file_offset);
            let end = block_offset
                .checked_add(count)
                .ok_or(SimpleFsError::BlockOutOfRange)?;
            let source = self
                .data
                .get(block_offset..end)
                .ok_or(SimpleFsError::BlockOutOfRange)?;
            buf[copied..copied + count].copy_from_slice(source);
            copied += count;
        }
        Ok(copied)
    }

    fn validate_inode_extent(&self, inode: &Inode) -> Result<(), SimpleFsError> {
        let extent_end = inode
            .first_block
            .checked_add(inode.block_count)
            .ok_or(SimpleFsError::BlockOutOfRange)?;
        if extent_end > self.superblock.data_blocks {
            return Err(SimpleFsError::BlockOutOfRange);
        }
        if !inode.is_directory() {
            let needed_blocks = inode
                .size
                .checked_add(SYSFS_BLOCK_SIZE - 1)
                .ok_or(SimpleFsError::InvalidInode)?
                / SYSFS_BLOCK_SIZE;
            if needed_blocks > inode.block_count {
                return Err(SimpleFsError::InvalidInode);
            }
        }
        Ok(())
    }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn inode_table_blocks(inode_count: u32) -> Result<usize, SimpleFsError> {
    let inode_table_size = usize::try_from(inode_count)
        .ok()
        .and_then(|count| count.checked_mul(SYSFS_INODE_SIZE))
        .ok_or(SimpleFsError::InvalidInode)?;
    inode_table_size
        .checked_add(SYSFS_BLOCK_SIZE as usize - 1)
        .map(|size| size / SYSFS_BLOCK_SIZE as usize)
        .ok_or(SimpleFsError::InvalidInode)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn make_image(data_blocks: u32) -> Vec<u8> {
        let mut data = vec![0; 512 * (2 + data_blocks as usize)];
        data[0..6].copy_from_slice(SYSFS_MAGIC);
        data[6..10].copy_from_slice(&SYSFS_VERSION.to_le_bytes());
        data[10..14].copy_from_slice(&SYSFS_BLOCK_SIZE.to_le_bytes());
        data[14..18].copy_from_slice(&2u32.to_le_bytes());
        data[18..22].copy_from_slice(&data_blocks.to_le_bytes());
        data[22..26].copy_from_slice(&0u32.to_le_bytes());

        data[512..514].copy_from_slice(&INODE_FLAG_DIRECTORY.to_le_bytes());
        data[578..582].copy_from_slice(&5u32.to_le_bytes());
        data[582..586].copy_from_slice(&0u32.to_le_bytes());
        data[586..590].copy_from_slice(&1u32.to_le_bytes());
        data[590..599].copy_from_slice(b"hello.txt");
        if data_blocks != 0 {
            data[1024..1029].copy_from_slice(b"hello");
        }
        data
    }

    #[test]
    fn parses_and_reads_a_valid_image() {
        let data = make_image(1);
        assert_eq!(SimpleFs::required_image_size(&data[..512]), Ok(data.len()));
        let fs = SimpleFs::parse(&data).expect("valid image");
        let root = fs.inode(0).expect("root inode");
        assert!(root.is_directory());
        let file_index = fs.find_in_dir(&root, "hello.txt").expect("file inode");
        let file = fs.inode(file_index).expect("file inode");
        let mut content = [0; 8];
        assert_eq!(fs.read_file(&file, &mut content), Ok(5));
        assert_eq!(&content[..5], b"hello");
    }

    #[test]
    fn validates_version_block_size_and_truncation() {
        let mut data = make_image(1);
        data[6] = 2;
        assert_eq!(SimpleFs::parse(&data), Err(SimpleFsError::BadVersion));

        let mut data = make_image(1);
        data[11] = 0;
        assert_eq!(SimpleFs::parse(&data), Err(SimpleFsError::BadVersion));

        let mut data = make_image(1);
        data.truncate(1400);
        assert_eq!(SimpleFs::parse(&data), Err(SimpleFsError::TruncatedImage));
        assert_eq!(SimpleFs::required_image_size(&data), Ok(1536));
    }

    #[test]
    fn rejects_invalid_root_and_file_extents() {
        let mut data = make_image(1);
        data[22..26].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(SimpleFs::parse(&data), Err(SimpleFsError::InvalidInode));

        let mut data = make_image(1);
        data[582..586].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            SimpleFs::parse(&data).unwrap().inode(1),
            Err(SimpleFsError::BlockOutOfRange)
        );
    }

    #[test]
    fn rejects_path_components_and_reads_from_offsets() {
        let data = make_image(1);
        let fs = SimpleFs::parse(&data).unwrap();
        let root = fs.inode(0).unwrap();
        assert_eq!(fs.find_in_dir(&root, "../hello.txt"), Err(SimpleFsError::InvalidPath));
        assert_eq!(fs.find_in_dir(&root, "x".repeat(47).as_str()), Err(SimpleFsError::NameTooLong));

        let file = fs.inode(1).unwrap();
        let mut content = [0; 8];
        assert_eq!(fs.read_file_at(&file, 2, &mut content), Ok(3));
        assert_eq!(&content[..3], b"llo");
        assert_eq!(fs.read_file_at(&file, 5, &mut content), Ok(0));
    }

    #[test]
    fn borrows_the_input_image() {
        let data = make_image(1);
        let fs = SimpleFs::parse(&data).unwrap();
        assert_eq!(fs.superblock.inode_count, 2);
    }
}
