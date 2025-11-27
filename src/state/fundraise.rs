use pinocchio::{ProgramResult, pubkey::Pubkey};

use crate::{AccountLoad, Prefix, SetInner, Space};
use core::mem::size_of;

#[repr(C)]
pub struct Fundraise {
    pub maker: Pubkey,
    pub mint_to_raise: Pubkey,
    amount_to_raise: [u8; 8],
    current_amount: [u8; 8],
    time_started: [u8; 8],
    duration: [u8; 8],
    pub bump: u8,
}

impl Prefix for Fundraise {
    const PREFIX: &'static [u8] = b"fundraise";
}

impl Space for Fundraise {
    const LEN: usize = size_of::<Self>();
}

impl AccountLoad for Fundraise {}

impl Fundraise {
    #[inline(always)]
    pub fn get_amount_to_raise(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_raise)
    }

    #[inline(always)]
    pub fn get_current_amount(&self) -> u64 {
        u64::from_le_bytes(self.current_amount)
    }

    #[inline(always)]
    pub fn get_time_started(&self) -> i64 {
        i64::from_le_bytes(self.time_started)
    }

    #[inline(always)]
    pub fn get_duration(&self) -> u64 {
        u64::from_le_bytes(self.duration)
    }

    #[inline(always)]
    pub fn set_amount_to_raise(&mut self, amount: u64) {
        self.amount_to_raise = amount.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_current_amount(&mut self, amount: u64) {
        self.current_amount = amount.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_time_started(&mut self, time: i64) {
        self.time_started = time.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_duration(&mut self, duration: u64) {
        self.duration = duration.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_inner(
        &mut self,
        maker: Pubkey,
        mint_to_raise: Pubkey,
        amount_to_raise: u64,
        time_started: i64,
        duration: u64,
        bump: u8,
    ) {
        self.maker = maker;
        self.mint_to_raise = mint_to_raise;
        self.set_amount_to_raise(amount_to_raise);
        self.set_current_amount(0);
        self.set_time_started(time_started);
        self.set_duration(duration);
        self.bump = bump;
    }

    #[inline(always)]
    pub fn check_mint_to_raise(&self, mint: &Pubkey) -> ProgramResult {
        if &self.mint_to_raise != mint {
            return Err(crate::FundraiserError::InvalidMintToRaise.into());
        }

        Ok(())
    }
}

pub struct FundraiseParams {
    pub maker: Pubkey,
    pub mint_to_raise: Pubkey,
    pub amount_to_raise: u64,
    pub time_started: i64,
    pub duration: u64,
    pub bump: u8,
}

impl FundraiseParams {
    pub fn new(
        maker: Pubkey,
        mint_to_raise: Pubkey,
        amount_to_raise: u64,
        time_started: i64,
        duration: u64,
        bump: u8,
    ) -> Self {
        Self {
            maker,
            mint_to_raise,
            amount_to_raise,
            time_started,
            duration,
            bump,
        }
    }
}

impl SetInner for Fundraise {
    type Params = FundraiseParams;

    fn set_inner(&mut self, params: Self::Params) {
        self.maker = params.maker;
        self.mint_to_raise = params.mint_to_raise;
        self.set_amount_to_raise(params.amount_to_raise);
        self.set_current_amount(0);
        self.set_time_started(params.time_started);
        self.set_duration(params.duration);
        self.bump = params.bump;
    }
}
