#![warn(unsafe_op_in_unsafe_fn)]
// TODO: Do some integration tests to make sure stdin/out works.

mod interpreter;
mod jit;

use std::{
    array,
    io::{self, Read, Write},
};
use thiserror::Error;

pub(crate) trait Consumer {
    fn consume_while(&mut self, target: u8) -> usize;
}

impl Consumer for std::slice::Iter<'_, u8> {
    #[inline(always)]
    fn consume_while(&mut self, target: u8) -> usize {
        // Calculate the span of continuous copies of the target.
        let mut clone = self.clone();
        while clone.next().copied() == Some(target) {}
        let span = (self.len() - clone.len()).saturating_sub(1);

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

#[inline(always)]
pub(crate) fn putchar(byte: &u8) -> io::Result<()> {
    #[cfg(test)]
    let mut writer = test::OUT.get().borrow_mut();
    // FIXME: use raw stdin? I don't need line-buffering here.
    // See https://github.com/rust-lang/rust/issues/58326
    #[cfg(not(test))]
    let mut writer = io::stdout();

    if cfg!(windows) && *byte == b'\n' {
        writer.write_all(&[b'\r', b'\n'])?;
    } else {
        writer.write_all(array::from_ref(byte))?;
    }

    #[cfg(not(test))]
    writer.flush()?;

    Ok(())
}

#[inline(always)]
pub(crate) fn getchar(byte: &mut u8) -> io::Result<()> {
    #[cfg(test)]
    let mut reader = test::IN.get().borrow_mut();
    #[cfg(not(test))]
    let mut reader = io::stdin();

    match reader.read_exact(array::from_mut(byte)) {
        Ok(_) => if cfg!(windows) && *byte == b'\r' {
            // We're assuming there's '\n' after '\r'. Even if there isn't, this skips '\r'.
            // Also, we call `UnexpectedEof` an error too. Basically, anything other than "\r\n" is unexpected.
            reader.read_exact(array::from_mut(byte))?;
        }
        // The value of `buf` is "unspecified" when `UnexpectedEof` happens,
        // Make sure it is 0 to be consistent.
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => *byte = 0,
        r @ Err(_) => return r,
    };

    Ok(())
}

fn main() {
    static START: &[u8] = b"+[<+++++++++++++++++++++++++++++++++.]";
    static END: &[u8] = b"+[>+++++++++++++++++++++++++++++++++.]";
    jit::run_dynasm(END).unwrap();
    // interpreter::run(START);
    // let mut buf = String::new();
    // io::stdin().read_line(&mut buf).unwrap();
    // interpreter::run(buf.as_bytes(), io::stdout(), io::stdin()).unwrap();
}

#[cfg(test)]
mod test {
    // The tests under this mod is adapted from http://brainfuck.org/tests.b one by one.
    // Credit goes to Daniel B Cristofani (cristofdathevanetdotcom).
    use super::*;
    use {
        // FIXME: Replace this with https://github.com/rust-lang/rust/issues/29594?
        state::LocalStorage,
        std::{cell::RefCell, collections::VecDeque},
    };

    pub(crate) static OUT: LocalStorage<RefCell<Vec<u8>>> = LocalStorage::new();

    // Keep in mind when converting this to #[thread_local] attribute that
    // `VecDequeue` or Curosr<Vec<u8>> fits better for `IN`, but their `new`s are not const.
    // See https://github.com/rust-lang/rust/issues/99805 and 68990
    // https://github.com/rust-lang/rust/issues/78812
    pub(crate) static IN: LocalStorage<RefCell<VecDeque<u8>>> = LocalStorage::new();

    fn run_tests(test: fn(fn(&[u8]) -> Result<(), Error>)) {
        OUT.set(|| RefCell::new(Vec::new()));
        IN.set(|| RefCell::new(VecDeque::new()));
        // For some reason, the result of other tests sneak in if I didn't do this.
        // That's probably because Rust's test system re-uses threads.
        // The confusing thing is that just calling `.get()` on them sometimes work.
        clear();

        eprintln!("running dynasm");
        test(jit::run_dynasm);
        clear();

        // eprintln!("running plain");
        // test(jit::run_plain);
        // clear();

        eprintln!("running interpreter");
        test(interpreter::run);

        fn clear() {
            IN.get().borrow_mut().clear();
            OUT.get().borrow_mut().clear();
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

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            IN.get().borrow_mut().write(b"\n").unwrap();
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), b"LB\nLB\n");
        }
        run_tests(test);
    }

    #[test]
    fn array_size() {
        static PROGRAM: &[u8] = b"++++[>++++++<-]>[>+++++>+++++++<<-]>>++++<[[>[[>>+<<-]<]>>>-]>-[>+>+<<-]>]+++++[>+++++++<<++>-]>.<<.";
        /* "Goes to cell 30000 and reports from there with a #. (Verifies that the
        array is big enough.)
        Daniel B Cristofani (cristofdathevanetdotcom)"
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), b"#\n");
        }
        run_tests(test);
    }

    #[test]
    fn bound_check() {
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
        static _START: &[u8] = b"+[<+++++++++++++++++++++++++++++++++.]";
        static _END: &[u8] = b"+[>+++++++++++++++++++++++++++++++++.]";

        // Our engine wraps around the pointer when it's out of bound. As a result, the original tests don't work.
        // This modified version below outputs '!' until it wraps around. Then, it goes back by one and outputs.
        // It should output 2^16 '!'s with '"' (the start) and '!' (the end) following.
        static PROGRAM: &[u8] = b"+[>+++++++++++++++++++++++++++++++++.----------------------------------]<.";

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().len(), u16::MAX as usize + 2);
        }
        run_tests(test);

    }

    #[test]
    fn obscure() {
        static PROGRAM: &[u8] = br#"[]++++++++++[>>+>+>++++++[<<+<+++>>>-]<<<<-]"A*$";?@![#>>+<<]>[>>]<<<<[>++<[-]]>.>."#;
        /* "Tests for several obscure problems. Should output an H.
        Daniel B Cristofani (cristofdathevanetdotcom)"
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), b"H\n");
        }
        run_tests(test);
    }

    #[test]
    fn unmatching_bracket_left() {
        static PROGRAM: &[u8] = b"+++++[>+++++++>++<<-]>.>.[";
        /* "Should ideally give error message "unmatched [" or the like, and not give
        any output. Not essential."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            assert!(matches!(run(PROGRAM), Err(Error::UnmatchedLeft)));
            assert_eq!(OUT.get().borrow().as_slice(), b"");
        }
        run_tests(test);
    }

    #[test]
    fn unmatching_bracket_right() {
        static PROGRAM: &[u8] = b"+++++[>+++++++>++<<-]>.>.][";
        /* "Should ideally give error message "unmatched ]" or the like, and not give
        any output. Not essential."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            assert!(matches!(run(PROGRAM), Err(Error::UnmatchedRight)));
            assert_eq!(OUT.get().borrow().as_slice(), b"");
        }
        run_tests(test);
    }

    #[test]
    fn rot13() {
        static PROGRAM: &[u8] = include_bytes!("./rot13.b");
        /* "My pathological program rot13.b is good for testing the response to deep
        brackets; the input "~mlk zyx" should produce the output "~zyx mlk"."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            IN.get().borrow_mut().write_all(b"~mlk zyx").unwrap();
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), b"~zyx mlk");
        }
        run_tests(test);
    }

    #[test]
    fn numwarp() {
        static PROGRAM: &[u8] = include_bytes!("./numwarp.b");
        /* "For an overall stress test, and also to check whether the output is
        monospaced as it ideally should be, I would run numwarp.b."
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            IN.get().borrow_mut().write_all(b"128.42-(171)").unwrap();
            run(PROGRAM).unwrap();
            assert_eq!(
                OUT.get().borrow().as_slice(),
                include_bytes!("./numwarp.stdout")
            );
        }
        run_tests(test);
    }
}
