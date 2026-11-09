#!/usr/bin/env python3
"""Build the deterministic read-only SYSFS image used by SYSTEMES CI and local QEMU runs."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

BLOCK_SIZE = 512
INODE_SIZE = 64
INODE_COUNT = 5
DATA_BLOCKS = 5
ROOT_INODE = 0


def put_inode(
    image: bytearray,
    index: int,
    *,
    directory: bool,
    size: bytes | int,
    first_block: int,
    block_count: int,
    name: str,
) -> None:
    encoded_name = name.encode("utf-8")
    if len(encoded_name) > 46:
        raise ValueError(f"inode name too long: {name!r}")
    offset = BLOCK_SIZE + index * INODE_SIZE
    struct.pack_into(
        "<HIII",
        image,
        offset,
        1 if directory else 0,
        len(size) if isinstance(size, bytes) else size,
        first_block,
        block_count,
    )
    image[offset + 14 : offset + 14 + len(encoded_name)] = encoded_name


def make_image() -> bytes:
    image = bytearray(BLOCK_SIZE * (2 + DATA_BLOCKS))
    image[0:6] = b"SYSFS1"
    struct.pack_into("<IIIII", image, 6, 1, BLOCK_SIZE, INODE_COUNT, DATA_BLOCKS, ROOT_INODE)

    # Directory data blocks store little-endian u32 inode indices.
    put_inode(image, 0, directory=True, size=8, first_block=0, block_count=1, name="root")
    put_inode(image, 1, directory=False, size=b"hello\n", first_block=2, block_count=1, name="hello.txt")
    put_inode(image, 2, directory=True, size=8, first_block=1, block_count=1, name="docs")
    put_inode(image, 3, directory=False, size=b"Readme\n", first_block=3, block_count=1, name="readme.txt")
    put_inode(image, 4, directory=False, size=b"Notes\n", first_block=4, block_count=1, name="notes.txt")

    data_offset = BLOCK_SIZE * 2
    struct.pack_into("<II", image, data_offset, 1, 2)
    struct.pack_into("<II", image, data_offset + BLOCK_SIZE, 3, 4)
    image[data_offset + 2 * BLOCK_SIZE : data_offset + 2 * BLOCK_SIZE + 6] = b"hello\n"
    image[data_offset + 3 * BLOCK_SIZE : data_offset + 3 * BLOCK_SIZE + 7] = b"Readme\n"
    image[data_offset + 4 * BLOCK_SIZE : data_offset + 4 * BLOCK_SIZE + 6] = b"Notes\n"

    expected_size = BLOCK_SIZE * (2 + DATA_BLOCKS)
    if len(image) != expected_size:
        raise AssertionError("unexpected SYSFS image size")
    return bytes(image)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("build/uefi/sysfs.img"),
        help="output image path (default: build/uefi/sysfs.img)",
    )
    args = parser.parse_args()
    image = make_image()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(image)
    print(f"SYSFS image created: {args.output} ({len(image)} bytes)")


if __name__ == "__main__":
    main()
