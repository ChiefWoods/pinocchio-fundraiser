use core::mem::size_of;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_token_2022::instructions::Transfer;

use crate::{
    AccountCheck, AccountLoad, AssociatedTokenAccount, Contributor, ContributorParams, Fundraise,
    FundraiserError, Handler, MAX_BPS, MAX_CONTRIBUTION_PERCENTAGE_BPS, MintInterface, Prefix,
    ProgramAccount, SECONDS_TO_DAYS, SignerAccount
};

pub struct ContributeAccounts<'a> {
    pub authority: &'a AccountInfo,
    pub mint_to_raise: &'a AccountInfo,
    pub fundraise: &'a AccountInfo,
    pub contributor: &'a AccountInfo,
    pub authority_token_account: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for ContributeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            authority,
            mint_to_raise,
            fundraise,
            contributor,
            authority_token_account,
            vault,
            system_program,
            token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(authority)?;
        MintInterface::check(mint_to_raise)?;
        ProgramAccount::check(fundraise)?;
        AssociatedTokenAccount::check(
            authority_token_account,
            authority,
            mint_to_raise,
            token_program,
        )?;
        AssociatedTokenAccount::check(vault, fundraise, mint_to_raise, token_program)?;

        Ok(Self {
            authority,
            mint_to_raise,
            fundraise,
            contributor,
            authority_token_account,
            vault,
            system_program,
            token_program,
        })
    }
}

pub struct ContributeInstructionData {
    pub amount: u64,
}

impl<'a> TryFrom<&'a [u8]> for ContributeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let offset = size_of::<u64>();
        let amount = u64::from_le_bytes(data[0..offset].try_into().unwrap());

        Ok(Self { amount })
    }
}

pub struct Contribute<'a> {
    pub accounts: ContributeAccounts<'a>,
    pub data: ContributeInstructionData,
    pub bump: u8,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Contribute<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = ContributeAccounts::try_from(accounts)?;
        let data = ContributeInstructionData::try_from(data)?;

        let (pda, bump) = find_program_address(
            &[
                Contributor::PREFIX,
                accounts.fundraise.key(),
                accounts.authority.key(),
            ],
            &crate::ID,
        );

        if pda != *accounts.contributor.key() {
            return Err(FundraiserError::InvalidAddress.into());
        }

        let bump_binding = [bump];
        let contributor_seeds = [
            Seed::from(Contributor::PREFIX),
            Seed::from(accounts.fundraise.key().as_ref()),
            Seed::from(accounts.authority.key().as_ref()),
            Seed::from(&bump_binding),
        ];
        let params =
            ContributorParams::new(*accounts.fundraise.key(), *accounts.authority.key(), bump);

        ProgramAccount::init_if_needed::<Contributor>(
            &contributor_seeds,
            accounts.contributor,
            accounts.authority,
            params,
        )?;

        Ok(Self {
            accounts,
            data,
            bump,
        })
    }
}

impl<'a> Handler<'a> for Contribute<'a> {
    const DISCRIMINATOR: &'a u8 = &1;

    fn process(&mut self) -> ProgramResult {
        let decimals = match *self.accounts.mint_to_raise.owner() {
            pinocchio_token::ID => {
                let mint = unsafe {
                    pinocchio_token::state::Mint::from_account_info_unchecked(
                        self.accounts.mint_to_raise,
                    )?
                };
                mint.decimals()
            }
            pinocchio_token_2022::ID => {
                let mint = unsafe {
                    pinocchio_token_2022::state::Mint::from_account_info_unchecked(
                        self.accounts.mint_to_raise,
                    )?
                };
                mint.decimals()
            }
            _ => return Err(ProgramError::IncorrectProgramId),
        };

        if self.data.amount <= 1u64.pow(decimals as u32) {
            return Err(FundraiserError::ContributionTooSmall.into());
        }

        let mut fundraise_data = self.accounts.fundraise.try_borrow_mut_data()?;
        let fundraise = Fundraise::load_mut(fundraise_data.as_mut())?;

        let fundraise_seeds = &[Fundraise::PREFIX, &fundraise.maker, &[fundraise.bump]];

        ProgramAccount::validate(fundraise_seeds, *self.accounts.fundraise.key())?;
        fundraise.check_mint_to_raise(self.accounts.mint_to_raise.key())?;

        let amount_to_raise = fundraise.get_amount_to_raise();

        if self.data.amount
            > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS)
        {
            return Err(FundraiserError::ContributionTooBig.into());
        }

        let now = Clock::get()?.unix_timestamp;

        let duration = fundraise.get_duration();
        let time_started = fundraise.get_time_started();

        if now != time_started
            && duration as i64 > (now - time_started) / i64::from(SECONDS_TO_DAYS)
        {
            return Err(FundraiserError::FundraiserEnded.into());
        }

        let mut contributor_data = self.accounts.contributor.try_borrow_mut_data()?;
        let contributor = Contributor::load_mut(contributor_data.as_mut())?;

        let contributor_amount = contributor.get_amount();

        if (contributor_amount
            > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS))
            || (contributor_amount + self.data.amount
                > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS))
        {
            return Err(FundraiserError::MaximumContributionsReached.into());
        }

        let current_amount = fundraise.get_current_amount();
        fundraise.set_current_amount(current_amount + self.data.amount);

        let contributor_amount = contributor.get_amount();
        contributor.set_amount(contributor_amount + self.data.amount);

        Transfer {
            amount: self.data.amount,
            authority: self.accounts.authority,
            from: self.accounts.authority_token_account,
            to: self.accounts.vault,
            token_program: self.accounts.token_program.key(),
        }
        .invoke()?;

        Ok(())
    }
}
