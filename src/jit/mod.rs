mod dynasm;
// mod plain;

use crate::Error;
use std::mem;

// Currently, this is not handling panicking, and it's probably technically an UB.
// Since brainf*ck has not way to handle exceptions, "sysv64-unwind" may be the best.
// https://github.com/rust-lang/rust/issues/74990
pub unsafe extern "sysv64" fn putchar(byte: *const u8) {
    crate::putchar(unsafe { &*byte })
}

// Currently, this is not handling panicking, and it's probably technically an UB.
// Since brainf*ck has not way to handle exceptions, "sysv64-unwind" may be the best.
// https://github.com/rust-lang/rust/issues/74990
pub unsafe extern "sysv64" fn getchar(byte: *mut u8) {
    crate::getchar(unsafe { &mut *byte })
}

fn run_inner(opcode: &[u8]) {
    // The safety depends on the correctness of the compilers. How dangerous.
    // Executing this is `unsafe` because it segfaults with out of bound indexing.
    let execute: unsafe extern "sysv64" fn(*mut u8) = unsafe { mem::transmute(opcode.as_ptr()) };

    // Create an array of bytes followed by a guard page where `array.len() + guard.len() == u16::MAX + 1`
    // This way, the Brainf*ck programme can only access the array.
    // That's provided that the value used to index the array is 16 bit.
    let array = MmapWithGuard::new(30_000, u16::MAX as usize + 1).unwrap();
    unsafe { execute(array.ptr()) };
}

pub fn run_dynasm(program: &[u8]) -> Result<(), Error> {
    run_inner(&dynasm::compile(program)?);
    Ok(())
}

// pub fn run_plain(program: &[u8]) -> Result<(), Error> {
//     run_inner(&plain::compile(program)?);
//     Ok(())
// }

mod mmap {
    use nix::sys::mman;

    /// A struct that represents a `mmap`ped region of memory.
    /// From `self.ptr` to `self.ptr + accesible_len` is readable and writable.
    /// Afterwords, there's a guard memory until `self.ptr + mapped_len`.
    #[derive(Debug)]
    pub struct MmapWithGuard {
        ptr: *mut u8,
        mapped_len: usize,
    }

    impl MmapWithGuard {
        pub fn new(accessible_len: usize, mapping_len: usize) -> nix::Result<Self> {
            // Influenced by https://docs.rs/wasmtime-runtime/latest/src/wasmtime_runtime/mmap.rs.html#280-300

            // Make sure we aren't making the region acceible more than the mapping length.
            assert!(accessible_len <= mapping_len);
            // For anonymous and private mapping without fixing, there is nothing unsafe about `mmap`.
            // The OS takes care of page aligning `len`.
            let pa = unsafe {
                mman::mmap(
                    std::ptr::null_mut(),
                    mapping_len,
                    mman::ProtFlags::PROT_NONE,
                    mman::MapFlags::MAP_PRIVATE | mman::MapFlags::MAP_ANONYMOUS,
                    // The below two means nothing because this is anonymous.
                    -1,
                    0,
                )
            }?;
            unsafe {
                mman::mprotect(
                    pa,
                    accessible_len,
                    mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
                )
            }?;
            Ok(Self {
                ptr: pa.cast(),
                mapped_len: mapping_len,
            })
        }

        pub fn ptr(&self) -> *mut u8 {
            self.ptr
        }
    }

    impl Drop for MmapWithGuard {
        fn drop(&mut self) {
            // Calling `munmap` is safe and should not fail as long as the parameters came from `mmap`.
            #[allow(unused_must_use)]
            unsafe {
                mman::munmap(self.ptr.cast(), self.mapped_len);
            }
        }
    }
}

use mmap::MmapWithGuard;