// TODO: Do some integration tests to make sure stdin/out works.

mod interpreter;
mod jit;

use std::io::{self, Read, Write};
use thiserror::Error;

#[cfg(test)]
use {
    // Replace this with https://github.com/rust-lang/rust/issues/29594?
    state::LocalStorage,
    std::{cell::RefCell, collections::VecDeque},
};

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

        // TODO: unstable `Iterator::advance_by` is better.
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
}

#[cfg(test)]
pub(crate) static OUT: LocalStorage<RefCell<Vec<u8>>> = LocalStorage::new();

// Keep in mind when converting this to #[thread_local] attribute that
// `VecDequeue` or Curosr<Vec<u8>> fits better for `IN`, but their `new`s are not const.
// See https://github.com/rust-lang/rust/issues/99805 and 68990
// https://github.com/rust-lang/rust/issues/78812
#[cfg(test)]
pub(crate) static IN: LocalStorage<RefCell<VecDeque<u8>>> = LocalStorage::new();

#[inline(always)]
pub(crate) fn putchar(char: u8) {
    #[cfg(test)]
    let mut writer = OUT.get().borrow_mut();
    #[cfg(not(test))]
    let mut writer = io::stdout();
    writer.write_all(&[char]).unwrap();
}

#[inline(always)]
pub(crate) fn getchar() -> u8 {
    #[cfg(test)]
    let mut reader = IN.get().borrow_mut();
    #[cfg(not(test))]
    let mut reader = io::stdin();

    let mut buf = [0; 1];
    match reader.read_exact(&mut buf) {
        Ok(_) => {}
        // TODO: Ideally, the cell stays the same when there's EOF.
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {}
        // TODO: might be better to return io::Error?
        r @ Err(_) => r.unwrap(),
    };
    buf[0]
}

fn main() {
    // let mut buf = String::new();
    // io::stdin().read_line(&mut buf).unwrap();
    // interpreter::run(buf.as_bytes(), io::stdout(), io::stdin()).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    fn run_tests(test: fn(fn(&[u8]) -> Result<(), Error>)) {
        OUT.set(|| RefCell::new(Vec::new()));
        IN.set(|| RefCell::new(VecDeque::new()));
        // For some reason, the result of other tests sneak in if I didn't do this.
        OUT.get();
        IN.get();

        eprintln!("running dynasm");
        test(jit::run_dynasm);
        clear();

        eprintln!("running plain");
        test(jit::run_plain);
        clear();

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
        /* Goes to cell 30000 and reports from there with a #. (Verifies that the
        array is big enough.)
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), b"#\n");
        }
        run_tests(test);
    }

    // #[test]
    // fn bound_check() {
    //     /* These next two test the array bounds checking. Bounds checking is not
    //     essential, and in a high-level implementation it is likely to introduce
    //     extra overhead. In a low-level implementation you can get bounds checking
    //     for free by using the OS's own memory protections; this is the best
    //     solution, which may require making the array size a multiple of the page
    //     size.
    //     Anyway. These two programs measure the "real" size of the array, in some
    //     sense, in cells left and right of the initial cell respectively. They
    //     output the result in unary; the easiest thing is to direct them to a file
    //     and measure its size, or (on Unix) pipe the output to wc. If bounds
    //     checking is present and working, the left should measure 0 and the right
    //     should be the array size minus one.
    //     Daniel B Cristofani (cristofdathevanetdotcom)
    //     http://www.hevanet.com/cristofd/brainfuck/ */
    //     let start = b"+[<+++++++++++++++++++++++++++++++++.]";
    //     let end = b"+[>+++++++++++++++++++++++++++++++++.]";

    //     let test = |run: fn(&[u8])| {
    //         run(start);
    //         assert_eq!(OUT.lock().unwrap().as_slice(), b"");
    //     };
    //     run_tests(test);

    //     let test = |run: fn(&[u8])| {
    //         run(end);
    //         assert_eq!(OUT.lock().unwrap().as_slice().len(), 30_000 - 1);
    //     };
    //     run_tests(test);
    // }

    #[test]
    fn obscure() {
        static PROGRAM: &[u8] = br#"[]++++++++++[>>+>+>++++++[<<+<+++>>>-]<<<<-]"A*$";?@![#>>+<<]>[>>]<<<<[>++<[-]]>.>."#;
        /* Tests for several obscure problems. Should output an H.
        Daniel B Cristofani (cristofdathevanetdotcom)
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
        /* Should ideally give error message "unmatched [" or the like, and not give
        any output. Not essential.
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
        /* Should ideally give error message "unmatched ]" or the like, and not give
        any output. Not essential.
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
        const PROGRAM: &[u8] = include_bytes!("./rot13.b");
        /* My pathological program rot13.b is good for testing the response to deep
        brackets; the input "~mlk zyx" should produce the output "~zyx mlk".
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
        const PROGRAM: &[u8] = include_bytes!("./numwarp.b");
        /* For an overall stress test, and also to check whether the output is
        monospaced as it ideally should be, I would run numwarp.b.
        Daniel B Cristofani (cristofdathevanetdotcom)
        http://www.hevanet.com/cristofd/brainfuck/ */

        fn test(run: fn(&[u8]) -> Result<(), Error>) {
            IN.get().borrow_mut().write_all(b"128.42-(171)").unwrap();
            run(PROGRAM).unwrap();
            assert_eq!(OUT.get().borrow().as_slice(), include_bytes!("./numwarp.stdout"));
        }
        run_tests(test);
    }
}
