use crate::{getchar, putchar, Consumer as _};

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

fn compile(program: &[u8]) -> Vec<Op> {
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
                let start_pos = loops.pop().expect("unmatching ]");
                operands[start_pos] = Op::JmpFwd {
                    to: operands.len() + 1,
                };
                Op::JmpBwd { to: start_pos }
            }
            _ => continue,
        };

        if !loops.is_empty() {
            todo!()
        }

        operands.push(op);
    }

    operands
}

pub fn run(program: &[u8]) {
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
        pos += 1;
    }
}

#[cfg(test)]
mod test {
    // #[test]
    // fn hello_world() {
    //     let program = b"++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
    //     run(program);
    //     assert_eq!(output, b"Hello World!\n");
    // }
}
