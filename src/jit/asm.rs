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

fn compile(program: &[u8]) -> Result<ExecutableBuffer, Error> {
    let mut ops = dynasmrt::x64::Assembler::new()?;

    my_dynasm!(ops
        ; push rbp
        ; mov rbp, rsp

        ; push pointer
        ; push r12
        ; mov pointer, rdi
        ; xor r12, r12 // Set the array index to 0
        ; mov rax, 1 // Set the initial return value to 1 in case no io happens.
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
                ; cmp al, 0
                ; jz ->throwing
            ),
            b',' => my_dynasm!(ops
                ; lea rdi, [pointer + r12]
                ; mov rax, QWORD getchar as _
                ; call rax
                ; cmp al, 0
                ; jz ->throwing
            ),
            b'[' => {
                let bwd_label = ops.new_dynamic_label();
                let fwd_label = ops.new_dynamic_label();
                loops.push((bwd_label, fwd_label));
                my_dynasm!(ops
                    ; cmp BYTE [pointer + r12], 0
                    ; jz =>fwd_label
                    ;=>bwd_label
                )
            }
            b']' => {
                let (bwd_label, fwd_label) = loops.pop().ok_or(Error::UnmatchedRight)?;
                my_dynasm!(ops
                    ; cmp BYTE [pointer + r12], 0
                    ; jnz =>bwd_label
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
        // Keep `rax` set by `putchar` and `getchar` functions as it is for the return value.
        ;->throwing:
        ; pop r12
        ; pop pointer

        ; mov rsp, rbp
        ; pop rbp
        ; ret
    );

    Ok(ops.finalize().expect("Finalising the exec buffer failed"))
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let opcode = compile(program)?;
    super::run_opcode(opcode.as_ref())
}