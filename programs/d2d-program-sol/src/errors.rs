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
  #[msg("Ephemeral key not set - deployment was never properly funded")]
  EphemeralKeyNotSet,
  #[msg("Calculation overflow")]
  CalculationOverflow,
  #[msg("Recovered funds exceed deployment cost")]
  InvalidRecoveredFunds,
  #[msg("Fee amount exceeds maximum allowed")]
  FeeAmountTooLarge,
  #[msg("Insufficient liquid balance for withdrawal")]
  InsufficientLiquidBalance,
  #[msg(
    "Invalid account data - account needs migration. Please call migrate_treasury_pool() first"
  )]
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

  // Authority Proxy errors
  #[msg("Program authority transfer failed")]
  AuthorityTransferFailed,
  #[msg("Program upgrade via proxy failed")]
  ProxyUpgradeFailed,
  #[msg("Cannot reclaim - subscription still active")]
  SubscriptionStillActive,
  #[msg("Subscription has expired")]
  SubscriptionExpired,
  #[msg("Program is not managed by D2D")]
  ProgramNotManaged,
  #[msg("Invalid program authority PDA")]
  InvalidAuthorityPda,

  // Escrow & Auto-Renewal errors
  #[msg("Escrow account not found")]
  EscrowNotFound,
  #[msg("Insufficient escrow balance for auto-renewal")]
  InsufficientEscrowBalance,
  #[msg("Auto-renewal is disabled")]
  AutoRenewalDisabled,
  #[msg("Program is currently in grace period")]
  GracePeriodActive,
  #[msg("Grace period has not yet expired")]
  GracePeriodNotExpired,
  #[msg("Invalid token type")]
  InvalidTokenType,
  #[msg("Token account does not match expected mint")]
  TokenAccountMismatch,
  #[msg("Program is already in grace period")]
  AlreadyInGracePeriod,
  #[msg("Cannot withdraw during pending auto-renewal")]
  WithdrawalLocked,
  #[msg("Escrow account already exists")]
  EscrowAlreadyExists,
  #[msg("Not in grace period")]
  NotInGracePeriod,

  // Pool utilization errors
  #[msg("Pool utilization exceeds 80% limit - cannot fund deployment")]
  PoolUtilizationTooHigh,
  #[msg("Subscription extension would cause overflow")]
  SubscriptionExtensionOverflow,
  #[msg("Maximum subscription extension is 120 months (10 years)")]
  SubscriptionExtensionTooLarge,
  #[msg("Reward debt exceeds accumulated rewards - data corruption")]
  RewardDebtExceedsAccumulated,

  // Withdrawal queue errors
  #[msg("Withdrawal already queued - cancel existing withdrawal first")]
  WithdrawalAlreadyQueued,
  #[msg("Withdrawal has already been processed")]
  WithdrawalAlreadyProcessed,
  #[msg("No queued withdrawal to cancel")]
  NoQueuedWithdrawal,
  #[msg("Withdrawal queue is empty")]
  WithdrawalQueueEmpty,
  #[msg("Invalid queue position")]
  InvalidQueuePosition,

  // Debt tracking errors
  #[msg("Debt not yet repaid - cannot close program")]
  DebtNotRepaid,
  #[msg("Recovery ratio too low - investigate before proceeding")]
  RecoveryRatioTooLow,
  #[msg("No debt to repay")]
  NoDebtToRepay,
  #[msg("Invalid debt repayment amount")]
  InvalidDebtRepayment,

  // Fair reward distribution errors
  #[msg("No stakers for reward distribution")]
  NoStakersForDistribution,
  #[msg("Invalid distribution percentage")]
  InvalidDistributionPercentage,
  #[msg("No pending rewards to distribute")]
  NoPendingRewards,
}
