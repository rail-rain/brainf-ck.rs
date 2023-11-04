#[cfg(feature = "asm")]
pub mod asm;

pub mod machine {
    use crate::Error;
    pub fn run(_opcode: &[u8]) -> Result<(), Error> {
        todo!("The aarch64 backend is not yet implemented");
    }
}
