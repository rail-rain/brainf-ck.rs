use crate::{getchar, putchar, Consumer};
use memmap2::{MmapMut, Mmap};
use std::mem;

trait SliceExt {
    fn write(&mut self, data: &[u8]);
}

impl SliceExt for &mut [u8] {
    // This is an infallible version of `io::Write for &mut [u8]`.
    // It also fails when `data` is longer than `self` as it is a logical error for this module.
    fn write(&mut self, data: &[u8]) {
        assert!(data.len() <= self.len());
        let (a, b) = mem::replace(self, &mut []).split_at_mut(data.len());
        a.copy_from_slice(data);
        *self = b;
    }
}

pub fn compile(program: &[u8]) -> Mmap {
    // The use of `mmap` is neccessary as POSIX defines `mprotect` only for `mmap`.
    let mut opcode = MmapMut::map_anon(512).unwrap();
    let mut writer = opcode.as_mut();
    let mut loops = Vec::new();

    // The signature of compiled routine is `fn(*mut u8)`.
    // Since it uses sysv64 calling convention, `rdi` stores the argument.
    // I use that register to store the pointer to the buffer throughtout.

    // Write sysv64's minimum prelude.
    // This preserves the 64-bit base pointer and stack pointer.
    writer.write(&[
        // 0x50 is for push with a register code. 5 is for rbp.
        0x50 + 5, // push rbp
        // 0x48 is an operand-size prefix for 64-bit.
        // 0x89 is for mov. 0b11_100_101 is for the operands in ModR/M bytes.
        // The last byte has three parts. The first 0b11 is a modifier for changing register values.
        // The second 0b100 (4) means rsp and 0b101 (5) means rbp.
        0x48, 0x89, 0b11_100_101, // mov QWORD rbp, rsp

        // 3 (0b011) is for rbx.
        0x50 + 3, // push rbx
        // rdi is the 7th register (0b111).
        0x48, 0x89, 0b11_111_011 // mov QWORD rbx, rdi
    ]);

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        match c {
            b'>' => {
                let amount = iter.consume_while(b'>') + 1;
                // 0x81 has an opcode exntension to switch 7 operations.
                // The last byte is kind of a ModR/M byte where the second part is for add.
                writer.write(&[0x48, 0x81, 0b11_000_011]); // add QWORD rbx
                writer.write(&(amount as i32).to_ne_bytes());
            }
            b'<' => {
                let amount = iter.consume_while(b'<') + 1;
                // sub is 0b101 (5).
                writer.write(&[0x48, 0x81, 0b11_101_011]); // sub QWORD rbx
                writer.write(&(amount as i32).to_ne_bytes());
            }
            b'+' => {
                let amount = iter.consume_while(b'+') + 1;
                // 0x80 works similar to 0x81.
                // The 0b00 modifier means one operand is a pointer to the value wanted.
                writer.write(&[0x80, 0b00_000_011]); // add BYTE [rbx],
                writer.write(&(amount as i8).to_ne_bytes());
            }
            b'-' => {
                let amount = iter.consume_while(b'-') + 1;
                writer.write(&[0x80, 0b00_101_011]); // sub BYTE [rbx],
                writer.write(&(amount as i8).to_ne_bytes());
            }
            b'.' => {
                writer.write(&[
                    // 0x8b is for mov that is like 0x89.
                    0x48, 0x8b, 0b00_111_011, // mov QWORD rdi, [rbx]
                    // 0xb8 is for mov with a register code.
                    0x48, 0xb8 + 0, // mov rax, QWORD
                ]);
                writer.write(&(putchar as u64).to_ne_bytes());
                writer.write(&[
                    // 0xff is for call when the ModR/M byte says 2 (0b010).
                    0xff, 0b11_010_000, // call rax
                ]);
            }
            b',' => {
                writer.write(&[
                    0x48, 0xb8 + 0, // mov rax, QWORD
                ]);
                writer.write(&(getchar as u64).to_ne_bytes());
                writer.write(&[
                    0xff, 0b11_010_000, // call rax
                    0x48, 0x89, 0b00_000_011, // mov [rbx], rax
                ]);
            }
            b'[' => {
                writer.write(&[
                    0x80, 0b00_111_011, 0, // cmp BYTE [rbx], 0
                    // The onces that start from 0x0f are two byte operands.
                    // 0x0f, 0x84 is je.
                    0x0f, 0x84, // je
                ]);
                let (fwd_label_dst, tmp) = writer.split_at_mut(4);
                writer = tmp;
                loops.push((writer.as_ptr() as i32, fwd_label_dst));
            }
            b']' => {
                if let Some((abs_bwd_label, fwd_label_dst)) = loops.pop() {
                    writer.write(&[
                        0x80, 0b00_111_011, 0, // cmp BYTE [rbx], 0
                        0x0f, 0x85, // jne
                    ]);
                    // je and jne use relative locations to jump.
                    let bwd_label = abs_bwd_label - (writer.as_ptr() as i32) - 4;
                    writer.write(&bwd_label.to_ne_bytes());
                    let fwd_label = -bwd_label;
                    fwd_label_dst.copy_from_slice(&fwd_label.to_ne_bytes());
                } else {
                    todo!("error: unmatching brackets");
                }
            }
            _ => {}
        }
    }

    if !loops.is_empty() {
        todo!("error");
    }

    // Write sysv64's postlude.
    // This undoes the prelude.
    writer.write(&[
        // 0x58 + 3 is for pop with a register code added.
        0x58 + 3, // pop rbx

        0x48, 0x89, 0b11_101_100, // mov QWORD rsp, rbp
        0x58 + 5, // pop rbp
        0xc3, // ret
    ]);

    opcode.make_exec().unwrap()
}
