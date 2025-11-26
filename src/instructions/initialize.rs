use core::mem::size_of;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Seed, program_error::ProgramError, pubkey::find_program_address, sysvars::{Sysvar, clock::Clock}
};

use crate::{
    AccountCheck, AccountLoad, AssociatedTokenAccount, Fundraise, FundraiserError, Handler,
    MIN_AMOUNT_TO_RAISE, MintInterface, Prefix, ProgramAccount, SignerAccount, Space,
};

pub struct InitializeAccounts<'a> {
    pub maker: &'a AccountInfo,
    pub mint_to_raise: &'a AccountInfo,
    pub fundraise: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub associated_token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            maker,
            mint_to_raise,
            fundraise,
            vault,
            system_program,
            token_program,
            associated_token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(maker)?;
        MintInterface::check(mint_to_raise)?;

        Ok(Self {
            maker,
            mint_to_raise,
            fundraise,
            vault,
            system_program,
            token_program,
            associated_token_program,
        })
    }
}

pub struct InitializeInstructionData {
    pub amount_to_raise: u64,
    pub duration: u64,
}

impl<'a> TryFrom<&'a [u8]> for InitializeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() + size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let offset = size_of::<u64>();
        let amount_to_raise = u64::from_le_bytes(data[0..offset].try_into().unwrap());
        let duration =
            u64::from_le_bytes(data[offset..offset + size_of::<u64>()].try_into().unwrap());

        Ok(Self {
            amount_to_raise,
            duration,
        })
    }
}

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub data: InitializeInstructionData,
    pub bump: u8,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let data = InitializeInstructionData::try_from(data)?;

        let (pda, bump) =
            find_program_address(&[Fundraise::PREFIX, accounts.maker.key()], &crate::ID);

        if pda != *accounts.fundraise.key() {
            return Err(FundraiserError::InvalidAddress.into());
        }

        let fundraise_bump = [bump];
        let fundraise_seeds = [
            Seed::from(Fundraise::PREFIX),
            Seed::from(accounts.maker.key().as_ref()),
            Seed::from(&fundraise_bump),
        ];

        ProgramAccount::init::<Fundraise>(
            accounts.maker,
            accounts.fundraise,
            &fundraise_seeds,
            Fundraise::LEN,
        )?;

        AssociatedTokenAccount::init(
            accounts.vault,
            accounts.mint_to_raise,
            accounts.maker,
            accounts.fundraise,
            accounts.system_program,
            accounts.token_program,
        )?;

        Ok(Self {
            accounts,
            data,
            bump,
        })
    }
}

// impl<'a> Initialize<'a> {
impl<'a> Handler<'a> for Initialize<'a> {
    const DISCRIMINATOR: &'a u8 = &0;

    fn process(&mut self) -> ProgramResult {
        let mut data = self.accounts.fundraise.try_borrow_mut_data()?;
        let fundraise = Fundraise::load_mut(data.as_mut())?;

        let decimals = match *self.accounts.mint_to_raise.owner() {
            pinocchio_token::ID => {
                let mint = unsafe { pinocchio_token::state::Mint::from_account_info_unchecked(self.accounts.mint_to_raise)? };
                mint.decimals()
            },
            pinocchio_token_2022::ID => {
                let mint = unsafe { pinocchio_token_2022::state::Mint::from_account_info_unchecked(self.accounts.mint_to_raise)? };
                mint.decimals()
            },
            _ => return Err(ProgramError::IncorrectProgramId),
        };

        if self.data.amount_to_raise <= u64::from(MIN_AMOUNT_TO_RAISE).pow(decimals as u32) {
            return Err(FundraiserError::BelowMinRaiseAmount.into());
        }

        let now = Clock::get()?.unix_timestamp;

        fundraise.set_inner(
            *self.accounts.maker.key(),
            *self.accounts.mint_to_raise.key(),
            self.data.amount_to_raise,
            now,
            self.data.duration,
            self.bump,
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_clock::Clock;
    use solana_instruction::{AccountMeta, Instruction};
    use solana_signer::Signer;
    use spl_associated_token_account::{get_associated_token_address_with_program_id, solana_program::{clock::SECONDS_PER_DAY, native_token::LAMPORTS_PER_SOL}};

    use crate::{AccountLoad, Fundraise, tests::{
        constants::{ASSOCIATED_TOKEN_PROGRAM_ID, MINT_DECIMALS, PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID}, pda::get_fundraise_pda, utils::{build_and_send_transaction, init_mint, init_wallet, setup}
    }};

    #[test]
    fn initialize() {
        let (litesvm, _default_payer) = &mut setup();
        let maker = init_wallet(litesvm, LAMPORTS_PER_SOL);
        let mint_to_raise = init_mint(litesvm, TOKEN_PROGRAM_ID, MINT_DECIMALS, 1_000_000_000);

        let amount_to_raise: u64 = 5_000_000;
        let duration: u64 = SECONDS_PER_DAY; // 1 day
        let fundraise_pda = get_fundraise_pda(&maker.pubkey());
        let vault = get_associated_token_address_with_program_id(&fundraise_pda, &mint_to_raise, &TOKEN_PROGRAM_ID);
        let now = litesvm.get_sysvar::<Clock>().unix_timestamp;

        let data = [
            vec![0u8],
            amount_to_raise.to_le_bytes().to_vec(),
            duration.to_le_bytes().to_vec(),
        ].concat();
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&maker], &maker.pubkey(), &[ix]);

        let fundraise_acc = litesvm.get_account(&fundraise_pda).unwrap();
        let fundraise = Fundraise::load(fundraise_acc.data.as_ref()).unwrap();

        assert_eq!(fundraise.maker, maker.pubkey().to_bytes());
        assert_eq!(fundraise.mint_to_raise, mint_to_raise.to_bytes());
        assert_eq!(fundraise.get_amount_to_raise(), amount_to_raise);
        assert_eq!(fundraise.get_time_started(), now);
        assert_eq!(fundraise.get_duration(), duration);
    }
}
