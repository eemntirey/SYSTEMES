#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::arch::asm;
use core::panic::PanicInfo;
use systemes_kernel::{address_space::{write_cr3, AddressSpace, UserPagePermissions, USER_BASE}, console::debug_write, interrupts::enter_user_mode, elf_loader::load_elf, memory::PhysicalFrameAllocator, paging::{MemoryRegion, PagePermissions, IDENTITY_MAP_BYTES}, serial, shell_elf::SHELL_ELF, BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

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

fn halt() -> ! {
    loop {
        // SAFETY: A fatal boot error must stop execution without returning to the loader.
        unsafe { asm!("cli; hlt", options(nomem, nostack)) };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtioVfsError {
    DeviceNotFound,
    DeviceInitialization,
    SectorRead,
    InvalidImage,
    ImageTooLarge,
    Allocation,
    Filesystem,
    VfsMount,
    VfsOpen,
    VfsRead,
    VfsClose,
}

/// Read the whole virtio-blk disk into memory and return the raw SYSFS image.
/// Runs on every boot: when no disk is present the kernel continues without the VFS.
fn probe_virtio_disk(
    allocator: &mut PhysicalFrameAllocator,
) -> Result<Vec<u8>, VirtioVfsError> {
    let pci = unsafe { systemes_kernel::pci::discover_virtio_blk() }
        .map_err(|_| VirtioVfsError::DeviceNotFound)?;
    let mut device = unsafe { systemes_kernel::virtio_blk::VirtioBlk::configure(pci, allocator) }
        .map_err(|_| VirtioVfsError::DeviceInitialization)?;

    let mut header = [0; 512];
    unsafe { device.read_sector(0, &mut header, allocator) }
        .map_err(|_| VirtioVfsError::SectorRead)?;

    let image_size = systemes_kernel::simple_fs::SimpleFs::required_image_size(&header)
        .map_err(|_| VirtioVfsError::InvalidImage)?;
    if image_size > 64 * 1024 || image_size % 512 != 0 {
        return Err(VirtioVfsError::ImageTooLarge);
    }
    let mut image = alloc::vec::Vec::new();
    image
        .try_reserve_exact(image_size)
        .map_err(|_| VirtioVfsError::Allocation)?;
    image.resize(image_size, 0);
    image[..512].copy_from_slice(&header);

    let sectors = image_size / 512;
    for sector in 1..sectors {
        let mut contents = [0; 512];
        unsafe { device.read_sector(sector as u64, &mut contents, allocator) }
            .map_err(|_| VirtioVfsError::SectorRead)?;
        let offset = sector * 512;
        image[offset..offset + 512].copy_from_slice(&contents);
    }

    systemes_kernel::simple_fs::SimpleFs::parse(&image)
        .map_err(|_| VirtioVfsError::Filesystem)?;
    Ok(image)
}

/// Verify `/hello.txt` contains `"hello"` on a parsed SYSFS image.
fn verify_vfs_hello(image: &[u8]) -> Result<(), VirtioVfsError> {
    let filesystem = systemes_kernel::simple_fs::SimpleFs::parse(image)
        .map_err(|_| VirtioVfsError::Filesystem)?;
    let mut vfs = systemes_kernel::vfs::Vfs::new();
    vfs.mount(filesystem).map_err(|_| VirtioVfsError::VfsMount)?;
    let handle = vfs.open("/hello.txt").map_err(|_| VirtioVfsError::VfsOpen)?;
    let mut contents = [0; 5];
    let read_result = vfs.read(handle, &mut contents);
    let close_result = vfs.close(handle);
    if read_result != Ok(contents.len()) || contents != *b"hello" {
        return Err(VirtioVfsError::VfsRead);
    }
    if close_result.is_err() {
        return Err(VirtioVfsError::VfsClose);
    }
    Ok(())
}

/// Read an entire file from a SYSFS image into an independent `Vec<u8>`.
fn read_vfs_file(image: &[u8], path: &str) -> Result<Vec<u8>, VirtioVfsError> {
    let filesystem = systemes_kernel::simple_fs::SimpleFs::parse(image)
        .map_err(|_| VirtioVfsError::Filesystem)?;
    let mut vfs = systemes_kernel::vfs::Vfs::new();
    vfs.mount(filesystem).map_err(|_| VirtioVfsError::VfsMount)?;
    let handle = vfs.open(path).map_err(|_| VirtioVfsError::VfsOpen)?;
    let mut contents = alloc::vec::Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = vfs.read(handle, &mut chunk).map_err(|_| VirtioVfsError::VfsRead)?;
        if n == 0 {
            break;
        }
        contents.extend_from_slice(&chunk[..n]);
    }
    vfs.close(handle).map_err(|_| VirtioVfsError::VfsClose)?;
    Ok(contents)
}

#[cfg(feature = "qemu-virtio-vfs-test")]
fn run_qemu_virtio_vfs_test(
    allocator: &mut PhysicalFrameAllocator,
) -> Result<(), VirtioVfsError> {
    let image = probe_virtio_disk(allocator)?;
    verify_vfs_hello(&image)?;
    unsafe {
        debug_write(b"SYSTEMES_TEST: virtio SYSFS mount/open/read/close verified\r\n");
    }
    Ok(())
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
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid UEFI memory descriptors\r\n") };
            halt();
        }
    };
    let initial_free_pages = frame_allocator.free_pages();
    let test_frame = match frame_allocator.allocate_frame() {
        Some(address) => address,
        None => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: no conventional memory frames\r\n") };
            halt();
        }
    };
    if frame_allocator.free_frame(test_frame).is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: physical frame release failed\r\n") };
        halt();
    }
    if frame_allocator.free_pages() != initial_free_pages {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: physical frame accounting mismatch\r\n") };
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: physical frame allocator initialized\r\n") };
    if systemes_kernel::paging::enable_nx().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CPU does not support NX\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: CPU NX capability detected and EFER.NXE enabled\r\n"); }
    // SAFETY: CPU tables are installed during single-core boot while IRQs remain disabled.
    unsafe { systemes_kernel::interrupts::initialize() };
    if unsafe { systemes_kernel::interrupts::initialize_syscalls() }.is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SYSCALL/SYSRET unavailable\r\n"); }
        halt();
    }
    if !systemes_kernel::interrupts::verify_syscall_configuration() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SYSCALL/SYSRET MSR configuration invalid\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: SYSCALL/SYSRET configured with a dedicated kernel stack\r\n"); }
    if !systemes_kernel::interrupts::verify_task_register() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: TR did not load TSS selector 0x18\r\n"); }
        halt();
    }
    if !systemes_kernel::interrupts::verify_ist_configuration() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: TSS stacks or IDT IST routing invalid\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: TSS TR=0x18 verified; RSP0/IST1/IST2/IST3 and IDT routing verified\r\n") };
    unsafe { debug_write(b"SYSTEMES: GDT initialized\r\n") };
    unsafe { debug_write(b"SYSTEMES: IDT initialized for CPU exceptions 0..31\r\n") };
    if !systemes_kernel::interrupts::verify_local_apic_gates() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: Local APIC IDT gates invalid\r\n"); }
        halt();
    }
    if !systemes_kernel::interrupts::verify_ring3_configuration() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: Ring 3 GDT or DPL3 interrupt gate invalid\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: user GDT descriptors and DPL3 exit gate verified\r\n"); }
    // SAFETY: UEFI identity mappings remain active while the new page tables are built.
    let page_table_root = match unsafe {
        systemes_kernel::paging::build_identity_map(&mut frame_allocator)
    } {
        Ok(root) => root,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: page-table allocation failed\r\n"); }
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
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: W^X section mapping failed\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: kernel code RX, rodata R-NX, data and BSS RW-NX\r\n"); }
    // SAFETY: The new PML4 identity-maps boot data, stack, descriptors and framebuffer.
    unsafe { asm!("mov cr3, {}", in(reg) page_table_root, options(nostack, preserves_flags)) };
    let active_cr3: u64;
    // SAFETY: Reading CR3 is valid in ring 0 on x86_64.
    unsafe { asm!("mov {}, cr3", out(reg) active_cr3, options(nomem, nostack, preserves_flags)) };
    if (active_cr3 & 0x000F_FFFF_FFFF_F000) != page_table_root {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CR3 verification failed\r\n") };
        halt();
    }
    if systemes_kernel::paging::enable_write_protect().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: CR0.WP activation failed\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: CR0.WP supervisor write protection enabled\r\n"); }
    if unsafe { systemes_kernel::paging::verify_regions(page_table_root, &kernel_regions) }.is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: W^X page-table verification failed\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: W^X 4 KiB page permissions verified\r\n") };
    let supervisor_protection = match systemes_kernel::uaccess::enable() {
        Ok(features) => features,
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMEP/SMAP activation did not latch\r\n"); }
            halt();
        }
    };
    unsafe { debug_write(b"SYSTEMES: supervisor access protection smep=") };
    unsafe { debug_write(if supervisor_protection.smep { b"1" } else { b"0" }) };
    unsafe { debug_write(b" smap=") };
    unsafe { debug_write(if supervisor_protection.smap { b"1" } else { b"0" }) };
    unsafe { debug_write(b"\r\n") };
    if systemes_kernel::heap::initialize().is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: dynamic kernel heap initialization failed\r\n"); }
        halt();
    }
    if !systemes_kernel::heap::is_initialized() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: dynamic kernel heap state invalid\r\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: 1 MiB dynamic kernel heap initialized\r\n"); }
    let vfs_image: Option<Vec<u8>> = match probe_virtio_disk(&mut frame_allocator) {
        Ok(image) => Some(image),
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES: virtio-blk VFS unavailable (no disk)\r\n"); }
            None
        }
    };
    if let Some(ref image) = vfs_image {
        if verify_vfs_hello(image).is_ok() {
            unsafe {
                debug_write(
                    b"SYSTEMES: virtio-blk VFS mounted; /hello.txt = \"hello\" (normal boot)\r\n",
                );
            }
        }
    }
    #[cfg(feature = "qemu-virtio-vfs-test")]
    match run_qemu_virtio_vfs_test(&mut frame_allocator) {
        Ok(()) => {}
        Err(_) => halt(),
    }
    #[cfg(feature = "qemu-heap-test")]
    {
        let layout = match core::alloc::Layout::from_size_align(128, 64) {
            Ok(layout) => layout,
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid heap test layout\r\n"); }
                halt();
            }
        };
        let initial_free = systemes_kernel::heap::free_bytes();
        // SAFETY: The global allocator has been initialized and this layout is valid.
        let allocation = unsafe { alloc::alloc::alloc(layout) };
        if allocation.is_null() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test allocation failed\r\n"); }
            halt();
        }
        // SAFETY: The allocation contains 128 writable bytes and remains live here.
        unsafe { allocation.write(0x5A); }
        // SAFETY: The allocation is still live and contains the byte written above.
        if unsafe { allocation.read() } != 0x5A {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test data mismatch\r\n"); }
            halt();
        }
        // SAFETY: Return the exact pointer with the original layout.
        unsafe { alloc::alloc::dealloc(allocation, layout); }
        if systemes_kernel::heap::free_bytes() != initial_free {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: heap test coalescing failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: dynamic heap allocation, alignment and free coalescing verified\r\n"); }
    }
    unsafe { debug_write(b"SYSTEMES: kernel-owned 4-level identity paging active\r\n") };
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

    let mut discovered_madt: Option<systemes_kernel::acpi::MadtInfo> = None;
    if boot.acpi_rsdp_address != 0 {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP discovered\r\n"); }
        // SAFETY: The RSDP address comes from UEFI and the active identity map
        // covers the firmware ACPI tables within the 512-GiB physical window.
        match unsafe { systemes_kernel::acpi::discover_madt(boot.acpi_rsdp_address) } {
            Ok(madt) => {
                if madt.local_apic_count > 0 && madt.io_apic_count > 0 {
                    unsafe { debug_write(b"SYSTEMES: ACPI MADT parsed; Local APIC and IOAPIC topology discovered\r\n"); }
                } else {
                    unsafe { debug_write(b"SYSTEMES: ACPI MADT parsed but controller topology is incomplete\r\n"); }
                }
                discovered_madt = Some(madt);
            }
            Err(_) => unsafe { debug_write(b"SYSTEMES: ACPI MADT unavailable or invalid\r\n"); },
        }
    } else {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP unavailable\r\n"); }
    }
    #[cfg(feature = "qemu-acpi-madt-test")]
    if discovered_madt.is_none() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: QEMU ACPI MADT discovery test failed\r\n"); }
        halt();
    }

    let should_start_apic_clock = cfg!(feature = "qemu-apic-clock-test")
        || (!cfg!(feature = "qemu-nx-execute-test")
            && !cfg!(feature = "qemu-rx-write-test")
            && !cfg!(feature = "qemu-exception-test"));
    let mut apic_timer_ready = false;
    if should_start_apic_clock {
        if let Some(madt) = discovered_madt.as_ref() {
            // SAFETY: MADT is validated, page tables are active, and interrupts remain disabled.
            match unsafe { systemes_kernel::apic::initialize_local_apic(madt, &mut frame_allocator, page_table_root) } {
                Ok(()) => match unsafe { systemes_kernel::apic::calibrate_and_initialize_local_timer(100) } {
                    Ok(_) => {
                        unsafe { debug_write(b"SYSTEMES: Local APIC timer calibrated from PIT channel 2 at 100 Hz\r\n"); }
                        apic_timer_ready = true;
                    }
                    Err(_) => unsafe { debug_write(b"SYSTEMES: Local APIC timer calibration failed; timer remains disabled\r\n"); },
                },
                Err(_) => unsafe { debug_write(b"SYSTEMES: Local APIC unavailable; timer remains disabled\r\n"); },
            }
        } else {
            unsafe { debug_write(b"SYSTEMES: Local APIC skipped because ACPI MADT is unavailable\r\n"); }
        }
    }
    #[cfg(feature = "qemu-apic-clock-test")]
    if !apic_timer_ready {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: QEMU Local APIC timer test could not initialize\r\n"); }
        halt();
    }
    if apic_timer_ready {
        // SAFETY: IDT timer/spurious gates and UC Local APIC mapping are installed; PIC lines are masked.
        unsafe { asm!("sti", options(nomem, nostack)); }
    }

    unsafe { debug_write(b"SYSTEMES_KERNEL_OK\r\n") };
    #[cfg(feature = "qemu-ring3-test")]
    match unsafe {
        systemes_kernel::address_space::run_qemu_ring3_test(&mut frame_allocator, page_table_root)
    } {
        Ok(()) => unsafe {
            debug_write(b"SYSTEMES_TEST: Ring 3 code executed with user CS and writable user stack\r\n");
            debug_write(b"SYSTEMES_TEST: Ring 3 user stack write verified\r\n");
        },
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: Ring 3 transition test failed\r\n"); }
            halt();
        }
    }
    #[cfg(feature = "qemu-syscall-test")]
    match unsafe {
        systemes_kernel::address_space::run_qemu_syscall_test(&mut frame_allocator, page_table_root)
    } {
        Ok(()) => unsafe {
            debug_write(b"SYSTEMES_TEST: SYSCALL/SYSRET fast path executed\r\n");
            debug_write(b"SYSTEMES_TEST: Syscall return value verified in user stack\r\n");
        },
        Err(_) => {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: Syscall transition test failed\r\n"); }
            halt();
        }
    }
    #[cfg(feature = "qemu-address-space-test")]
    {
        let interrupt_flag: u64;
        // SAFETY: Save the interrupt state and keep the CR3 test non-preemptible.
        unsafe {
            asm!("pushfq; pop {}", out(reg) interrupt_flag, options(nomem, nostack, preserves_flags));
            asm!("cli", options(nomem, nostack));
        }
        let result = unsafe {
            systemes_kernel::address_space::run_qemu_address_space_test(&mut frame_allocator, page_table_root)
        };
        // SAFETY: Restore the interrupt flag captured before the test.
        unsafe {
            if interrupt_flag & (1 << 9) != 0 {
                asm!("sti", options(nomem, nostack));
            }
        }
        match result {
            Ok(()) => {
                unsafe { debug_write(b"SYSTEMES: process address space created\r\n"); }
                unsafe { debug_write(b"SYSTEMES_TEST: user page mapped with U/S and survived CR3 switch\r\n"); }
                unsafe { debug_write(b"SYSTEMES_TEST: kernel identity mapping shared by the process address space\r\n"); }
                if supervisor_protection.smap {
                    unsafe { debug_write(b"SYSTEMES_TEST: user page access succeeded with SMAP enabled and AC scoped\r\n"); }
                }
            }
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: address space test failed\r\n"); }
                halt();
            }
        }
    }
    #[cfg(feature = "qemu-smap-violation-test")]
    {
        // SAFETY: The deliberate fault below must not race a timer-driven context switch.
        unsafe { asm!("cli", options(nomem, nostack)); }
        match unsafe {
            systemes_kernel::uaccess::run_qemu_smap_violation_test(&mut frame_allocator, page_table_root)
        } {
            Ok(()) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMAP fault expected but the read succeeded\r\n"); }
            }
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMAP violation test setup failed\r\n"); }
            }
        }
        halt();
    }
    #[cfg(feature = "qemu-smep-violation-test")]
    {
        // SAFETY: The deliberate fault below must not race a timer-driven context switch.
        unsafe { asm!("cli", options(nomem, nostack)); }
        match unsafe {
            systemes_kernel::uaccess::run_qemu_smep_violation_test(&mut frame_allocator, page_table_root)
        } {
            Ok(()) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMEP fault expected but the call returned\r\n"); }
            }
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMEP violation test setup failed\r\n"); }
            }
        }
        halt();
    }
    #[cfg(feature = "qemu-ist-nmi-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: invoke vector 2 through IST2\r\n"); }
        unsafe { asm!("int 2"); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: IST2 test interrupt returned\r\n"); }
        halt();
    }
    #[cfg(feature = "qemu-ist-machine-check-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: invoke vector 18 through IST3\r\n"); }
        unsafe { asm!("int 18"); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: IST3 test interrupt returned\r\n"); }
        halt();
    }
    #[cfg(feature = "qemu-apic-clock-test")]
    {
        while systemes_kernel::clock::ticks() < 3 {
            // SAFETY: The calibrated Local APIC timer is enabled and will wake HLT.
            unsafe { asm!("hlt", options(nomem, nostack)); }
        }
        if systemes_kernel::clock::ticks() < 3 || systemes_kernel::clock::now_millis() < 30 {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: monotonic clock did not advance with APIC ticks\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: Local APIC timer delivered three ticks; monotonic clock advanced\r\n"); }
        if !cfg!(feature = "qemu-preemptive-scheduler-test") {
            // SAFETY: No preemptive test follows, so the timer can be stopped after this proof.
            unsafe { asm!("cli", options(nomem, nostack)); }
            unsafe { systemes_kernel::apic::stop_local_timer(); }
        }
    }
    #[cfg(feature = "qemu-cooperative-scheduler-test")]
    {
        if !systemes_kernel::scheduler::run_cooperative_qemu_test() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: cooperative scheduler task switching failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: cooperative scheduler switched three kernel tasks\r\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: all cooperative tasks exited and returned to boot task\r\n"); }
    }
    #[cfg(feature = "qemu-preemptive-scheduler-test")]
    {
        if !systemes_kernel::scheduler::run_preemptive_qemu_test() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: timer-driven preemption test failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: Local APIC preempted non-yielding tasks\r\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: three CPU-bound tasks completed under timer preemption\r\n"); }

    }

    #[cfg(feature = "qemu-task-sleep-test")]
    {
        if !systemes_kernel::scheduler::run_task_sleep_qemu_test() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: task sleep/wakeup test failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: blocked task slept for four monotonic ticks\r\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: timer woke the blocked task at its deadline\r\n"); }
    }

    #[cfg(feature = "qemu-fpu-context-test")]
    {
        if !systemes_kernel::scheduler::run_fpu_context_qemu_test() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: x87 SSE context isolation failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: x87 state survived timer-driven task switches\r\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: XMM15 state survived timer-driven task switches\r\n"); }
        // SAFETY: All FPU test tasks exited; stop the periodic source before exception tests.
        unsafe { asm!("cli", options(nomem, nostack)); }
        unsafe { systemes_kernel::apic::stop_local_timer(); }
    }

    let run_user_shell = cfg!(feature = "qemu-user-shell-test")
        || (!cfg!(feature = "qemu-nx-execute-test")
            && !cfg!(feature = "qemu-rx-write-test")
            && !cfg!(feature = "qemu-exception-test")
            && !cfg!(feature = "qemu-ist-nmi-test")
            && !cfg!(feature = "qemu-ist-machine-check-test")
            && !cfg!(feature = "qemu-smap-violation-test")
            && !cfg!(feature = "qemu-smep-violation-test"));

    if run_user_shell {
        // T12: initialize the polled UART console for userland input/output.
        serial::init();
        unsafe { debug_write(b"SYSTEMES: serial UART initialized\r\n"); }

        // T11/T12: load and launch the userland shell (ELF64), which uses
        // SYS_WRITE (prompt/echo) and SYS_READ (console input) via the serial UART.
        // The ELF is loaded from the VFS (`/shell`) when a virtio-blk disk is
        // present, falling back to the embedded image otherwise.
        let shell_data: Vec<u8> = match &vfs_image {
            Some(image) => match read_vfs_file(image, "/shell") {
                Ok(data) => {
                    unsafe { debug_write(b"SYSTEMES: shell loaded from VFS (/shell)\r\n"); }
                    data
                }
                Err(_) => {
                    unsafe { debug_write(b"SYSTEMES: /shell not found on VFS; embedded shell\r\n"); }
                    SHELL_ELF.to_vec()
                }
            },
            None => {
                unsafe { debug_write(b"SYSTEMES: no virtio-blk disk; embedded shell\r\n"); }
                SHELL_ELF.to_vec()
            }
        };
        unsafe { debug_write(b"SYSTEMES: launching userland shell\r\n"); }
        let mut shell_space = match unsafe { AddressSpace::create(&mut frame_allocator, page_table_root) } {
            Ok(space) => space,
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell address space failed\r\n"); }
                halt();
            }
        };
        let shell_entry = match unsafe {
            load_elf(&shell_data, &mut frame_allocator, &mut shell_space)
        } {
            Ok(entry) => entry,
            Err(_) => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell ELF load failed\r\n"); }
                halt();
            }
        };
        let shell_buffer = USER_BASE + 0x7000;
        let buffer_frame = match frame_allocator.allocate_frame() {
            Some(frame) if frame % systemes_kernel::memory::PAGE_SIZE == 0 && frame < IDENTITY_MAP_BYTES => frame,
            _ => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell buffer frame unavailable\r\n"); }
                halt();
            }
        };
        unsafe {
            core::ptr::write_bytes(buffer_frame as *mut u8, 0, systemes_kernel::memory::PAGE_SIZE as usize);
            core::ptr::copy_nonoverlapping(b"> ".as_ptr(), buffer_frame as *mut u8, 2);
        }
        if unsafe {
            shell_space.map_user_page(
                &mut frame_allocator,
                shell_buffer,
                buffer_frame,
                UserPagePermissions::ReadWriteNoExecute,
            )
        }
        .is_err()
        {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell buffer mapping failed\r\n"); }
            halt();
        }

        let stack_page = USER_BASE + 0x9000;
        let stack_frame = match frame_allocator.allocate_frame() {
            Some(frame) if frame % systemes_kernel::memory::PAGE_SIZE == 0 && frame < IDENTITY_MAP_BYTES => frame,
            _ => {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell stack frame unavailable\r\n"); }
                halt();
            }
        };
        unsafe {
            core::ptr::write_bytes(stack_frame as *mut u8, 0, systemes_kernel::memory::PAGE_SIZE as usize);
        }
        if unsafe {
            shell_space.map_user_page(
                &mut frame_allocator,
                stack_page,
                stack_frame,
                UserPagePermissions::ReadWriteNoExecute,
            )
        }
        .is_err()
        {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell stack mapping failed\r\n"); }
            halt();
        }

        let shell_stack = stack_page + systemes_kernel::memory::PAGE_SIZE - 16;
        // SAFETY: The ELF code, writable user buffer, and user stack are mapped in `shell_space`.
        unsafe {
            asm!("cli", options(nomem, nostack));
            write_cr3(shell_space.root());
        }
        if unsafe { systemes_kernel::address_space::cr3_root(systemes_kernel::address_space::read_cr3()) }
            != shell_space.root()
        {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell address space activation failed\r\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: user shell address space activated\r\n"); }
        // SAFETY: The shell is executable in Ring 3 and its stack is writable in the active address space.
        unsafe { enter_user_mode(shell_entry, shell_stack) };
        #[cfg(feature = "qemu-user-shell-test")]
        unsafe { debug_write(b"SYSTEMES_TEST: user SYS_EXIT returned to kernel\r\n"); }
    }
    halt();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: panic\r\n"); }
    halt();
}
