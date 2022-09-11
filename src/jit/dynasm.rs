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
        ; mov pointer, rdi
    );

    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        match c {
            b'>' => my_dynasm!(ops; add pointer, (iter.consume_while(b'>') + 1) as _),
            b'<' => my_dynasm!(ops; sub pointer, (iter.consume_while(b'<') + 1) as _),
            b'+' => my_dynasm!(ops; add BYTE [pointer], (iter.consume_while(b'+') + 1) as _),
            b'-' => my_dynasm!(ops; sub BYTE [pointer], (iter.consume_while(b'-') + 1) as _),
            b'.' => my_dynasm!(ops
                ; mov rdi, [pointer]
                ; mov rax, QWORD putchar as _
                ; call rax
            ),
            b',' => my_dynasm!(ops
                ; mov rax, QWORD getchar as _
                ; call rax
                ; mov [pointer], rax
            ),
            b'[' => {
                let bwd_label = ops.new_dynamic_label();
                let fwd_label = ops.new_dynamic_label();
                loops.push((bwd_label, fwd_label));
                my_dynasm!(ops
                    ; cmp BYTE [pointer], 0
                    ; je =>fwd_label
                    ;=>bwd_label
                )
            }
            b']' => {
                let (bwd_label, fwd_label) = loops.pop().ok_or(Error::UnmatchedRight)?;
                my_dynasm!(ops
                    ; cmp BYTE [pointer], 0
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
        ; pop pointer

        ; mov rsp, rbp
        ; pop rbp
        ; ret
    );

    Ok(ops.finalize().unwrap())
}
