use pinocchio::program_error::{ProgramError, ToStr};

impl From<FundraiserError> for ProgramError {
    fn from(e: FundraiserError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

pub enum FundraiserError {
    NotSigner,
    InvalidAddress,
    TargetNotMet,
    TargetMet,
    ContributionTooBig,
    ContributionTooSmall,
    MaximumContributionsReached,
    FundraiserNotEnded,
    FundraiserEnded,
    InvalidAmount,
    InvalidMintToRaise,
    BelowMinRaiseAmount,
}

impl ToStr for FundraiserError {
    fn to_str<E>(&self) -> &'static str {
        match self {
            FundraiserError::NotSigner => "Account is not a signer",
            FundraiserError::InvalidAddress => "Account address is invalid",
            FundraiserError::TargetNotMet => "The amount to raise has not been met",
            FundraiserError::TargetMet => "The amount to raise has been achieved",
            FundraiserError::ContributionTooBig => "The contribution is too big",
            FundraiserError::ContributionTooSmall => "The contribution is too small",
            FundraiserError::MaximumContributionsReached => {
                "The maximum amount to contribute has been reached"
            }
            FundraiserError::FundraiserNotEnded => "The fundraiser has not ended yet",
            FundraiserError::FundraiserEnded => "The fundraiser has ended",
            FundraiserError::InvalidAmount => "Invalid total amount. i should be bigger than 3",
            FundraiserError::InvalidMintToRaise => "Mint to raise does not match",
            FundraiserError::BelowMinRaiseAmount => {
                "The amount to raise is below the minimum required"
            }
        }
    }
}
