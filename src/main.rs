#![warn(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "interpreter")]
mod interpreter;
#[cfg(any(feature = "asm", feature = "machine"))]
mod jit;

use argh::FromArgs;
use std::{
    array,
    io::{self, Read, Write},
    str::FromStr,
};
use thiserror::Error;

pub(crate) trait Consumer {
    fn consume_while(&mut self, target: u8) -> usize;
}

impl Consumer for std::slice::Iter<'_, u8> {
    #[inline(always)]
    fn consume_while(&mut self, target: u8) -> usize {
        // Calculate the span of continuous copies of the target.
        let span = self.clone().take_while(|c| **c == target).count();

        // FIXME: unstable `Iterator::advance_by` is better.
        // Need https://github.com/rust-lang/rust/issues/77404
        if span != 0 {
            self.nth(span - 1);
        }
        // self.advance_by(span);
        return span;
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("unmatched [")]
    UnmatchedLeft,
    #[error("unmatched ]")]
    UnmatchedRight,
    #[error("io-error during execution")]
    Io(#[from] io::Error),
}

/// Writes `byte` into the stdout.
/// A few advantages of this over directly using `libstd`:
///
/// - a more convinient API to write only one byte.
/// - converting "\n" to "\r\n" in Windows.
#[inline(always)]
pub(crate) fn putchar(byte: &u8) -> io::Result<()> {
    #[cfg(test)]
    return test::OUT.with(|writer| inner(&mut *writer.borrow_mut(), byte));
    #[cfg(not(test))]
    {
        let mut writer = io::stdout();
        inner(&mut writer, byte)?;
        return writer.flush();
    }

    fn inner(writer: &mut impl Write, byte: &u8) -> io::Result<()> {
        if cfg!(windows) && *byte == b'\n' {
            writer.write_all(&[b'\r', b'\n'])
        } else {
            writer.write_all(array::from_ref(byte))
        }
    }
}

/// Reads one byte from the stdin and writes it to `byte`.
/// A few advantages of this over directly using `libstd`:
///
/// - a more convinient API to read only one byte.
/// - skipping "\r" in Windows to make "\n" a single newline sequence.
#[inline(always)]
pub(crate) fn getchar(byte: &mut u8) -> io::Result<()> {
    #[cfg(test)]
    return test::IN.with(|reader| inner(&mut *reader.borrow_mut(), byte));
    #[cfg(not(test))]
    return inner(&mut io::stdin(), byte);

    fn inner(reader: &mut impl Read, byte: &mut u8) -> io::Result<()> {
        let res = reader.read_exact(array::from_mut(byte));

        match res {
            Ok(_) => {
                if cfg!(windows) && *byte == b'\r' {
                    // We're assuming there's '\n' after '\r'. Even if there isn't, this skips '\r'.
                    // Also, we call `UnexpectedEof` an error too. Basically, anything other than "\r\n" is unexpected.
                    reader.read_exact(array::from_mut(byte))?;
                }
            }
            // The value of `buf` is "unspecified" when `UnexpectedEof` happens,
            // Make sure it is 0 to be consistent.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => *byte = 0,
            r @ Err(_) => return r,
        };

        Ok(())
    }
}

enum EngineType {
    #[cfg(feature = "interpreter")]
    Interpreter,
    #[cfg(feature = "machine")]
    Machine,
    #[cfg(feature = "asm")]
    Asm,
}

impl FromStr for EngineType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            #[cfg(feature = "interpreter")]
            "interpreter" => Ok(Self::Interpreter),
            #[cfg(feature = "machine")]
            "machine" => Ok(Self::Machine),
            #[cfg(feature = "asm")]
            "asm" => Ok(Self::Asm),
            _ => Err("Invalid engine type"),
        }
    }
}

#[derive(FromArgs)]
/// A brainf*ck language compiler and interpreter
struct BrainFck {
    /// a brainf*ck source file to run
    #[argh(positional)]
    filename: String,

    /// an engine type: either "interpreter", "machine" or "asm"
    #[argh(option)]
    engine: EngineType,
}

fn main() {
    let BrainFck { filename, engine } = argh::from_env();
    let Ok(program) = std::fs::read(filename) else {
        eprintln!("io-error while reading the file");
        return;
    };

    let res = match engine {
        #[cfg(feature = "interpreter")]
        EngineType::Interpreter => interpreter::run(&program),
        #[cfg(feature = "machine")]
        EngineType::Machine => jit::machine::run(&program),
        #[cfg(feature = "asm")]
        EngineType::Asm => jit::asm::run(&program),
    };
    if let Err(e) = res {
        eprintln!("{}", e);
    }
}

#[cfg(test)]
mod test {
    // All of the test cases under this module are adapted from http://brainfuck.org/tests.b
    // by [Daniel B Cristofani](1) (cristofdathevanetdotcom), under [CC BY-SA 4.0](2).
    // The adapted cases are all in the arrays at the start of each test cases, followed by the author's comment.
    //
    // [1]: http://www.hevanet.com/cristofd/brainfuck/
    // [2]: https://creativecommons.org/licenses/by-sa/4.0/
    use super::*;
    use std::{cell::RefCell, collections::VecDeque};

    thread_local! {
        pub static OUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        pub static IN: RefCell<VecDeque<u8>> = RefCell::new(VecDeque::new());
    }

    fn run_tests(test: fn(fn(&[u8]) -> Result<(), Error>)) {
        test(interpreter::run);
        clear();
        test(jit::machine::run);
        clear();
        test(jit::asm::run);

        fn clear() {
            OUT.with(|o| o.borrow_mut().clear());
            IN.with(|i| i.borrow_mut().clear());
        }
    }

    #[test]
    fn io() {
        static PROGRAM: &[u8] = b">,>+++++++++,>+++++++++++[<++++++<++++++<+>>>-]<<.>.<<-.>.>.<<.";
        /* "This is for testing i/o; give it a return followed by an EOF. (Try it both
        with file input--a file consisting only of one blank line--and with
        keyboard input, i.e. hit return and then ctrl-d (Unix) or ctrl-z
        (Windows).)
        It should give two lines of output; the two lines should be identical, and
        should be lined up one over the other. If that doesn't happen, ten is not
        coming through as newline on output.
        The content of the lines tells how input is being processed; each line
        should be two uppercase letters.
        Anything with O in it means newline is not coming through as ten on input.
        LK means newline input is working fine, and EOF leaves the cell unchanged
        (which I recommend).
        LB means newline input is working fine, and EOF translates as 0.
        LA means newline input is working fine, and EOF translates as -1.
        Anything else is fairly unexpected."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            IN.with(|input| input.borrow_mut().extend(b"\n"));
            run(PROGRAM).unwrap();
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b"LB\nLB\n"));
        });
    }

    #[test]
    fn array_size() {
        static PROGRAM: &[u8] = b"++++[>++++++<-]>[>+++++>+++++++<<-]>>++++<[[>[[>>+<<-]<]>>>-]>-[>+>+<<-]>]+++++[>+++++++<<++>-]>.<<.";
        /* "Goes to cell 30000 and reports from there with a #. (Verifies that the
        array is big enough.)
        Daniel B Cristofani (cristofdathevanetdotcom)"
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            run(PROGRAM).unwrap();
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b"#\n"));
        });
    }

    #[test]
    fn bound_check() {
        static _START: &[u8] = b"+[<+++++++++++++++++++++++++++++++++.]";
        static _END: &[u8] = b"+[>+++++++++++++++++++++++++++++++++.]";
        /* "These next two test the array bounds checking. Bounds checking is not
        essential, and in a high-level implementation it is likely to introduce
        extra overhead. In a low-level implementation you can get bounds checking
        for free by using the OS's own memory protections; this is the best
        solution, which may require making the array size a multiple of the page
        size.
        Anyway. These two programs measure the "real" size of the array, in some
        sense, in cells left and right of the initial cell respectively. They
        output the result in unary; the easiest thing is to direct them to a file
        and measure its size, or (on Unix) pipe the output to wc. If bounds
        checking is present and working, the left should measure 0 and the right
        should be the array size minus one.
        Daniel B Cristofani (cristofdathevanetdotcom)"
        http://www.hevanet.com/cristofd/brainfuck/ */

        // Our engine wraps around the pointer when it's out of bound. As a result, the original tests don't work.
        // This modified version below outputs '!' until it wraps around. Then, it goes back by one and outputs.
        // It should output 2^16 '!'s with '"' (the start) and '!' (the end) following.
        static PROGRAM: &[u8] =
            b"+[>+++++++++++++++++++++++++++++++++.----------------------------------]<.";

        run_tests(|run| {
            run(PROGRAM).unwrap();
            OUT.with(|output| assert_eq!(output.borrow().len(), u16::MAX as usize + 2));
        });
    }

    #[test]
    fn obscure() {
        static PROGRAM: &[u8] = br#"[]++++++++++[>>+>+>++++++[<<+<+++>>>-]<<<<-]"A*$";?@![#>>+<<]>[>>]<<<<[>++<[-]]>.>."#;
        /* "Tests for several obscure problems. Should output an H.
        Daniel B Cristofani (cristofdathevanetdotcom)"
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            run(PROGRAM).unwrap();
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b"H\n"));
        });
    }

    #[test]
    fn unmatching_bracket_left() {
        static PROGRAM: &[u8] = b"+++++[>+++++++>++<<-]>.>.[";
        /* "Should ideally give error message "unmatched [" or the like, and not give
        any output. Not essential."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            assert!(matches!(run(PROGRAM), Err(Error::UnmatchedLeft)));
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b""));
        });
    }

    #[test]
    fn unmatching_bracket_right() {
        static PROGRAM: &[u8] = b"+++++[>+++++++>++<<-]>.>.][";
        /* "Should ideally give error message "unmatched ]" or the like, and not give
        any output. Not essential."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            assert!(matches!(run(PROGRAM), Err(Error::UnmatchedRight)));
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b""));
        });
    }

    #[test]
    fn rot13() {
        static PROGRAM: &[u8] = include_bytes!("./rot13.b");
        /* "My pathological program rot13.b is good for testing the response to deep
        brackets; the input "~mlk zyx" should produce the output "~zyx mlk"."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            IN.with(|input| input.borrow_mut().extend(b"~mlk zyx"));
            run(PROGRAM).unwrap();
            OUT.with(|output| assert_eq!(output.borrow().as_slice(), b"~zyx mlk"));
        });
    }

    #[test]
    fn numwarp() {
        static PROGRAM: &[u8] = include_bytes!("./numwarp.b");
        /* "For an overall stress test, and also to check whether the output is
        monospaced as it ideally should be, I would run numwarp.b."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        run_tests(|run| {
            IN.with(|input| input.borrow_mut().extend(b"128.42-(171)"));
            run(PROGRAM).unwrap();
            OUT.with(|output| {
                assert_eq!(
                    output.borrow().as_slice(),
                    include_bytes!("./numwarp.stdout")
                )
            });
        });
    }
}
