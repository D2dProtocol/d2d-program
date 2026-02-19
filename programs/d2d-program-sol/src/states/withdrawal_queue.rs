use anchor_lang::prelude::*;

/// Entry in the staker withdrawal queue
/// Created when a staker requests to withdraw but liquid_balance is insufficient
#[account]
#[derive(InitSpace)]
pub struct WithdrawalQueueEntry {
  /// Queue position (sequential counter, used as PDA seed)
  pub position: u32,
  /// Staker requesting withdrawal
  pub staker: Pubkey,
  /// Amount requested for withdrawal
  pub amount: u64,
  /// Timestamp when request was queued
  pub queued_at: i64,
  /// Whether this entry has been fully processed
  pub processed: bool,
  /// Amount actually withdrawn so far (may be partial)
  pub amount_withdrawn: u64,
  /// Timestamp when fully processed (0 if pending)
  pub processed_at: i64,
  /// PDA bump
  pub bump: u8,
}

impl WithdrawalQueueEntry {
  pub const PREFIX_SEED: &'static [u8] = b"withdrawal_queue";

  /// Check if this entry is pending (not yet fully processed)
  pub fn is_pending(&self) -> bool {
    !self.processed && self.amount > self.amount_withdrawn
  }

  /// Get remaining amount to be withdrawn
  pub fn get_remaining_amount(&self) -> u64 {
    self.amount.saturating_sub(self.amount_withdrawn)
  }

  /// Process a partial or full withdrawal
  /// Returns the amount that was processed
  pub fn process_withdrawal(&mut self, available_amount: u64, current_time: i64) -> u64 {
    let remaining = self.get_remaining_amount();
    let to_process = available_amount.min(remaining);

    self.amount_withdrawn = self.amount_withdrawn.saturating_add(to_process);

    // Mark as fully processed if complete
    if self.amount_withdrawn >= self.amount {
      self.processed = true;
      self.processed_at = current_time;
    }

    to_process
  }

  /// Cancel this queue entry (mark as processed without transferring)
  pub fn cancel(&mut self, current_time: i64) {
    self.processed = true;
    self.processed_at = current_time;
  }

  /// Get percentage completed (0-100)
  pub fn get_completion_percentage(&self) -> u8 {
    if self.amount == 0 {
      return 100;
    }
    ((self.amount_withdrawn as u128) * 100 / (self.amount as u128)) as u8
  }

  /// Estimate wait time based on recovery rate (seconds)
  /// recovery_rate_per_day is in lamports per day
  pub fn estimate_wait_time(&self, recovery_rate_per_day: u64) -> i64 {
    if recovery_rate_per_day == 0 {
      return i64::MAX; // Unknown/infinite wait
    }

    let remaining = self.get_remaining_amount();
    let seconds_per_day: i64 = 24 * 60 * 60;

    // wait_days = remaining / recovery_rate_per_day
    let wait_seconds = (remaining as i128)
      .saturating_mul(seconds_per_day as i128)
      .saturating_div(recovery_rate_per_day as i128);

    wait_seconds.min(i64::MAX as i128) as i64
  }
}
