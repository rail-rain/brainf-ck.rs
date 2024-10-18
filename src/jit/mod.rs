#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

use crate::Error;
use std::{io, mem};

/// A wrapper around [`crate::putchar`] to for the JIT to call.
/// Writes the value pointed by `byte` into the output.
/// ## Error
/// This returns 0 if the output failed and stores the details in `errorno`.
/// ## Safety
/// The caller must ensure `byte` is safe to dereference.
pub unsafe extern "C" fn putchar(byte: *const u8) -> u8 {
    // Catch panicking as it is UB to unwind from Rust into a foreign language.
    // "sysv64-unwind" may be a better alternative.
    // https://github.com/rust-lang/rust/issues/74990
    std::panic::catch_unwind(||
        // It is the caller's responsibility to ensure `byte` is a valid pointer.
        crate::putchar(unsafe { &*byte }).is_ok() as u8)
    .unwrap_or(0) // The caller cannot know why this panicked, but it's unlikely to happen anyway.
}

/// A wrapper around [`crate::getchar`] to for the JIT to call.
/// Reads a value from the input into the memory pointed by `byte`.
/// ## Error
/// This returns 0 if the output failed and stores the details in `errorno`.
/// ## Safety
/// The caller must ensure `byte` is safe to dereference.
pub unsafe extern "C" fn getchar(byte: *mut u8) -> u8 {
    // Catch panicking as it is UB to unwind from Rust into a foreign language.
    // "sysv64-unwind" may be a better alternative.
    // https://github.com/rust-lang/rust/issues/74990
    std::panic::catch_unwind(||
        // It is the caller's responsibility to ensure `byte` is a valid pointer.
        crate::getchar(unsafe { &mut *byte }).is_ok() as u8)
    .unwrap_or(0) // The caller cannot know why this panicked, but it's unlikely to happen anyway.
}

fn run_opcode(opcode: &[u8]) -> Result<(), Error> {
    // Safety: it must be safe to access the given pointer up to it plus 2^16.
    let execute: unsafe extern "C" fn(*mut u8) -> u8 =
        // The safety of this block depends on the correctness of the compilers. How dangerous.
        unsafe { mem::transmute(opcode.as_ptr()) };

    // Create an array of bytes with the size of `u16::MAX + 1`
    // This way, the Brainf*ck programme can only access regions inside the array.
    // That's provided that the value used to index the array is 16 bit.
    // There's a better way that wastes no memory, but it is too hard to do correctly.
    //
    // Address masking seems good but I have to control virtual address to give Brainf*ck code.
    // https://www.cse.psu.edu/~gxt29/papers/sfi-final.pdf
    // https://hacks.mozilla.org/2021/12/webassembly-and-back-again-fine-grained-sandboxing-in-firefox-95/
    //
    // According to the documentation, Wasmtime has both guard page and bound checking.
    // Wasmtime puts enough guard pages so that 32-bit wasm cannot access outside of it.
    // This is super hard because I had to catch SIGSEGV or SIGBUS from guarded pages and recover from it.
    // https://github.com/bytecodealliance/wasmtime/issues/15
    let mut array = vec![0u8; u16::MAX as usize + 1].into_boxed_slice();
    let result = unsafe { execute(array.as_mut_ptr()) };

    if result == 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}
