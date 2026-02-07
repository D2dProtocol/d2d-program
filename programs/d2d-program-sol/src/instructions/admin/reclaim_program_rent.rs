use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::errors::ErrorCode;
use crate::events::{DebtRepaid, ProgramRentReclaimed};
use crate::states::{DeployRequest, DeployRequestStatus, ManagedProgram, TreasuryPool};

/// Admin/Cron calls this instruction to close expired programs and recover rent
/// This is used when a developer's subscription expires and they haven't renewed
/// 
/// Flow:
/// 1. Validate subscription is expired
/// 2. Close the program via BPF Loader (PDA signs)
/// 3. Transfer recovered lamports to treasury
/// 4. Mark managed program as inactive
#[derive(Accounts)]
pub struct ReclaimProgramRent<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// The program to be closed
    /// CHECK: Validated by managed_program
    #[account(mut)]
    pub program_account: UncheckedAccount<'info>,
    
    /// Program data account (will be closed)
    /// CHECK: Will be validated by BPF Loader during CPI
    #[account(mut)]
    pub program_data: UncheckedAccount<'info>,
    
    /// PDA that holds the upgrade authority
    /// CHECK: Validated by seeds and managed_program.authority_pda
    #[account(
        seeds = [ManagedProgram::AUTHORITY_SEED, program_account.key().as_ref()],
        bump
    )]
    pub authority_pda: SystemAccount<'info>,
    
    /// Managed program state
    #[account(
        mut,
        seeds = [ManagedProgram::PREFIX_SEED, program_account.key().as_ref()],
        bump = managed_program.bump,
        constraint = managed_program.is_active @ ErrorCode::ProgramNotManaged,
        constraint = managed_program.authority_pda == authority_pda.key() @ ErrorCode::InvalidAuthorityPda,
    )]
    pub managed_program: Account<'info, ManagedProgram>,
    
    /// Deploy request - check subscription expiration
    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump,
    )]
    pub deploy_request: Account<'info, DeployRequest>,
    
    /// Account to receive recovered lamports (treasury pool PDA)
    /// CHECK: Validated as treasury pool
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub close_recipient: UncheckedAccount<'info>,
    
    /// Admin who is reclaiming
    #[account(
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    /// BPF Loader Upgradeable Program
    /// CHECK: Known program ID
    #[account(
        constraint = bpf_loader_upgradeable_program.key() == bpf_loader_upgradeable::ID
    )]
    pub bpf_loader_upgradeable_program: UncheckedAccount<'info>,
}

pub fn reclaim_program_rent(ctx: Context<ReclaimProgramRent>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;
    let managed_program = &mut ctx.accounts.managed_program;
    let current_time = Clock::get()?.unix_timestamp;
    
    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    
    // Validate subscription is expired
    let is_expired = !deploy_request.is_subscription_valid()?;
    require!(is_expired, ErrorCode::SubscriptionStillActive);
    
    // Get current program data lamports before closing
    let program_data_lamports = ctx.accounts.program_data.lamports();
    
    // Build the Close instruction for BPF Loader Upgradeable
    let close_ix = bpf_loader_upgradeable::close_any(
        &ctx.accounts.program_data.key(),
        &ctx.accounts.close_recipient.key(),
        Some(&ctx.accounts.authority_pda.key()),
        Some(&ctx.accounts.program_account.key()),
    );
    
    // Prepare PDA signer seeds
    let program_key = ctx.accounts.program_account.key();
    let seeds = &[
        ManagedProgram::AUTHORITY_SEED,
        program_key.as_ref(),
        &[ctx.bumps.authority_pda],
    ];
    let signer_seeds = &[&seeds[..]];
    
    // Execute CPI with PDA signature
    anchor_lang::solana_program::program::invoke_signed(
        &close_ix,
        &[
            ctx.accounts.program_data.to_account_info(),
            ctx.accounts.close_recipient.to_account_info(),
            ctx.accounts.authority_pda.to_account_info(),
            ctx.accounts.program_account.to_account_info(),
        ],
        signer_seeds,
    )?;
    
    // Update states
    managed_program.is_active = false;
    deploy_request.status = DeployRequestStatus::Closed;

    // === DEBT REPAYMENT LOGIC ===
    // Record rent recovery in deploy_request (tracks per-deployment debt)
    let remaining_debt = deploy_request.get_remaining_debt();
    let (debt_repayment, excess_to_rewards) = deploy_request.record_rent_recovery(program_data_lamports)?;

    // Record debt repayment in treasury pool (tracks global debt)
    // This also restores liquid_balance for the debt_repayment portion
    treasury_pool.record_debt_repayment(program_data_lamports, remaining_debt)?;

    // If there's excess beyond debt repayment, credit it to reward pool for stakers
    if excess_to_rewards > 0 {
        treasury_pool.credit_fee_to_pool(excess_to_rewards, 0)?;
    }

    // Emit events
    emit!(ProgramRentReclaimed {
        program_id: ctx.accounts.program_account.key(),
        developer: managed_program.developer,
        lamports_recovered: program_data_lamports,
        reclaimed_at: current_time,
    });

    // Emit debt repayment event
    emit!(DebtRepaid {
        deploy_request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        borrowed_amount: deploy_request.borrowed_amount,
        repaid_amount: deploy_request.repaid_amount,
        remaining_debt: deploy_request.get_remaining_debt(),
        recovery_ratio_bps: deploy_request.recovery_ratio_bps,
        repaid_at: current_time,
    });

    Ok(())
}
