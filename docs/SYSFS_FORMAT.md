# SYSFS image format

SYSFS v1 uses 512-byte sectors and a read-only, contiguous-extent layout.

| Region | Layout |
|---|---|
| Superblock | 512 bytes; magic `SYSFS1`, version, block size, inode count, data-block count, root inode index |
| Inode table | Rounded up to whole 512-byte blocks; 64 bytes per inode |
| Data region | `data_blocks` sectors; file bytes and directory-entry arrays share this region |

Each 64-byte inode stores a 16-bit mode flag, 32-bit byte size, 32-bit first data block, 32-bit block count, and a 46-byte UTF-8 name. Mode bit `1` marks a directory.

A directory inode's data extent contains a sequence of little-endian `u32` inode indices. Its `size` is the byte size of that index array and must be divisible by four. The iterator validates every referenced inode and is allocation-free. File inodes use the same extent fields for contiguous file data.

The root directory in legacy images may have `size = 0` and `block_count = 0`; for compatibility, the parser then treats all non-root inode records as its flat children. This keeps the historical 1,536-byte SYSFS image readable. New images should use explicit directory-entry arrays.

Generate the deterministic CI/local image with:

```sh
python3 scripts/make-sysfs.py --output build/uefi/sysfs.img
```

The current fixture contains `/hello.txt`, `/docs/readme.txt`, and `/docs/notes.txt`, with the root and `/docs` directories storing their child inode indices explicitly. The generated image is 3,584 bytes. The kernel still treats a missing or invalid disk as non-fatal.
