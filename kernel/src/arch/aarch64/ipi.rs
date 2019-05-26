use rcore_memory::Page;
use aarch64::asm::tlb_invalidate;
use aarch64::VirtAddr;

pub fn invoke_on_allcpu<A: 'static>(f: fn(&A) -> (), arg: A, wait: bool) {
    // Since this is single-core we just do this.
    f(&arg);
}

// Shootdown for aarch64.

pub fn tlb_shootdown(tuple: &(usize, usize)) {
    let (start_addr, end_addr) = *tuple;
    for p in Page::range_of(start_addr, end_addr) {
        tlb_invalidate(VirtAddr::new(p.start_address() as u64));
    }
}