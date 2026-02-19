use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::EscrowWithdrawn,
  states::{DeveloperEscrow, TreasuryPool},
};

#[derive(Accounts)]
pub struct WithdrawEscrowSol<'info> {
  #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        mut,
        seeds = [DeveloperEscrow::PREFIX_SEED, developer.key().as_ref()],
        bump = developer_escrow.bump,
        constraint = developer_escrow.developer == developer.key() @ ErrorCode::Unauthorized
    )]
  pub developer_escrow: Account<'info, DeveloperEscrow>,

  #[account(mut)]
  pub developer: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn withdraw_escrow_sol(ctx: Context<WithdrawEscrowSol>, amount: u64) -> Result<()> {
  let treasury_pool = &ctx.accounts.treasury_pool;
  let developer_escrow = &mut ctx.accounts.developer_escrow;
  let developer = &ctx.accounts.developer;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(amount > 0, ErrorCode::InvalidAmount);
  require!(
    developer_escrow.sol_balance >= amount,
    ErrorCode::InsufficientEscrowBalance
  );

  // Update escrow balance first
  developer_escrow.sol_balance = developer_escrow
    .sol_balance
    .checked_sub(amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  // Transfer SOL from escrow PDA to developer
  // We need to transfer lamports from the escrow account
  let escrow_account_info = developer_escrow.to_account_info();
  let developer_account_info = developer.to_account_info();

  **escrow_account_info.try_borrow_mut_lamports()? = escrow_account_info
    .lamports()
    .checked_sub(amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  **developer_account_info.try_borrow_mut_lamports()? = developer_account_info
    .lamports()
    .checked_add(amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  emit!(EscrowWithdrawn {
    developer: developer.key(),
    token_type: 0, // SOL
    amount,
    remaining_balance: developer_escrow.sol_balance,
    withdrawn_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
