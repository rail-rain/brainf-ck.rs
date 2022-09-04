// TODO: do testing with http://www.brainfuck.org/tests.b.
// unit test or integration test?

mod jit_dynasm;
mod jit_plain;

use std::{io::{self, Read, Write}, mem};

trait Consumer {
    fn consume_while(&mut self, target: u8) -> usize;
}

impl Consumer for std::slice::Iter<'_, u8> {
    #[inline(always)]
    fn consume_while(&mut self, target: u8) -> usize {
        // Calculate the span of continuous copies of the target.
        let mut clone = self.clone();
        while clone.next().copied() == Some(target) {}
        let span = (self.len() - clone.len()).saturating_sub(1);

        // TODO: unstable `Iterator::advance_by` is better.
        // Need https://github.com/rust-lang/rust/issues/77404
        if span != 0 {
            self.nth(span - 1);
        }
        // self.advance_by(span);
        return span;
    }
}

pub extern "sysv64" fn putchar(char: u8) {
    io::stdout().write_all(&[char]).unwrap()
}

pub extern "sysv64" fn getchar() -> u8 {
    let mut buf = [0; 1];
    io::stdin().read_exact(&mut buf).unwrap();
    buf[0]
}

pub fn run(opcode: impl AsRef<[u8]>) {
    inner(opcode.as_ref());
    fn inner(opcode: &[u8]) {
        // The safety depends on the correctness of the compilers. How dangerous.
        let execute: extern "sysv64" fn(*mut u8) = unsafe { mem::transmute(opcode.as_ptr()) };
        
        let mut array = [0; 30_000];
        execute(array.as_mut_ptr());
    }
}

fn main() {
    // use std::io;

    // let mut buf = String::new();
    // io::stdin().read_line(&mut buf).unwrap();
    // interpreter::run(buf.as_bytes(), io::stdout(), io::stdin()).unwrap();

    let program = b"++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";

    // pretty_assertions::assert_eq!(&*jit_dynasm::compile(parse(program)), &*jit_plain::compile(parse(program)));
    run(&*jit_dynasm::compile(program));
    run(jit_plain::compile(program));

}
