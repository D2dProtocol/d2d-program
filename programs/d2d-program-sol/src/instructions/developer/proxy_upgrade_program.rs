use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::errors::ErrorCode;
use crate::events::ProgramUpgraded;
use crate::states::{DeployRequest, DeployRequestStatus, ManagedProgram, TreasuryPool};

/// Developer calls this instruction to upgrade their program
/// D2D PDA will sign on their behalf via CPI (invoke_signed)
///
/// Requirements:
/// 1. Developer must be the owner of the managed program
/// 2. Subscription must be active (not expired)
/// 3. Buffer must be pre-uploaded by developer
#[derive(Accounts)]
pub struct ProxyUpgradeProgram<'info> {
    /// The program to be upgraded
    /// CHECK: Validated by program_data and managed_program
    #[account(mut)]
    pub program_account: UncheckedAccount<'info>,

    /// Program data account (will be updated with new bytecode)
    /// CHECK: Will be validated by BPF Loader during CPI
    #[account(mut)]
    pub program_data: UncheckedAccount<'info>,

    /// Buffer containing the new program bytecode
    /// Developer must upload this before calling proxy_upgrade
    /// CHECK: Will be validated by BPF Loader during CPI
    #[account(mut)]
    pub buffer_account: UncheckedAccount<'info>,

    /// PDA that holds the upgrade authority
    /// CHECK: Validated by seeds and managed_program.authority_pda
    #[account(
        seeds = [ManagedProgram::AUTHORITY_SEED, program_account.key().as_ref()],
        bump
    )]
    pub authority_pda: SystemAccount<'info>,

    /// Managed program state - validates developer ownership
    #[account(
        mut,
        seeds = [ManagedProgram::PREFIX_SEED, program_account.key().as_ref()],
        bump = managed_program.bump,
        constraint = managed_program.is_active @ ErrorCode::ProgramNotManaged,
        constraint = managed_program.developer == developer.key() @ ErrorCode::Unauthorized,
        constraint = managed_program.authority_pda == authority_pda.key() @ ErrorCode::InvalidAuthorityPda,
    )]
    pub managed_program: Account<'info, ManagedProgram>,

    /// CHECK: Deploy request - validated manually for migration compatibility
    pub deploy_request: UncheckedAccount<'info>,

    /// Developer who owns the program (must sign)
    pub developer: Signer<'info>,

    /// Account to receive any excess lamports from buffer
    /// CHECK: Can be any account, typically the developer
    #[account(mut)]
    pub spill_account: UncheckedAccount<'info>,

    /// BPF Loader Upgradeable Program
    /// CHECK: Known program ID
    #[account(
        constraint = bpf_loader_upgradeable_program.key() == bpf_loader_upgradeable::ID
    )]
    pub bpf_loader_upgradeable_program: UncheckedAccount<'info>,

    /// SECURITY FIX L-02: Add treasury_pool to check emergency_pause
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn proxy_upgrade_program(ctx: Context<ProxyUpgradeProgram>) -> Result<()> {
    let managed_program = &mut ctx.accounts.managed_program;
    let treasury_pool = &ctx.accounts.treasury_pool;
    let current_time = Clock::get()?.unix_timestamp;

    // SECURITY FIX L-02: Check emergency pause
    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    // Manually deserialize deploy_request with migration support
    let deploy_request_info = ctx.accounts.deploy_request.to_account_info();

    // Verify account is owned by this program
    require!(
        deploy_request_info.owner == &crate::ID,
        ErrorCode::InvalidAccountOwner
    );

    // Read account data, pad with zeros if old schema (migration compatibility)
    // Old accounts may be smaller than the new struct; padding with zeros
    // gives correct defaults (0) for new debt tracking fields
    let required_space = 8 + DeployRequest::INIT_SPACE;
    let account_data = deploy_request_info.data.borrow();
    let data_to_deserialize = if account_data.len() < required_space {
        let mut padded = vec![0u8; required_space];
        padded[..account_data.len()].copy_from_slice(&account_data);
        padded
    } else {
        account_data[..required_space].to_vec()
    };
    drop(account_data);

    let deploy_request = DeployRequest::try_deserialize(&mut &data_to_deserialize[..])
        .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

    // Validate PDA seeds
    let (expected_pda, _) = Pubkey::find_program_address(
        &[DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        &crate::ID,
    );
    require!(
        expected_pda == deploy_request_info.key(),
        ErrorCode::InvalidRequestId
    );

    // Validate deploy request constraints
    require!(
        deploy_request.developer == ctx.accounts.developer.key(),
        ErrorCode::Unauthorized
    );
    require!(
        deploy_request.status == DeployRequestStatus::Active,
        ErrorCode::InvalidDeploymentStatus
    );

    // 1. Validate subscription is still active
    require!(
        deploy_request.is_subscription_valid()?,
        ErrorCode::SubscriptionExpired
    );

    // 2. Step 1: Transfer buffer authority to the PDA
    let set_buffer_authority_ix = bpf_loader_upgradeable::set_buffer_authority(
        &ctx.accounts.buffer_account.key(),
        &ctx.accounts.developer.key(),
        &ctx.accounts.authority_pda.key(),
    );

    anchor_lang::solana_program::program::invoke(
        &set_buffer_authority_ix,
        &[
            ctx.accounts.buffer_account.to_account_info(),
            ctx.accounts.developer.to_account_info(),
            ctx.accounts.authority_pda.to_account_info(),
        ],
    )?;

    // 3. Step 2: Build the Upgrade instruction for BPF Loader Upgradeable
    let upgrade_ix = bpf_loader_upgradeable::upgrade(
        &ctx.accounts.program_account.key(),
        &ctx.accounts.buffer_account.key(),
        &ctx.accounts.authority_pda.key(),
        &ctx.accounts.spill_account.key(),
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
        &upgrade_ix,
        &[
            ctx.accounts.program_data.to_account_info(),
            ctx.accounts.program_account.to_account_info(),
            ctx.accounts.buffer_account.to_account_info(),
            ctx.accounts.spill_account.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.clock.to_account_info(),
            ctx.accounts.authority_pda.to_account_info(),
        ],
        signer_seeds,
    )?;

    // Update managed program state
    managed_program.last_upgraded_at = current_time;
    managed_program.upgrade_count = managed_program.upgrade_count.saturating_add(1);

    emit!(ProgramUpgraded {
        program_id: ctx.accounts.program_account.key(),
        developer: ctx.accounts.developer.key(),
        buffer_address: ctx.accounts.buffer_account.key(),
        upgraded_at: current_time,
    });

    Ok(())
}
