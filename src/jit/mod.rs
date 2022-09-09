mod dynasm;
mod plain;

use std::mem;

pub extern "sysv64" fn putchar(char: u8) {
    crate::putchar(char)
}

pub extern "sysv64" fn getchar() -> u8 {
    crate::getchar()
}

fn run_inner(opcode: &[u8]) {
    // The safety depends on the correctness of the compilers. How dangerous.
    let execute: extern "sysv64" fn(*mut u8) = unsafe { mem::transmute(opcode.as_ptr()) };

    let mut array = [0; 30_000];
    execute(array.as_mut_ptr());
}

pub fn run_dynasm(program: &[u8]) {
    run_inner(&dynasm::compile(program))
}

pub fn run_plain(program: &[u8]) {
    run_inner(&plain::compile(program))
}
