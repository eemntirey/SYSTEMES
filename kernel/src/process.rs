//! Multi-process management for Ring 3 user processes.

use crate::{
    address_space::{AddressSpace, UserPagePermissions, USER_BASE, cr3_root, read_cr3, write_cr3},
    elf_loader::load_elf,
    file_descriptors::ProcessFileTable,
    interrupts::{self, set_yield_requested},
    memory::{PhysicalFrameAllocator, PAGE_SIZE},
    paging::IDENTITY_MAP_BYTES,
    rootfs,
    syscall::SyscallFrame,
};
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use core::ptr;

pub const MAX_PROCESSES: usize = 8;
const USER_STACK_PAGE: u64 = USER_BASE + 0xA000;
/// Scratch page used by the Ring 3 shell for its line buffer and argument copies.
/// It must stay clear of the syscall stack at USER_BASE + 0x7000.
const USER_DATA_PAGE: u64 = USER_BASE + 0x8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessState {
    Empty = 0,
    Runnable = 1,
    Running = 2,
    Exited = 3,
}

#[repr(C, align(16))]
pub struct UserContext {
    pub r15: u64, pub r14: u64, pub r13: u64, pub r12: u64,
    pub r11: u64, pub r10: u64, pub r9: u64, pub r8: u64,
    pub rbp: u64, pub rdi: u64, pub rsi: u64, pub rdx: u64,
    pub rcx: u64, pub rbx: u64, pub rax: u64,
    pub user_rsp: u64,
}

impl UserContext {
    const fn empty() -> Self {
        Self {
            r15: 0, r14: 0, r13: 0, r12: 0, r11: 0x202, r10: 0, r9: 0, r8: 0,
            rbp: 0, rdi: 0, rsi: 0, rdx: 0, rcx: 0, rbx: 0, rax: 0,
            user_rsp: 0,
        }
    }

    fn from_syscall_frame(frame: &SyscallFrame) -> Self {
        Self {
            r15: frame.r15, r14: frame.r14, r13: frame.r13, r12: frame.r12,
            r11: frame.r11, r10: frame.r10, r9: frame.r9, r8: frame.r8,
            rbp: frame.rbp, rdi: frame.rdi, rsi: frame.rsi, rdx: frame.rdx,
            rcx: frame.rcx, rbx: frame.rbx, rax: frame.rax,
            user_rsp: frame.user_rsp,
        }
    }

    fn to_syscall_frame(&self, frame: &mut SyscallFrame) {
        frame.r15 = self.r15; frame.r14 = self.r14; frame.r13 = self.r13; frame.r12 = self.r12;
        frame.r11 = self.r11; frame.r10 = self.r10; frame.r9 = self.r9; frame.r8 = self.r8;
        frame.rbp = self.rbp; frame.rdi = self.rdi; frame.rsi = self.rsi; frame.rdx = self.rdx;
        frame.rcx = self.rcx; frame.rbx = self.rbx; frame.rax = self.rax;
        frame.user_rsp = self.user_rsp;
    }
}

pub struct Process {
    pub pid: u32,
    pub state: AtomicU8,
    pub address_space: AddressSpace,
    pub entry: u64,
    pub user_stack: u64,
    pub context: UserContext,
    pub file_table: ProcessFileTable,
    pub exit_code: u32,
}

impl Process {
    pub fn new(
        pid: u32,
        address_space: AddressSpace,
        entry: u64,
        user_stack: u64,
        file_table: ProcessFileTable,
    ) -> Self {
        Self {
            pid,
            state: AtomicU8::new(ProcessState::Runnable as u8),
            address_space, entry, user_stack,
            context: UserContext {
                rcx: entry, user_rsp: user_stack, r11: 0x202,
                ..UserContext::empty()
            },
            file_table,
            exit_code: 0,
        }
    }
}

pub struct ProcessTable {
    processes: [Option<Process>; MAX_PROCESSES],
    next_pid: AtomicU32,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            processes: [const { None }; MAX_PROCESSES],
            next_pid: AtomicU32::new(1),
        }
    }

    pub fn insert(&mut self, mut process: Process) -> Result<u32, &'static str> {
        let pid = self.next_pid.fetch_add(1, Ordering::Relaxed);
        if pid == 0 || pid >= MAX_PROCESSES as u32 {
            return Err("pid out of range");
        }
        let slot = pid as usize;
        if self.processes[slot].is_some() {
            return Err("pid slot occupied");
        }
        process.pid = pid;
        self.processes[slot] = Some(process);
        Ok(pid)
    }

    pub fn get(&self, pid: u32) -> Option<&Process> {
        if pid == 0 || pid as usize >= MAX_PROCESSES {
            return None;
        }
        self.processes[pid as usize].as_ref()
    }

    pub fn get_mut(&mut self, pid: u32) -> Option<&mut Process> {
        if pid == 0 || pid as usize >= MAX_PROCESSES {
            return None;
        }
        self.processes[pid as usize].as_mut()
    }

    pub fn remove(&mut self, pid: u32) -> Option<Process> {
        if pid == 0 || pid as usize >= MAX_PROCESSES {
            return None;
        }
        self.processes[pid as usize].take()
    }

    pub fn next_runnable(&self, current: u32) -> Option<u32> {
        let mut pid = current.wrapping_add(1);
        for _ in 0..MAX_PROCESSES {
            if pid as usize >= MAX_PROCESSES {
                pid = 1;
            }
            if let Some(p) = self.processes[pid as usize].as_ref() {
                if p.state.load(Ordering::Acquire) == ProcessState::Runnable as u8 {
                    return Some(pid);
                }
            }
            pid = pid.wrapping_add(1);
        }
        None
    }

    pub fn count_runnable(&self) -> usize {
        self.processes.iter().flatten().filter(|p| {
            p.state.load(Ordering::Acquire) == ProcessState::Runnable as u8
        }).count()
    }
}

static mut PROCESS_TABLE: ProcessTable = ProcessTable::new();
static CURRENT_PID: AtomicU32 = AtomicU32::new(0);
static mut FRAME_ALLOCATOR: Option<PhysicalFrameAllocator> = None;

pub fn process_table() -> &'static mut ProcessTable {
    unsafe {
        let ptr = &raw mut PROCESS_TABLE;
        &mut *ptr
    }
}

pub fn current_pid() -> u32 {
    CURRENT_PID.load(Ordering::Acquire)
}

pub fn set_current_pid(pid: u32) {
    CURRENT_PID.store(pid, Ordering::Release);
}

pub unsafe fn init_frame_allocator(allocator: PhysicalFrameAllocator) {
    FRAME_ALLOCATOR = Some(allocator);
}

pub fn frame_allocator() -> Option<&'static mut PhysicalFrameAllocator> {
    unsafe {
        let ptr = &raw mut FRAME_ALLOCATOR as *mut Option<PhysicalFrameAllocator>;
        match *ptr {
            Some(ref mut a) => Some(a),
            None => None,
        }
    }
}

fn kernel_pml4_root() -> u64 {
    unsafe { cr3_root(read_cr3()) }
}

pub fn spawn_from_vfs(path: &str) -> Result<u32, &'static str> {
    let data = rootfs::read_file(path).ok_or("file not found on VFS")?;
    spawn_from_elf(&data)
}

/// Spawn a new process from an ELF image in memory.
pub fn spawn_from_elf(data: &[u8]) -> Result<u32, &'static str> {
    let allocator = frame_allocator().ok_or("frame allocator not initialized")?;
    let mut space = unsafe {
        AddressSpace::create(allocator, kernel_pml4_root())
            .map_err(|_| "failed to create address space")?
    };
    let entry = unsafe { load_elf(data, allocator, &mut space) }
        .map_err(|_| "failed to load ELF")?;
    let stack_frame = allocator.allocate_frame().ok_or("out of frames")?;
    if stack_frame % PAGE_SIZE != 0 || stack_frame >= IDENTITY_MAP_BYTES {
        let _ = allocator.free_frame(stack_frame);
        return Err("invalid stack frame");
    }
    unsafe { ptr::write_bytes(stack_frame as *mut u8, 0, PAGE_SIZE as usize); }
    unsafe {
        space.map_user_page(
            allocator, USER_STACK_PAGE, stack_frame,
            UserPagePermissions::ReadWriteNoExecute,
        ).map_err(|_| "failed to map user stack")?;
    }
    let data_frame = allocator.allocate_frame().ok_or("out of frames")?;
    if data_frame % PAGE_SIZE != 0 || data_frame >= IDENTITY_MAP_BYTES {
        let _ = allocator.free_frame(data_frame);
        return Err("invalid data frame");
    }
    unsafe { ptr::write_bytes(data_frame as *mut u8, 0, PAGE_SIZE as usize); }
    unsafe {
        space.map_user_page(
            allocator, USER_DATA_PAGE, data_frame,
            UserPagePermissions::ReadWriteNoExecute,
        ).map_err(|_| "failed to map user data page")?;
    }
    let user_stack = USER_STACK_PAGE + PAGE_SIZE - 16;
    let file_table = ProcessFileTable::new();
    let process = Process::new(0, space, entry, user_stack, file_table);
    let pid = process_table().insert(process)?;
    Ok(pid)
}

pub fn fork_current() -> Result<u32, &'static str> {
    let _current = current_pid();
    Err("fork not yet implemented")
}

fn save_current_and_select_next(frame: &SyscallFrame) -> Option<u32> {
    let current = current_pid();
    let table = process_table();
    if current != 0 {
        if let Some(p) = table.get_mut(current) {
            p.context = UserContext::from_syscall_frame(frame);
            p.state.store(ProcessState::Runnable as u8, Ordering::Release);
        }
    }
    table.next_runnable(current)
}

pub fn yield_to_next(frame: &mut SyscallFrame) -> u64 {
    set_yield_requested(true);
    let next_pid = save_current_and_select_next(frame);
    match next_pid {
        Some(pid) => {
            set_current_pid(pid);
            let table = process_table();
            let p = table.get_mut(pid).expect("next process exists");
            p.state.store(ProcessState::Running as u8, Ordering::Release);
            unsafe { write_cr3(p.address_space.root()); }
            p.context.to_syscall_frame(frame);
            unsafe { interrupts::set_syscall_user_rsp(p.context.user_rsp); }
            0
        }
        None => {
            if current_pid() != 0 {
                let table = process_table();
                if let Some(p) = table.get_mut(current_pid()) {
                    p.state.store(ProcessState::Running as u8, Ordering::Release);
                }
            }
            0
        }
    }
}

pub fn exit_current(exit_code: u32, frame: &mut SyscallFrame) -> u64 {
    set_yield_requested(false);
    let current = current_pid();
    // Close all file descriptors of the exiting process while it is still in the table.
    if current != 0 {
        let table = process_table();
        if let Some(p) = table.get_mut(current) {
            let handles = p.file_table.take_all();
            let _ = crate::rootfs::with_root(|vfs| {
                for handle in handles.iter().flatten().copied() {
                    let _ = vfs.close(handle);
                }
            });
        }
    }
    let table = process_table();
    if current != 0 {
        if let Some(mut p) = table.remove(current) {
            p.state.store(ProcessState::Exited as u8, Ordering::Release);
            p.exit_code = exit_code;
            let allocator = frame_allocator().expect("frame allocator not initialized");
            unsafe { write_cr3(kernel_pml4_root()); }
            let _ = unsafe { p.address_space.destroy(allocator) };
        }
    }
    set_current_pid(0);
    let table = process_table();
    let next_pid = table.processes.iter().enumerate().find_map(|(i, opt)| {
        if let Some(p) = opt {
            if p.state.load(Ordering::Acquire) == ProcessState::Runnable as u8 {
                Some(i as u32)
            } else { None }
        } else { None }
    });
    match next_pid {
        Some(pid) => {
            set_current_pid(pid);
            let mut table = process_table();
            let p = table.get_mut(pid).expect("next process exists");
            p.state.store(ProcessState::Running as u8, Ordering::Release);
            unsafe { write_cr3(p.address_space.root()); }
            p.context.to_syscall_frame(frame);
            unsafe { interrupts::set_syscall_user_rsp(p.context.user_rsp); }
            0
        }
        None => 0,
    }
}

pub fn getpid() -> u32 {
    current_pid()
}

pub(crate) fn current_file_table() -> Option<&'static mut ProcessFileTable> {
    let pid = current_pid();
    if pid == 0 { return None; }
    let table = process_table();
    table.get_mut(pid).map(|p| &mut p.file_table)
}

pub(crate) fn close_current_process_files() {
    let pid = current_pid();
    if pid == 0 { return; }
    let table = process_table();
    if let Some(p) = table.get_mut(pid) {
        let handles = p.file_table.take_all();
        let _ = crate::rootfs::with_root(|vfs| {
            for handle in handles.iter().flatten().copied() {
                let _ = vfs.close(handle);
            }
        });
    }
}