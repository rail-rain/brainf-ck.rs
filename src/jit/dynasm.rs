#![cfg(target_arch = "x86_64")]

use crate::{
    jit::{getchar, putchar},
    Consumer as _,
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

pub fn compile(program: &[u8]) -> ExecutableBuffer {
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
                if let Some((bwd_label, fwd_label)) = loops.pop() {
                    my_dynasm!(ops
                        ; cmp BYTE [pointer], 0
                        ; jne =>bwd_label
                        ;=>fwd_label
                    )
                }
            }
            _ => {}
        }
    }

    my_dynasm!(ops
        ; pop pointer

        ; mov rsp, rbp
        ; pop rbp
        ; ret
    );

    ops.finalize().unwrap()
}
