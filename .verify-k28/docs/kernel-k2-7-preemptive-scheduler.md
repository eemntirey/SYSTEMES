# K2.7 — Exécution multitâche du noyau

## Scope

K2.7 introduces the first single-core preemptive kernel scheduler on top of the
xAPIC/IOAPIC interrupt foundation. It is a freestanding Rust implementation
inside the independent UEFI-booted kernel; it does not use Windows, WPF,
WebView2, or the .NET host at runtime.

## Implemented

- A periodic Local APIC timer on vector 0x40, with EOI handling and a monotonic
  interrupt counter. The current initial count is a bootstrap value, not yet a
  calibrated wall-clock frequency.
- A fixed-capacity task table, per-task 16-byte-aligned kernel stacks, saved
  interrupt contexts, task identifiers, and Runnable/Blocked/Exited states.
- Round-robin preemption by saving the interrupted RSP and restoring the next
  runnable task's interrupt frame from the IRQ assembly exit path.
- Blocking and wake-up primitives for kernel tasks.
- A single-core IRQ-safe SpinMutex that disables maskable interrupts while
  held, preventing a timer preemption from deadlocking the lock owner.
- A QEMU/OVMF integration test with two independent worker tasks and one
  blocked/woken task. The test requires progress counters, context-switch
  counters, Local APIC timer ticks, and shared-lock updates.

## Current limits

- Single-core only. SMP startup, per-CPU scheduler state, and inter-processor
  interrupts are not implemented.
- Tasks currently execute in ring 0 and share the kernel address space. User
  mode, per-process page tables, syscall entry, and isolation are later phases.
- The timer initial count is provisional. TSC/PIT-based calibration and a
  stable monotonic time service are still required.
- FPU/SIMD task-state preservation is not yet part of the context frame.
  Kernel code must not rely on task-private SIMD state until that support lands.
- The scheduler uses a fixed static task table and static stacks; dynamic task
  creation and reclamation are not part of this phase.

## Validation

The `qemu-scheduler-test` feature boots the generated UEFI ISO in QEMU/OVMF and
requires the marker:

`SYSTEMES_TEST: Local APIC preemption switched three independent tasks`

It also requires:

`SYSTEMES_TEST: blocked task wakeup and IRQ-safe mutex verified`

Host-side no_std contract tests and the x86_64-unknown-none build remain part
of the kernel workflow. A green CI run is required on the exact PR HEAD before
merging.
