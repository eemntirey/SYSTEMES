#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use systemes_kernel::{memory::PhysicalFrameAllocator, paging::{MemoryRegion, PagePermissions}, BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

#[cfg(feature = "qemu-scheduler-test")]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "qemu-scheduler-test")]
use systemes_kernel::scheduler::{SpinMutex, TaskId};

#[cfg(feature = "qemu-scheduler-test")]
static SCHEDULER_WORK_A: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "qemu-scheduler-test")]
static SCHEDULER_WORK_B: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "qemu-scheduler-test")]
static SCHEDULER_BLOCKED_RESUMES: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "qemu-scheduler-test")]
static SCHEDULER_SHARED_COUNT: SpinMutex<usize> = SpinMutex::new(0);

#[cfg(feature = "qemu-scheduler-test")]
extern "C" fn scheduler_worker_a() -> ! {
    unsafe { debug_write(b"SYSTEMES_TEST: scheduler task A entered\\r\\n"); }
    loop {
        SCHEDULER_WORK_A.fetch_add(1, Ordering::Relaxed);
        *SCHEDULER_SHARED_COUNT.lock() += 1;
        core::hint::spin_loop();
    }
}

#[cfg(feature = "qemu-scheduler-test")]
extern "C" fn scheduler_worker_b() -> ! {
    unsafe { debug_write(b"SYSTEMES_TEST: scheduler task B entered\\r\\n"); }
    loop {
        SCHEDULER_WORK_B.fetch_add(1, Ordering::Relaxed);
        *SCHEDULER_SHARED_COUNT.lock() += 1;
        let _ = systemes_kernel::scheduler::wake(TaskId(3));
        core::hint::spin_loop();
    }
}

#[cfg(feature = "qemu-scheduler-test")]
extern "C" fn scheduler_blocked_worker() -> ! {
    unsafe { debug_write(b"SYSTEMES_TEST: scheduler blocked task entered\\r\\n"); }
    loop {
        systemes_kernel::scheduler::block_current();
        SCHEDULER_BLOCKED_RESUMES.fetch_add(1, Ordering::Relaxed);
    }
}

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
    if !systemes_kernel::interrupts::verify_task_register() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: TR did not load TSS selector 0x18\\r\\n"); }
        halt();
    }
    if !systemes_kernel::interrupts::verify_ist_configuration() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: TSS stacks or IDT IST routing invalid\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: TSS TR=0x18 verified; RSP0/IST1/IST2/IST3 and IDT routing verified\\r\\n") };
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

    let mut apic_initialized = false;
    let mut discovered_madt = None;
    if boot.acpi_rsdp_address != 0 {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP discovered\r\n") };
        // SAFETY: The RSDP address is supplied by UEFI and the active identity
        // map covers firmware physical addresses below 512 GiB.
        match unsafe { systemes_kernel::acpi::discover_madt(boot.acpi_rsdp_address) } {
            Ok(madt) => {
                if madt.io_apic_count == 0 || madt.local_apic_count == 0 {
                    unsafe { debug_write(b"SYSTEMES: ACPI MADT valid but incomplete for APIC routing\r\n") };
                } else {
                    discovered_madt = Some(madt);
                    unsafe { debug_write(b"SYSTEMES: ACPI MADT validated; Local APIC and IOAPIC topology discovered\r\n") };
                    match unsafe { systemes_kernel::interrupts::initialize_apic_ioapic(&madt, &mut frame_allocator, page_table_root) } {
                        Ok(()) => {
                            apic_initialized = true;
                            unsafe { debug_write(b"SYSTEMES: xAPIC enabled; IOAPIC PIT IRQ0 route installed\r\n") };
                        }
                        Err(_) => unsafe { debug_write(b"SYSTEMES: APIC initialization unavailable; retaining legacy PIC/PIT\r\n") },
                    }
                }
            }
            Err(_) => unsafe { debug_write(b"SYSTEMES: ACPI MADT validation unavailable; retaining legacy PIC path\r\n") },
        }
    } else {
        unsafe { debug_write(b"SYSTEMES: ACPI RSDP unavailable; retaining legacy PIC path\r\n") };
    }

    // Keep all IRQ lines masked except PIT IRQ0 until the timer is fully programmed.
    unsafe { systemes_kernel::interrupts::initialize_pic_pit() };
    if !apic_initialized && systemes_kernel::interrupts::legacy_pic_masks() != (0xFE, 0xFF) {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: legacy PIC masks are not IRQ0-only\\r\\n"); }
        halt();
    }
    if !systemes_kernel::interrupts::verify_irq_gates() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PIC IRQ IDT gates invalid\\\\r\\\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: 8259 PIC remapped to 0x20/0x28; only IRQ0 unmasked\\r\\n"); }
    unsafe { debug_write(b"SYSTEMES: PIT channel 0 programmed at 100 Hz\\r\\n"); }
    if apic_initialized {
        unsafe { systemes_kernel::interrupts::mask_legacy_pic() };
        if systemes_kernel::interrupts::legacy_pic_masks() != (0xFF, 0xFF) {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: legacy PIC not fully masked in APIC mode\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES: legacy PIC fully masked; IRQ0 delivered through IOAPIC\\r\\n"); }
    }
    let (ps2_ready, auxiliary_ready) = match unsafe { systemes_kernel::ps2_controller::initialize() } {
        Ok(status) if status.keyboard_interface_ready && status.irq_lines_disabled && status.set1_translation_enabled => {
            unsafe { debug_write(b"SYSTEMES: PS/2 8042 initialized with IRQ1/IRQ12 disabled\\r\\n"); }
            (true, status.auxiliary_interface_detected)
        }
        Ok(_) => { unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PS/2 controller violated safe IRQ/configuration policy\\r\\n"); halt(); } }
        Err(_) => { unsafe { debug_write(b"SYSTEMES: PS/2 8042 unavailable; continuing without PS/2 input\\r\\n"); } (false, false) }
    };
    if apic_initialized && ps2_ready {
        match unsafe { systemes_kernel::interrupts::prepare_ps2_ioapic_routes(
            discovered_madt.as_ref().unwrap_or_else(|| halt()),
            &mut frame_allocator, page_table_root, auxiliary_ready
        ) } {
            Ok(()) => {
                if unsafe { systemes_kernel::interrupts::enable_ps2_ioapic_routes(auxiliary_ready) }.is_ok()
                    && unsafe { systemes_kernel::ps2_controller::enable_interrupts(auxiliary_ready) }.is_ok() {
                    if auxiliary_ready {
                        unsafe { debug_write(b"SYSTEMES: PS/2 IRQ1 keyboard and IRQ12 auxiliary routes enabled via IOAPIC\\r\\n"); }
                    } else {
                        unsafe { debug_write(b"SYSTEMES: PS/2 IRQ1 keyboard route enabled; IRQ12 kept masked (no auxiliary device)\\r\\n"); }
                    }
                } else {
                    unsafe { systemes_kernel::interrupts::mask_ps2_ioapic_routes(); }
                    unsafe { debug_write(b"SYSTEMES: PS/2 IRQ activation failed; IOAPIC routes re-masked\\r\\n"); }
                }
            }
            Err(_) => unsafe { debug_write(b"SYSTEMES: PS/2 IOAPIC routes unavailable; input IRQs remain disabled\\r\\n"); }
        }
    } else if !apic_initialized {
        unsafe { debug_write(b"SYSTEMES: PS/2 IRQs kept disabled because xAPIC/IOAPIC is unavailable\\r\\n"); }
    } else if !ps2_ready {
        unsafe { debug_write(b"SYSTEMES: PS/2 controller unavailable; IRQs remain disabled\\r\\n"); }
    }
    if ps2_ready {
        // Device-level reporting is distinct from enabling the 8042 controller
        // clocks and IRQ bits. Require ACKs before allowing PS/2 IRQ delivery.
        if unsafe { systemes_kernel::ps2_controller::initialize_devices(auxiliary_ready) }.is_err() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PS/2 device reporting initialization failed\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES: PS/2 keyboard scanning and mouse reporting enabled\\r\\n"); }
    }
    #[cfg(feature = "qemu-scheduler-test")]
    let scheduler_entries: [systemes_kernel::scheduler::TaskEntry; 3] = [
        scheduler_worker_a,
        scheduler_worker_b,
        scheduler_blocked_worker,
    ];
    #[cfg(not(feature = "qemu-scheduler-test"))]
    let scheduler_entries: [systemes_kernel::scheduler::TaskEntry; 0] = [];

    if systemes_kernel::scheduler::initialize(&scheduler_entries).is_err() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: scheduler initialization failed\\r\\n"); }
        halt();
    }
    if !systemes_kernel::scheduler::validate_initial_contexts() {
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: synthetic task IRETQ frame invalid\\r\\n"); }
        halt();
    }
    unsafe { debug_write(b"SYSTEMES: synthetic task IRETQ frames validated\\r\\n"); }
    if apic_initialized {
        if unsafe { systemes_kernel::interrupts::initialize_local_apic_timer(250_000) }.is_err() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: Local APIC scheduler timer initialization failed\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES: Local APIC periodic scheduler timer configured\\r\\n"); }
    } else {
        #[cfg(feature = "qemu-scheduler-test")]
        {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: scheduler test requires active Local APIC\\r\\n"); }
            halt();
        }
        #[cfg(not(feature = "qemu-scheduler-test"))]
        unsafe { debug_write(b"SYSTEMES: Local APIC timer unavailable; scheduler remains idle\\r\\n"); }
    }
    unsafe { systemes_kernel::interrupts::enable_hardware_interrupts() };
    unsafe { debug_write(b"SYSTEMES: hardware interrupts enabled\\r\\n"); }

    unsafe { debug_write(b"SYSTEMES_KERNEL_OK\\r\\n") };
    #[cfg(feature = "qemu-scheduler-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: waiting for preemptive task progress\\r\\n"); }
        while SCHEDULER_WORK_A.load(Ordering::Acquire) < 100
            || SCHEDULER_WORK_B.load(Ordering::Acquire) < 100
            || SCHEDULER_BLOCKED_RESUMES.load(Ordering::Acquire) < 1
            || *SCHEDULER_SHARED_COUNT.lock() < 100
            || systemes_kernel::interrupts::local_timer_ticks() < 8
        {
            if systemes_kernel::interrupts::local_timer_ticks() > 2000 {
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: scheduler progress timeout\\r\\n"); }
                halt();
            }
            core::hint::spin_loop();
        }
        let switches_a = systemes_kernel::scheduler::switch_count(TaskId(1)).unwrap_or(0);
        let switches_b = systemes_kernel::scheduler::switch_count(TaskId(2)).unwrap_or(0);
        let switches_blocked = systemes_kernel::scheduler::switch_count(TaskId(3)).unwrap_or(0);
        if switches_a == 0 || switches_b == 0 || switches_blocked == 0 {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: scheduler context switch counters invalid\\r\\n"); }
            halt();
        }
        unsafe { systemes_kernel::interrupts::stop_local_apic_timer(); }
        unsafe { systemes_kernel::interrupts::disable_hardware_interrupts(); }
        unsafe { debug_write(b"SYSTEMES_TEST: Local APIC preemption switched three independent tasks\\r\\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: blocked task wakeup and IRQ-safe mutex verified\\r\\n"); }
        halt();
    }
    #[cfg(feature = "qemu-pit-timer-test")]
    {
        if !apic_initialized || !systemes_kernel::interrupts::apic_active() {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: APIC route not active for PIT test\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: waiting for three IOAPIC-routed PIT IRQ0 ticks\\r\\n"); }
        while systemes_kernel::interrupts::timer_ticks() < 3 {
            // SAFETY: STI is active and PIT IRQ0 is the only unmasked PIC line.
            unsafe { asm!("hlt", options(nomem, nostack)); }
        }
        unsafe { systemes_kernel::interrupts::disable_hardware_interrupts() };
        let ticks = systemes_kernel::interrupts::timer_ticks();
        if ticks < 3 {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PIT timer did not advance\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: PIT IRQ0 delivered at least three timer ticks\\r\\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: IOAPIC delivered PIT IRQ0 through Local APIC EOI\\r\\n"); }
    }
    #[cfg(feature = "qemu-apic-spurious-test")]
    {
        let before = systemes_kernel::interrupts::apic_spurious_count();
        let _ = systemes_kernel::interrupts::systemes_irq_dispatch(0xFF, 0);
        if systemes_kernel::interrupts::apic_spurious_count() != before + 1 {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: APIC spurious vector accounting failed\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: APIC spurious vector 0xFF counted without EOI\\r\\n"); }
    }
    #[cfg(feature = "qemu-ist-nmi-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: invoke vector 2 through IST2\r\n"); }
        // SAFETY: The ring-0 IDT vector 2 is installed with IST2 and has a valid stub.
        unsafe { asm!("int 2"); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: IST2 test interrupt returned\r\n"); }
        halt();
    }
    #[cfg(feature = "qemu-ist-machine-check-test")]
    {
        unsafe { debug_write(b"SYSTEMES_TEST: invoke vector 18 through IST3\r\n"); }
        // SAFETY: The ring-0 IDT vector 18 is installed with IST3 and has a valid stub.
        unsafe { asm!("int 18"); }
        unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: IST3 test interrupt returned\r\n"); }
        halt();
    }
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
    #[cfg(feature = "qemu-ps2-input-test")]
    {
        if !apic_initialized || !systemes_kernel::interrupts::apic_active() || !ps2_ready || !auxiliary_ready {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PS/2 input integration prerequisites unavailable\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: PS2_INPUT_READY\\r\\n"); }
        let start_ticks = systemes_kernel::interrupts::timer_ticks();
        loop {
            systemes_kernel::input::poll_events();
            let elapsed = systemes_kernel::interrupts::timer_ticks().wrapping_sub(start_ticks);
            if systemes_kernel::input::key_event_count() >= 2
                && systemes_kernel::input::mouse_event_count() >= 2 {
                break;
            }
            if elapsed >= 5000 {
                if systemes_kernel::interrupts::irq_count(1).unwrap_or(0) > 0 {
                    unsafe { debug_write(b"SYSTEMES_TEST: PS2_IRQ1_OBSERVED\\r\\n"); }
                } else {
                    unsafe { debug_write(b"SYSTEMES_TEST: PS2_IRQ1_NOT_OBSERVED\\r\\n"); }
                }
                if systemes_kernel::interrupts::irq_count(12).unwrap_or(0) > 0 {
                    unsafe { debug_write(b"SYSTEMES_TEST: PS2_IRQ12_OBSERVED\\r\\n"); }
                } else {
                    unsafe { debug_write(b"SYSTEMES_TEST: PS2_IRQ12_NOT_OBSERVED\\r\\n"); }
                }
                unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PS/2 input event timeout\\r\\n"); }
                halt();
            }
            unsafe { asm!("pause", options(nomem, nostack, preserves_flags)); }
        }
        let mouse = systemes_kernel::input::mouse_state();
        if systemes_kernel::input::key_is_pressed(0x1E, false)
            || mouse.x == 0 || mouse.y == 0 || mouse.buttons != 0 {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: PS/2 input state mismatch\\r\\n"); }
            halt();
        }
        unsafe { debug_write(b"SYSTEMES_TEST: PS/2 keyboard make/break consumed\\r\\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: PS/2 mouse motion and button release consumed\\r\\n"); }
        unsafe { debug_write(b"SYSTEMES_TEST: PS/2 IRQ input end-to-end verified\\r\\n"); }
        halt();
    }
    systemes_kernel::input::run_event_loop();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: panic\r\n"); }
    halt();
}
