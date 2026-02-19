use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::{DurationBonusClaimed, RewardsClaimed},
  states::{LenderStake, TreasuryPool},
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  /// CHECK: Reward Pool PDA
  #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
  pub reward_pool: UncheckedAccount<'info>,

  #[account(
        mut,
        seeds = [LenderStake::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
  pub lender_stake: Account<'info, LenderStake>,

  #[account(mut)]
  pub lender: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
  let reward_pool_info = ctx.accounts.reward_pool.to_account_info();

  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let lender_stake = &mut ctx.accounts.lender_stake;
  let current_time = Clock::get()?.unix_timestamp;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

  // Update duration weight before calculating rewards
  let weight_delta = lender_stake.update_duration_weight(current_time)?;
  if weight_delta > 0 {
    treasury_pool.update_stake_duration_weight(weight_delta)?;
  }

  // Calculate base claimable rewards from reward_per_share
  let base_claimable = lender_stake.calculate_claimable_rewards(treasury_pool.reward_per_share)?;

  // Calculate duration-weighted bonus from pending_undistributed_rewards
  let duration_bonus =
    treasury_pool.calculate_duration_bonus(lender_stake.stake_duration_weight)?;

  // Total claimable = base + duration bonus
  let total_claimable = base_claimable
    .checked_add(duration_bonus)
    .ok_or(ErrorCode::CalculationOverflow)?;

  require!(total_claimable > 0, ErrorCode::NoRewardsToClaim);

  // Verify we have enough funds
  require!(
    treasury_pool.reward_pool_balance >= base_claimable,
    ErrorCode::InsufficientTreasuryFunds
  );

  // For duration bonus, it comes from pending_undistributed_rewards
  // Verify total available
  let total_available = reward_pool_info.lamports();
  require!(
    total_available >= total_claimable,
    ErrorCode::InsufficientTreasuryFunds
  );

  // Update lender stake
  lender_stake.claimed_total = lender_stake
    .claimed_total
    .checked_add(total_claimable)
    .ok_or(ErrorCode::CalculationOverflow)?;

  lender_stake.pending_rewards = 0;
  lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;

  // Debit base from reward_pool_balance
  treasury_pool.debit_reward_pool(base_claimable)?;
  treasury_pool.record_claimed_rewards(base_claimable)?;

  // Debit duration bonus from pending_undistributed_rewards
  if duration_bonus > 0 {
    treasury_pool.pending_undistributed_rewards = treasury_pool
      .pending_undistributed_rewards
      .saturating_sub(duration_bonus);
  }

  // Reset staker's duration weight after claiming
  lender_stake.reset_duration_weight(current_time);

  // Transfer SOL from reward pool to lender
  {
    let lender_info = ctx.accounts.lender.to_account_info();
    let mut reward_pool_lamports = reward_pool_info.try_borrow_mut_lamports()?;
    let mut lender_lamports = lender_info.try_borrow_mut_lamports()?;

    **reward_pool_lamports = (**reward_pool_lamports)
      .checked_sub(total_claimable)
      .ok_or(ErrorCode::CalculationOverflow)?;
    **lender_lamports = (**lender_lamports)
      .checked_add(total_claimable)
      .ok_or(ErrorCode::CalculationOverflow)?;
  }

  // Emit events
  emit!(RewardsClaimed {
    lender: lender_stake.backer,
    amount: total_claimable,
    total_claimed: lender_stake.claimed_total,
  });

  if duration_bonus > 0 {
    emit!(DurationBonusClaimed {
      staker: lender_stake.backer,
      duration_bonus,
      stake_duration_weight: lender_stake.stake_duration_weight,
      claimed_at: current_time,
    });
  }

  emit!(crate::events::Claimed {
    backer: lender_stake.backer,
    amount: total_claimable,
    claimed_total: lender_stake.claimed_total,
    reward_per_share: treasury_pool.reward_per_share,
    claimed_at: current_time,
  });

  Ok(())
}
