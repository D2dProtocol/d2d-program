use anchor_lang::{prelude::*, system_program};

use crate::{
  errors::ErrorCode,
  events::EscrowDeposited,
  states::{DeveloperEscrow, TokenType, TreasuryPool},
};

#[derive(Accounts)]
pub struct DepositEscrowSol<'info> {
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

pub fn deposit_escrow_sol(ctx: Context<DepositEscrowSol>, amount: u64) -> Result<()> {
  let treasury_pool = &ctx.accounts.treasury_pool;
  let developer_escrow = &mut ctx.accounts.developer_escrow;
  let developer = &ctx.accounts.developer;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(amount > 0, ErrorCode::InvalidAmount);

  // Transfer SOL from developer to escrow PDA
  let cpi_context = CpiContext::new(
    ctx.accounts.system_program.to_account_info(),
    system_program::Transfer {
      from: developer.to_account_info(),
      to: developer_escrow.to_account_info(),
    },
  );
  system_program::transfer(cpi_context, amount)?;

  // Update escrow balance
  developer_escrow.add_balance(amount, TokenType::SOL)?;

  emit!(EscrowDeposited {
    developer: developer.key(),
    token_type: 0, // SOL
    amount,
    new_balance: developer_escrow.sol_balance,
    deposited_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
