use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::EscrowInitialized,
  states::{DeveloperEscrow, TokenType, TreasuryPool},
};

#[derive(Accounts)]
pub struct InitializeEscrow<'info> {
  #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        init,
        payer = developer,
        space = 8 + DeveloperEscrow::INIT_SPACE,
        seeds = [DeveloperEscrow::PREFIX_SEED, developer.key().as_ref()],
        bump
    )]
  pub developer_escrow: Account<'info, DeveloperEscrow>,

  #[account(mut)]
  pub developer: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn initialize_escrow(ctx: Context<InitializeEscrow>) -> Result<()> {
  let treasury_pool = &ctx.accounts.treasury_pool;
  let developer_escrow = &mut ctx.accounts.developer_escrow;
  let developer = &ctx.accounts.developer;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

  let current_time = Clock::get()?.unix_timestamp;

  developer_escrow.developer = developer.key();
  developer_escrow.sol_balance = 0;
  developer_escrow.usdc_balance = 0;
  developer_escrow.usdt_balance = 0;
  developer_escrow.auto_renew_enabled = true; // Enabled by default
  developer_escrow.preferred_token = TokenType::SOL;
  developer_escrow.min_balance_alert = 100_000_000; // 0.1 SOL default threshold
  developer_escrow.total_deposited_sol = 0;
  developer_escrow.total_deposited_usdc = 0;
  developer_escrow.total_deposited_usdt = 0;
  developer_escrow.total_auto_deducted = 0;
  developer_escrow.created_at = current_time;
  developer_escrow.last_deposit_at = 0;
  developer_escrow.last_auto_deduct_at = 0;
  developer_escrow.bump = ctx.bumps.developer_escrow;

  emit!(EscrowInitialized {
    developer: developer.key(),
    escrow_pda: developer_escrow.key(),
    auto_renew_enabled: true,
    initialized_at: current_time,
  });

  Ok(())
}
