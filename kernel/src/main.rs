#![no_std]
#![no_main]

extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;
use systemes_kernel::{memory::PhysicalFrameAllocator, paging::{MemoryRegion, PagePermissions}, BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
    static __bss_start: u8;
    static __bss_end: u8;
}

const DEBUG_PORT: u16 = 0xE9;

unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The boot environment is x86_64 and the QEMU debug port 0xE9.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

unsafe fn debug_write(text: &[u8]) {
    for byte in text {
        unsafe { outb(DEBUG_PORT, *byte) };
    }
}

fn halt() -> ! {
    loop {
        // SAFETY: A fatal boot error must stop execution without returning to the loader.
        unsafe { asm!("cli; hlt", options(nomem, nostack)) };
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "sysv64" fn _start(info: *const BootInfo) -> ! {
    
    unsafe { debug_write(b"SYSTEMES: kernel entry reached\r\n") };

    if info.is_null() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: null BootInfo\r\n") };
        halt();
    }

    // SAFETY: The UEFI loader owns this stable BootInfo allocation and passes its address.
    let boot = unsafe { &*info };
    if boot.magic != BOOT_INFO_MAGIC || boot.version != BOOT_INFO_VERSION {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid BootInfo contract\r\n") };
        halt();
    }
    if boot.memory_map_address == 0 || boot.memory_map_size == 0 || boot.memory_map_descriptor_size == 0 {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: missing UEFI memory map\r\n") };
        halt();
    }

    unsafe { debug_write(b"SYSTEMES: UEFI memory map accepted\r\n") };

    let map_size = match usize::try_from(boot.memory_map_size) {
        Ok(size) => size,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: memory map size overflow\r\n") };
            halt();
        }
    };
    let descriptor_size = match usize::try_from(boot.memory_map_descriptor_size) {
        Ok(size) => size,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: descriptor size overflow\r\n") };
            halt();
        }
    };

    // SAFETY: BootInfo was validated by the loader contract; UEFI keeps this
    // identity-mapped allocation valid after ExitBootServices for the boot phase.
    let memory_map = unsafe {
        core::slice::from_raw_parts(boot.memory_map_address as *const u8, map_size)
    };
    let mut frame_allocator = match PhysicalFrameAllocator::from_uefi_memory_map(memory_map, descriptor_size) {
        Ok(allocator) => allocator,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid UEFI memory descriptors\\r\\n") };
            halt();
        }
    };
    let initial_free_pages = frame_allocator.free_pages();
    let test_frame = match frame_allocator.allocate_frame() {
        Some(address) => address,
        None => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: no conventional memory frames\\r\\n") };
            halt();
        }
    };
    if frame_allocator.free_frame(test_frame).is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: physical frame release failed\\r\\n") };
        halt();
    }
    if frame_allocator.free_pages() != initial_free_pages {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: physical frame accounting mismatch\\r\\n") };
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: physical frame allocator initialized\\r\\n") };
    if systemes_kernel::paging::enable_nx().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CPU does not support NX\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: CPU NX capability detected and EFER.NXE enabled\\r\\n"); }
    // SAFETY: CPU tables are installed during single-core boot while IRQs remain disabled.
    unsafe { systemes_kernel::interrupts::initialize() };
    unsafe { debug_write(b"SYSTEMES: GDT initialized\\r\\n") };
    unsafe { debug_write(b"SYSTEMES: IDT initialized for CPU exceptions 0..31\\r\\n") };
    // SAFETY: UEFI identity mappings remain active while the new page tables are built.
    let page_table_root = match unsafe {
        systemes_kernel::paging::build_identity_map(&mut frame_allocator)
    } {
        Ok(root) => root,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: page-table allocation failed\\r\\n"); }
            halt();
        }
    };
    let kernel_regions = [
        MemoryRegion { start: core::ptr::addr_of!(__text_start) as u64, end: core::ptr::addr_of!(__text_end) as u64, permissions: PagePermissions::ReadOnlyExecute },
        MemoryRegion { start: core::ptr::addr_of!(__rodata_start) as u64, end: core::ptr::addr_of!(__rodata_end) as u64, permissions: PagePermissions::ReadOnlyNoExecute },
        MemoryRegion { start: core::ptr::addr_of!(__data_start) as u64, end: core::ptr::addr_of!(__data_end) as u64, permissions: PagePermissions::ReadWriteNoExecute },
        MemoryRegion { start: core::ptr::addr_of!(__bss_start) as u64, end: core::ptr::addr_of!(__bss_end) as u64, permissions: PagePermissions::ReadWriteNoExecute },
    ];
    // SAFETY: The inherited UEFI identity map is active and linker bounds are page aligned.
    if unsafe { systemes_kernel::paging::protect_regions(&mut frame_allocator, page_table_root, &kernel_regions) }.is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: W^X section mapping failed\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: kernel code RX, rodata R-NX, data and BSS RW-NX\\r\\n"); }
    // SAFETY: The new PML4 identity-maps boot data, stack, descriptors and framebuffer.
    unsafe { asm!("mov cr3, {}", in(reg) page_table_root, options(nostack, preserves_flags)) };
    let active_cr3: u64;
    // SAFETY: Reading CR3 is valid in ring 0 on x86_64.
    unsafe { asm!("mov {}, cr3", out(reg) active_cr3, options(nomem, nostack, preserves_flags)) };
    if (active_cr3 & 0x000F_FFFF_FFFF_F000) != page_table_root {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CR3 verification failed\\r\\n") };
        halt();
    }
    if systemes_kernel::paging::enable_write_protect().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CR0.WP activation failed\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: CR0.WP supervisor write protection enabled\\r\\n"); }
    if unsafe { systemes_kernel::paging::verify_regions(page_table_root, &kernel_regions) }.is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: W^X page-table verification failed\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: W^X 4 KiB page permissions verified\\r\\n") };
    if systemes_kernel::heap::initialize().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: dynamic kernel heap initialization failed\\r\\n"); }
        halt();
    }
    if !systemes_kernel::heap::is_initialized() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: dynamic kernel heap state invalid\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: 1 MiB dynamic kernel heap initialized\\r\\n"); }
    #[cfg(feature = "qemu-heap-test")]
    {
        let layout = match core::alloc::Layout::from_size_align(128, 64) {
            Ok(layout) => layout,
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid heap test layout\\r\\n"); }
                halt();
            }
        };
        let initial_free = systemes_kernel::heap::free_bytes();
        // SAFETY: The global allocator has been initialized and this layout is valid.
        let allocation = unsafe { alloc::alloc::alloc(layout) };
        if allocation.is_null() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test allocation failed\\r\\n"); }
            halt();
        }
        // SAFETY: The allocation contains 128 writable bytes and remains live here.
        unsafe { allocation.write(0x5A); }
        // SAFETY: The allocation is still live and contains the byte written above.
        if unsafe { allocation.read() } != 0x5A {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test data mismatch\\r\\n"); }
            halt();
        }
        // SAFETY: Return the exact pointer with the original layout.
        unsafe { alloc::alloc::dealloc(allocation, layout); }
        if systemes_kernel::heap::free_bytes() != initial_free {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test coalescing failed\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: dynamic heap allocation, alignment and free coalescing verified\\r\\n"); }
    }
    unsafe { debug_write(b"SYSTEMES: kernel-owned 4-level identity paging active\\r\\n") };
    if boot.framebuffer_address != 0 && boot.framebuffer_width > 0 && boot.framebuffer_height > 0 {
        let pixel = match boot.framebuffer_pixel_format {
            0 => 0x00_20_C0_70u32, // GOP PixelRedGreenBlueReserved8BitPerColor
            1 => 0x00_70_C0_20u32, // GOP PixelBlueGreenRedReserved8BitPerColor
            _ => 0,
        };
        if pixel != 0 {
            let stride = boot.framebuffer_stride as usize;
            let width = boot.framebuffer_width as usize;
            let height = boot.framebuffer_height as usize;
            let framebuffer = boot.framebuffer_address as *mut u32;
            let bar_height = core::cmp::min(height, 12);
            for y in 0..bar_height {
                for x in 0..width {
                    // SAFETY: The GOP framebuffer dimensions and base are supplied by UEFI.
                    unsafe { core::ptr::write_volatile(framebuffer.add(y * stride + x), pixel) };
                }
            }
            unsafe { debug_write(b"SYSTEMES: GOP framebuffer initialized\r\n") };
        }
    }

    #[cfg(feature = "qemu-acpi-madt-test")]
    let mut acpi_madt_discovered = false;
    if boot.acpi_rsdp_address != 0 {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP discovered\\r\\n"); }
        // SAFETY: The RSDP address comes from the UEFI configuration table and the
        // active kernel identity map covers the firmware ACPI tables below 512 GiB.
        match unsafe { systemes_kernel::acpi::discover_madt(boot.acpi_rsdp_address) } {
            Ok(madt) if madt.local_apic_address != 0
                && madt.local_apic_count > 0 && madt.io_apic_count > 0 => {
                unsafe { debug_write(b"SYSTEMES: ACPI MADT parsed; Local APIC and IOAPIC topology discovered\\r\\n"); }
                #[cfg(feature = "qemu-acpi-madt-test")]
                { acpi_madt_discovered = true; }
            }
            Ok(_) => unsafe { debug_write(b"SYSTEMES: ACPI MADT incomplete; APIC topology unavailable\\r\\n"); },
            Err(_) => unsafe { debug_write(b"SYSTEMES: ACPI MADT unavailable or invalid\\r\\n"); },
        }
    } else {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP unavailable\\r\\n"); }
    }
    #[cfg(feature = "qemu-acpi-madt-test")]
    if !acpi_madt_discovered {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: QEMU ACPI MADT discovery test failed\\r\\n"); }
        halt();
    }

    unsafe { debug_write(b"SYSTEMES_KERNEL_OK\r\n") };
    #[cfg(feature = "qemu-nx-execute-test")]
    {
        let test_frame = match frame_allocator.allocate_frame() {
            Some(address) => address,
            None => { unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX test frame unavailable\\r\\n"); } halt(); }
        };
        let test_end = match test_frame.checked_add(systemes_kernel::memory::PAGE_SIZE) {
            Some(end) if test_frame < systemes_kernel::paging::IDENTITY_MAP_BYTES => end,
            _ => { unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid NX test frame\\r\\n"); } halt(); }
        };
        let test_region = [MemoryRegion { start: test_frame, end: test_end, permissions: PagePermissions::ReadWriteNoExecute }];
        if unsafe { systemes_kernel::paging::protect_regions(&mut frame_allocator, page_table_root, &test_region) }.is_err()
            || unsafe { systemes_kernel::paging::verify_regions(page_table_root, &test_region) }.is_err() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX test mapping verification failed\\r\\n"); }
            halt();
        }
        // SAFETY: The allocated identity-mapped frame is deliberately RW+NX for this fault test.
        unsafe { core::ptr::write_volatile(test_frame as *mut u8, 0xC3); }
        unsafe { debug_write(b"SYSTEMES_TEST: attempting instruction fetch from NX page\\r\\n"); }
        // SAFETY: This call must fault with #PF(P=1, I/D=1), error code 0x11.
        let test_function: extern "C" fn() = unsafe { core::mem::transmute(test_frame as usize) };
        test_function();
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX execution was not blocked\\r\\n"); }
        halt();
    }
    #[cfg(feature = "qemu-rx-write-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: attempting write to RX kernel text\\r\\n"); }
        // SAFETY: This deliberate volatile write must fault with #PF(P=1, W/R=1), error code 0x3.
        unsafe { core::ptr::write_volatile(core::ptr::addr_of!(__text_start) as *mut u8, 0x90); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: RX text write was not blocked\\r\\n"); }
        halt();
    }
    #[cfg(feature = "qemu-exception-test")]
    {
        // The CI boot test deliberately raises #BP to prove the installed IDT dispatches.
        unsafe { asm!("int3", options(nomem, nostack)) };
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: breakpoint handler returned\\r\\n") };
        halt();
    }
    halt();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: panic\r\n"); }
    halt();
}
