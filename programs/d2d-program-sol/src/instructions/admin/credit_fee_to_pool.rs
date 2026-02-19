use anchor_lang::{prelude::*, system_program};

use crate::{errors::ErrorCode, events::RewardCredited, states::TreasuryPool};

/// Credit fees to pools (developer pays fees)
///
/// This instruction is called when developers pay fees for deployments.
/// Updates reward_per_share accumulator.
///
/// SECURITY: Developer (fee_payer) pays the fees, not admin
#[derive(Accounts)]
pub struct CreditFeeToPool<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  /// CHECK: Reward Pool PDA (receives reward fees)
  #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
  pub reward_pool: UncheckedAccount<'info>,

  /// CHECK: Platform Pool PDA (receives platform fees)
  #[account(
        mut,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump = treasury_pool.platform_pool_bump
    )]
  pub platform_pool: UncheckedAccount<'info>,

  /// Admin signer to authorize the fee credit operation
  #[account(
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
  pub admin: Signer<'info>,

  /// SECURITY FIX: Developer/fee payer who pays the fees (not admin)
  #[account(mut)]
  pub fee_payer: Signer<'info>,

  pub system_program: Program<'info, System>,
}

/// Credit fees to pools and update reward_per_share
///
/// SECURITY FIX Flow:
/// 1. Developer (fee_payer) transfers fees to RewardPool and PlatformPool PDAs
/// 2. Admin authorizes the fee credit operation
/// 3. Call treasury_pool.credit_fee_to_pool() which updates reward_per_share
///
/// IMPORTANT: Developer (fee_payer) pays the fees, NOT admin
pub fn credit_fee_to_pool(
  ctx: Context<CreditFeeToPool>,
  fee_reward: u64,
  fee_platform: u64,
) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(fee_reward > 0 || fee_platform > 0, ErrorCode::InvalidAmount);

  // SECURITY FIX: Check fee_payer (developer) has enough lamports, not admin
  let fee_payer_lamports = ctx.accounts.fee_payer.lamports();
  let total_fees = fee_reward
    .checked_add(fee_platform)
    .ok_or(ErrorCode::CalculationOverflow)?;

  require!(
    fee_payer_lamports >= total_fees,
    ErrorCode::InsufficientDeposit
  );

  // SECURITY FIX: Transfer reward fee from fee_payer (developer) to Reward Pool PDA
  if fee_reward > 0 {
    let reward_fee_cpi = CpiContext::new(
      ctx.accounts.system_program.to_account_info(),
      system_program::Transfer {
        from: ctx.accounts.fee_payer.to_account_info(),
        to: ctx.accounts.reward_pool.to_account_info(),
      },
    );
    system_program::transfer(reward_fee_cpi, fee_reward)?;
  }

  // SECURITY FIX: Transfer platform fee from fee_payer (developer) to Platform Pool PDA
  if fee_platform > 0 {
    let platform_fee_cpi = CpiContext::new(
      ctx.accounts.system_program.to_account_info(),
      system_program::Transfer {
        from: ctx.accounts.fee_payer.to_account_info(),
        to: ctx.accounts.platform_pool.to_account_info(),
      },
    );
    system_program::transfer(platform_fee_cpi, fee_platform)?;
  }

  // Credit fees to pools and update reward_per_share
  // This is the key function that updates the accumulator
  treasury_pool.credit_fee_to_pool(fee_reward, fee_platform)?;

  emit!(RewardCredited {
    fee_reward,
    fee_platform,
    reward_per_share: treasury_pool.reward_per_share,
    total_deposited: treasury_pool.total_deposited,
    credited_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
