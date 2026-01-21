use crate::errors::ErrorCode;
use crate::events::WithdrawalVetoed;
use crate::states::{PendingWithdrawal, TreasuryPool, WithdrawalType};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct GuardianVeto<'info> {
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
        close = guardian
    )]
    pub pending_withdrawal: Account<'info, PendingWithdrawal>,

    #[account(mut)]
    pub guardian: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn guardian_veto(ctx: Context<GuardianVeto>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let pending_withdrawal = &ctx.accounts.pending_withdrawal;

    require!(treasury_pool.has_guardian(), ErrorCode::GuardianNotSet);
    require!(
        ctx.accounts.guardian.key() == treasury_pool.guardian,
        ErrorCode::OnlyGuardian
    );

    let current_time = Clock::get()?.unix_timestamp;

    require!(!pending_withdrawal.executed, ErrorCode::NoPendingWithdrawal);
    require!(!pending_withdrawal.vetoed, ErrorCode::NoPendingWithdrawal);
    require!(
        pending_withdrawal.can_veto(current_time),
        ErrorCode::TimelockNotExpired
    );

    let amount = pending_withdrawal.amount;
    let withdrawal_type_str = match pending_withdrawal.withdrawal_type {
        WithdrawalType::PlatformPool => "PlatformPool",
        WithdrawalType::RewardPool => "RewardPool",
    };

    treasury_pool.pending_withdrawal_count = 0;

    emit!(WithdrawalVetoed {
        guardian: ctx.accounts.guardian.key(),
        withdrawal_type: withdrawal_type_str.to_string(),
        amount,
        vetoed_at: current_time,
    });

    Ok(())
}
