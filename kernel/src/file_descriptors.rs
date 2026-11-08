//! Per-process descriptor bookkeeping for the single active Ring 3 process.
//!
//! This module is deliberately allocation-free so its lifecycle rules can be
//! tested by the host contract suite.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::FileHandle;

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

    #[test]
    fn closing_one_descriptor_does_not_alias_another() {
        let mut table = ProcessFileTable::new();
        let first = table.insert(FileHandle(11)).unwrap();
        let second = table.insert(FileHandle(12)).unwrap();
        assert_eq!(table.remove(first), Ok(FileHandle(11)));
        assert_eq!(table.get(second), Ok(FileHandle(12)));
    }
}
