#include <efi.h>
#include <efilib.h>

#define SYSTEMES_BOOT_MAGIC 0x53595354454D4553ULL
#define SYSTEMES_BOOT_VERSION 2U
#define KERNEL_LOAD_ADDRESS 0x00200000ULL
#define KERNEL_MAX_BYTES (8U * 1024U * 1024U)
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

static EFI_GUID Acpi20Guid = {0x8868e871, 0xe4f1, 0x11d3, {0xbc,0x22,0x00,0x80,0xc7,0x3c,0x88,0x81}};
static EFI_GUID Acpi10Guid = {0xeb9d2d30, 0x2d88, 0x11d3, {0x9a,0x16,0x00,0x90,0x27,0x3f,0xc1,0x4d}};

static VOID debug_out(UINT8 value) {
    __asm__ __volatile__("outb %0, $0xE9" : : "a"(value));
}

static VOID serial_text(const CHAR8 *text) {
    while (*text) debug_out((UINT8)*text++);
}

static VOID console_text(EFI_SYSTEM_TABLE *st, CHAR16 *text) {
    uefi_call_wrapper(st->ConOut->OutputString, 2, st->ConOut, text);
}

static VOID fail(EFI_SYSTEM_TABLE *st, CHAR16 *message) {
    serial_text("SYSTEMES_UEFI_FAIL\r\n");
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

    status = uefi_call_wrapper(st->BootServices->HandleProtocol, 3, image, &LoadedImageProtocol, (VOID **)&loaded);
    if (EFI_ERROR(status)) { serial_text("SYSTEMES_UEFI_FAIL_LOADED_IMAGE\r\n"); return status; }
    serial_text("SYSTEMES_UEFI_LOADED_IMAGE_OK\r\n");

    status = uefi_call_wrapper(st->BootServices->HandleProtocol, 3, loaded->DeviceHandle, &FileSystemProtocol, (VOID **)&filesystem);
    if (EFI_ERROR(status)) { serial_text("SYSTEMES_UEFI_FAIL_FILESYSTEM\r\n"); return status; }
    serial_text("SYSTEMES_UEFI_FILESYSTEM_OK\r\n");

    status = uefi_call_wrapper(filesystem->OpenVolume, 2, filesystem, &root);
    if (EFI_ERROR(status)) { serial_text("SYSTEMES_UEFI_FAIL_OPEN_VOLUME\r\n"); return status; }
    serial_text("SYSTEMES_UEFI_VOLUME_OK\r\n");

    status = uefi_call_wrapper(root->Open, 5, root, &file, L"\\EFI\\BOOT\\KERNEL.BIN", EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) {
        serial_text("SYSTEMES_UEFI_FAIL_KERNEL_OPEN\r\n");
        uefi_call_wrapper(root->Close, 1, root);
        return status;
    }
    serial_text("SYSTEMES_UEFI_KERNEL_OPEN_OK\r\n");

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, KERNEL_MAX_BYTES, buffer);
    if (EFI_ERROR(status)) goto cleanup;

    *bytes = KERNEL_MAX_BYTES;
    status = uefi_call_wrapper(file->Read, 3, file, bytes, *buffer);
    if (!EFI_ERROR(status) && *bytes == KERNEL_MAX_BYTES) status = EFI_BAD_BUFFER_SIZE;
    if (!EFI_ERROR(status)) serial_text("SYSTEMES_UEFI_KERNEL_READ_OK\r\n");
    else serial_text("SYSTEMES_UEFI_FAIL_KERNEL_READ\r\n");

cleanup:
    uefi_call_wrapper(file->Close, 1, file);
    uefi_call_wrapper(root->Close, 1, root);
    return status;
}

static UINT64 find_acpi_rsdp(EFI_SYSTEM_TABLE *st) {
    UINTN index;
    for (index = 0; index < st->NumberOfTableEntries; ++index) {
        EFI_CONFIGURATION_TABLE *entry = &st->ConfigurationTable[index];
        if (CompareGuid(&entry->VendorGuid, &Acpi20Guid) || CompareGuid(&entry->VendorGuid, &Acpi10Guid))
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

    InitializeLib(image, st);
    serial_text("SYSTEMES_UEFI_START\r\n");
    console_text(st, L"SYSTEMES UEFI loader: start\r\n");

    status = read_kernel(image, st, &kernel_buffer, &kernel_bytes);
    if (EFI_ERROR(status) || kernel_bytes == 0) fail(st, L"SYSTEMES: cannot read EFI\\BOOT\\KERNEL.BIN");
    if (kernel_bytes > KERNEL_MAX_BYTES) fail(st, L"SYSTEMES: kernel image exceeds loader limit");
    serial_text("SYSTEMES_UEFI_KERNEL_FILE_OK\r\n");

    kernel_pages = EFI_SIZE_TO_PAGES(kernel_bytes);
    status = uefi_call_wrapper(st->BootServices->AllocatePages, 4, AllocateAddress, EfiLoaderCode, kernel_pages, &kernel_address);
    if (EFI_ERROR(status)) fail(st, L"SYSTEMES: cannot reserve kernel load address 0x200000");
    CopyMem((VOID *)(UINTN)kernel_address, kernel_buffer, kernel_bytes);
    serial_text("SYSTEMES_UEFI_KERNEL_LOADED\r\n");
    uefi_call_wrapper(st->BootServices->FreePool, 1, kernel_buffer);
    kernel_buffer = NULL;

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, boot_info_size, (VOID **)&boot_info);
    if (EFI_ERROR(status)) fail(st, L"SYSTEMES: cannot allocate boot information");

    status = LibLocateProtocol(&GraphicsOutputProtocol, (VOID **)&gop);
    if (!EFI_ERROR(status) && gop && gop->Mode && gop->Mode->Info) {
        framebuffer = gop->Mode->FrameBufferBase;
        width = gop->Mode->Info->HorizontalResolution;
        height = gop->Mode->Info->VerticalResolution;
        stride = gop->Mode->Info->PixelsPerScanLine;
        pixel_format = (UINT32)gop->Mode->Info->PixelFormat;
    }

    acpi_rsdp = find_acpi_rsdp(st);
    serial_text(framebuffer ? "SYSTEMES_UEFI_GOP_OK\r\n" : "SYSTEMES_UEFI_GOP_UNAVAILABLE\r\n");
    serial_text(acpi_rsdp ? "SYSTEMES_UEFI_ACPI_OK\r\n" : "SYSTEMES_UEFI_ACPI_UNAVAILABLE\r\n");

    status = uefi_call_wrapper(st->BootServices->AllocatePool, 3, EfiLoaderData, memory_map_size, (VOID **)&memory_map);
    if (EFI_ERROR(status)) fail(st, L"SYSTEMES: cannot allocate UEFI memory map buffer");

    for (index = 0; index < 4; ++index) {
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

        serial_text("SYSTEMES_UEFI_MEMORY_MAP_OK\r\n");
        status = uefi_call_wrapper(st->BootServices->ExitBootServices, 2, image, map_key);
        if (!EFI_ERROR(status)) {
            serial_text("SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK\r\n");
            typedef VOID (__attribute__((sysv_abi)) *kernel_entry_t)(SYSTEMES_BOOT_INFO *);
            kernel_entry_t entry = (kernel_entry_t)(UINTN)kernel_address;
            __asm__ __volatile__("cli" ::: "memory");
            entry(boot_info);
            return EFI_SUCCESS;
        }
        serial_text("SYSTEMES_UEFI_EXIT_BOOT_SERVICES_RETRY\r\n");
        if (status != EFI_INVALID_PARAMETER) break;
    }

    fail(st, L"SYSTEMES: ExitBootServices failed after memory-map retries");
    return EFI_ABORTED;
}
