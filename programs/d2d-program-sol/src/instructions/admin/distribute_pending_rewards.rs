use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, events::PendingRewardsDistributed, states::TreasuryPool};

/// Distribute accumulated pending rewards to stakers
/// This instruction gradually releases rewards that were accumulated
/// (e.g., from first-depositor protection or excess rent recovery)
///
/// Called periodically by admin/cron to ensure fair reward distribution
#[derive(Accounts)]
pub struct DistributePendingRewards<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        constraint = treasury_pool.is_admin_or_guardian(&caller.key()) @ ErrorCode::Unauthorized
    )]
  pub caller: Signer<'info>,
}

/// Distribute a percentage of pending undistributed rewards
///
/// Args:
/// - distribution_percentage_bps: Percentage to distribute (basis points, max 10000 = 100%)
///   Recommended: 1000 (10%) for daily distribution, 500 (5%) for more gradual
pub fn distribute_pending_rewards(
  ctx: Context<DistributePendingRewards>,
  distribution_percentage_bps: u64,
) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let current_time = Clock::get()?.unix_timestamp;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(
    distribution_percentage_bps > 0 && distribution_percentage_bps <= 10000,
    ErrorCode::InvalidDistributionPercentage
  );
  require!(
    treasury_pool.pending_undistributed_rewards > 0,
    ErrorCode::NoPendingRewards
  );
  require!(
    treasury_pool.total_deposited > 0,
    ErrorCode::NoStakersForDistribution
  );

  // Distribute rewards
  let amount_distributed = treasury_pool.distribute_pending_rewards(distribution_percentage_bps)?;

  // Update last weight update timestamp
  treasury_pool.last_weight_update = current_time;

  emit!(PendingRewardsDistributed {
    amount_distributed,
    remaining_pending: treasury_pool.pending_undistributed_rewards,
    new_reward_per_share: treasury_pool.reward_per_share,
    distributed_at: current_time,
  });

  Ok(())
}
