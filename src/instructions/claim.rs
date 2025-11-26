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

#[cfg(test)]
mod tests {
    use solana_instruction::{AccountMeta, Instruction};
    use solana_signer::Signer;
    use spl_associated_token_account::{
        get_associated_token_address_with_program_id,
        solana_program::{clock::SECONDS_PER_DAY, native_token::LAMPORTS_PER_SOL},
    };
    use spl_token_2022::state::Account;

    use crate::{
        tests::{
            constants::{
                ASSOCIATED_TOKEN_PROGRAM_ID, MINT_DECIMALS, PROGRAM_ID, SYSTEM_PROGRAM_ID,
                TOKEN_PROGRAM_ID,
            },
            pda::{get_contributor_pda, get_fundraise_pda},
            utils::{
                build_and_send_transaction, fetch_account, forward_time, init_ata, init_mint,
                init_wallet, setup,
            },
        },
    };

    #[test]
    fn claim() {
        let (litesvm, _default_payer) = &mut setup();
        let maker = init_wallet(litesvm, LAMPORTS_PER_SOL);
        let authority = init_wallet(litesvm, LAMPORTS_PER_SOL);
        let mint_to_raise = init_mint(litesvm, TOKEN_PROGRAM_ID, MINT_DECIMALS, 10_000_000_000);
        let authority_ata = init_ata(litesvm, mint_to_raise, authority.pubkey(), 5_000_000_000);

        let amount_to_raise: u64 = 5_000_000;
        let duration: u64 = SECONDS_PER_DAY; // 1 day
        let fundraise_pda = get_fundraise_pda(&maker.pubkey());
        let vault = get_associated_token_address_with_program_id(
            &fundraise_pda,
            &mint_to_raise,
            &TOKEN_PROGRAM_ID,
        );

        let data = [
            vec![0u8],
            amount_to_raise.to_le_bytes().to_vec(),
            duration.to_le_bytes().to_vec(),
        ]
        .concat();
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

        let contribute_amount: u64 = 5_000_000;
        let contributor_pda = get_contributor_pda(&fundraise_pda, &authority.pubkey());

        let data = [vec![1u8], contribute_amount.to_le_bytes().to_vec()].concat();
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(contributor_pda, false),
                AccountMeta::new(authority_ata, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&authority], &authority.pubkey(), &[ix]);

        forward_time(litesvm, 3600); // jump forward 1 hour

        let maker_ata = init_ata(litesvm, mint_to_raise, maker.pubkey(), 0);

        let data = vec![3u8];
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(vault, false),
                AccountMeta::new(maker_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&maker], &maker.pubkey(), &[ix]);

        let vault_bal = fetch_account::<Account>(litesvm, &vault).amount;

        assert_eq!(vault_bal, 0);
    }
}
