use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum WithdrawalType {
  PlatformPool,
  RewardPool,
}

#[account]
#[derive(InitSpace)]
pub struct PendingWithdrawal {
  pub withdrawal_type: WithdrawalType,
  pub amount: u64,
  pub destination: Pubkey,
  pub initiator: Pubkey,
  pub initiated_at: i64,
  pub execute_after: i64,
  pub expires_at: i64,
  #[max_len(200)]
  pub reason: String,
  pub executed: bool,
  pub vetoed: bool,
  pub bump: u8,
}

impl PendingWithdrawal {
  pub const PREFIX_SEED: &'static [u8] = b"pending_withdrawal";
  pub const DEFAULT_TIMELOCK_DURATION: i64 = 24 * 60 * 60;
  pub const MIN_TIMELOCK_DURATION: i64 = 60 * 60;
  pub const MAX_TIMELOCK_DURATION: i64 = 7 * 24 * 60 * 60;
  pub const VALIDITY_PERIOD: i64 = 7 * 24 * 60 * 60;

  pub fn can_execute(&self, current_time: i64) -> bool {
    !self.executed && !self.vetoed && current_time >= self.execute_after
  }

  pub fn is_expired(&self, current_time: i64) -> bool {
    current_time > self.expires_at
  }

  pub fn can_veto(&self, current_time: i64) -> bool {
    !self.executed && !self.vetoed && current_time < self.execute_after
  }
}
