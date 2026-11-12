//! Fixed-capacity write-back block cache.
//!
//! The cache is backend-agnostic. Callers must serialize access (normally
//! with SpinLock) and avoid holding unrelated locks across device I/O.

pub const BLOCK_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockError { OutOfRange, Io, Unsupported, CacheFull }

pub trait BlockDevice {
    fn capacity_blocks(&self) -> u64;
    fn read_block(&mut self, lba: u64, out: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError>;
    fn write_block(&mut self, lba: u64, data: &[u8; BLOCK_SIZE]) -> Result<(), BlockError>;
    /// Must issue a real durability barrier or return Unsupported.
    fn flush(&mut self) -> Result<(), BlockError>;
}

#[derive(Clone, Copy)]
struct CacheEntry {
    lba: u64,
    bytes: [u8; BLOCK_SIZE],
    age: u64,
    valid: bool,
    dirty: bool,
}
impl CacheEntry {
    const fn empty() -> Self {
        Self { lba: 0, bytes: [0; BLOCK_SIZE], age: 0, valid: false, dirty: false }
    }
}

/// Fixed-size LRU write-back cache. N must be greater than zero.
pub struct BlockCache<D, const N: usize> {
    device: D,
    entries: [CacheEntry; N],
    clock: u64,
}
impl<D: BlockDevice, const N: usize> BlockCache<D, N> {
    pub const fn new(device: D) -> Self {
        Self { device, entries: [const { CacheEntry::empty() }; N], clock: 1 }
    }
    pub fn device(&self) -> &D { &self.device }
    pub fn device_mut(&mut self) -> &mut D { &mut self.device }
    fn touch(&mut self, index: usize) {
        self.clock = self.clock.wrapping_add(1).max(1);
        self.entries[index].age = self.clock;
    }
    fn locate(&self, lba: u64) -> Option<usize> {
        self.entries.iter().position(|e| e.valid && e.lba == lba)
    }
    fn acquire(&mut self, lba: u64) -> Result<usize, BlockError> {
        if lba >= self.device.capacity_blocks() { return Err(BlockError::OutOfRange); }
        if let Some(index) = self.locate(lba) { self.touch(index); return Ok(index); }
        let index = self.entries.iter().position(|e| !e.valid)
            .or_else(|| self.entries.iter().enumerate().min_by_key(|(_, e)| e.age).map(|(i, _)| i))
            .ok_or(BlockError::CacheFull)?;
        if self.entries[index].valid && self.entries[index].dirty {
            self.device.write_block(self.entries[index].lba, &self.entries[index].bytes)?;
        }
        let mut bytes = [0u8; BLOCK_SIZE];
        self.device.read_block(lba, &mut bytes)?;
        self.entries[index] = CacheEntry { lba, bytes, age: 0, valid: true, dirty: false };
        self.touch(index);
        Ok(index)
    }
    pub fn read(&mut self, lba: u64, out: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        let index = self.acquire(lba)?;
        out.copy_from_slice(&self.entries[index].bytes);
        Ok(())
    }
    /// Update cache only. Durability requires sync.
    pub fn write(&mut self, lba: u64, data: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        let index = self.acquire(lba)?;
        self.entries[index].bytes.copy_from_slice(data);
        self.entries[index].dirty = true;
        self.touch(index);
        Ok(())
    }
    pub fn flush_block(&mut self, lba: u64) -> Result<(), BlockError> {
        let Some(index) = self.locate(lba) else { return Ok(()); };
        if self.entries[index].dirty {
            self.device.write_block(lba, &self.entries[index].bytes)?;
            self.entries[index].dirty = false;
        }
        Ok(())
    }
    /// Write all dirty blocks, then request a real device durability barrier.
    /// Dirty bits clear only after each corresponding write succeeds.
    pub fn sync(&mut self) -> Result<(), BlockError> {
        for index in 0..N {
            if self.entries[index].valid && self.entries[index].dirty {
                let lba = self.entries[index].lba;
                self.device.write_block(lba, &self.entries[index].bytes)?;
                self.entries[index].dirty = false;
            }
        }
        self.device.flush()
    }
    pub fn dirty_blocks(&self) -> usize {
        self.entries.iter().filter(|e| e.valid && e.dirty).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;
    struct MemoryDisk { blocks: Vec<[u8; BLOCK_SIZE]>, fail_write: bool, flushes: usize }
    impl BlockDevice for MemoryDisk {
        fn capacity_blocks(&self) -> u64 { self.blocks.len() as u64 }
        fn read_block(&mut self, lba: u64, out: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
            out.copy_from_slice(self.blocks.get(lba as usize).ok_or(BlockError::OutOfRange)?); Ok(())
        }
        fn write_block(&mut self, lba: u64, data: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
            if self.fail_write { return Err(BlockError::Io); }
            self.blocks.get_mut(lba as usize).ok_or(BlockError::OutOfRange)?.copy_from_slice(data); Ok(())
        }
        fn flush(&mut self) -> Result<(), BlockError> { self.flushes += 1; Ok(()) }
    }
    #[test]
    fn write_back_and_sync() {
        let disk = MemoryDisk { blocks: vec![[0; BLOCK_SIZE]; 4], fail_write: false, flushes: 0 };
        let mut cache = BlockCache::<_, 2>::new(disk);
        let data = [0xA5; BLOCK_SIZE];
        cache.write(1, &data).unwrap();
        let mut out = [0; BLOCK_SIZE];
        cache.read(1, &mut out).unwrap();
        assert_eq!(out, data);
        cache.sync().unwrap();
        assert_eq!(cache.dirty_blocks(), 0);
        assert_eq!(cache.device().blocks[1], data);
        assert_eq!(cache.device().flushes, 1);
    }
    #[test]
    fn failed_write_remains_dirty() {
        let disk = MemoryDisk { blocks: vec![[0; BLOCK_SIZE]; 2], fail_write: true, flushes: 0 };
        let mut cache = BlockCache::<_, 2>::new(disk);
        cache.write(0, &[7; BLOCK_SIZE]).unwrap();
        assert_eq!(cache.sync(), Err(BlockError::Io));
        assert_eq!(cache.dirty_blocks(), 1);
    }
    #[test]
    fn eviction_writes_back_lru_entry() {
        let disk = MemoryDisk { blocks: vec![[0; BLOCK_SIZE]; 3], fail_write: false, flushes: 0 };
        let mut cache = BlockCache::<_, 2>::new(disk);
        cache.write(0, &[1; BLOCK_SIZE]).unwrap();
        cache.write(1, &[2; BLOCK_SIZE]).unwrap();
        cache.read(0, &mut [0; BLOCK_SIZE]).unwrap();
        cache.write(2, &[3; BLOCK_SIZE]).unwrap();
        assert_eq!(cache.device().blocks[1], [2; BLOCK_SIZE]);
        assert_eq!(cache.dirty_blocks(), 2);
    }
    #[test]
    fn rejects_out_of_range_lba() {
        let disk = MemoryDisk { blocks: vec![[0; BLOCK_SIZE]; 1], fail_write: false, flushes: 0 };
        let mut cache = BlockCache::<_, 1>::new(disk);
        assert_eq!(cache.read(1, &mut [0; BLOCK_SIZE]), Err(BlockError::OutOfRange));
    }
}
