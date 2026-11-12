# Filesystem and block-storage validation

## Host tests

Run from the repository root:

```sh
cargo test --manifest-path kernel/Cargo.toml --all-features
cargo check --manifest-path kernel/Cargo.toml --all-features --target x86_64-unknown-none
```

The new block-cache tests cover write-back visibility, sync to a memory backend,
dirty-bit retention after an I/O error, LRU eviction write-back, and out-of-range
LBA rejection.

## Not yet validated

The virtio OUT path has not yet been exercised under QEMU. No claim is made for
disk persistence across reboot, FAT32 interoperability, filesystem crash
recovery, SMP concurrent filesystem access, or fsck-style consistency.
These remain mandatory before this branch can be considered complete.

## Required QEMU scenario (future gate)

1. Create a FAT32 disk image with standard host tooling.
2. Attach it as a virtio-blk device.
3. Write files from the SYSTEMES VFS and call a real device-backed sync.
4. Power off QEMU and restart with the exact same image.
5. Read and compare contents, then run fsck.fat on the image.
6. Repeat with 1, 2, and 4 vCPUs and concurrent readers/writers.
