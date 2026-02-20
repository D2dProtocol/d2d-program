use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

#[account]
#[derive(InitSpace)]
pub struct BackerDeposit {
  // === EXISTING FIELDS ===
  pub backer: Pubkey,
  pub deposited_amount: u64,
  pub reward_debt: u128,
  pub pending_rewards: u64,
  pub claimed_total: u64,
  pub is_active: bool,
  pub bump: u8,

  // === DURATION-WEIGHTED STAKING ===
  /// Timestamp when first deposit was made
  pub first_deposit_at: i64,
  /// Timestamp of last deposit/withdrawal action
  pub last_action_at: i64,
  /// Cumulative duration-weighted stake contribution
  /// Updated on each action: += deposited_amount * (now - last_action_at)
  pub stake_duration_weight: u128,
  /// Snapshot of reward_per_share at last weight update
  pub last_reward_per_share_snapshot: u128,

  // === WITHDRAWAL QUEUE ===
  /// Amount currently in withdrawal queue (0 if none)
  pub queued_withdrawal: u64,
  /// Position in withdrawal queue (0 if not queued)
  pub queue_position: u32,
  /// Timestamp when withdrawal was queued
  pub queued_at: i64,
}

pub type LenderStake = BackerDeposit;

impl BackerDeposit {
  pub const PREFIX_SEED: &'static [u8] = b"lender_stake";

  pub fn calculate_claimable_rewards(&self, reward_per_share: u128) -> Result<u64> {
    use crate::states::TreasuryPool;

    let accumulated = (self.deposited_amount as u128)
      .checked_mul(reward_per_share)
      .ok_or(ErrorCode::CalculationOverflow)?;

    // SECURITY FIX H-04: Use saturating_sub to handle edge case where
    // reward_debt > accumulated (can happen due to precision/timing issues)
    // Instead of erroring, gracefully return 0 new rewards in that case
    let from_reward_per_share = if accumulated >= self.reward_debt {
      accumulated
        .saturating_sub(self.reward_debt)
        .checked_div(TreasuryPool::PRECISION)
        .ok_or(ErrorCode::CalculationOverflow)?
    } else {
      // Edge case: reward_debt exceeds accumulated
      // This shouldn't happen normally, but handle gracefully
      0
    };

    let total_claimable = self
      .pending_rewards
      .checked_add(from_reward_per_share as u64)
      .ok_or(ErrorCode::CalculationOverflow)?;

    Ok(total_claimable)
  }

  pub fn settle_pending_rewards(&mut self, reward_per_share: u128) -> Result<()> {
    use crate::states::TreasuryPool;

    let accumulated = (self.deposited_amount as u128)
      .checked_mul(reward_per_share)
      .ok_or(ErrorCode::CalculationOverflow)?;

    // SECURITY FIX H-04: Use saturating_sub to handle edge case where
    // reward_debt > accumulated (can happen due to precision/timing issues)
    let new_rewards = if accumulated >= self.reward_debt {
      accumulated
        .saturating_sub(self.reward_debt)
        .checked_div(TreasuryPool::PRECISION)
        .ok_or(ErrorCode::CalculationOverflow)?
    } else {
      // Edge case: reward_debt exceeds accumulated
      // This shouldn't happen normally, but handle gracefully
      0
    };

    self.pending_rewards = self
      .pending_rewards
      .checked_add(new_rewards as u64)
      .ok_or(ErrorCode::CalculationOverflow)?;

    Ok(())
  }

  pub fn update_reward_debt(&mut self, reward_per_share: u128) -> Result<()> {
    self.reward_debt = (self.deposited_amount as u128)
      .checked_mul(reward_per_share)
      .ok_or(ErrorCode::CalculationOverflow)?;
    Ok(())
  }

  // === DURATION-WEIGHTED STAKING METHODS ===

  /// Update duration weight based on time elapsed since last action
  /// Returns the weight delta that was added
  pub fn update_duration_weight(&mut self, current_time: i64) -> Result<u128> {
    if self.last_action_at == 0 {
      // First time - no weight to add yet
      self.last_action_at = current_time;
      return Ok(0);
    }

    let duration = current_time.saturating_sub(self.last_action_at).max(0) as u128;

    let weight_delta = (self.deposited_amount as u128)
      .checked_mul(duration)
      .ok_or(ErrorCode::CalculationOverflow)?;

    self.stake_duration_weight = self
      .stake_duration_weight
      .checked_add(weight_delta)
      .ok_or(ErrorCode::CalculationOverflow)?;

    self.last_action_at = current_time;

    Ok(weight_delta)
  }

  /// Reset duration weight after claiming rewards
  pub fn reset_duration_weight(&mut self, current_time: i64) {
    self.stake_duration_weight = 0;
    self.last_action_at = current_time;
  }

  /// Initialize timestamps for a new deposit
  pub fn initialize_timestamps(&mut self, current_time: i64) {
    if self.first_deposit_at == 0 {
      self.first_deposit_at = current_time;
    }
    self.last_action_at = current_time;
  }

  /// Get staking duration in seconds
  pub fn get_staking_duration(&self, current_time: i64) -> i64 {
    if self.first_deposit_at == 0 {
      return 0;
    }
    current_time.saturating_sub(self.first_deposit_at)
  }

  // === WITHDRAWAL QUEUE METHODS ===

  /// Check if staker has a pending withdrawal in queue
  pub fn has_queued_withdrawal(&self) -> bool {
    self.queued_withdrawal > 0
  }

  /// Queue a withdrawal request
  pub fn queue_withdrawal(&mut self, amount: u64, position: u32, current_time: i64) -> Result<()> {
    require!(
      self.queued_withdrawal == 0,
      ErrorCode::WithdrawalAlreadyQueued
    );
    require!(
      amount <= self.deposited_amount,
      ErrorCode::InsufficientStake
    );

    self.queued_withdrawal = amount;
    self.queue_position = position;
    self.queued_at = current_time;

    Ok(())
  }

  /// Process (partial) withdrawal from queue
  pub fn process_queued_withdrawal(&mut self, amount: u64) -> Result<()> {
    self.queued_withdrawal = self.queued_withdrawal.saturating_sub(amount);

    // Clear queue fields if fully processed
    if self.queued_withdrawal == 0 {
      self.queue_position = 0;
      self.queued_at = 0;
    }

    Ok(())
  }

  /// Cancel queued withdrawal
  pub fn cancel_queued_withdrawal(&mut self) -> Result<u64> {
    let amount = self.queued_withdrawal;
    self.queued_withdrawal = 0;
    self.queue_position = 0;
    self.queued_at = 0;
    Ok(amount)
  }

  /// Get effective deposited amount (excluding queued withdrawals)
  pub fn get_effective_deposit(&self) -> u64 {
    self.deposited_amount.saturating_sub(self.queued_withdrawal)
  }

  pub fn init(&mut self, backer: Pubkey, bump: u8, current_time: i64) {
    self.backer = backer;
    self.deposited_amount = 0;
    self.reward_debt = 0;
    self.pending_rewards = 0;
    self.claimed_total = 0;
    self.is_active = true;
    self.bump = bump;

    // Initialize duration tracking timestamps for new deposit
    self.initialize_timestamps(current_time);
  }
}
