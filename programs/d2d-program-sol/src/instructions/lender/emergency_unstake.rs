use crate::errors::ErrorCode;
use crate::events::EmergencyUnstake;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct EmergencyUnstakeSol<'info> {
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

pub fn emergency_unstake_sol(ctx: Context<EmergencyUnstakeSol>, amount: u64) -> Result<()> {
    require!(
        ctx.accounts.treasury_pda.key() == ctx.accounts.treasury_pool.key(),
        ErrorCode::InvalidAccountOwner
    );

    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();

    if current_space < required_space {
        treasury_pool_info.realloc(required_space, false)?;
    }

    let mut treasury_pool =
        TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
            .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        amount <= lender_stake.deposited_amount,
        ErrorCode::InsufficientStake
    );

    if lender_stake.deposited_amount == 0 {
        return Err(ErrorCode::InsufficientStake.into());
    }

    let treasury_lamports = treasury_pda_info.lamports();
    let account_data_size = treasury_pda_info.data_len();
    let rent_exemption =
        anchor_lang::solana_program::rent::Rent::get()?.minimum_balance(account_data_size);
    let available_balance = treasury_lamports
        .checked_sub(rent_exemption)
        .ok_or(ErrorCode::CalculationOverflow)?;

    if available_balance < amount {
        return Err(ErrorCode::InsufficientLiquidBalance.into());
    }

    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    if lender_stake.deposited_amount == 0 {
        lender_stake.is_active = false;
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

    emit!(EmergencyUnstake {
        lender: lender_stake.backer,
        amount,
        remaining_staked: lender_stake.deposited_amount,
        unstaked_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
