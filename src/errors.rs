

use thiserror::Error;

use solana_program::program_error::ProgramError;

pub enum EscrowError {

    #[error("Invalid Instruction")]
    InvalidInstruction,
}

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgrammError::Custom(e as u32)
    }
}