use crate::{getchar, putchar, Consumer as _, Error};

enum Op {
    IncPtr { amount: usize },
    DecPtr { amount: usize },
    IncCell { amount: u8 },
    DecCell { amount: u8 },
    Output,
    Input,
    JmpFwd { to: usize },
    JmpBwd { to: usize },
}

fn compile(program: &[u8]) -> Result<Vec<Op>, Error> {
    let mut operands = Vec::with_capacity(program.len() / 2);
    let mut loops = Vec::new();

    let mut iter = program.iter();
    while let Some(&c) = iter.next() {
        let op = match c {
            b'>' => Op::IncPtr {
                amount: iter.consume_while(b'>') + 1,
            },
            b'<' => Op::DecPtr {
                amount: iter.consume_while(b'<') + 1,
            },
            b'+' => Op::IncCell {
                amount: iter.consume_while(b'+') as u8 + 1,
            },
            b'-' => Op::DecCell {
                amount: iter.consume_while(b'-') as u8 + 1,
            },
            b'.' => Op::Output,
            b',' => Op::Input,
            b'[' => {
                loops.push(operands.len());
                Op::JmpFwd { to: 0 } // stub
            }
            b']' => {
                let start_pos = loops.pop().ok_or(Error::UnmatchedRight)?;
                operands[start_pos] = Op::JmpFwd {
                    to: operands.len() + 1,
                };
                Op::JmpBwd { to: start_pos }
            }
            _ => continue,
        };

        operands.push(op);
    }

    if loops.is_empty() {
        Ok(operands)
    } else {
        Err(Error::UnmatchedLeft)
    }
}

pub fn run(program: &[u8]) -> Result<(), Error> {
    let mut array = [0u8; 30_000];
    let mut pointer = 0;

    let ops = compile(program)?;

    let mut pos = 0;
    while let Some(c) = ops.get(pos) {
        pos += 1;
        match c {
            Op::IncPtr { amount } => pointer += amount,
            Op::DecPtr { amount } => pointer -= amount,
            Op::IncCell { amount } => array[pointer] += amount,
            Op::DecCell { amount } => array[pointer] -= amount,
            Op::Output => putchar(array[pointer]),
            Op::Input => {
                array[pointer] = getchar();
            }
            Op::JmpFwd { to } => {
                if array[pointer] == 0 {
                    pos = *to;
                }
            }
            Op::JmpBwd { to } => {
                if array[pointer] != 0 {
                    pos = *to;
                }
            }
        }
    }

    Ok(())
}
