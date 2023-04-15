use crate::{getchar, putchar, Consumer as _, Error};

#[derive(Clone, Copy)]
enum Ins {
    IncPtr { amount: u16 },
    DecPtr { amount: u16 },
    IncCell { amount: u8 },
    DecCell { amount: u8 },
    Output,
    Input,
    JmpFwd { to: usize },
    JmpBwd { to: usize },
    End,
}

fn compile(program: &[u8]) -> Result<Vec<Ins>, Error> {
    let mut instructions = Vec::with_capacity(program.len() / 2);
    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        let ins = match c {
            b'>' => Ins::IncPtr {
                amount: (iter.consume_while(b'>') + 1) as u16,
            },
            b'<' => Ins::DecPtr {
                amount: (iter.consume_while(b'<') + 1) as u16,
            },
            b'+' => Ins::IncCell {
                amount: iter.consume_while(b'+') as u8 + 1,
            },
            b'-' => Ins::DecCell {
                amount: iter.consume_while(b'-') as u8 + 1,
            },
            b'.' => Ins::Output,
            b',' => Ins::Input,
            b'[' => {
                loops.push(instructions.len());
                Ins::JmpFwd { to: 0 } // stub
            }
            b']' => {
                let start_pos = loops.pop().ok_or(Error::UnmatchedRight)?;
                instructions[start_pos] = Ins::JmpFwd {
                    to: instructions.len() + 1,
                };
                Ins::JmpBwd { to: start_pos }
            }
            _ => continue,
        };

        instructions.push(ins);
    }

    if loops.is_empty() {
        instructions.push(Ins::End);
        Ok(instructions)
    } else {
        Err(Error::UnmatchedLeft)
    }
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let mut array = [0u8; u16::MAX as usize + 1];
    let mut pointer = 0u16;

    let instructions = compile(program)?;

    let mut programming_counter = 0;
    loop {
        // The programming counter should always be in-bounds as
        // it increments by one, there's `Ins::End` at the end of the list
        // and `Ins::JmpFwd/Bwd { to }` is in-bounds.
        // FIXME: introduce a proptester, verifier or fuzzer to find an UB here.
        // With `proptest`, I'm not sure what to do with inputs that cause infinite loop.
        let ins = *unsafe { instructions.get_unchecked(programming_counter) };
        programming_counter += 1;
        match ins {
            Ins::IncPtr { amount } => pointer = pointer.wrapping_add(amount),
            Ins::DecPtr { amount } => pointer = pointer.wrapping_sub(amount),
            Ins::IncCell { amount } => {
                array[pointer as usize] = array[pointer as usize].wrapping_add(amount)
            }
            Ins::DecCell { amount } => {
                array[pointer as usize] = array[pointer as usize].wrapping_sub(amount)
            }
            Ins::Output => putchar(&array[pointer as usize])?,
            Ins::Input => getchar(&mut array[pointer as usize])?,
            Ins::JmpFwd { to } => {
                if array[pointer as usize] == 0 {
                    programming_counter = to;
                }
            }
            Ins::JmpBwd { to } => {
                if array[pointer as usize] != 0 {
                    programming_counter = to;
                }
            }
            Ins::End => break,
        }
    }

    Ok(())
}
