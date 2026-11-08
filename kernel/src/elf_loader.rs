//! ELF64 static program loader for the first userland process (`init`).
//!
//! Parses a statically linked ELF64 executable, validates its program headers,
//! and copies each `PT_LOAD` segment into the caller-provided page frames.
//! No file system or disk access is involved here: the ELF image is supplied
//! as an in-memory byte slice (embedded in the kernel or passed by the loader).

use crate::address_space::{AddressSpace, UserPagePermissions, USER_BASE, USER_LIMIT};
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};

const ELF_MAGIC: &[u8; 4] = b"\x7fELF";
const ELFCLASS64: u8 = 2;
const EM_X86_64: u16 = 0x3E;
const ET_EXEC: u16 = 2;
const EV_CURRENT: u32 = 1;
const PT_LOAD: u32 = 1;
const PF_R: u32 = 0x4;
const PF_W: u32 = 0x2;
const PF_X: u32 = 0x1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    BadClass,
    BadEndianness,
    BadVersion,
    UnsupportedMachine,
    UnsupportedType,
    BadHeaderSize,
    ProgramHeaderOffsetOverflow,
    ProgramHeaderCountTooLarge,
    ProgramHeaderSizeMismatch,
    ProgramHeaderReadOverflow,
    InvalidAlignment,
    SegmentOutsideUserWindow,
    SegmentUnaligned,
    OverlappingSegments,
    InvalidFlags,
    SegmentFileszLargerThanMemsz,
    SegmentOutsideImage,
    EntryOutsideLoadedSegments,
    OutOfFrames,
    InvalidFrameAlignment,
    InvalidFrameAddress,
    WritableExecutableSegment,
}

impl From<crate::address_space::AddressSpaceError> for ElfError {
    fn from(_: crate::address_space::AddressSpaceError) -> Self {
        ElfError::InvalidFrameAddress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElfSegment {
    pub vaddr: u64,
    pub memsz: u64,
    pub filesz: u64,
    pub offset: u64,
    pub flags: u32,
}

/// Parsed ELF64 executable ready to be loaded into an address space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElfImage {
    pub entry: u64,
    pub segments: [ElfSegment; 8],
    pub segment_count: usize,
}

impl ElfImage {
    /// Parse an ELF64 image from a byte slice.
    ///
    /// Returns the entry point and a fixed-size array of loadable segments.
    /// Only `PT_LOAD` segments are collected; up to 8 are supported.
    pub fn parse(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<Elf64Ehdr>() {
            return Err(ElfError::TooSmall);
        }

        if data.get(0..4) != Some(ELF_MAGIC.as_slice()) {
            return Err(ElfError::BadMagic);
        }
        if data[4] != ELFCLASS64 {
            return Err(ElfError::BadClass);
        }
        if data[5] != 1 {
            return Err(ElfError::BadEndianness);
        }
        if data[6] != EV_CURRENT as u8 || read_u32(data, 20) != Some(EV_CURRENT) {
            return Err(ElfError::BadVersion);
        }
        if read_u16(data, 18) != Some(EM_X86_64) {
            return Err(ElfError::UnsupportedMachine);
        }
        if read_u16(data, 16) != Some(ET_EXEC) {
            return Err(ElfError::UnsupportedType);
        }
        if read_u16(data, 52) != Some(core::mem::size_of::<Elf64Ehdr>() as u16) {
            return Err(ElfError::BadHeaderSize);
        }
        let phoff = usize::try_from(read_u64(data, 32).ok_or(ElfError::ProgramHeaderOffsetOverflow)?)
            .map_err(|_| ElfError::ProgramHeaderOffsetOverflow)?;
        let phnum = usize::from(read_u16(data, 56).ok_or(ElfError::ProgramHeaderOffsetOverflow)?);
        if phoff < core::mem::size_of::<Elf64Ehdr>() || phnum == 0 {
            return Err(ElfError::ProgramHeaderOffsetOverflow);
        }
        let phentsize = usize::from(
            read_u16(data, 54).ok_or(ElfError::ProgramHeaderSizeMismatch)?,
        );
        if phentsize != core::mem::size_of::<Elf64Phdr>() {
            return Err(ElfError::ProgramHeaderSizeMismatch);
        }
        if phnum > 64 {
            return Err(ElfError::ProgramHeaderCountTooLarge);
        }

        let table_size = phentsize
            .checked_mul(phnum)
            .ok_or(ElfError::ProgramHeaderOffsetOverflow)?;
        let phdr_table_end = phoff
            .checked_add(table_size)
            .ok_or(ElfError::ProgramHeaderOffsetOverflow)?;
        if phdr_table_end > data.len() {
            return Err(ElfError::ProgramHeaderReadOverflow);
        }

        let mut segments = [ElfSegment {
            vaddr: 0,
            memsz: 0,
            filesz: 0,
            offset: 0,
            flags: 0,
        }; 8];
        let mut segment_count = 0usize;
        let entry = read_u64(data, 24).ok_or(ElfError::TooSmall)?;
        let mut entry_in_executable_segment = false;

        for i in 0..phnum {
            let offset = phoff + i * phentsize;
            if read_u32(data, offset) != Some(PT_LOAD) {
                continue;
            }
            if segment_count >= segments.len() {
                return Err(ElfError::ProgramHeaderCountTooLarge);
            }

            let flags = read_u32(data, offset + 4).ok_or(ElfError::ProgramHeaderReadOverflow)?;
            let file_offset = read_u64(data, offset + 8).ok_or(ElfError::ProgramHeaderReadOverflow)?;
            let virtual_address = read_u64(data, offset + 16).ok_or(ElfError::ProgramHeaderReadOverflow)?;
            let file_size = read_u64(data, offset + 32).ok_or(ElfError::ProgramHeaderReadOverflow)?;
            let memory_size = read_u64(data, offset + 40).ok_or(ElfError::ProgramHeaderReadOverflow)?;
            let alignment = read_u64(data, offset + 48).ok_or(ElfError::ProgramHeaderReadOverflow)?;

            if file_size > memory_size {
                return Err(ElfError::SegmentFileszLargerThanMemsz);
            }
            if flags & !(PF_R | PF_W | PF_X) != 0 {
                return Err(ElfError::InvalidFlags);
            }
            if (flags & PF_W) != 0 && (flags & PF_X) != 0 {
                return Err(ElfError::WritableExecutableSegment);
            }
            if alignment > 1
                && (alignment & (alignment - 1) != 0
                    || file_offset % alignment != virtual_address % alignment)
            {
                return Err(ElfError::InvalidAlignment);
            }
            if virtual_address % PAGE_SIZE != 0 {
                return Err(ElfError::SegmentUnaligned);
            }
            if memory_size == 0 {
                continue;
            }
            let segment_end = virtual_address
                .checked_add(memory_size)
                .ok_or(ElfError::SegmentOutsideUserWindow)?;
            if virtual_address < USER_BASE || segment_end > USER_LIMIT {
                return Err(ElfError::SegmentOutsideUserWindow);
            }
            let rounded_end = segment_end
                .checked_add(PAGE_SIZE - 1)
                .ok_or(ElfError::SegmentOutsideUserWindow)?
                & !(PAGE_SIZE - 1);
            if segments[..segment_count].iter().any(|segment| {
                let old_end = segment.vaddr + segment.memsz;
                let old_rounded_end = (old_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
                virtual_address < old_rounded_end && segment.vaddr < rounded_end
            }) {
                return Err(ElfError::OverlappingSegments);
            }

            let file_data_offset =
                usize::try_from(file_offset).map_err(|_| ElfError::SegmentOutsideImage)?;
            let filesz = usize::try_from(file_size).map_err(|_| ElfError::SegmentOutsideImage)?;
            let file_end = file_data_offset
                .checked_add(filesz)
                .ok_or(ElfError::SegmentOutsideImage)?;
            if file_end > data.len() {
                return Err(ElfError::SegmentOutsideImage);
            }

            segments[segment_count] = ElfSegment {
                vaddr: virtual_address,
                memsz: memory_size,
                filesz: file_size,
                offset: file_offset,
                flags,
            };
            segment_count += 1;
            if entry >= virtual_address && entry < segment_end && flags & PF_X != 0 {
                entry_in_executable_segment = true;
            }
        }

        if segment_count == 0 {
            return Err(ElfError::ProgramHeaderOffsetOverflow);
        }

        if !entry_in_executable_segment {
            return Err(ElfError::EntryOutsideLoadedSegments);
        }

        Ok(ElfImage {
            entry,
            segments,
            segment_count,
        })
    }

    /// Load every segment into `space`, allocating frames through `allocator`.
    ///
    /// Each segment is mapped at its virtual address with permissions derived
    /// from the ELF flags (`PF_R`/`PF_W`/`PF_X`). Writable-executable segments
    /// are rejected (`W^X`).
    ///
    /// # Safety
    ///
    /// `space` must be a valid address space whose PML4 tables are identity
    /// accessible through the active kernel mapping.
    pub unsafe fn load_into(
        &self,
        data: &[u8],
        allocator: &mut PhysicalFrameAllocator,
        space: &mut AddressSpace,
    ) -> Result<u64, ElfError> {
        for i in 0..self.segment_count {
            let seg = self.segments[i];

            let permissions = if (seg.flags & PF_X) != 0 {
                UserPagePermissions::ReadOnlyExecute
            } else if (seg.flags & PF_W) != 0 {
                UserPagePermissions::ReadWriteNoExecute
            } else {
                UserPagePermissions::ReadOnlyNoExecute
            };

            let mut offset = 0u64;
            while offset < seg.memsz {
                let vaddr = seg.vaddr.checked_add(offset).ok_or(ElfError::SegmentOutsideUserWindow)?;
                let frame = allocator
                    .allocate_frame()
                    .ok_or(ElfError::OutOfFrames)?;
                if frame % PAGE_SIZE != 0
                    || frame
                        .checked_add(PAGE_SIZE)
                        .is_none_or(|end| end > crate::paging::IDENTITY_MAP_BYTES)
                {
                    return Err(ElfError::InvalidFrameAddress);
                }

                unsafe {
                    core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize);
                }
                if offset < seg.filesz {
                    let chunk = (seg.filesz - offset).min(PAGE_SIZE as u64) as usize;
                    let src = unsafe { data.as_ptr().add(seg.offset as usize + offset as usize) };
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, frame as *mut u8, chunk);
                    }
                }

                unsafe {
                    space.map_user_page(allocator, vaddr, frame, permissions)?;
                }

                offset += PAGE_SIZE as u64;
            }
        }

        Ok(self.entry)
    }
}

/// Load an ELF64 image from `data` into `space`, returning the entry point.
///
/// # Safety
///
/// Same invariants as [`ElfImage::load_into`].
pub unsafe fn load_elf(
    data: &[u8],
    allocator: &mut PhysicalFrameAllocator,
    space: &mut AddressSpace,
) -> Result<u64, ElfError> {
    let image = ElfImage::parse(data)?;
    unsafe { image.load_into(data, allocator, space) }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use crate::address_space::USER_BASE;
    use std::vec::Vec;

    use super::*;

    /// Build a minimal ELF64 image in memory with one PT_LOAD segment.
    fn make_test_elf(entry: u64, vaddr: u64, payload: &[u8]) -> Vec<u8> {
        let page = PAGE_SIZE as u64;
        let filesz = payload.len() as u64;
        let memsz = (filesz + page - 1) / page * page;
        let phdr_count = 1u64;
        let phdr_table_size = core::mem::size_of::<Elf64Phdr>() as u64 * phdr_count;
        let ehdr_size = core::mem::size_of::<Elf64Ehdr>() as u64;
        let phoff = ehdr_size;
        let file_offset = (phoff + phdr_table_size + page - 1) & !(page - 1);
        let total = file_offset + filesz;

        let mut data = Vec::new();
        data.resize(total as usize, 0);

        let ehdr = Elf64Ehdr {
            e_ident: {
                let mut ident = [0u8; 16];
                ident[0..4].copy_from_slice(ELF_MAGIC);
                ident[4] = ELFCLASS64;
                ident[5] = 1; // little-endian
                ident[6] = EV_CURRENT as u8;
                ident
            },
            e_type: ET_EXEC,
            e_machine: EM_X86_64,
            e_version: EV_CURRENT,
            e_entry: entry,
            e_phoff: phoff,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: core::mem::size_of::<Elf64Ehdr>() as u16,
            e_phentsize: core::mem::size_of::<Elf64Phdr>() as u16,
            e_phnum: phdr_count as u16,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                &ehdr as *const Elf64Ehdr as *const u8,
                data.as_mut_ptr(),
                core::mem::size_of::<Elf64Ehdr>(),
            );
        }

        let phdr = Elf64Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: file_offset,
            p_vaddr: vaddr,
            p_paddr: vaddr,
            p_filesz: filesz,
            p_memsz: memsz,
            p_align: page,
        };
        unsafe {
            let phdr_ptr = data.as_mut_ptr().add(phoff as usize) as *mut Elf64Phdr;
            phdr_ptr.write(phdr);
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                payload.as_ptr(),
                data.as_mut_ptr().add(file_offset as usize),
                payload.len(),
            );
        }

        data
    }

    #[test]
    fn parses_valid_elf64_exec() {
        let payload = [0x48u8, 0xB8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &payload);
        let image = ElfImage::parse(&data).expect("valid ELF should parse");
        assert_eq!(image.entry, USER_BASE + 0x4000);
        assert_eq!(image.segment_count, 1);
        assert_eq!(image.segments[0].vaddr, USER_BASE + 0x4000);
        assert_eq!(image.segments[0].filesz, payload.len() as u64);
        assert!(image.segments[0].memsz >= image.segments[0].filesz);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0u8; 16]);
        data[0] = 0;
        assert_eq!(ElfImage::parse(&data), Err(ElfError::BadMagic));
    }

    #[test]
    fn rejects_unsupported_machine() {
        let payload = [0u8; 16];
        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &payload);
        let machine_ptr = unsafe { data.as_mut_ptr().add(18) } as *mut u16;
        unsafe { machine_ptr.write(0x00); }
        assert_eq!(ElfImage::parse(&data), Err(ElfError::UnsupportedMachine));
    }

    #[test]
    fn rejects_non_user_address() {
        let payload = [0u8; 16];
        let data = make_test_elf(0x1000, 0x1000, &payload);
        assert_eq!(ElfImage::parse(&data), Err(ElfError::SegmentOutsideUserWindow));
    }

    #[test]
    fn rejects_filesz_larger_than_memsz() {
        let page = PAGE_SIZE as u64;
        let phdr_table_size = core::mem::size_of::<Elf64Phdr>() as u64;
        let ehdr_size = core::mem::size_of::<Elf64Ehdr>() as u64;
        let phoff = ehdr_size;
        let file_offset = (phoff + phdr_table_size + page - 1) & !(page - 1);
        let filesz = 2 * page;
        let memsz = page;
        let total = file_offset + filesz;
        let mut data = Vec::new();
        data.resize(total as usize, 0);

        let ehdr = Elf64Ehdr {
            e_ident: {
                let mut ident = [0u8; 16];
                ident[0..4].copy_from_slice(ELF_MAGIC);
                ident[4] = ELFCLASS64;
                ident[5] = 1;
                ident[6] = EV_CURRENT as u8;
                ident
            },
            e_type: ET_EXEC,
            e_machine: EM_X86_64,
            e_version: EV_CURRENT,
            e_entry: USER_BASE + 0x4000,
            e_phoff: phoff,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: core::mem::size_of::<Elf64Ehdr>() as u16,
            e_phentsize: core::mem::size_of::<Elf64Phdr>() as u16,
            e_phnum: 1,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                &ehdr as *const Elf64Ehdr as *const u8,
                data.as_mut_ptr(),
                core::mem::size_of::<Elf64Ehdr>(),
            );
        }

        let phdr = Elf64Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: file_offset,
            p_vaddr: USER_BASE + 0x4000,
            p_paddr: USER_BASE + 0x4000,
            p_filesz: filesz,
            p_memsz: memsz,
            p_align: page,
        };
        unsafe {
            let phdr_ptr = data.as_mut_ptr().add(phoff as usize) as *mut Elf64Phdr;
            phdr_ptr.write(phdr);
        }

        assert_eq!(ElfImage::parse(&data), Err(ElfError::SegmentFileszLargerThanMemsz));
    }

    #[test]
    fn rejects_entry_outside_segments() {
        let payload = [0u8; 16];
        let data = make_test_elf(USER_BASE + 0x8000, USER_BASE + 0x4000, &payload);
        assert_eq!(ElfImage::parse(&data), Err(ElfError::EntryOutsideLoadedSegments));
    }

    #[test]
    fn requires_entry_inside_executable_segment() {
        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0; 16]);
        data[68..72].copy_from_slice(&(PF_R | PF_W).to_le_bytes());
        assert_eq!(ElfImage::parse(&data), Err(ElfError::EntryOutsideLoadedSegments));
    }

    #[test]
    fn rejects_truncated_and_misaligned_program_headers() {
        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0; 16]);
        data.truncate(100);
        assert_eq!(ElfImage::parse(&data), Err(ElfError::ProgramHeaderReadOverflow));

        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0; 16]);
        data[112..120].copy_from_slice(&3u64.to_le_bytes());
        assert_eq!(ElfImage::parse(&data), Err(ElfError::InvalidAlignment));
    }

    #[test]
    fn rejects_segments_that_share_a_mapped_page() {
        let mut data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0; 16]);
        data[56..58].copy_from_slice(&2u16.to_le_bytes());
        data.copy_within(64..120, 120);
        assert_eq!(ElfImage::parse(&data), Err(ElfError::OverlappingSegments));
    }

    #[test]
    fn parses_an_unaligned_input_slice_without_unaligned_references() {
        let data = make_test_elf(USER_BASE + 0x4000, USER_BASE + 0x4000, &[0; 16]);
        let mut unaligned = Vec::with_capacity(data.len() + 1);
        unaligned.push(0);
        unaligned.extend_from_slice(&data);
        assert!(ElfImage::parse(&unaligned[1..]).is_ok());
    }

    #[test]
    fn rejects_writable_executable_segment() {
        let page = PAGE_SIZE as u64;
        let phdr_table_size = core::mem::size_of::<Elf64Phdr>() as u64;
        let ehdr_size = core::mem::size_of::<Elf64Ehdr>() as u64;
        let phoff = ehdr_size;
        let file_offset = (phoff + phdr_table_size + page - 1) & !(page - 1);
        let filesz = page;
        let memsz = page;
        let total = file_offset + filesz;
        let mut data = Vec::new();
        data.resize(total as usize, 0);

        let ehdr = Elf64Ehdr {
            e_ident: {
                let mut ident = [0u8; 16];
                ident[0..4].copy_from_slice(ELF_MAGIC);
                ident[4] = ELFCLASS64;
                ident[5] = 1;
                ident[6] = EV_CURRENT as u8;
                ident
            },
            e_type: ET_EXEC,
            e_machine: EM_X86_64,
            e_version: EV_CURRENT,
            e_entry: USER_BASE + 0x4000,
            e_phoff: phoff,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: core::mem::size_of::<Elf64Ehdr>() as u16,
            e_phentsize: core::mem::size_of::<Elf64Phdr>() as u16,
            e_phnum: 1,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                &ehdr as *const Elf64Ehdr as *const u8,
                data.as_mut_ptr(),
                core::mem::size_of::<Elf64Ehdr>(),
            );
        }

        let phdr = Elf64Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_W | PF_X,
            p_offset: file_offset,
            p_vaddr: USER_BASE + 0x4000,
            p_paddr: USER_BASE + 0x4000,
            p_filesz: filesz,
            p_memsz: memsz,
            p_align: page,
        };
        unsafe {
            let phdr_ptr = data.as_mut_ptr().add(phoff as usize) as *mut Elf64Phdr;
            phdr_ptr.write(phdr);
        }

        assert_eq!(ElfImage::parse(&data), Err(ElfError::WritableExecutableSegment));
    }
}
