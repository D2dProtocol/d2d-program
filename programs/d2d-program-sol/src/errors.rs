use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Program is currently paused")]
    ProgramPaused,
    #[msg("Insufficient deposit amount")]
    InsufficientDeposit,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Insufficient stake amount")]
    InsufficientStake,
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    #[msg("Insufficient treasury funds")]
    InsufficientTreasuryFunds,
    #[msg("Invalid request ID")]
    InvalidRequestId,
    #[msg("Invalid request status")]
    InvalidRequestStatus,
    #[msg("Invalid deployment status")]
    InvalidDeploymentStatus,
    #[msg("Invalid treasury wallet")]
    InvalidTreasuryWallet,
    #[msg("Invalid ephemeral key")]
    InvalidEphemeralKey,
    #[msg("Calculation overflow")]
    CalculationOverflow,
    #[msg("Recovered funds exceed deployment cost")]
    InvalidRecoveredFunds,
    #[msg("Fee amount exceeds maximum allowed")]
    FeeAmountTooLarge,
    #[msg("Insufficient liquid balance for withdrawal")]
    InsufficientLiquidBalance,
    #[msg("Invalid account data - account needs migration. Please call migrate_treasury_pool() first")]
    InvalidAccountData,
    #[msg("Invalid account owner - account must be owned by this program")]
    InvalidAccountOwner,

    // Security & Timelock errors
    #[msg("Timelock period has not expired yet")]
    TimelockNotExpired,
    #[msg("No pending withdrawal to execute")]
    NoPendingWithdrawal,
    #[msg("Pending withdrawal has expired")]
    PendingWithdrawalExpired,
    #[msg("A pending withdrawal already exists")]
    PendingWithdrawalExists,
    #[msg("Guardian not set")]
    GuardianNotSet,
    #[msg("Only guardian can perform this action")]
    OnlyGuardian,
    #[msg("Daily withdrawal limit exceeded")]
    DailyWithdrawalLimitExceeded,
    #[msg("Invalid timelock duration")]
    InvalidTimelockDuration,
    #[msg("Cannot set guardian to zero address")]
    InvalidGuardianAddress,
    #[msg("Cannot withdraw protected rewards - only excess rewards can be withdrawn")]
    CannotWithdrawProtectedRewards,
}
