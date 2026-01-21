use crate::errors::ErrorCode;
use crate::events::SolStaked;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::rent::Rent;
use anchor_lang::system_program;

#[derive(Accounts)]
pub struct StakeSol<'info> {
    /// CHECK: Treasury Pool - will be migrated if needed
    #[account(mut)]
    pub treasury_pool: UncheckedAccount<'info>,

    /// CHECK: Treasury Pool PDA
    #[account(mut)]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = lender,
        space = 8 + BackerDeposit::INIT_SPACE,
        seeds = [BackerDeposit::PREFIX_SEED, lender.key().as_ref()],
        bump
    )]
    pub lender_stake: Account<'info, BackerDeposit>,

    #[account(mut)]
    pub lender: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn stake_sol(ctx: Context<StakeSol>, deposit_amount: u64, _lock_period: i64) -> Result<()> {
    let (expected_treasury_pool, _bump) =
        Pubkey::find_program_address(&[TreasuryPool::PREFIX_SEED], ctx.program_id);
    require!(
        ctx.accounts.treasury_pool.key() == expected_treasury_pool,
        ErrorCode::InvalidAccountOwner
    );
    require!(
        ctx.accounts.treasury_pda.key() == expected_treasury_pool,
        ErrorCode::InvalidAccountOwner
    );

    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();

    if current_space < required_space {
        treasury_pool_info.realloc(required_space, false)?;
    }

    let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
        .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(deposit_amount > 0, ErrorCode::InvalidAmount);

    let lender_lamports = ctx.accounts.lender.lamports();
    let is_new_account = lender_stake.backer == Pubkey::default();

    let rent_exemption_needed = if is_new_account {
        Rent::get()?.minimum_balance(8 + BackerDeposit::INIT_SPACE)
    } else {
        0
    };

    const TRANSACTION_FEE_ESTIMATE: u64 = 10_000;

    let total_required = deposit_amount
        .checked_add(rent_exemption_needed)
        .and_then(|x| x.checked_add(TRANSACTION_FEE_ESTIMATE))
        .ok_or(ErrorCode::CalculationOverflow)?;

    require!(
        lender_lamports >= total_required,
        ErrorCode::InsufficientDeposit
    );

    let is_new_deposit = lender_stake.backer == Pubkey::default();

    if is_new_deposit {
        lender_stake.backer = ctx.accounts.lender.key();
        lender_stake.deposited_amount = 0;
        lender_stake.reward_debt = 0;
        lender_stake.pending_rewards = 0;
        lender_stake.claimed_total = 0;
        lender_stake.is_active = true;
        lender_stake.bump = ctx.bumps.lender_stake;
    } else {
        if !lender_stake.is_active {
            lender_stake.is_active = true;
        }
        lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;
    }

    let total_deposited_before = treasury_pool.total_deposited;
    if total_deposited_before == 0 && treasury_pool.reward_pool_balance > 0 {
        let excess_rewards = treasury_pool.reward_pool_balance;
        let new_total_deposited = deposit_amount;

        let excess_reward_per_share = (excess_rewards as u128)
            .checked_mul(TreasuryPool::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(new_total_deposited as u128)
            .ok_or(ErrorCode::CalculationOverflow)?;

        treasury_pool.reward_per_share = treasury_pool
            .reward_per_share
            .checked_add(excess_reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }

    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    treasury_pool.total_deposited = treasury_pool
        .total_deposited
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    treasury_pool.liquid_balance = treasury_pool
        .liquid_balance
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    let deposit_cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.lender.to_account_info(),
            to: ctx.accounts.treasury_pda.to_account_info(),
        },
    );
    system_program::transfer(deposit_cpi, deposit_amount)?;

    lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;

    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    treasury_pool.try_serialize(&mut &mut data[..])?;

    emit!(SolStaked {
        lender: lender_stake.backer,
        amount: deposit_amount,
        total_staked: lender_stake.deposited_amount,
        lock_period: 0,
    });

    emit!(crate::events::DepositMade {
        backer: lender_stake.backer,
        deposit_amount,
        net_deposit: deposit_amount,
        reward_fee: 0,
        platform_fee: 0,
        total_deposited: treasury_pool.total_deposited,
        liquid_balance: treasury_pool.liquid_balance,
        deposited_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
