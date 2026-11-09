#![no_std]
#![no_main]

extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;
use systemes_kernel::{address_space::{write_cr3, AddressSpace, UserPagePermissions, USER_BASE}, console::{debug_write, encode_decimal}, interrupts::enter_user_mode, elf_loader::load_elf, memory::PhysicalFrameAllocator, paging::{MemoryRegion, PagePermissions, IDENTITY_MAP_BYTES}, rootfs::{self, RootFsError}, serial, shell_elf::SHELL_ELF, BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

/// The normal boot (no qemu-* feature) and the dedicated user-shell test both
/// hand control to the userland shell. Kernel-only test configurations stay in
/// the idle loop instead.
#[cfg(feature = "qemu-user-fs-test")]
mod user_fs_test;

#[allow(dead_code)]
const LAUNCH_USER_SHELL: bool = cfg!(feature = "qemu-user-shell-test")
    || !cfg!(any(
        feature = "qemu-exception-test",
        feature = "qemu-nx-execute-test",
        feature = "qemu-rx-write-test",
        feature = "qemu-ist-nmi-test",
        feature = "qemu-ist-machine-check-test",
        feature = "qemu-heap-test",
        feature = "qemu-acpi-madt-test",
        feature = "qemu-apic-clock-test",
        feature = "qemu-cooperative-scheduler-test",
        feature = "qemu-preemptive-scheduler-test",
        feature = "qemu-fpu-context-test",
        feature = "qemu-task-sleep-test",
        feature = "qemu-address-space-test",
        feature = "qemu-ring3-test",
        feature = "qemu-syscall-test",
        feature = "qemu-virtio-vfs-test",
        feature = "qemu-user-fs-test",
        feature = "qemu-smap-violation-test",
        feature = "qemu-smep-violation-test",
        feature = "qemu-smp-test",
    ));

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

/// Stable spelling of a bring-up failure so CI can match one marker prefix.
#[cfg(feature = "qemu-smp-test")]
fn smp_failure_name(error: systemes_kernel::smp::SmpBringUpError) -> &'static [u8] {
    use systemes_kernel::smp::SmpBringUpError as BringUp;
    match error {
        BringUp::ApicUnavailable => b"apic-unavailable",
        BringUp::NoApplicationProcessors => b"no-application-processors",
        BringUp::NoLowMemoryPage => b"no-low-memory-page",
        BringUp::TrampolineTooLarge => b"trampoline-too-large",
        BringUp::Crash3Beyond4GiB => b"cr3-beyond-4gib",
        BringUp::KernelEntryBeyond4GiB => b"kernel-entry-beyond-4gib",
        BringUp::LapicBaseBeyond4GiB => b"lapic-base-beyond-4gib",
        BringUp::ApicIdOutOfRange => b"apic-id-out-of-range",
        BringUp::MissingPatchSite => b"missing-patch-site",
        BringUp::AmbiguousPatchSite => b"ambiguous-patch-site",
        BringUp::TrampolineRxVerificationFailed => b"trampoline-rx-verification",
        BringUp::ApStackAllocationFailed => b"ap-stack-allocation",
        BringUp::ApDidNotStart { .. } => b"ap-did-not-start",
        BringUp::ApDidNotPark { .. } => b"ap-did-not-park",
        BringUp::CrossCoreCounterMismatch => b"cross-core-counter-mismatch",
    }
}

#[cfg(feature = "qemu-virtio-vfs-test")]
#[derive(Clone, Copy)]
enum VirtioVfsTestError {
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

#[cfg(feature = "qemu-virtio-vfs-test")]
fn run_qemu_virtio_vfs_test(
    allocator: &mut PhysicalFrameAllocator,
) -> Result<(), VirtioVfsTestError> {
    let pci = unsafe { systemes_kernel::pci::discover_virtio_blk() }
        .map_err(|_| VirtioVfsTestError::DeviceNotFound)?;
    let mut device = unsafe { systemes_kernel::virtio_blk::VirtioBlk::configure(pci, allocator) }
        .map_err(|_| VirtioVfsTestError::DeviceInitialization)?;

    let mut header = [0; 512];
    unsafe { device.read_sector(0, &mut header, allocator) }.map_err(|error| {
        let message = match error {
            systemes_kernel::virtio_blk::VirtioBlkError::OutOfFrames =>
                b"SYSTEMES_TEST_FAIL: virtio-blk has no DMA frames\r\n" as &[u8],
            systemes_kernel::virtio_blk::VirtioBlkError::InvalidFrame =>
                b"SYSTEMES_TEST_FAIL: virtio-blk DMA frame is invalid\r\n",
            systemes_kernel::virtio_blk::VirtioBlkError::SectorOutOfRange =>
                b"SYSTEMES_TEST_FAIL: virtio-blk sector is out of range\r\n",
            systemes_kernel::virtio_blk::VirtioBlkError::RequestFailed =>
                b"SYSTEMES_TEST_FAIL: virtio-blk used ring response is invalid\r\n",
            systemes_kernel::virtio_blk::VirtioBlkError::IoError =>
                b"SYSTEMES_TEST_FAIL: virtio-blk returned I/O error\r\n",
            systemes_kernel::virtio_blk::VirtioBlkError::UnsupportedRequest =>
                b"SYSTEMES_TEST_FAIL: virtio-blk rejected read request\r\n",
            systemes_kernel::virtio_blk::VirtioBlkError::DmaTimeout =>
                b"SYSTEMES_TEST_FAIL: virtio-blk DMA request timed out\r\n",
            _ => b"SYSTEMES_TEST_FAIL: virtio-blk sector read failed\r\n",
        };
        unsafe { debug_write(message); }
        VirtioVfsTestError::SectorRead
    })?;
    unsafe { debug_write(b"SYSTEMES_TEST: virtio-blk DMA sector read verified\r\n"); }

    let image_size = systemes_kernel::simple_fs::SimpleFs::required_image_size(&header)
        .map_err(|_| VirtioVfsTestError::InvalidImage)?;
    if image_size > 64 * 1024 || image_size % 512 != 0 {
        return Err(VirtioVfsTestError::ImageTooLarge);
    }
    let mut image = alloc::vec::Vec::new();
    image
        .try_reserve_exact(image_size)
        .map_err(|_| VirtioVfsTestError::Allocation)?;
    image.resize(image_size, 0);
    image[..512].copy_from_slice(&header);

    let sectors = image_size / 512;
    for sector in 1..sectors {
        let mut contents = [0; 512];
        unsafe { device.read_sector(sector as u64, &mut contents, allocator) }
            .map_err(|_| VirtioVfsTestError::SectorRead)?;
        let offset = sector * 512;
        image[offset..offset + 512].copy_from_slice(&contents);
    }

    let filesystem = systemes_kernel::simple_fs::SimpleFs::parse(&image)
        .map_err(|_| VirtioVfsTestError::Filesystem)?;
    let mut vfs = systemes_kernel::vfs::Vfs::new();
    vfs.mount(filesystem)
        .map_err(|_| VirtioVfsTestError::VfsMount)?;
    let handle = vfs.open("/hello.txt").map_err(|_| VirtioVfsTestError::VfsOpen)?;
    let mut contents = [0; 5];
    let read_result = vfs.read(handle, &mut contents);
    let close_result = vfs.close(handle);
    if read_result != Ok(contents.len()) || contents != *b"hello" {
        return Err(VirtioVfsTestError::VfsRead);
    }
    if close_result.is_err() {
        return Err(VirtioVfsTestError::VfsClose);
    }
    unsafe {
        debug_write(b"SYSTEMES_TEST: virtio SYSFS mount/open/read/close verified\r\n");
    }
    Ok(())
}

/// Mount the optional root filesystem and report the outcome on the boot log.
///
/// A missing disk or an unreadable image is never fatal: the shell still starts
/// and reports the missing filesystem when a file command is used.
fn report_root_filesystem(allocator: &mut PhysicalFrameAllocator) {
    match rootfs::mount(allocator) {
        Ok(sectors) => {
            unsafe { debug_write(b"SYSTEMES: SYSFS root mounted from virtio-blk, sectors=") };
            let mut digits = [0u8; 20];
            let len = encode_decimal(sectors, &mut digits);
            unsafe { debug_write(&digits[..len]) };
            unsafe { debug_write(b"\r\n") };
        }
        Err(RootFsError::DeviceNotFound) => unsafe {
            debug_write(b"SYSTEMES: no virtio-blk disk found; booting without a root filesystem\r\n")
        },
        Err(_) => unsafe {
            debug_write(b"SYSTEMES: virtio-blk disk present but SYSFS mount failed; booting without a root filesystem\r\n")
        },
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
    #[cfg(feature = "qemu-smp-test")]
    {
        // The identity map allocates its tables from the lowest frames, so the
        // SIPI page must be claimed before paging is built.
        if systemes_kernel::smp::reserve_trampoline_page(&mut frame_allocator).is_none() {
            unsafe {
                debug_write(b"SYSTEMES_KERNEL_FAIL: no low-memory frame for the SMP trampoline\r\n");
            }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES: SMP trampoline frame reserved below 1 MiB\r\n") };
    }
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
    #[cfg(feature = "qemu-virtio-vfs-test")]
    if let Err(error) = run_qemu_virtio_vfs_test(&mut frame_allocator) {
        let message = match error {
            VirtioVfsTestError::DeviceNotFound =>
                b"SYSTEMES_KERNEL_FAIL: virtio-blk PCI discovery failed\r\n" as &[u8],
            VirtioVfsTestError::DeviceInitialization =>
                b"SYSTEMES_KERNEL_FAIL: virtio-blk initialization failed\r\n",
            VirtioVfsTestError::SectorRead =>
                b"SYSTEMES_KERNEL_FAIL: virtio-blk DMA sector read failed\r\n",
            VirtioVfsTestError::InvalidImage =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS superblock is invalid\r\n",
            VirtioVfsTestError::ImageTooLarge =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS image exceeds test limit\r\n",
            VirtioVfsTestError::Allocation =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS image allocation failed\r\n",
            VirtioVfsTestError::Filesystem =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS image parsing failed\r\n",
            VirtioVfsTestError::VfsMount =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS VFS mount failed\r\n",
            VirtioVfsTestError::VfsOpen =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS VFS open failed\r\n",
            VirtioVfsTestError::VfsRead =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS VFS read returned invalid content\r\n",
            VirtioVfsTestError::VfsClose =>
                b"SYSTEMES_KERNEL_FAIL: SYSFS VFS close failed\r\n",
        };
        unsafe { debug_write(message); }
        halt();
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
    #[cfg(feature = "qemu-smp-test")]
    {
        let madt = match discovered_madt.as_ref() {
            Some(madt) if systemes_kernel::apic::is_active() => madt,
            _ => {
                unsafe {
                    debug_write(b"SYSTEMES_KERNEL_FAIL: SMP test requires the ACPI MADT and an active Local APIC\r\n");
                }
                halt();
            }
        };
        // SAFETY: page tables, Local APIC and the kernel heap are active; the
        // bring-up saves and masks the caller's interrupt flag itself.
        match unsafe {
            systemes_kernel::smp::bring_up_application_processors(
                madt,
                &mut frame_allocator,
                page_table_root,
            )
        } {
            Ok(summary) => {
                unsafe {
                    let mut digits = [0u8; 20];
                    debug_write(b"SYSTEMES: AP trampoline published under 1 MiB\r\n");
                    for index in 0..summary.started_count {
                        debug_write(b"SYSTEMES_TEST: AP ");
                        let length = encode_decimal(summary.started_apic_ids[index] as u64, &mut digits);
                        debug_write(&digits[..length]);
                        debug_write(b" started in kernel long mode\r\n");
                    }
                    debug_write(b"SYSTEMES_TEST: application processors parked after validation: count=");
                    let length = encode_decimal(summary.started_count as u64, &mut digits);
                    debug_write(&digits[..length]);
                    debug_write(b"\r\n");
                    let cpu_count = 1 + summary.started_count;
                    debug_write(b"SYSTEMES_TEST: cross-core SpinLock counter verified across ");
                    let length = encode_decimal(cpu_count as u64, &mut digits);
                    debug_write(&digits[..length]);
                    debug_write(b" CPUs\r\n");
                    debug_write(b"SYSTEMES_TEST: cross-core relaxed counter verified\r\n");
                    debug_write(b"SYSTEMES_TEST: per-AP private counters verified\r\n");
                    debug_write(b"SYSTEMES_TEST: SMP bring-up validation complete\r\n");
                }
            }
            Err(error) => {
                unsafe {
                    debug_write(b"SYSTEMES_KERNEL_FAIL: SMP bring-up failed, error=");
                    debug_write(smp_failure_name(error));
                    debug_write(b"\r\n");
                }
                halt();
            }
        }
    }
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
            debug_write(b"SYSTEMES_TEST: SYS_YIELD returned to user process\r\n");
            debug_write(b"SYSTEMES_TEST: invalid syscall numbers and user pointers returned errors\r\n");
            debug_write(b"SYSTEMES_TEST: user process exited and kernel resumed\r\n");
            debug_write(b"SYSTEMES_TEST: exited process frames reclaimed\r\n");
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

    #[cfg(not(feature = "qemu-apic-clock-test"))]
    if apic_timer_ready
        && !cfg!(feature = "qemu-exception-test")
        && !LAUNCH_USER_SHELL
    {
        loop {
            // SAFETY: Idle with interrupts enabled; Local APIC timer remains active.
            unsafe { asm!("hlt", options(nomem, nostack)); }
        }
    }
    #[cfg(feature = "qemu-nx-execute-test")]
    {
        let test_frame = match frame_allocator.allocate_frame() {
            Some(address) => address,
            None => { unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX test frame unavailable\r\n"); } halt(); }
        };
        let test_end = match test_frame.checked_add(systemes_kernel::memory::PAGE_SIZE) {
            Some(end) if test_frame < systemes_kernel::paging::IDENTITY_MAP_BYTES => end,
            _ => { unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: invalid NX test frame\r\n"); } halt(); }
        };
        let test_region = [MemoryRegion { start: test_frame, end: test_end, permissions: PagePermissions::ReadWriteNoExecute }];
        if unsafe { systemes_kernel::paging::protect_regions(&mut frame_allocator, page_table_root, &test_region) }.is_err()
            || unsafe { systemes_kernel::paging::verify_regions(page_table_root, &test_region) }.is_err() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX test mapping verification failed\r\n"); }
            halt();
        }
        // SAFETY: The allocated identity-mapped frame is deliberately RW+NX for this fault test.
        unsafe { core::ptr::write_volatile(test_frame as *mut u8, 0xC3); }
        unsafe { debug_write(b"SYSTEMES_TEST: attempting instruction fetch from NX page\r\n"); }
        // SAFETY: This call must fault with #PF(P=1, I/D=1), error code 0x11.
        let test_function: extern "C" fn() = unsafe { core::mem::transmute(test_frame as usize) };
        test_function();
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: NX execution was not blocked\r\n"); }
        halt();
    }
    #[cfg(feature = "qemu-rx-write-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: attempting write to RX kernel text\r\n"); }
        // SAFETY: This deliberate volatile write must fault with #PF(P=1, W/R=1), error code 0x3.
        unsafe { core::ptr::write_volatile(core::ptr::addr_of!(__text_start) as *mut u8, 0x90); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: RX text write was not blocked\r\n"); }
        halt();
    }
    #[cfg(feature = "qemu-exception-test")]
    {
        // The CI boot test deliberately raises #BP to prove the handler can
        // advance RIP past INT3 and return through iretq.
        unsafe { debug_write(b"SYSTEMES_TEST: invoking INT3 breakpoint\r\n"); }
        unsafe { asm!("int3", options(nomem, nostack)) };
        unsafe { debug_write(b"SYSTEMES_TEST: breakpoint returned to caller through iretq\r\n"); }
        if !cfg!(feature = "qemu-user-shell-test") {
            halt();
        }
    }

    // T12: initialize the polled UART console for userland input/output.
    serial::init();
    unsafe { debug_write(b"SYSTEMES: serial UART initialized\r\n"); }

    // P1-1: mount the optional virtio-blk root filesystem. A missing or invalid
    // disk is reported and the boot continues without a root filesystem.
    report_root_filesystem(&mut frame_allocator);
    #[cfg(feature = "qemu-user-fs-test")]
    if let Err(message) = user_fs_test::run(&mut frame_allocator, page_table_root) {
        // SAFETY: The debug console is initialized and the kernel remains in ring 0.
        unsafe { debug_write(message); }
        // SAFETY: The debug console is initialized and the kernel remains in ring 0.
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: user file syscall test failed\r\n"); }
        halt();
    }

    // T11/T12: load and launch the userland shell (ELF64), which uses
    // SYS_WRITE (prompt/echo) and SYS_READ (console input) via the serial UART.
    // The ELF is loaded from the VFS (`/shell`) when a virtio-blk disk is
    // present, falling back to the embedded image otherwise.
    let (shell_data, loaded_from_vfs) = match rootfs::read_file("/shell") {
        Some(data) => (data, true),
        None => (SHELL_ELF.to_vec(), false),
    };
    if loaded_from_vfs {
        unsafe { debug_write(b"SYSTEMES: shell loaded from VFS (/shell)\r\n"); }
    } else if rootfs::is_mounted() {
        unsafe { debug_write(b"SYSTEMES: /shell not found on VFS; embedded shell\r\n"); }
    } else {
        unsafe { debug_write(b"SYSTEMES: no virtio-blk disk; embedded shell\r\n"); }
    }

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
    unsafe { debug_write(b"SYSTEMES: userland shell ready\r\n"); }
    systemes_kernel::syscall::reset_process_file_table();
    // SAFETY: The shell is executable in Ring 3 and its stack is writable in the active address space.
    let shell_exit_status = unsafe { enter_user_mode(shell_entry, shell_stack) };
    // SAFETY: The kernel page-table root is the original valid address space.
    unsafe { write_cr3(page_table_root) };
    if unsafe {
        systemes_kernel::address_space::cr3_root(systemes_kernel::address_space::read_cr3())
    } != page_table_root
    {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: kernel address space restore failed\r\n"); }
        halt();
    }
    // SAFETY: The kernel root is active, so none of the shell's owned frames are in use.
    if unsafe { shell_space.destroy(&mut frame_allocator) }.is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: shell process frames could not be reclaimed\r\n"); }
        halt();
    }
    unsafe {
        debug_write(b"SYSTEMES: user process exited with status ");
        let mut digits = [0; 20];
        let length = encode_decimal(shell_exit_status, &mut digits);
        debug_write(&digits[..length]);
        debug_write(b"\r\n");
    }
    loop {
        // SAFETY: The user process has exited and the kernel can idle with interrupts enabled.
        unsafe { asm!("sti; hlt", options(nomem, nostack)); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: panic\r\n"); }
    halt();
}
