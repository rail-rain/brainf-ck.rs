use crate::{
    jit::{getchar, putchar, run_opcode},
    Consumer as _, Error,
};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi, ExecutableBuffer};

macro_rules! my_dynasm {
    ($ops:ident $($t:tt)*) => {
        dynasm!($ops
            ; .arch aarch64
            ; .alias ptr, x19
            ; .alias idx, w20
            ; .alias xidx, x20
            $($t)*
        )
    }
}

fn compile(program: &[u8]) -> Result<ExecutableBuffer, Error> {
    let mut ops = dynasmrt::aarch64::Assembler::new()?;

    my_dynasm!(ops
        ; sub sp, sp, #32 // allocate an enough stack
        ; str x30, [sp, #16] // save a special register

        ; stp ptr, xidx, [sp] // save callee-saved register
        ; mov ptr, x0
        ; mov idx, wzr // Set the array index to 0
        ; mov w0, #1 // Set the initial return value to 1 in case no io happens.
    );

    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        match c {
            b'>' => my_dynasm!(ops
                ; add idx, idx, (iter.consume_while(b'>') + 1) as u32
                // Make sure the index stays within 16 bit values for memory protection.
                // (There's no such thing as 16 bit registers. Zero-extension is the only way.)
                // `add` has an option to perform `uxth`, but that's a bit different from what I'm doing.
                ; uxth idx, idx
            ),
            b'<' => my_dynasm!(ops
                ; sub idx, idx, (iter.consume_while(b'<') + 1) as u32
                ; uxth idx, idx
            ),
            b'+' => my_dynasm!(ops
                ; ldrb w9, [ptr, xidx]
                ; add w9, w9, (iter.consume_while(b'+') + 1) as u32
                ; strb w9, [ptr, xidx]
            ),
            b'-' => my_dynasm!(ops
                ; ldrb w9, [ptr, xidx]
                ; sub w9, w9, (iter.consume_while(b'-') + 1) as u32
                ; strb w9, [ptr, xidx]
            ),
            b'.' => my_dynasm!(ops
                ; add x0, ptr, idx
                ; ldr x9, ->putchar_off // use load-literal as a function pointer is too large
                ; blr x9
                ; cbz w0, ->throwing
            ),
            b',' => my_dynasm!(ops
                ; add x0, ptr, idx
                ; ldr x9, ->getchar_off
                ; blr x9
                ; cbz w0, ->throwing
            ),
            b'[' => {
                let bwd_label = ops.new_dynamic_label();
                let fwd_label = ops.new_dynamic_label();
                loops.push((bwd_label, fwd_label));
                my_dynasm!(ops
                    ; ldrb w9, [ptr, xidx]
                    ; cbz w9, =>fwd_label
                    ;=>bwd_label
                )
            }
            b']' => {
                let (bwd_label, fwd_label) = loops.pop().ok_or(Error::UnmatchedRight)?;
                my_dynasm!(ops
                    ; ldrb w9, [ptr, xidx]
                    ; cbnz w9, =>bwd_label
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
        // Keep `x0` set by `putchar` and `getchar` functions as it is for the return value.
        ;->throwing:
        ; ldp ptr, xidx, [sp]

        ; ldr x30, [sp, #16]
        ; add sp, sp, #32
        ; ret
        
        // Literal pool to store 64 bit constants:
        ; ->putchar_off:
        ; .qword putchar as _
        ; ->getchar_off:
        ; .qword getchar as _
    );

    Ok(ops.finalize().expect("Finalising the exec buffer failed"))
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let opcode = compile(program)?;
    run_opcode(opcode.as_ref())
}
