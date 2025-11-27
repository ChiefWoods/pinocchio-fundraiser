use pinocchio::{ProgramResult, account_info::AccountInfo, program_error::ProgramError};

pub mod associated_token;
pub mod mint;
pub mod mint_2022;
pub mod mint_interface;
pub mod program;
pub mod signer;
pub mod system;
pub mod token;
pub mod token_2022;
pub mod token_interface;

pub use associated_token::*;
pub use mint::*;
pub use mint_2022::*;
pub use mint_interface::*;
pub use program::*;
pub use signer::*;
pub use system::*;
pub use token::*;
pub use token_2022::*;
pub use token_interface::*;

const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;
const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;
const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

pub trait AccountCheck {
    fn check(account: &AccountInfo) -> Result<(), ProgramError>;
}

pub trait MintInit {
    fn init(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult;

    fn init_if_needed(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult;
}

pub trait TokenInit {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult;

    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult;
}
