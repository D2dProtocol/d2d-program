use crate::errors::ErrorCode;
use crate::events::WithdrawalCancelled;
use crate::states::{PendingWithdrawal, TreasuryPool, WithdrawalType};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CancelWithdrawal<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        mut,
        seeds = [PendingWithdrawal::PREFIX_SEED, treasury_pool.key().as_ref()],
        bump = pending_withdrawal.bump,
        close = admin
    )]
    pub pending_withdrawal: Account<'info, PendingWithdrawal>,

    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn cancel_withdrawal(ctx: Context<CancelWithdrawal>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let pending_withdrawal = &ctx.accounts.pending_withdrawal;

    require!(!pending_withdrawal.executed, ErrorCode::NoPendingWithdrawal);
    require!(!pending_withdrawal.vetoed, ErrorCode::NoPendingWithdrawal);

    let amount = pending_withdrawal.amount;
    let withdrawal_type_str = match pending_withdrawal.withdrawal_type {
        WithdrawalType::PlatformPool => "PlatformPool",
        WithdrawalType::RewardPool => "RewardPool",
    };

    treasury_pool.pending_withdrawal_count = 0;

    emit!(WithdrawalCancelled {
        admin: ctx.accounts.admin.key(),
        withdrawal_type: withdrawal_type_str.to_string(),
        amount,
        cancelled_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
