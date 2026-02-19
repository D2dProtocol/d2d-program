use anchor_lang::prelude::*;

use crate::{events::TreasuryInitialized, states::TreasuryPool};

#[derive(Accounts)]
pub struct ReinitializeTreasuryPool<'info> {
  /// CHECK: Treasury Pool PDA - will be reinitialized
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
  pub treasury_pool: UncheckedAccount<'info>,

  /// CHECK: Reward Pool PDA
  #[account(
        init_if_needed,
        payer = admin,
        space = 8,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump
    )]
  pub reward_pool: UncheckedAccount<'info>,

  /// CHECK: Platform Pool PDA
  #[account(
        init_if_needed,
        payer = admin,
        space = 8,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump
    )]
  pub platform_pool: UncheckedAccount<'info>,

  #[account(mut)]
  pub admin: Signer<'info>,

  /// CHECK: Dev wallet
  pub dev_wallet: UncheckedAccount<'info>,

  pub system_program: Program<'info, System>,
}

pub fn reinitialize_treasury_pool(
  ctx: Context<ReinitializeTreasuryPool>,
  _initial_apy: u64,
  dev_wallet: Pubkey,
) -> Result<()> {
  let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
  let required_space = 8 + TreasuryPool::INIT_SPACE;

  let current_space = treasury_pool_info.data_len();
  if current_space < required_space {
    treasury_pool_info.resize(required_space)?;
  }

  let mut data = treasury_pool_info.try_borrow_mut_data()?;
  data[..].fill(0);

  let treasury_pool = TreasuryPool {
    reward_per_share: 0,
    total_deposited: 0,
    liquid_balance: 0,
    reward_pool_balance: 0,
    platform_pool_balance: 0,
    reward_fee_bps: TreasuryPool::REWARD_FEE_BPS,
    platform_fee_bps: TreasuryPool::PLATFORM_FEE_BPS,
    admin: ctx.accounts.admin.key(),
    dev_wallet,
    emergency_pause: false,
    guardian: Pubkey::default(),
    timelock_duration: TreasuryPool::DEFAULT_TIMELOCK_DURATION,
    pending_withdrawal_count: 0,
    daily_withdrawal_limit: TreasuryPool::DEFAULT_DAILY_LIMIT,
    last_withdrawal_day: 0,
    withdrawn_today: 0,
    total_credited_rewards: 0,
    total_claimed_rewards: 0,
    reward_pool_bump: ctx.bumps.reward_pool,
    platform_pool_bump: ctx.bumps.platform_pool,
    bump: ctx.bumps.treasury_pool,
    // Debt tracking fields
    total_borrowed: 0,
    total_recovered: 0,
    total_debt_repaid: 0,
    active_deployment_count: 0,
    // Fair reward distribution fields
    total_stake_duration_weight: 0,
    last_weight_update: 0,
    pending_undistributed_rewards: 0,
    // Withdrawal queue fields
    withdrawal_queue_head: 0,
    withdrawal_queue_tail: 0,
    queued_withdrawal_amount: 0,
    // Dynamic APY fields
    base_apy_bps: TreasuryPool::DEFAULT_BASE_APY_BPS,
    max_apy_multiplier_bps: TreasuryPool::DEFAULT_MAX_APY_MULTIPLIER_BPS,
    target_utilization_bps: TreasuryPool::DEFAULT_TARGET_UTILIZATION_BPS,
  };

  treasury_pool.try_serialize(&mut &mut data[..])?;

  emit!(TreasuryInitialized {
    admin: ctx.accounts.admin.key(),
    treasury_wallet: dev_wallet,
    initial_apy: 0,
  });

  Ok(())
}
