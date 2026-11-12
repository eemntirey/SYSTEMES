# SYSTEMES filesystem state

Status recorded for branch `feature/p2-writable-vfs-storage`.

| Component | Implemented | Compiled | Tested | Notes |
|---|---:|---:|---:|---|
| SYSFS parser | Yes | Existing baseline | Existing tests | Read-only |
| VFS read/open/readdir/close | Yes | Existing baseline | Existing tests | Single mount, read-only |
| virtio-blk sector read | Yes | Existing baseline | Existing tests | Legacy PCI I/O transport |
| virtio-blk sector write | Partial | Not verified | Not QEMU-tested | Synchronous OUT path added |
| Block device contract | Yes | Not verified | Interface only | Backend implementations pending |
| Write-back block cache | Yes | Not verified | Unit tests added | Not connected to rootfs/VFS |
| FAT32 read/write | No | No | No | Not started |
| Multi-mount VFS | No | No | No | Not started |
| devfs | No | No | No | Not started |
| tmpfs | No | No | No | Not started |
| Journaled native FS | No | No | No | Not started |
| Device flush/barrier | No | No | No | Virtio BLK FLUSH not implemented |
| Reboot persistence | No | No | No | Requires real flush and QEMU validation |
| fsck-like validation | No | No | No | Not started |

Do not upgrade any status in this table without a reproducible test result.
