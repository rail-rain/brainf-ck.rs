# Yet another Brainf*ck in Rust

[![Creative Commons License Logo](https://i.creativecommons.org/l/by-sa/4.0/88x31.png)](http://creativecommons.org/licenses/by-sa/4.0/)

A toy AMD64 and ARMv8-A JIT compiler and interpreter for Brainf*ck written in Rust.

This project includes 3 implementations of Brainf*ck, complying [the reference](http://www.brainfuck.org/brainfuck.html) by Daniel B. Cristofani.

One is an interpreter (`./src/interpreter.rs`) in pure Rust. It is used as a reference implementation for more complex other two implementations.

The second one is a JIT compiler written using `dynasm-rs` (a project to write JIT compiler using an assembly syntax). It supports both AMD64 and ARMv8-A. It can be found in (`./src/jit/asm.rs`).

The last (`./src/jit/machine.rs`) is also a JIT compiler but written without `dynasm-rs`. That means the file directly contains a piece of machine code. It only supports AMD64.

Note that the `dynasm-rs` based JIT doesn't support Windows on AMD64.

## Memory Protection

One somewhat unique feature of this project is that all three implement memory protection by allocating more memory than a guest's address space to avoid bound checking. This allocates $2^{16} + 1$ bytes of memory for the guest (a Brainf*ck programme), and the pointer size is 16 bit. The project doesn't use OS's memory protection facility since recovering from such signals are hard to get it right.

## License

This work is licensed under a [Creative Commons Attribution-ShareAlike 4.0 International License](http://creativecommons.org/licenses/by-sa/4.0/).