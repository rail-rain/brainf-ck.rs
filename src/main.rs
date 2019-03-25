use std::io::{self, Write, Read};

enum Instruction {
    IncrementPointer,
    DecrementPointer,
    IncrementData,
    DecrementData,
    Output,
    Input,
    LoopStart,
    LoopEnd(usize),
}

fn run(instructions: &[u8]) {
    let mut instruction_pointer = 0;
    let mut data = [0u8; 30_000];
    let mut data_pointer = 0;
    let mut stamp = Vec::new();

    while instruction_pointer == instructions.len() {
        match instructions[instruction_pointer] {
            b'>' => data_pointer += 1,
            b'<' => data_pointer -= 1,
            b'+' => data[data_pointer] += 1,
            b'-' => data[data_pointer] -= 1,
            b'.' => io::stdout().write_all(&data[data_pointer..data_pointer+1]).unwrap(),
            b',' => io::stdin().read_exact(&mut data[data_pointer..data_pointer+1]).unwrap(),
            b'[' => if data[data_pointer] == 0 {
                unimplemented!()
            } else {
                stamp.push(instruction_pointer)
            },
            b']' => if data[data_pointer] != 0 {
                instruction_pointer = *stamp.last().unwrap()
            } else {
                stamp.pop();
            },
            _ => ()
        }
        instruction_pointer += 1;
    }
}

fn main() {
    println!("Hello, world!");
}
