use anchor_lang::prelude::*;

use crate::{events::TreasuryInitialized, states::TreasuryPool};

#[derive(Accounts)]
pub struct Initialize<'info> {
  #[account(
    init,
    payer = admin,
    space = 8 + TreasuryPool::INIT_SPACE,
    seeds = [TreasuryPool::PREFIX_SEED],
    bump
  )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  /// CHECK: Reward Pool PDA
  #[account(
    init,
    payer = admin,
    space = 8,
    seeds = [TreasuryPool::REWARD_POOL_SEED],
    bump
  )]
  pub reward_pool: UncheckedAccount<'info>,

  /// CHECK: Platform Pool PDA
  #[account(
    init,
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

pub fn initialize(ctx: Context<Initialize>, _initial_apy: u64, dev_wallet: Pubkey) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;

  treasury_pool.reward_per_share = 0;
  treasury_pool.total_deposited = 0;
  treasury_pool.liquid_balance = 0;
  treasury_pool.reward_pool_balance = 0;
  treasury_pool.platform_pool_balance = 0;
  treasury_pool.reward_fee_bps = TreasuryPool::REWARD_FEE_BPS;
  treasury_pool.platform_fee_bps = TreasuryPool::PLATFORM_FEE_BPS;

  treasury_pool.admin = ctx.accounts.admin.key();
  treasury_pool.dev_wallet = dev_wallet;
  treasury_pool.emergency_pause = false;

  treasury_pool.guardian = Pubkey::default();
  treasury_pool.timelock_duration = TreasuryPool::DEFAULT_TIMELOCK_DURATION;
  treasury_pool.pending_withdrawal_count = 0;

  treasury_pool.daily_withdrawal_limit = TreasuryPool::DEFAULT_DAILY_LIMIT;
  treasury_pool.last_withdrawal_day = 0;
  treasury_pool.withdrawn_today = 0;

  treasury_pool.total_credited_rewards = 0;
  treasury_pool.total_claimed_rewards = 0;

  treasury_pool.reward_pool_bump = ctx.bumps.reward_pool;
  treasury_pool.platform_pool_bump = ctx.bumps.platform_pool;
  treasury_pool.bump = ctx.bumps.treasury_pool;

  emit!(TreasuryInitialized {
    admin: treasury_pool.admin,
    treasury_wallet: dev_wallet,
    initial_apy: 0,
  });

  Ok(())
}
