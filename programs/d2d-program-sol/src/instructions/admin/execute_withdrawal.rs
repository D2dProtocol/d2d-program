use crate::errors::ErrorCode;
use crate::events::WithdrawalExecuted;
use crate::states::{PendingWithdrawal, TreasuryPool, WithdrawalType};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ExecuteWithdrawal<'info> {
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

    /// CHECK: Platform Pool PDA
    #[account(
        mut,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump = treasury_pool.platform_pool_bump
    )]
    pub platform_pool: UncheckedAccount<'info>,

    /// CHECK: Reward Pool PDA
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,

    /// CHECK: Destination wallet
    #[account(
        mut,
        constraint = destination.key() == pending_withdrawal.destination @ ErrorCode::InvalidTreasuryWallet
    )]
    pub destination: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn execute_withdrawal(ctx: Context<ExecuteWithdrawal>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let pending_withdrawal = &ctx.accounts.pending_withdrawal;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    let current_time = Clock::get()?.unix_timestamp;

    require!(!pending_withdrawal.executed, ErrorCode::NoPendingWithdrawal);
    require!(!pending_withdrawal.vetoed, ErrorCode::NoPendingWithdrawal);
    require!(
        pending_withdrawal.can_execute(current_time),
        ErrorCode::TimelockNotExpired
    );
    require!(
        !pending_withdrawal.is_expired(current_time),
        ErrorCode::PendingWithdrawalExpired
    );

    let amount = pending_withdrawal.amount;

    treasury_pool.check_and_update_daily_limit(amount, current_time)?;

    let withdrawal_type_str = match pending_withdrawal.withdrawal_type {
        WithdrawalType::PlatformPool => {
            let platform_pool_info = ctx.accounts.platform_pool.to_account_info();
            let destination_info = ctx.accounts.destination.to_account_info();

            require!(
                platform_pool_info.lamports() >= amount,
                ErrorCode::InsufficientTreasuryFunds
            );
            require!(
                treasury_pool.platform_pool_balance >= amount,
                ErrorCode::InsufficientTreasuryFunds
            );

            {
                let mut platform_pool_lamports = platform_pool_info.try_borrow_mut_lamports()?;
                let mut destination_lamports = destination_info.try_borrow_mut_lamports()?;

                **platform_pool_lamports = (**platform_pool_lamports)
                    .checked_sub(amount)
                    .ok_or(ErrorCode::CalculationOverflow)?;
                **destination_lamports = (**destination_lamports)
                    .checked_add(amount)
                    .ok_or(ErrorCode::CalculationOverflow)?;
            }

            treasury_pool.platform_pool_balance = treasury_pool
                .platform_pool_balance
                .checked_sub(amount)
                .ok_or(ErrorCode::CalculationOverflow)?;

            "PlatformPool"
        }
        WithdrawalType::RewardPool => {
            let reward_pool_info = ctx.accounts.reward_pool.to_account_info();
            let destination_info = ctx.accounts.destination.to_account_info();

            let excess_rewards = treasury_pool.get_excess_rewards();
            require!(
                amount <= excess_rewards,
                ErrorCode::CannotWithdrawProtectedRewards
            );

            require!(
                reward_pool_info.lamports() >= amount,
                ErrorCode::InsufficientTreasuryFunds
            );
            require!(
                treasury_pool.reward_pool_balance >= amount,
                ErrorCode::InsufficientTreasuryFunds
            );

            {
                let mut reward_pool_lamports = reward_pool_info.try_borrow_mut_lamports()?;
                let mut destination_lamports = destination_info.try_borrow_mut_lamports()?;

                **reward_pool_lamports = (**reward_pool_lamports)
                    .checked_sub(amount)
                    .ok_or(ErrorCode::CalculationOverflow)?;
                **destination_lamports = (**destination_lamports)
                    .checked_add(amount)
                    .ok_or(ErrorCode::CalculationOverflow)?;
            }

            treasury_pool.reward_pool_balance = treasury_pool
                .reward_pool_balance
                .checked_sub(amount)
                .ok_or(ErrorCode::CalculationOverflow)?;

            "RewardPool"
        }
    };

    treasury_pool.pending_withdrawal_count = 0;

    emit!(WithdrawalExecuted {
        executor: ctx.accounts.admin.key(),
        withdrawal_type: withdrawal_type_str.to_string(),
        amount,
        destination: pending_withdrawal.destination,
        executed_at: current_time,
    });

    Ok(())
}
