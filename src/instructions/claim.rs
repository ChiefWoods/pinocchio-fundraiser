use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::{Seed, Signer}, program_error::ProgramError
};
use pinocchio_token_2022::instructions::Transfer;

use crate::{
    AccountCheck, AccountLoad, AssociatedTokenAccount, Fundraise, FundraiserError, Handler,
    MintInterface, Prefix, ProgramAccount, SignerAccount,
};

pub struct ClaimAccounts<'a> {
    pub maker: &'a AccountInfo,
    pub mint_to_raise: &'a AccountInfo,
    pub fundraise: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub maker_token_account: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub associated_token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for ClaimAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            maker,
            mint_to_raise,
            fundraise,
            vault,
            maker_token_account,
            system_program,
            token_program,
            associated_token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(maker)?;
        MintInterface::check(mint_to_raise)?;
        ProgramAccount::check(fundraise)?;
        AssociatedTokenAccount::check(vault, fundraise, mint_to_raise, token_program)?;

        Ok(Self {
            maker,
            mint_to_raise,
            fundraise,
            vault,
            maker_token_account,
            system_program,
            token_program,
            associated_token_program,
        })
    }
}

pub struct Claim<'a> {
    pub accounts: ClaimAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Claim<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = ClaimAccounts::try_from(accounts)?;

        AssociatedTokenAccount::init_if_needed(
            accounts.maker_token_account,
            accounts.mint_to_raise,
            accounts.maker,
            accounts.maker,
            accounts.system_program,
            accounts.token_program,
        )?;

        Ok(Self { accounts })
    }
}

impl<'a> Handler<'a> for Claim<'a> {
    const DISCRIMINATOR: &'a u8 = &3;

    fn process(&mut self) -> ProgramResult {
        let vault_amount = match *self.accounts.vault.owner() {
            pinocchio_token::ID => {
                let vault = unsafe {
                    pinocchio_token::state::TokenAccount::from_account_info_unchecked(
                        self.accounts.vault,
                    )?
                };
                vault.amount()
            }
            pinocchio_token_2022::ID => {
                let vault = unsafe {
                    pinocchio_token_2022::state::TokenAccount::from_account_info_unchecked(
                        self.accounts.vault,
                    )?
                };
                vault.amount()
            }
            _ => return Err(ProgramError::IncorrectProgramId),
        };

        let fundraise_data = self.accounts.fundraise.try_borrow_data()?;
        let fundraise = Fundraise::load(&fundraise_data)?;
        let amount_to_raise = fundraise.get_amount_to_raise();

        if vault_amount < amount_to_raise {
            return Err(FundraiserError::TargetNotMet.into());
        }

        let fundraise_bump = [fundraise.bump];
        let fundraise_seeds = [
            Seed::from(Fundraise::PREFIX),
            Seed::from(fundraise.maker.as_ref()),
            Seed::from(&fundraise_bump),
        ];
        let fundraise_signer = Signer::from(&fundraise_seeds);

        Transfer {
            amount: vault_amount,
            authority: self.accounts.fundraise,
            from: self.accounts.vault,
            to: self.accounts.maker_token_account,
            token_program: self.accounts.token_program.key(),
        }
        .invoke_signed(&[fundraise_signer])?;

        Ok(())
    }
}
