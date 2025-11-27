use pinocchio::{
    ProgramResult, account_info::AccountInfo, entrypoint, program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_pubkey::declare_id;

pub mod instructions;
pub use instructions::*;

pub mod state;
pub use state::*;

pub mod errors;
pub use errors::*;

pub mod helpers;
pub use helpers::*;

pub mod constants;
pub use constants::*;

pub mod tests;

declare_id!("961YdRKb41e47DoC8JM973Xp52dVQ1NQ3P4bUm82eT8D");
entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((Initialize::DISCRIMINATOR, data)) => {
            Initialize::try_from((data, accounts))?.process()
        }
        Some((Contribute::DISCRIMINATOR, data)) => {
            Contribute::try_from((data, accounts))?.process()
        }
        Some((Claim::DISCRIMINATOR, _)) => Claim::try_from(accounts)?.process(),
        Some((Refund::DISCRIMINATOR, _)) => Refund::try_from(accounts)?.process(),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
