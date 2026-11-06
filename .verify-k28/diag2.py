from pathlib import Path

p = Path('kernel/src/main.rs')
t = p.read_text(encoding='utf-8')

anchor = '    static WORKERS_DONE: AtomicUsize = AtomicUsize::new(0);'
assert t.count(anchor) == 1
t = t.replace(anchor, anchor + '''
    pub static FIRST_A: AtomicUsize = AtomicUsize::new(usize::MAX);
    pub static FIRST_B: AtomicUsize = AtomicUsize::new(usize::MAX);
    pub static FIRST_C: AtomicUsize = AtomicUsize::new(usize::MAX);''', 1)

old = '    fn run(spec: FpuSpec, verified: &\'static AtomicUsize) -> ! {'
new = '    fn run(spec: FpuSpec, verified: &\'static AtomicUsize, first: &\'static AtomicUsize) -> ! {\n        first.store(systemes_kernel::interrupts::local_timer_ticks() as usize, Ordering::Release);'
assert t.count(old) == 1
t = t.replace(old, new, 1)

old = '        run(SPEC_A, &VERIFIED_A)'
new = '        run(SPEC_A, &VERIFIED_A, &FIRST_A)'
assert t.count(old) == 1
t = t.replace(old, new, 1)
old = '        run(SPEC_B, &VERIFIED_B)'
new = '        run(SPEC_B, &VERIFIED_B, &FIRST_B)'
assert t.count(old) == 1
t = t.replace(old, new, 1)
old = '        run(SPEC_C, &VERIFIED_C)'
new = '        run(SPEC_C, &VERIFIED_C, &FIRST_C)'
assert t.count(old) == 1
t = t.replace(old, new, 1)

old = '    pub fn debug_hex(label: &[u8], value: usize) {'
assert t.count(old) == 1
t = t.replace(old, '''    pub fn debug_first(label: &[u8], value: &AtomicUsize) {
        debug_hex(label, value.load(Ordering::Acquire));
    }

    pub fn debug_hex(label: &[u8], value: usize) {''', 1)

old = '''            fpu_context_test::debug_hex(b"SYSTEMES_TEST: C=", verified[2]);'''
assert t.count(old) == 1
t = t.replace(old, old + '''
            fpu_context_test::debug_first(b"SYSTEMES_TEST: firstA=", &fpu_context_test::FIRST_A);
            fpu_context_test::debug_first(b"SYSTEMES_TEST: firstB=", &fpu_context_test::FIRST_B);
            fpu_context_test::debug_first(b"SYSTEMES_TEST: firstC=", &fpu_context_test::FIRST_C);
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: swA=", systemes_kernel::scheduler::switch_count(systemes_kernel::scheduler::TaskId(1)).unwrap_or(0));
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: swB=", systemes_kernel::scheduler::switch_count(systemes_kernel::scheduler::TaskId(2)).unwrap_or(0));
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: swC=", systemes_kernel::scheduler::switch_count(systemes_kernel::scheduler::TaskId(3)).unwrap_or(0));''', 1)

p.write_text(t, encoding='utf-8')
print('patched2')
