mod dynasm;
// mod plain;

use crate::Error;
use std::{io, mem};

/// A wrapper around [`crate::putchar`] to for the JIT to call.
/// Writes the value pointed by `byte` into the output.
/// ## Error
/// This returns 0 if the output failed and stores the details in `errorno`.
/// ## Safety
/// The caller must ensure `byte` is safe to dereference.
pub unsafe extern "sysv64" fn putchar(byte: *const u8) -> u8 {
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
pub unsafe extern "sysv64" fn getchar(byte: *mut u8) -> u8 {
    // Catch panicking as it is UB to unwind from Rust into a foreign language.
    // "sysv64-unwind" may be a better alternative.
    // https://github.com/rust-lang/rust/issues/74990
    std::panic::catch_unwind(||
        // It is the caller's responsibility to ensure `byte` is a valid pointer.
        crate::getchar(unsafe { &mut *byte }).is_ok() as u8)
    .unwrap_or(0) // The caller cannot know why this panicked, but it's unlikely to happen anyway.
}

fn run_inner(opcode: &[u8]) -> io::Result<()> {
    // The safety depends on the correctness of the compilers. How dangerous.
    // Safety: it must be safe to access the given pointer up to it plus 2^16.
    let execute: unsafe extern "sysv64" fn(*mut u8) -> u8 =
        unsafe { mem::transmute(opcode.as_ptr()) };

    // Create an array of bytes followed by a guard page where `array.len() + guard.len() == u32::MAX + 1`
    // This way, the Brainf*ck programme can only access the array.
    // That's provided that the value used to index the array is 16 bit.
    let mut array = vec![0u8; u16::MAX as usize + 1].into_boxed_slice();
    if unsafe { execute(array.as_mut_ptr()) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn run_dynasm(program: &[u8]) -> Result<(), Error> {
    run_inner(&dynasm::compile(program)?).map_err(|e| e.into())
}

// pub fn run_plain(program: &[u8]) -> Result<(), Error> {
//     run_inner(&plain::compile(program)?);
//     Ok(())
// }

mod mmap {
    use nix::sys::mman;

    /// A struct that represents a `mmap`ped region of memory.
    /// From `self.ptr` to at least `self.ptr + accesible_len` is readable and writable.
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

// https://github.com/bytecodealliance/wasmtime/issues/15
// https://github.com/bytecodealliance/wasmtime/blob/main/crates/runtime/src/helpers.c#L59
// https://github.com/bytecodealliance/wasmtime/blob/main/crates/runtime/src/traphandlers/unix.rs
// https://github.com/bytecodealliance/wasmtime/blob/main/crates/runtime/src/traphandlers.rs
// mod trap {
//     use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, SIGSEGV, Signal};
//     use std::{
//         ffi,
//         sync::atomic::{AtomicBool, Ordering, AtomicPtr},
//     };
//     use once_cell::race::OnceBox;

//     extern "C" {
//         fn setjmp() -> *const u8;
//         fn longjmp(jmp_buf: *const u8) -> !;
//     }

//     // We don't do multi threading.
//     static IS_JIT: AtomicBool = AtomicBool::new(false);
//     static JMP_BUF: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
//     static PREV_ACTION: OnceBox<SigAction> = OnceBox::new();

//     /// ## Safety
//     /// We may pass `PREV_ACTION` to `sigaction`. The handler must be async-signal-safe.
//     extern "C" fn trap_handler(signal: ffi::c_int, siginfo: *mut libc::siginfo_t, context: *mut ffi::c_void) {
//         if IS_JIT.load(Ordering::Acquire) {
//             IS_JIT.store(false, Ordering::Release);
//             // TODO: Care MacOS?
//             unsafe { longjmp(JMP_BUF.load(Ordering::Acquire)) };
//         } else {
//             let Some(prev_action) = PREV_ACTION.get() else { unreachable!() };
//             match prev_action.handler() {
//                 SigHandler::Handler(h) => (h)(signal),
//                 SigHandler::SigAction(a) => (a)(signal, siginfo, context),
//                 SigHandler::SigDfl | SigHandler::SigIgn => {
//                     unsafe { sigaction(signal.try_into().unwrap(), prev_action) }.unwrap();
//                 },
//             }
//         }
//     }

//     pub fn register_trap_handler() -> nix::Result<()> {
//         let action = SigAction::new(
//             SigHandler::SigAction(trap_handler),
//             SaFlags::SA_ONSTACK | SaFlags::SA_NODEFER,
//             SigSet::empty(),
//         );
//         // TODO: do SIGBUS too for some platform
//         let prev_action = unsafe { sigaction(SIGSEGV, &action) }?;
//         PREV_ACTION.set(Box::new(prev_action)).unwrap();
//         Ok(())
//     }

//     pub fn catch_trap(execute: impl Fn()) {
//         // setjmp
//         IS_JIT.store(true, Ordering::Release);
//         execute();
//         IS_JIT.store(false, Ordering::Release);
//     }
// }
