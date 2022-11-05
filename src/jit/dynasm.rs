#![cfg(target_arch = "x86_64")]

use crate::{
    jit::{getchar, putchar},
    Consumer as _, Error,
};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi, ExecutableBuffer};

macro_rules! my_dynasm {
    ($ops:ident $($t:tt)*) => {
        dynasm!($ops
            ; .arch x64
            ; .alias pointer, rbx
            $($t)*
        )
    }
}

pub fn compile(program: &[u8]) -> Result<ExecutableBuffer, Error> {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    my_dynasm!(ops
        ; push rbp
        ; mov rbp, rsp

        ; push pointer
        ; push r12
        ; mov pointer, rdi
        ; xor r12, r12 // Set the array index to 0
    );

    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        match c {
            b'>' => my_dynasm!(ops
                ; add r12d, (iter.consume_while(b'>') + 1) as _
                // Make sure the index stays within 16 bit values for memory protection.
                // Use zero-extension instead of writing directly to 16 bit register
                // See https://stackoverflow.com/questions/34058101/referencing-the-contents-of-a-memory-location-x86-addressing-modes
                ; movzx r12d, r12w
            ),
            b'<' => my_dynasm!(ops
                ; sub r12d, (iter.consume_while(b'<') + 1) as _
                ; movzx r12d, r12w
            ),
            b'+' => my_dynasm!(ops; add BYTE [pointer + r12], (iter.consume_while(b'+') + 1) as _),
            b'-' => my_dynasm!(ops; sub BYTE [pointer + r12], (iter.consume_while(b'-') + 1) as _),
            b'.' => my_dynasm!(ops
                ; lea rdi, [pointer + r12]
                ; mov rax, QWORD putchar as _
                ; call rax
            ),
            b',' => my_dynasm!(ops
                ; lea rdi, [pointer + r12]
                ; mov rax, QWORD getchar as _
                ; call rax
            ),
            b'[' => {
                let bwd_label = ops.new_dynamic_label();
                let fwd_label = ops.new_dynamic_label();
                loops.push((bwd_label, fwd_label));
                my_dynasm!(ops
                    ; cmp BYTE [pointer + r12], 0
                    ; je =>fwd_label
                    ;=>bwd_label
                )
            }
            b']' => {
                let (bwd_label, fwd_label) = loops.pop().ok_or(Error::UnmatchedRight)?;
                my_dynasm!(ops
                    ; cmp BYTE [pointer + r12], 0
                    ; jne =>bwd_label
                    ;=>fwd_label
                )
            }
            _ => {}
        }
    }

    if !loops.is_empty() {
        return Err(Error::UnmatchedLeft);
    }

    my_dynasm!(ops
        ; pop r12
        ; pop pointer

        ; mov rsp, rbp
        ; pop rbp
        ; ret
    );

    Ok(ops.finalize().unwrap())
}
