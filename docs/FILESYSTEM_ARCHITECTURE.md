# SYSTEMES filesystem architecture

## Current implementation boundary

The current root filesystem is still the legacy SYSFS image parser mounted by
`rootfs::mount`. It is read-only. The VFS currently exposes open/read/read_dir/
close over one mounted SYSFS instance. No FAT32 implementation or writable
filesystem is present yet.

## Storage primitives added on P2 branch

- `virtio_blk::VirtioBlk::write_sector`: synchronous single-sector virtio-blk
  OUT request using a DMA staging frame. Existing IN reads remain unchanged.
- `block::BlockDevice`: backend contract for fixed 512-byte blocks.
- `block::BlockCache<D, N>`: fixed-capacity LRU write-back cache. Dirty data is
  retained after failed writes. `sync` writes all dirty entries and then calls
  the backend durability barrier. A backend must return Unsupported when it
  cannot provide a real flush/barrier.

The cache is currently a standalone primitive and is not wired into rootfs or
the VFS. The `VirtioBlockDevice` adapter exposes sector reads/writes through the
block contract, but its flush method deliberately returns Unsupported because
the legacy driver has not negotiated VIRTIO_BLK_F_FLUSH.

- `tmpfs::TmpFs`: bounded in-memory tree with create/mkdir/read/write/stat,
  truncate/unlink/rename and payload-byte accounting. It is a standalone
  filesystem primitive and is not yet mounted through VFS.
- `devfs::DevFs`: /dev/null, /dev/zero and /dev/console read/write semantics.
  Console I/O is delegated to kernel callbacks; it is not yet mounted through VFS. The current legacy virtio driver does not yet negotiate/implement the
virtio BLK FLUSH feature, so a production backend must not claim durable sync.

## Planned layering

VFS -> filesystem implementations -> block cache -> block device -> virtio-blk.

FAT32 is the first persistent filesystem target. SYSFS remains available as a
legacy read-only boot image until a separately validated migration is complete.
tmpfs and devfs will be independent in-memory/virtual filesystem implementations.
A journaled native filesystem is deferred until block ordering and flush
semantics are implemented and tested.
