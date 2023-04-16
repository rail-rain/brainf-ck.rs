#![cfg(target_arch = "x86_64")]

use crate::{
    jit::{getchar, putchar},
    Consumer as _, Error,
};
use memmap2::{Mmap, MmapMut};

fn compile(program: &[u8]) -> Result<Mmap, Error> {
    // Although the length of `program` include comments, it is still a good indicator.
    let mut writer = Vec::with_capacity(program.len());
    let mut loops = Vec::new();
    let mut throwing_dsts = Vec::new();

    // The signature of compiled routine is `fn(*mut u8)`.
    // Since it uses sysv64 calling convention, `rdi` stores the argument.
    // Use that register to store the pointer to the buffer throughtout.

    // Write sysv64's minimum prelude.
    // This preserves the 64-bit base pointer and stack pointer.
    #[rustfmt::skip]
    writer.extend_from_slice(&[
        // 0x50 is for push with a register code. 5 is for rbp.
        0x50 + 5, // push rbp
        // 0x48 is an operand-size prefix for 64-bit, called REX.W.
        // 0x89 is for mov. 0b11_100_101 is for the operands in ModR/M bytes.
        // The last byte has three parts. The first 0b11 is a modifier for changing register values.
        // The second 0b100 (4) means rsp and 0b101 (5) means rbp.
        0x48, 0x89, 0b11_100_101, // mov QWORD rbp, rsp
        // 3 (0b011) is for rbx.
        0x50 + 3, // push rbx
        // "+ 4" is usually rsp, but the 0x41 prefix changes it to r12. It's called REX.B.
        0x41, 0x50 + 4, // push r12
        // rdi is the 7th register (0b111).
        0x48, 0x89, 0b11_111_011, // mov QWORD rbx, rdi
        // 0x4d acts both as REX.W, REX.R and REX.B.
        // REX.B alternates the first part byte while REX.R changes the second part.
        // 0x31 is xor.
        0x4d, 0x31, 0b11_100_100, // xor r12, r12
        // 0xb8 means mov that takes a register and an immediate value.
        0x48, 0xc7, 0b11_000_000, 1, 0, 0, 0 // mov QWORD rax, 1
    ]);

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        match c {
            b'>' => {
                let amount = iter.consume_while(b'>') + 1;
                // 0x81 has an opcode exntension to switch 7 operations.
                // The last byte is kind of a ModR/M byte where the second part is for add.
                writer.extend_from_slice(&[0x41, 0x81, 0b11_000_100]); // add r12d,
                writer.extend_from_slice(&(amount as i32).to_ne_bytes());
                // 0x45 is REX.B and REX.R. 0x0fb7 is for movzx.
                writer.extend_from_slice(&[0x45, 0x0f, 0xb7, 0b11_100_100]); // mov r12d, r12w
            }
            b'<' => {
                let amount = iter.consume_while(b'<') + 1;
                // sub is 0b101 (5).
                writer.extend_from_slice(&[0x41, 0x81, 0b11_101_100]); // sub r12d,
                writer.extend_from_slice(&(amount as i32).to_ne_bytes());
                writer.extend_from_slice(&[0x45, 0x0f, 0xb7, 0b11_100_100]); // mov r12d, r12w
            }
            b'+' => {
                let amount = iter.consume_while(b'+') + 1;
                // 0x42 is REX.X, which alternates the displacement register of the SIB.
                // 0x80 is the 8 bit version of 0x81.
                // The 0b00 modifier means one operand is a pointer to the value wanted.
                // 0x100 in the last three byte of ModR/M doesn't specify registers. It says I'm using the SIB byte.
                // The SIB byte follows ModR/M and specifies the base and displacement registers.
                // The first two bit of SIB changes the scale of displacement.
                writer.extend_from_slice(&[0x42, 0x80, 0b00_000_100, 0b00_100_011]); // add BYTE [rbx + r12],
                writer.extend_from_slice(&(amount as i8).to_ne_bytes());
            }
            b'-' => {
                let amount = iter.consume_while(b'-') + 1;
                writer.extend_from_slice(&[0x42, 0x80, 0b00_101_100, 0b00_100_011]); // sub BYTE [rbx + r12],
                writer.extend_from_slice(&(amount as i8).to_ne_bytes());
            }
            b'.' => {
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    0x4a, 0x8d, 0b00_111_100, 0b00_100_011, // lea QWORD rdi, [rbx + r12]
                    // 0xb8 is for mov with a register code.
                    0x48, 0xb8 + 0, // mov rax, QWORD
                ]);
                writer.extend_from_slice(&(putchar as u64).to_ne_bytes());
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    // 0xff is for call when the ModR/M byte says 2 (0b010).
                    0xff, 0b11_010_000, // call rax
                    // 0x3c is cmp that only takes al.
                    0x3c, 0, // cmp al, 0
                    // 0x0f84 is a two byte operand and the 32 bit version of 0x74 as the opcode could be long.
                    0x0f, 0x84, // jz
                    0, 0, 0, 0 // stub for the relocation offset.
                ]);
                throwing_dsts.push(writer.len() - 4..writer.len());
            }
            b',' => {
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    0x4a, 0x8d, 0b00_111_100, 0b00_100_011, // lea QWORD rdi, [rbx + r12]
                    0x48, 0xb8 + 0, // mov rax, QWORD
                ]);
                writer.extend_from_slice(&(getchar as u64).to_ne_bytes());
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    0xff, 0b11_010_000, // call rax
                    0x3c, 0, // cmp al, 0
                    0x0f, 0x84, // jz
                    0, 0, 0, 0 // stub for the relocation offset.
                ]);
                throwing_dsts.push(writer.len() - 4..writer.len());
            }
            b'[' => {
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    0x42, 0x80, 0b00_111_100, 0b00_100_011,
                    0, // cmp BYTE [rbx + r12], 0
                    // 0x0f, 0x84 is je.
                    0x0f, 0x84, // je
                ]);
                let fwd_label_dst = writer.len()..writer.len() + 4;
                writer.extend_from_slice(&[0; 4]);
                loops.push(fwd_label_dst);
            }
            b']' => {
                let fwd_label_dst = loops.pop().ok_or(Error::UnmatchedRight)?;
                #[rustfmt::skip]
                writer.extend_from_slice(&[
                    0x42, 0x80, 0b00_111_100, 0b00_100_011,
                    0, // cmp BYTE [rbx + r12], 0
                    0x0f, 0x85, // jne
                ]);
                // je and jne use relative locations to jump
                // Subtract the start of the backward label location from
                // the start of the forward label location to get the difference.
                let bwd_label = fwd_label_dst.start as i32 - writer.len() as i32;
                writer.extend_from_slice(&bwd_label.to_ne_bytes());
                let fwd_label = -bwd_label;
                writer[fwd_label_dst].copy_from_slice(&fwd_label.to_ne_bytes());
            }
            _ => {}
        }
    }

    if !loops.is_empty() {
        return Err(Error::UnmatchedLeft);
    }

    for throwing_dst in throwing_dsts {
        let fwd_label = writer.len() as i32 - throwing_dst.start as i32 - 4;
        writer[throwing_dst].copy_from_slice(&fwd_label.to_ne_bytes());
    }

    // Write sysv64's postlude.
    // This undoes the prelude.
    #[rustfmt::skip]
    writer.extend_from_slice(&[
        // 0x58 + 3 is for pop with a register code added.
        0x41, 0x58 + 4, // pop r12
        0x58 + 3, // pop rbx

        0x48, 0x89, 0b11_101_100, // mov QWORD rsp, rbp
        0x58 + 5,     // pop rbp
        0xc3,         // ret
    ]);

    // The use of `mmap` is neccessary as POSIX defines `mprotect` only for `mmap`.
    let mut opcode = MmapMut::map_anon(writer.len())?;
    opcode.copy_from_slice(&writer);
    Ok(opcode.make_exec()?)
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let opcode = compile(program)?;
    super::run_opcode(opcode.as_ref())
}
