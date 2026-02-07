use crate::errors::ErrorCode;
use crate::events::SolUnstaked;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UnstakeSol<'info> {
    /// CHECK: Treasury Pool - will be migrated if needed
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pool: UncheckedAccount<'info>,

    /// CHECK: Treasury Pool PDA (holds deposits)
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
    pub lender_stake: Account<'info, BackerDeposit>,

    #[account(mut)]
    pub lender: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn unstake_sol(ctx: Context<UnstakeSol>, amount: u64) -> Result<()> {
    require!(
        ctx.accounts.treasury_pda.key() == ctx.accounts.treasury_pool.key(),
        ErrorCode::InvalidAccountOwner
    );

    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();

    if current_space < required_space {
        treasury_pool_info.resize(required_space)?;
    }

    let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
        .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        amount <= lender_stake.deposited_amount,
        ErrorCode::InsufficientStake
    );

    if lender_stake.deposited_amount == 0 {
        return Err(ErrorCode::InsufficientStake.into());
    }

    // Check if staker has a pending queued withdrawal
    require!(
        !lender_stake.has_queued_withdrawal(),
        ErrorCode::WithdrawalAlreadyQueued
    );

    lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;

    // Update duration weight before withdrawal
    let current_time = Clock::get()?.unix_timestamp;
    let weight_delta = lender_stake.update_duration_weight(current_time)?;
    if weight_delta > 0 {
        treasury_pool.update_stake_duration_weight(weight_delta)?;
    }

    let treasury_lamports = treasury_pda_info.lamports();
    let account_data_size = treasury_pda_info.data_len();
    let rent_exemption = anchor_lang::solana_program::rent::Rent::get()?
        .minimum_balance(account_data_size);

    let available_balance = treasury_lamports
        .checked_sub(rent_exemption)
        .ok_or(ErrorCode::CalculationOverflow)?;

    if available_balance < amount {
        return Err(ErrorCode::InsufficientLiquidBalance.into());
    }

    let balance_diff = available_balance.abs_diff(treasury_pool.liquid_balance);
    if balance_diff > 1_000_000 {
        treasury_pool.liquid_balance = available_balance;
    }

    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    if lender_stake.deposited_amount == 0 {
        lender_stake.is_active = false;
        lender_stake.reward_debt = 0;
    } else {
        lender_stake.is_active = true;
        lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;
    }

    treasury_pool.total_deposited = treasury_pool
        .total_deposited
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    treasury_pool.liquid_balance = treasury_pool
        .liquid_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    {
        let lender_info = ctx.accounts.lender.to_account_info();
        let mut treasury_lamports = treasury_pda_info.try_borrow_mut_lamports()?;
        let mut lender_lamports = lender_info.try_borrow_mut_lamports()?;

        **treasury_lamports = (**treasury_lamports)
            .checked_sub(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        **lender_lamports = (**lender_lamports)
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }

    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    treasury_pool.try_serialize(&mut &mut data[..])?;

    emit!(SolUnstaked {
        lender: lender_stake.backer,
        amount,
        remaining_staked: lender_stake.deposited_amount,
    });

    Ok(())
}
