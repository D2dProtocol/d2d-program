use crate::errors::ErrorCode;
use crate::events::RewardsClaimed;
use crate::states::{LenderStake, TreasuryPool};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    /// CHECK: Reward Pool PDA
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LenderStake::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
    pub lender_stake: Account<'info, LenderStake>,

    #[account(mut)]
    pub lender: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let reward_pool_info = ctx.accounts.reward_pool.to_account_info();

    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    let claimable_rewards = lender_stake.calculate_claimable_rewards(treasury_pool.reward_per_share)?;
    require!(claimable_rewards > 0, ErrorCode::NoRewardsToClaim);

    require!(
        treasury_pool.reward_pool_balance >= claimable_rewards,
        ErrorCode::InsufficientTreasuryFunds
    );

    require!(
        reward_pool_info.lamports() >= claimable_rewards,
        ErrorCode::InsufficientTreasuryFunds
    );

    lender_stake.claimed_total = lender_stake
        .claimed_total
        .checked_add(claimable_rewards)
        .ok_or(ErrorCode::CalculationOverflow)?;

    lender_stake.pending_rewards = 0;
    lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;
    treasury_pool.debit_reward_pool(claimable_rewards)?;
    treasury_pool.record_claimed_rewards(claimable_rewards)?;

    {
        let lender_info = ctx.accounts.lender.to_account_info();
        let mut reward_pool_lamports = reward_pool_info.try_borrow_mut_lamports()?;
        let mut lender_lamports = lender_info.try_borrow_mut_lamports()?;

        **reward_pool_lamports = (**reward_pool_lamports)
            .checked_sub(claimable_rewards)
            .ok_or(ErrorCode::CalculationOverflow)?;
        **lender_lamports = (**lender_lamports)
            .checked_add(claimable_rewards)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }

    emit!(RewardsClaimed {
        lender: lender_stake.backer,
        amount: claimable_rewards,
        total_claimed: lender_stake.claimed_total,
    });

    emit!(crate::events::Claimed {
        backer: lender_stake.backer,
        amount: claimable_rewards,
        claimed_total: lender_stake.claimed_total,
        reward_per_share: treasury_pool.reward_per_share,
        claimed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
