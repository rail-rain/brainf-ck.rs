pub mod dynasm;
pub mod plain;

use std::mem;

pub extern "sysv64" fn putchar(char: u8) {
    crate::putchar(char)
}

pub extern "sysv64" fn getchar() -> u8 {
    crate::getchar()
}

pub fn run(opcode: impl AsRef<[u8]>) {
    inner(opcode.as_ref());
    fn inner(opcode: &[u8]) {
        // The safety depends on the correctness of the compilers. How dangerous.
        let execute: extern "sysv64" fn(*mut u8) = unsafe { mem::transmute(opcode.as_ptr()) };

        let mut array = [0; 30_000];
        execute(array.as_mut_ptr());
    }
}
