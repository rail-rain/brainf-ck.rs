use crate::parser::Ins;
use std::{io, slice};

fn write_u8(buf: &u8, to: &mut impl io::Write) -> io::Result<()> {
    to.write_all(slice::from_ref(buf))
}

fn read_u8(buf: &mut u8, from: &mut impl io::Read) -> io::Result<()> {
    from.read_exact(slice::from_mut(buf))
}

pub enum Op {
    IncPtr { amount: usize },
    DecPtr { amount: usize },
    IncCell { amount: u8 },
    DecCell { amount: u8 },
    Output,
    Input,
    JmpFwd { to: usize },
    JmpBwd { to: usize },
}

fn compile(program: impl Iterator<Item = Ins>) -> Vec<Op> {
    let mut buf = Vec::new();
    let mut loops = Vec::new();

    for (pos, c) in program.enumerate() {

        let op = match c {
            Ins::IncPtr { amount } => Op::IncPtr { amount },
            Ins::DecPtr { amount } => Op::DecPtr { amount },
            Ins::IncCell { amount } => Op::IncCell { amount },
            Ins::DecCell { amount } => Op::DecCell { amount },
            Ins::Output => Op::Output,
            Ins::Input => Op::Input,
            Ins::JmpFwd => {
                loops.push(pos);
                Op::JmpFwd { to: 0 } // stub
            }
            Ins::JmpBwd => {
                let start_pos = loops.pop().unwrap();
                buf[start_pos] = Op::JmpFwd { to: pos + 1 };
                Op::JmpBwd { to: start_pos }
            }
        };

        buf.push(op);
    }

    buf
}

pub fn run(
    program: impl Iterator<Item = Ins>,
    mut output: impl io::Write,
    mut input: impl io::Read,
) -> io::Result<()> {
    let mut array = [0u8; 30_000];
    let mut pointer = 0;

    let ops = compile(program);

    let mut pos = 0;
    while let Some(c) = ops.get(pos) {
        match c {
            Op::IncPtr { amount } => pointer += amount,
            Op::DecPtr { amount } => pointer -= amount,
            Op::IncCell { amount } => array[pointer] += amount,
            Op::DecCell { amount } => array[pointer] -= amount,
            Op::Output => write_u8(&array[pointer], &mut output)?,
            Op::Input => {
                output.flush()?;
                read_u8(&mut array[pointer], &mut input)?;
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
        pos += 1;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io;
    #[test]
    fn hello_world() {
        let program = b"++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
        let mut output = Vec::with_capacity(13);
        run(crate::parser::parse(program), &mut output, io::empty()).unwrap();
        assert_eq!(output, b"Hello World!\n");
    }
}
