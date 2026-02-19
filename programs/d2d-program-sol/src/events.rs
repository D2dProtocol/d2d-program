use anchor_lang::prelude::*;

#[event]
pub struct TreasuryInitialized {
  pub admin: Pubkey,
  pub treasury_wallet: Pubkey,
  pub initial_apy: u64,
}

#[event]
pub struct SolStaked {
  pub lender: Pubkey,
  pub amount: u64,
  pub total_staked: u64,
  pub lock_period: i64,
}

#[event]
pub struct SolUnstaked {
  pub lender: Pubkey,
  pub amount: u64,
  pub remaining_staked: u64,
}

#[event]
pub struct RewardsClaimed {
  pub lender: Pubkey,
  pub amount: u64,
  pub total_claimed: u64,
}

#[event]
pub struct DeploymentFundsRequested {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub program_hash: [u8; 32],
  pub service_fee: u64,
  pub monthly_fee: u64,
  pub initial_months: u32,
  pub deployment_cost: u64,
  pub total_payment: u64,
  pub requested_at: i64,
}

#[event]
pub struct TemporaryWalletFunded {
  pub request_id: [u8; 32],
  pub temporary_wallet: Pubkey,
  pub amount: u64,
  pub funded_at: i64,
}

#[event]
pub struct DeploymentConfirmed {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub deployed_program_id: Pubkey,
  pub deployment_cost: u64,
  pub recovered_funds: u64,
  pub confirmed_at: i64,
}

#[event]
pub struct DeploymentFailed {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub failure_reason: String,
  pub refund_amount: u64,
  pub deployment_cost_returned: u64,
  pub failed_at: i64,
}

#[event]
pub struct SubscriptionPaid {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub months: u32,
  pub payment_amount: u64,
  pub subscription_valid_until: i64,
}

#[event]
pub struct EmergencyPauseToggled {
  pub paused: bool,
  pub toggled_at: i64,
}

#[event]
pub struct ProgramClosed {
  pub request_id: [u8; 32],
  pub program_id: Pubkey,
  pub developer: Pubkey,
  pub recovered_lamports: u64,
  pub closed_at: i64,
}

#[event]
pub struct AdminWithdrew {
  pub admin: Pubkey,
  pub amount: u64,
  pub destination: Pubkey,
  pub reason: String,
  pub withdrawn_at: i64,
}

#[event]
pub struct DepositMade {
  pub backer: Pubkey,
  pub deposit_amount: u64,
  pub net_deposit: u64,
  pub reward_fee: u64,
  pub platform_fee: u64,
  pub total_deposited: u64,
  pub liquid_balance: u64,
  pub deposited_at: i64,
}

#[event]
pub struct RewardCredited {
  pub fee_reward: u64,
  pub fee_platform: u64,
  pub reward_per_share: u128,
  pub total_deposited: u64,
  pub credited_at: i64,
}

#[event]
pub struct Claimed {
  pub backer: Pubkey,
  pub amount: u64,
  pub claimed_total: u64,
  pub reward_per_share: u128,
  pub claimed_at: i64,
}

#[event]
pub struct GuardianSet {
  pub admin: Pubkey,
  pub old_guardian: Pubkey,
  pub new_guardian: Pubkey,
  pub set_at: i64,
}

#[event]
pub struct GuardianPaused {
  pub guardian: Pubkey,
  pub paused_at: i64,
}

#[event]
pub struct WithdrawalInitiated {
  pub initiator: Pubkey,
  pub withdrawal_type: String,
  pub amount: u64,
  pub destination: Pubkey,
  pub execute_after: i64,
  pub expires_at: i64,
  pub reason: String,
  pub initiated_at: i64,
}

#[event]
pub struct WithdrawalExecuted {
  pub executor: Pubkey,
  pub withdrawal_type: String,
  pub amount: u64,
  pub destination: Pubkey,
  pub executed_at: i64,
}

#[event]
pub struct WithdrawalVetoed {
  pub guardian: Pubkey,
  pub withdrawal_type: String,
  pub amount: u64,
  pub vetoed_at: i64,
}

#[event]
pub struct WithdrawalCancelled {
  pub admin: Pubkey,
  pub withdrawal_type: String,
  pub amount: u64,
  pub cancelled_at: i64,
}

#[event]
pub struct TimelockDurationChanged {
  pub admin: Pubkey,
  pub old_duration: i64,
  pub new_duration: i64,
  pub changed_at: i64,
}

#[event]
pub struct DailyLimitChanged {
  pub admin: Pubkey,
  pub old_limit: u64,
  pub new_limit: u64,
  pub changed_at: i64,
}

#[event]
pub struct EmergencyUnstake {
  pub lender: Pubkey,
  pub amount: u64,
  pub remaining_staked: u64,
  pub unstaked_at: i64,
}

// Authority Proxy events
#[event]
pub struct AuthorityTransferred {
  pub program_id: Pubkey,
  pub old_authority: Pubkey,
  pub new_authority_pda: Pubkey,
  pub transferred_at: i64,
}

#[event]
pub struct ProgramUpgraded {
  pub program_id: Pubkey,
  pub developer: Pubkey,
  pub buffer_address: Pubkey,
  pub upgraded_at: i64,
}

#[event]
pub struct ProgramRentReclaimed {
  pub program_id: Pubkey,
  pub developer: Pubkey,
  pub lamports_recovered: u64,
  pub reclaimed_at: i64,
}

// Escrow & Auto-Renewal events

#[event]
pub struct EscrowInitialized {
  pub developer: Pubkey,
  pub escrow_pda: Pubkey,
  pub auto_renew_enabled: bool,
  pub initialized_at: i64,
}

#[event]
pub struct EscrowDeposited {
  pub developer: Pubkey,
  pub token_type: u8, // 0=SOL, 1=USDC, 2=USDT
  pub amount: u64,
  pub new_balance: u64,
  pub deposited_at: i64,
}

#[event]
pub struct EscrowWithdrawn {
  pub developer: Pubkey,
  pub token_type: u8,
  pub amount: u64,
  pub remaining_balance: u64,
  pub withdrawn_at: i64,
}

#[event]
pub struct AutoRenewalExecuted {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub token_type: u8,
  pub amount_deducted: u64,
  pub months_renewed: u32,
  pub new_expiry: i64,
  pub escrow_remaining: u64,
  pub renewed_at: i64,
}

#[event]
pub struct AutoRenewalFailed {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub reason: String,
  pub escrow_balance: u64,
  pub required_amount: u64,
  pub failed_at: i64,
}

#[event]
pub struct GracePeriodStarted {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub grace_period_days: u8,
  pub grace_period_end: i64,
  pub started_at: i64,
}

#[event]
pub struct GracePeriodEnded {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub action: String, // "renewed" or "closed"
  pub ended_at: i64,
}

#[event]
pub struct ProgramClosedAfterGrace {
  pub request_id: [u8; 32],
  pub developer: Pubkey,
  pub program_id: Pubkey,
  pub grace_period_days: u8,
  pub closed_at: i64,
}

#[event]
pub struct AutoRenewSettingsChanged {
  pub developer: Pubkey,
  pub auto_renew_enabled: bool,
  pub preferred_token: u8,
  pub changed_at: i64,
}

// === DEBT TRACKING EVENTS ===

#[event]
pub struct DebtRepaid {
  pub deploy_request_id: [u8; 32],
  pub developer: Pubkey,
  pub borrowed_amount: u64,
  pub repaid_amount: u64,
  pub remaining_debt: u64,
  pub recovery_ratio_bps: u64,
  pub repaid_at: i64,
}

#[event]
pub struct DeploymentBorrowed {
  pub deploy_request_id: [u8; 32],
  pub developer: Pubkey,
  pub borrowed_amount: u64,
  pub total_borrowed: u64,
  pub active_deployment_count: u32,
  pub borrowed_at: i64,
}

// === WITHDRAWAL QUEUE EVENTS ===

#[event]
pub struct StakerWithdrawalQueued {
  pub staker: Pubkey,
  pub amount: u64,
  pub queue_position: u32,
  pub queued_withdrawal_total: u64,
  pub queued_at: i64,
}

#[event]
pub struct WithdrawalQueueProcessed {
  pub entries_processed: u32,
  pub total_amount: u64,
  pub remaining_queued: u64,
  pub processed_at: i64,
}

#[event]
pub struct StakerWithdrawalCancelled {
  pub staker: Pubkey,
  pub amount_cancelled: u64,
  pub cancelled_at: i64,
}

#[event]
pub struct QueuedWithdrawalFulfilled {
  pub staker: Pubkey,
  pub amount: u64,
  pub partial: bool,
  pub fulfilled_at: i64,
}

// === FAIR REWARD DISTRIBUTION EVENTS ===

#[event]
pub struct PendingRewardsDistributed {
  pub amount_distributed: u64,
  pub remaining_pending: u64,
  pub new_reward_per_share: u128,
  pub distributed_at: i64,
}

#[event]
pub struct DurationBonusClaimed {
  pub staker: Pubkey,
  pub duration_bonus: u64,
  pub stake_duration_weight: u128,
  pub claimed_at: i64,
}

#[event]
pub struct RewardsMovedToPending {
  pub amount: u64,
  pub reason: String,
  pub moved_at: i64,
}

// === PROTOCOL HEALTH EVENTS ===

#[event]
pub struct ProtocolHealthUpdated {
  pub utilization_bps: u64,
  pub current_apy_bps: u64,
  pub total_borrowed: u64,
  pub total_deposited: u64,
  pub queued_withdrawals: u64,
  pub recovery_ratio_bps: u64,
  pub updated_at: i64,
}
