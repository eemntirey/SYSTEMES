//! ACPI table discovery and MADT parsing for x86_64 interrupt-controller setup.
 //!
 //! The UEFI loader supplies the physical RSDP address in BootInfo. The kernel's
 //! current identity map makes firmware tables below the 512-GiB window readable.
 
 use core::mem::size_of;
 
 const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
 const SDT_HEADER_SIZE: usize = 36;
 const MAX_TABLE_LENGTH: usize = 1024 * 1024;
 const MAX_IO_APICS: usize = 8;
 const MAX_ISO: usize = 16;
 const MAX_LOCAL_APICS: usize = 64;
 
 #[derive(Clone, Copy, Debug, Eq, PartialEq)]
 pub enum AcpiError {
     NullAddress,
     InvalidRsdpSignature,
     InvalidChecksum,
     InvalidRsdpLength,
     InvalidSdtSignature,
     InvalidTableLength,
     UnsupportedRootTable,
     MissingMadt,
     MalformedMadtEntry,
     TooManyEntries,
 }
 
 #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
 pub struct LocalApic {
     pub processor_uid: u8,
     pub apic_id: u8,
     pub flags: u32,
 }
 
 #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
 pub struct IoApic {
     pub id: u8,
     pub address: u32,
     pub gsi_base: u32,
 }
 
 #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
 pub struct InterruptOverride {
     pub bus: u8,
     pub source: u8,
     pub gsi: u32,
     pub flags: u16,
 }
 
 #[derive(Clone, Copy, Debug, Eq, PartialEq)]
 pub struct MadtInfo {
     pub local_apic_address: u64,
     pub flags: u32,
     pub local_apics: [LocalApic; MAX_LOCAL_APICS],
     pub local_apic_count: usize,
     pub io_apics: [IoApic; MAX_IO_APICS],
     pub io_apic_count: usize,
     pub overrides: [InterruptOverride; MAX_ISO],
     pub override_count: usize,
 }
 
 impl MadtInfo {
     const fn empty() -> Self {
         Self {
             local_apic_address: 0,
             flags: 0,
             local_apics: [LocalApic { processor_uid: 0, apic_id: 0, flags: 0 }; MAX_LOCAL_APICS],
             local_apic_count: 0,
             io_apics: [IoApic { id: 0, address: 0, gsi_base: 0 }; MAX_IO_APICS],
             io_apic_count: 0,
             overrides: [InterruptOverride { bus: 0, source: 0, gsi: 0, flags: 0 }; MAX_ISO],
             override_count: 0,
         }
     }
 }
 
 fn checksum_valid(bytes: &[u8]) -> bool {
     bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0
 }
 
 fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AcpiError> {
     let field = bytes.get(offset..offset + 4).ok_or(AcpiError::InvalidTableLength)?;
     Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
 }
 
 fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, AcpiError> {
     let field = bytes.get(offset..offset + 8).ok_or(AcpiError::InvalidTableLength)?;
     Ok(u64::from_le_bytes([field[0], field[1], field[2], field[3], field[4], field[5], field[6], field[7]]))
 }
 
 /// Parse a complete APIC (MADT) SDT, including its header and checksum.
 pub fn parse_madt(table: &[u8]) -> Result<MadtInfo, AcpiError> {
     if table.len() < SDT_HEADER_SIZE + 8 || table.get(0..4) != Some(&b"APIC"[..]) {
         return Err(AcpiError::InvalidSdtSignature);
     }
     let declared = read_u32(table, 4)? as usize;
     if declared < SDT_HEADER_SIZE + 8 || declared > MAX_TABLE_LENGTH || declared != table.len() {
         return Err(AcpiError::InvalidTableLength);
     }
     if !checksum_valid(table) {
         return Err(AcpiError::InvalidChecksum);
     }
 
     let mut info = MadtInfo::empty();
     info.local_apic_address = read_u32(table, SDT_HEADER_SIZE)? as u64;
     info.flags = read_u32(table, SDT_HEADER_SIZE + 4)?;
     let mut cursor = SDT_HEADER_SIZE + 8;
     while cursor < declared {
         let kind = *table.get(cursor).ok_or(AcpiError::MalformedMadtEntry)?;
         let length = *table.get(cursor + 1).ok_or(AcpiError::MalformedMadtEntry)? as usize;
         if length < 2 || cursor.checked_add(length).filter(|end| *end <= declared).is_none() {
             return Err(AcpiError::MalformedMadtEntry);
         }
         let entry = &table[cursor..cursor + length];
         match kind {
             0 => {
                 if length != 8 { return Err(AcpiError::MalformedMadtEntry); }
                 if info.local_apic_count == MAX_LOCAL_APICS { return Err(AcpiError::TooManyEntries); }
                 let index = info.local_apic_count;
                 info.local_apics[index] = LocalApic {
                     processor_uid: entry[2],
                     apic_id: entry[3],
                     flags: u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]),
                 };
                 info.local_apic_count += 1;
             }
             1 => {
                 if length != 12 { return Err(AcpiError::MalformedMadtEntry); }
                 if info.io_apic_count == MAX_IO_APICS { return Err(AcpiError::TooManyEntries); }
                 let index = info.io_apic_count;
                 info.io_apics[index] = IoApic {
                     id: entry[2],
                     address: u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]),
                     gsi_base: u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]),
                 };
                 info.io_apic_count += 1;
             }
             2 => {
                 if length != 10 { return Err(AcpiError::MalformedMadtEntry); }
                 if info.override_count == MAX_ISO { return Err(AcpiError::TooManyEntries); }
                 let index = info.override_count;
                 info.overrides[index] = InterruptOverride {
                     bus: entry[2],
                     source: entry[3],
                     gsi: u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]),
                     flags: u16::from_le_bytes([entry[8], entry[9]]),
                 };
                 info.override_count += 1;
             }
             // NMI source, Local APIC NMI, address override and x2APIC records are
             // intentionally not consumed until their semantics are implemented.
             _ => {}
         }
         cursor += length;
     }
     if info.local_apic_address == 0 {
         return Err(AcpiError::MalformedMadtEntry);
     }
     Ok(info)
 }
 
 /// Discover and parse MADT using the RSDP address supplied by the UEFI loader.
 ///
 /// # Safety
 /// The caller must pass the firmware-provided physical RSDP address while the
 /// kernel's identity map covers the RSDP and all referenced ACPI tables.
 pub unsafe fn discover_madt(rsdp_address: u64) -> Result<MadtInfo, AcpiError> {
     if rsdp_address == 0 { return Err(AcpiError::NullAddress); }
     // SAFETY: The caller guarantees this firmware-provided physical range is mapped.
     let rsdp_prefix = unsafe { core::slice::from_raw_parts(rsdp_address as *const u8, 20) };
     if rsdp_prefix.get(0..8) != Some(&RSDP_SIGNATURE[..]) { return Err(AcpiError::InvalidRsdpSignature); }
     if !checksum_valid(rsdp_prefix) { return Err(AcpiError::InvalidChecksum); }
 
     let revision = rsdp_prefix[15];
     let (root_address, root_signature, entry_width) = if revision >= 2 {
         // SAFETY: ACPI 2.0+ RSDP has a fixed 36-byte prefix before its declared length.
         let extended = unsafe { core::slice::from_raw_parts(rsdp_address as *const u8, 36) };
         let length = read_u32(extended, 20)? as usize;
         if !(36..=4096).contains(&length) { return Err(AcpiError::InvalidRsdpLength); }
         // SAFETY: The firmware-provided RSDP allocation is expected to contain its declared length.
         let complete = unsafe { core::slice::from_raw_parts(rsdp_address as *const u8, length) };
         if !checksum_valid(complete) { return Err(AcpiError::InvalidChecksum); }
         (read_u64(extended, 24)?, b"XSDT" as &[u8], 8usize)
     } else {
         (read_u32(rsdp_prefix, 16)? as u64, b"RSDT" as &[u8], 4usize)
     };
     if root_address == 0 { return Err(AcpiError::UnsupportedRootTable); }
 
     // SAFETY: The root table address comes from the validated firmware RSDP.
     let root_header = unsafe { core::slice::from_raw_parts(root_address as *const u8, SDT_HEADER_SIZE) };
     if root_header.get(0..4) != Some(root_signature) { return Err(AcpiError::InvalidSdtSignature); }
     let root_length = read_u32(root_header, 4)? as usize;
     if root_length < SDT_HEADER_SIZE || root_length > MAX_TABLE_LENGTH
         || (root_length - SDT_HEADER_SIZE) % entry_width != 0 {
         return Err(AcpiError::InvalidTableLength);
     }
     // SAFETY: The firmware root table declares the complete table length.
     let root = unsafe { core::slice::from_raw_parts(root_address as *const u8, root_length) };
     if !checksum_valid(root) { return Err(AcpiError::InvalidChecksum); }
     for offset in (SDT_HEADER_SIZE..root_length).step_by(entry_width) {
         let address = if entry_width == 8 { read_u64(root, offset)? } else { read_u32(root, offset)? as u64 };
         if address == 0 { continue; }
         // SAFETY: Each SDT address is firmware-provided and covered by the identity map.
         let header = unsafe { core::slice::from_raw_parts(address as *const u8, SDT_HEADER_SIZE) };
         if header.get(0..4) != Some(&b"APIC"[..]) { continue; }
         let length = read_u32(header, 4)? as usize;
         if !(SDT_HEADER_SIZE + 8..=MAX_TABLE_LENGTH).contains(&length) {
             return Err(AcpiError::InvalidTableLength);
         }
         // SAFETY: The MADT header provides its bounded complete length.
         let madt = unsafe { core::slice::from_raw_parts(address as *const u8, length) };
         return parse_madt(madt);
     }
     Err(AcpiError::MissingMadt)
 }
 
 #[cfg(test)]
 mod tests {
     use super::*;
 
     fn checksum(bytes: &mut [u8], offset: usize) {
         bytes[offset] = 0;
         let sum = bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
         bytes[offset] = 0u8.wrapping_sub(sum);
     }
 
     fn sample_madt() -> [u8; 74] {
         let mut bytes = [0u8; 74];
         bytes[0..4].copy_from_slice(b"APIC");
         bytes[4..8].copy_from_slice(&(74u32).to_le_bytes());
         bytes[SDT_HEADER_SIZE..SDT_HEADER_SIZE + 4].copy_from_slice(&0xFEE0_0000u32.to_le_bytes());
         bytes[SDT_HEADER_SIZE + 4..SDT_HEADER_SIZE + 8].copy_from_slice(&1u32.to_le_bytes());
         // Processor Local APIC record.
         bytes[44..52].copy_from_slice(&[0, 8, 0, 2, 1, 0, 0, 0]);
         // IO APIC record.
         bytes[52..64].copy_from_slice(&[1, 12, 3, 0, 0, 0, 0xC0, 0xFE, 0, 0, 0, 0]);
         // ISA IRQ0 -> GSI2 override.
         bytes[64..74].copy_from_slice(&[2, 10, 0, 0, 2, 0, 0, 0, 0, 0]);
         checksum(&mut bytes, 9);
         bytes
     }
 
     #[test]
     fn rejects_bad_madt_signature_and_checksum() {
         let mut bytes = sample_madt();
         bytes[0] = b'X';
         assert_eq!(parse_madt(&bytes), Err(AcpiError::InvalidSdtSignature));
         let mut bytes = sample_madt();
         bytes[40] ^= 1;
         assert_eq!(parse_madt(&bytes), Err(AcpiError::InvalidChecksum));
     }
 
     #[test]
     fn rejects_malformed_entry_lengths() {
         let mut bytes = sample_madt();
         bytes[45] = 1;
         checksum(&mut bytes, 9);
         assert_eq!(parse_madt(&bytes), Err(AcpiError::MalformedMadtEntry));
     }
 
     #[test]
     fn checksum_validator_accepts_zero_sum_tables() {
         let mut bytes = [0u8; 4];
         bytes[0] = 0xA5;
         checksum(&mut bytes, 1);
         assert!(checksum_valid(&bytes));
     }
 }
