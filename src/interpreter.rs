use crate::{getchar, putchar, Consumer as _, Error};

enum Ins {
    IncPtr { amount: usize },
    DecPtr { amount: usize },
    IncCell { amount: u8 },
    DecCell { amount: u8 },
    Output,
    Input,
    JmpFwd { to: usize },
    JmpBwd { to: usize },
}

fn compile(program: &[u8]) -> Result<Vec<Ins>, Error> {
    let mut instructions = Vec::with_capacity(program.len() / 2);
    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        let ins = match c {
            b'>' => Ins::IncPtr {
                amount: iter.consume_while(b'>') + 1,
            },
            b'<' => Ins::DecPtr {
                amount: iter.consume_while(b'<') + 1,
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
        Ok(instructions)
    } else {
        Err(Error::UnmatchedLeft)
    }
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let mut array = [0u8; 30_000];
    let mut pointer = 0usize;

    let instructions = compile(program)?;

    let mut pos = 0;
    while let Some(i) = instructions.get(pos) {
        pos += 1;
        match i {
            Ins::IncPtr { amount } => if pointer + amount < array.len() {
                pointer += amount;
            } else {
                // TODO: Return more nuanced error?
                return Ok(());
            },
            Ins::DecPtr { amount } => if let Some(n) = pointer.checked_sub(*amount) {
                pointer = n
            } else {
                // TODO: Return more nuanced error?
                return Ok(());
            },
            Ins::IncCell { amount } => array[pointer] = array[pointer].overflowing_add(*amount).0,
            Ins::DecCell { amount } => array[pointer] = array[pointer].overflowing_sub(*amount).0,
            Ins::Output => putchar(&array[pointer])?,
            Ins::Input => getchar(&mut array[pointer])?,
            Ins::JmpFwd { to } => {
                if array[pointer] == 0 {
                    pos = *to;
                }
            }
            Ins::JmpBwd { to } => {
                if array[pointer] != 0 {
                    pos = *to;
                }
            }
        }
    }

    Ok(())
}
