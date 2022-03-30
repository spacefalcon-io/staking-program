use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient funds to unstake.")]
    InsufficientFundUnstake,
    #[msg("Amount must be greater than zero.")]
    AmountMustBeGreaterThanZero,
    #[msg("Pool is paused.")]
    PoolPaused,
    #[msg("Duration cannot be shorter than one day.")]
    DurationTooShort,
    #[msg("Provided funder is already authorized to fund.")]
    FunderAlreadyAuthorized,
    #[msg("Maximum funders already authorized.")]
    MaxFunders,
    #[msg("Cannot deauthorize the primary pool authority.")]
    CannotDeauthorizePoolAuthority,
    #[msg("Authority not found for deauthorization.")]
    CannotDeauthorizeMissingAuthority,
    #[msg("Need to wait until maturity time to stake or claim.")]
    CannotStakeOrClaimBeforeMaturity,
}
