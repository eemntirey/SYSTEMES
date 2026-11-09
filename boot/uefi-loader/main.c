#include <efi.h>
#include <efilib.h>

#define SYSTEMES_BOOT_MAGIC 0x53595354454D4553ULL
#define SYSTEMES_BOOT_VERSION 2U
#define KERNEL_LOAD_ADDRESS 0x02000000ULL
#ifndef KERNEL_IMAGE_BYTES
#error KERNEL_IMAGE_BYTES must be provided from the linked kernel image
#endif
#define MEMORY_MAP_CAPACITY (128U * 1024U)

typedef struct {
    UINT64 magic;
    UINT32 version;
    UINT32 reserved;
    UINT64 memory_map_address;
    UINT64 memory_map_size;
    UINT64 memory_map_entries;
    UINT64 memory_map_descriptor_size;
    UINT32 memory_map_descriptor_version;
    UINT32 reserved2;
    UINT64 acpi_rsdp_address;
    UINT64 framebuffer_address;
    UINT32 framebuffer_width;
    UINT32 framebuffer_height;
    UINT32 framebuffer_stride;
    UINT32 framebuffer_pixel_format;
} SYSTEMES_BOOT_INFO;

static EFI_GUID LoadedImageGuid = {0x5b1b31a1, 0x9562, 0x11d2, {0x8e,0x3f,0x00,0xa0,0xc9,0x69,0x72,0x3b}};
static EFI_GUID SimpleFileSystemGuid = {0x964e5b22, 0x6459, 0x11d2, {0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b}};
static EFI_GUID GraphicsOutputGuid = {0x9042a9de, 0x23dc, 0x4a38, {0x96,0xfb,0x7a,0xde,0xd0,0x80,0x51,0x6a}};
static EFI_GUID Acpi20Guid = {0x8868e871, 0xe4f1, 0x11d3, {0xbc,0x22,0x00,0x80,0xc7,0x3c,0x88,0x81}};
static EFI_GUID Acpi10Guid = {0xeb9d2d30, 0x2d88, 0x11d3, {0x9a,0x16,0x00,0x90,0x27,0x3f,0xc1,0x4d}};

static BOOLEAN guid_equal(const EFI_GUID *left, const EFI_GUID *right) {
    UINTN index;
    if (left->Data1 != right->Data1 || left->Data2 != right->Data2 || left->Data3 != right->Data3) return FALSE;
    for (index = 0; index < 8; ++index) if (left->Data4[index] != right->Data4[index]) return FALSE;
    return TRUE;
}

static VOID copy_bytes(VOID *destination, const VOID *source, UINTN length) {
    UINT8 *out = (UINT8 *)destination;
    const UINT8 *in = (const UINT8 *)source;
    UINTN index;
    for (index = 0; index < length; ++index) out[index] = in[index];
}

static VOID zero_bytes(VOID *destination, UINTN length) {
    UINT8 *out = (UINT8 *)destination;
    UINTN index;
    for (index = 0; index < length; ++index) out[index] = 0;
}

static VOID debug_out(UINT8 value) {
    __asm__ __volatile__("outb %0, $0xE9" : : "a"(value));
}

static VOID debug_text(const CHAR8 *text) {
    while (*text) debug_out((UINT8)*text++);
}

static VOID debug_hex64(const CHAR8 *label, UINT64 value) {
    static const CHAR8 digits[] = "0123456789ABCDEF";
    INTN shift;
    debug_text(label);
    for (shift = 60; shift >= 0; shift -= 4) {
        debug_out((UINT8)digits[(value >> shift) & 0x0F]);
    }
    debug_text("\r\n");
}

static EFI_GUID FileInfoGuid = {0x09576e92, 0x6d3f, 0x11d2, {0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b}};

static VOID console_text(EFI_SYSTEM_TABLE *st, CHAR16 *text) {
    uefi_call_wrapper(st->ConOut->OutputString, 2, st->ConOut, text);
}

static VOID fail(EFI_SYSTEM_TABLE *st, CHAR16 *message) {
    debug_text("SYSTEMES_UEFI_FAIL\r\n");
    console_text(st, message);
    console_text(st, L"\r\n");
    for (;;) __asm__ __volatile__("cli; hlt");
}

static EFI_STATUS read_kernel(EFI_HANDLE image, EFI_SYSTEM_TABLE *st, VOID **buffer, UINTN *bytes) {
    EFI_STATUS status;
    EFI_LOADED_IMAGE *loaded = NULL;
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *filesystem = NULL;
    EFI_FILE_HANDLE root = NULL;
    EFI_FILE_HANDLE file = NULL;
    EFI_FILE_INFO *file_info = NULL;
    UINTN info_size = 0;
    UINT64 file_size = 0;
    UINTN total_read = 0;
    UINT8 trailing_byte = 0;

    *buffer = NULL;
    *bytes = 0;

    status = uefi_call_wrapper(st->BootServices->HandleProtocol, 3, image, &LoadedImageGuid, (VOID **)&loaded);
    if (EFI_ERROR(status) || loaded == NULL || loaded->DeviceHandle == NULL) {
        debug_hex64("SYSTEMES_UEFI_FAIL_LOADED_IMAGE_STATUS=", (UINT64)status);
        return EFI_ERROR(status) ? status : EFI_NOT_FOUND;
    }
    debug_text("SYSTEMES_UEFI_LOADED_IMAGE_OK\r\n");

    status = uefi_call_wrapper(st->BootServices->HandleProtocol, 3, loaded->DeviceHandle, &SimpleFileSystemGuid, (VOID **)&filesystem);
    if (EFI_ERROR(status) || filesystem == NULL) {
        debug_hex64("SYSTEMES_UEFI_FAIL_FILESYSTEM_STATUS=", (UINT64)status);
        return EFI_ERROR(status) ? status : EFI_NOT_FOUND;
    }
    debug_text("SYSTEMES_UEFI_FILESYSTEM_OK\r\n");

    status = uefi_call_wrapper(filesystem->OpenVolume, 2, filesystem, &root);
    if (EFI_ERROR(status) || root == NULL) {
        debug_hex64("SYSTEMES_UEFI_FAIL_OPEN_VOLUME_STATUS=", (UINT64)status);
        return EFI_ERROR(status) ? status : EFI_DEVICE_ERROR;
    }
    debug_text("SYSTEMES_UEFI_VOLUME_OK\r\n");

    status = uefi_call_wrapper(root->Open, 5, root, &file, L"\\EFI\\BOOT\\KERNEL.BIN", EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status) || file == NULL) {
        debug_hex64("SYSTEMES_UEFI_FAIL_KERNEL_OPEN_STATUS=", (UINT64)status);
        uefi_call_wrapper(root->Close, 1, root);
        return EFI_ERROR(status) ? status : EFI_NOT_FOUND;
    }
    debug_text("SYSTEMES_UEFI_KERNEL_OPEN_OK\r\n");

    /* Read the authoritative file length before allocating. The linked image
       extent includes .bss; KERNEL.BIN contains the initialized load image. */
    status = uefi_call_wrapper(file->GetInfo, 4, file, &FileInfoGuid, &info_size, NULL);
    if (status != EFI_BUFFER_TOO_SMALL || info_size < sizeof(EFI_FILE_INFO)) {
        debug_hex64("SYSTEMES_UEFI_FAIL_FILE_INFO_STATUS=", (UINT64)status);
        goto cleanup;
    }
    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, info_size, (VOID **)&file_info);
    if (EFI_ERROR(status)) {
        debug_hex64("SYSTEMES_UEFI_FAIL_FILE_INFO_ALLOC_STATUS=", (UINT64)status);
        goto cleanup;
    }
    status = uefi_call_wrapper(file->GetInfo, 4, file, &FileInfoGuid, &info_size, file_info);
    if (EFI_ERROR(status)) {
        debug_hex64("SYSTEMES_UEFI_FAIL_FILE_INFO_READ_STATUS=", (UINT64)status);
        goto cleanup;
    }
    file_size = file_info->FileSize;
    debug_hex64("SYSTEMES_UEFI_KERNEL_FILE_BYTES=", file_size);
    debug_hex64("SYSTEMES_UEFI_KERNEL_IMAGE_BYTES=", (UINT64)KERNEL_IMAGE_BYTES);

    if (file_size == 0 || file_size > (UINT64)KERNEL_IMAGE_BYTES || file_size > (UINT64)(~(UINTN)0)) {
        debug_text("SYSTEMES_UEFI_FAIL_KERNEL_SIZE_BOUNDS\r\n");
        status = EFI_BAD_BUFFER_SIZE;
        goto cleanup;
    }

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, (UINTN)file_size, buffer);
    if (EFI_ERROR(status) || *buffer == NULL) {
        debug_hex64("SYSTEMES_UEFI_FAIL_KERNEL_ALLOC_STATUS=", (UINT64)status);
        if (!EFI_ERROR(status)) status = EFI_OUT_OF_RESOURCES;
        goto cleanup;
    }

    /* Loop over short reads; a successful zero-byte read before EOF is a
       truncation, not a completed kernel load. */
    while ((UINT64)total_read < file_size) {
        UINTN remaining = (UINTN)file_size - total_read;
        UINTN chunk = remaining;
        status = uefi_call_wrapper(file->Read, 3, file, &chunk, (UINT8 *)(*buffer) + total_read);
        if (EFI_ERROR(status)) {
            debug_hex64("SYSTEMES_UEFI_FAIL_KERNEL_READ_STATUS=", (UINT64)status);
            debug_hex64("SYSTEMES_UEFI_KERNEL_BYTES_READ=", (UINT64)total_read);
            goto cleanup;
        }
        if (chunk == 0 || chunk > remaining) {
            debug_text("SYSTEMES_UEFI_FAIL_KERNEL_SHORT_READ\r\n");
            status = EFI_END_OF_FILE;
            goto cleanup;
        }
        total_read += chunk;
    }

    /* Detect a file that grew or whose reported size was inconsistent. */
    {
        UINTN extra = 1;
        status = uefi_call_wrapper(file->Read, 3, file, &extra, &trailing_byte);
        if (EFI_ERROR(status)) {
            debug_hex64("SYSTEMES_UEFI_FAIL_KERNEL_EOF_CHECK_STATUS=", (UINT64)status);
            goto cleanup;
        }
        if (extra != 0) {
            debug_text("SYSTEMES_UEFI_FAIL_KERNEL_SIZE_CHANGED\r\n");
            status = EFI_BAD_BUFFER_SIZE;
            goto cleanup;
        }
    }

    *bytes = total_read;
    debug_hex64("SYSTEMES_UEFI_KERNEL_BYTES_READ=", (UINT64)total_read);
    debug_text("SYSTEMES_UEFI_KERNEL_READ_OK\r\n");
    status = EFI_SUCCESS;

cleanup:
    if (file_info != NULL) uefi_call_wrapper(st->BootServices->FreePool, 1, file_info);
    if (file != NULL) uefi_call_wrapper(file->Close, 1, file);
    if (root != NULL) uefi_call_wrapper(root->Close, 1, root);
    if (EFI_ERROR(status) && *buffer != NULL) {
        uefi_call_wrapper(st->BootServices->FreePool, 1, *buffer);
        *buffer = NULL;
        *bytes = 0;
    }
    return status;
}

static UINT64 find_acpi_rsdp(EFI_SYSTEM_TABLE *st) {
    UINTN index;
    for (index = 0; index < st->NumberOfTableEntries; ++index) {
        EFI_CONFIGURATION_TABLE *entry = &st->ConfigurationTable[index];
        if (guid_equal(&entry->VendorGuid, &Acpi20Guid) || guid_equal(&entry->VendorGuid, &Acpi10Guid))
            return (UINT64)(UINTN)entry->VendorTable;
    }
    return 0;
}

EFI_STATUS EFIAPI efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *st) {
    EFI_STATUS status;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop = NULL;
    EFI_MEMORY_DESCRIPTOR *memory_map = NULL;
    UINTN memory_map_size = MEMORY_MAP_CAPACITY;
    UINTN map_key = 0;
    UINTN descriptor_size = 0;
    UINT32 descriptor_version = 0;
    UINTN kernel_bytes = 0;
    UINTN kernel_pages;
    EFI_PHYSICAL_ADDRESS kernel_address = KERNEL_LOAD_ADDRESS;
    VOID *kernel_buffer = NULL;
    SYSTEMES_BOOT_INFO *boot_info = NULL;
    UINTN boot_info_size = sizeof(SYSTEMES_BOOT_INFO);
    UINTN index;
    UINT64 acpi_rsdp = 0;
    UINT64 framebuffer = 0;
    UINT32 width = 0, height = 0, stride = 0, pixel_format = 0;

    debug_text("SYSTEMES_UEFI_ENTRY_REACHED\r\n");
    debug_text("SYSTEMES_UEFI_START\r\n");
    console_text(st, L"SYSTEMES UEFI loader: start\r\n");

    status = read_kernel(image, st, &kernel_buffer, &kernel_bytes);
    if (EFI_ERROR(status) || kernel_bytes == 0) {
        debug_hex64("SYSTEMES_UEFI_KERNEL_FINAL_STATUS=", (UINT64)status);
        fail(st, L"SYSTEMES: cannot read EFI\\BOOT\\KERNEL.BIN");
    }
    if (kernel_bytes > (UINTN)KERNEL_IMAGE_BYTES) fail(st, L"SYSTEMES: kernel image exceeds linked image bounds");
    debug_text("SYSTEMES_UEFI_KERNEL_FILE_OK\r\n");

    /* Reserve the exact linked image extent, including zero-filled .bss. */
    if ((UINT64)KERNEL_IMAGE_BYTES == 0 || (UINT64)KERNEL_IMAGE_BYTES > (UINT64)(~(UINTN)0)) {
        debug_text("SYSTEMES_UEFI_FAIL_KERNEL_IMAGE_SIZE_INVALID\\r\\n");
        fail(st, L"SYSTEMES: invalid linked kernel image size");
    }
    kernel_pages = EFI_SIZE_TO_PAGES((UINTN)KERNEL_IMAGE_BYTES);
    if (kernel_pages == 0 || kernel_pages > (~(UINTN)0) / EFI_PAGE_SIZE
        || kernel_address > (~(EFI_PHYSICAL_ADDRESS)0) - ((EFI_PHYSICAL_ADDRESS)kernel_pages * EFI_PAGE_SIZE)) {
        debug_text("SYSTEMES_UEFI_FAIL_KERNEL_PAGE_RANGE_INVALID\\r\\n");
        fail(st, L"SYSTEMES: kernel page range overflows physical address space");
    }
    debug_hex64("SYSTEMES_UEFI_KERNEL_LOAD_ADDRESS=", (UINT64)kernel_address);
    debug_hex64("SYSTEMES_UEFI_KERNEL_PAGE_COUNT=", (UINT64)kernel_pages);
    debug_hex64("SYSTEMES_UEFI_KERNEL_RESERVATION_BYTES=", (UINT64)kernel_pages * EFI_PAGE_SIZE);
    status = uefi_call_wrapper(st->BootServices->AllocatePages, 4, AllocateAddress, EfiLoaderCode, kernel_pages, &kernel_address);
    if (EFI_ERROR(status)) {
        debug_hex64("SYSTEMES_UEFI_FAIL_KERNEL_ALLOC_PAGES_STATUS=", (UINT64)status);
        fail(st, L"SYSTEMES: cannot reserve linked kernel physical range");
    }
    copy_bytes((VOID *)(UINTN)kernel_address, kernel_buffer, kernel_bytes);
    zero_bytes((VOID *)((UINTN)kernel_address + kernel_bytes), (UINTN)KERNEL_IMAGE_BYTES - kernel_bytes);
    debug_text("SYSTEMES_UEFI_KERNEL_LOADED\r\n");
    uefi_call_wrapper(st->BootServices->FreePool, 1, kernel_buffer);
    kernel_buffer = NULL;

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, boot_info_size, (VOID **)&boot_info);
    if (EFI_ERROR(status)) fail(st, L"SYSTEMES: cannot allocate boot information");
    debug_text("SYSTEMES_UEFI_BOOT_INFO_ALLOCATED\r\n");

    status = uefi_call_wrapper(st->BootServices->LocateProtocol, 3, &GraphicsOutputGuid, NULL, (VOID **)&gop);
    if (!EFI_ERROR(status) && gop && gop->Mode && gop->Mode->Info) {
        framebuffer = gop->Mode->FrameBufferBase;
        width = gop->Mode->Info->HorizontalResolution;
        height = gop->Mode->Info->VerticalResolution;
        stride = gop->Mode->Info->PixelsPerScanLine;
        pixel_format = (UINT32)gop->Mode->Info->PixelFormat;
    }

    debug_text("SYSTEMES_UEFI_GOP_LOOKUP_DONE\r\n");
    acpi_rsdp = find_acpi_rsdp(st);
    debug_text("SYSTEMES_UEFI_ACPI_LOOKUP_DONE\r\n");
    debug_text(framebuffer ? "SYSTEMES_UEFI_GOP_OK\r\n" : "SYSTEMES_UEFI_GOP_UNAVAILABLE\r\n");
    debug_text(acpi_rsdp ? "SYSTEMES_UEFI_ACPI_OK\r\n" : "SYSTEMES_UEFI_ACPI_UNAVAILABLE\r\n");

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, memory_map_size, (VOID **)&memory_map);
    if (EFI_ERROR(status)) fail(st, L"SYSTEMES: cannot allocate UEFI memory map buffer");
    debug_text("SYSTEMES_UEFI_MAP_BUFFER_ALLOCATED\r\n");

    for (index = 0; index < 4; ++index) {
        debug_text("SYSTEMES_UEFI_GET_MAP_START\r\n");
        memory_map_size = MEMORY_MAP_CAPACITY;
        status = uefi_call_wrapper(st->BootServices->GetMemoryMap, 5, &memory_map_size, memory_map, &map_key, &descriptor_size, &descriptor_version);
        if (status == EFI_BUFFER_TOO_SMALL) fail(st, L"SYSTEMES: memory map buffer capacity exceeded");
        if (EFI_ERROR(status)) fail(st, L"SYSTEMES: GetMemoryMap failed");

        boot_info->magic = SYSTEMES_BOOT_MAGIC;
        boot_info->version = SYSTEMES_BOOT_VERSION;
        boot_info->reserved = 0;
        boot_info->memory_map_address = (UINT64)(UINTN)memory_map;
        boot_info->memory_map_size = memory_map_size;
        boot_info->memory_map_entries = descriptor_size ? memory_map_size / descriptor_size : 0;
        boot_info->memory_map_descriptor_size = descriptor_size;
        boot_info->memory_map_descriptor_version = descriptor_version;
        boot_info->reserved2 = 0;
        boot_info->acpi_rsdp_address = acpi_rsdp;
        boot_info->framebuffer_address = framebuffer;
        boot_info->framebuffer_width = width;
        boot_info->framebuffer_height = height;
        boot_info->framebuffer_stride = stride;
        boot_info->framebuffer_pixel_format = pixel_format;

        debug_text("SYSTEMES_UEFI_MEMORY_MAP_OK\r\n");
        status = uefi_call_wrapper(st->BootServices->ExitBootServices, 2, image, map_key);
        if (!EFI_ERROR(status)) {
            debug_text("SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK\r\n");
            typedef VOID (__attribute__((sysv_abi)) *kernel_entry_t)(SYSTEMES_BOOT_INFO *);
            kernel_entry_t entry = (kernel_entry_t)(UINTN)kernel_address;
            __asm__ __volatile__("cli" ::: "memory");
            entry(boot_info);
            return EFI_SUCCESS;
        }
        debug_text("SYSTEMES_UEFI_EXIT_BOOT_SERVICES_RETRY\r\n");
        if (status != EFI_INVALID_PARAMETER) break;
    }

    fail(st, L"SYSTEMES: ExitBootServices failed after memory-map retries");
    return EFI_ABORTED;
}
