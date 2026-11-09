OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

SECTIONS
{
    . = 0x02000000;
    .text : ALIGN(4096) {
        __text_start = .;
        KEEP(*(.text._start))
        *(.text .text.*)
        . = ALIGN(4096);
        __text_end = .;
    }
    .rodata : ALIGN(4096) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        . = ALIGN(4096);
        __rodata_end = .;
    }
    .data : ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(4096);
        __data_end = .;
    }
    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
    }
    __kernel_image_end = .;
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note .note.*) }
}
