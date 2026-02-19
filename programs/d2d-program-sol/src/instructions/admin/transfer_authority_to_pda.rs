use anchor_lang::{prelude::*, solana_program::bpf_loader_upgradeable};

use crate::{
  errors::ErrorCode,
  events::AuthorityTransferred,
  states::{DeployRequest, DeployRequestStatus, ManagedProgram, TreasuryPool},
};

/// Transfer program upgrade authority from temporary wallet to D2D PDA
/// This instruction is called by backend after successful deployment
///
/// Flow:
/// 1. Backend deploys program using temporary wallet (authority = temp wallet)
/// 2. Backend calls this instruction to transfer authority to PDA
/// 3. PDA becomes the permanent authority, enabling trustless upgrades
#[derive(Accounts)]
pub struct TransferAuthorityToPda<'info> {
  #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  /// The program that was just deployed
  /// CHECK: Validated by programdata account owner
  #[account(mut)]
  pub program_account: UncheckedAccount<'info>,

  /// Program data account (contains upgrade authority)
  /// CHECK: Validated by checking owner is BPF Loader Upgradeable
  #[account(mut)]
  pub program_data: UncheckedAccount<'info>,

  /// The new authority PDA that will hold upgrade rights
  /// CHECK: Derived from seeds
  #[account(
        seeds = [ManagedProgram::AUTHORITY_SEED, program_account.key().as_ref()],
        bump
    )]
  pub new_authority_pda: SystemAccount<'info>,

  /// Current authority (temporary wallet used for deployment)
  /// Must sign to authorize the transfer
  #[account(mut)]
  pub current_authority: Signer<'info>,

  /// Deploy request for this program
  #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump,
        constraint = deploy_request.status == DeployRequestStatus::Active @ ErrorCode::InvalidDeploymentStatus,
    )]
  pub deploy_request: Account<'info, DeployRequest>,

  /// New managed program account to track this program
  #[account(
        init,
        payer = admin,
        space = 8 + ManagedProgram::INIT_SPACE,
        seeds = [ManagedProgram::PREFIX_SEED, program_account.key().as_ref()],
        bump
    )]
  pub managed_program: Account<'info, ManagedProgram>,

  /// Admin who initiated the deployment
  #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
  pub admin: Signer<'info>,

  /// BPF Loader Upgradeable Program
  /// CHECK: Known program ID
  #[account(
        constraint = bpf_loader_upgradeable_program.key() == bpf_loader_upgradeable::ID
    )]
  pub bpf_loader_upgradeable_program: UncheckedAccount<'info>,

  pub system_program: Program<'info, System>,
}

pub fn transfer_authority_to_pda(ctx: Context<TransferAuthorityToPda>) -> Result<()> {
  let treasury_pool = &ctx.accounts.treasury_pool;
  let deploy_request = &ctx.accounts.deploy_request;
  let managed_program = &mut ctx.accounts.managed_program;
  let current_time = Clock::get()?.unix_timestamp;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

  // Initialize managed program state
  managed_program.program_id = ctx.accounts.program_account.key();
  managed_program.developer = deploy_request.developer;
  managed_program.deploy_request = ctx.accounts.deploy_request.key();
  managed_program.authority_pda = ctx.accounts.new_authority_pda.key();
  managed_program.created_at = current_time;
  managed_program.last_upgraded_at = current_time;
  managed_program.upgrade_count = 0;
  managed_program.is_active = true;
  managed_program.bump = ctx.bumps.managed_program;

  // Build the SetAuthority instruction for BPF Loader Upgradeable
  let set_authority_ix = bpf_loader_upgradeable::set_upgrade_authority(
    &ctx.accounts.program_account.key(),
    &ctx.accounts.current_authority.key(),
    Some(&ctx.accounts.new_authority_pda.key()),
  );

  // Execute CPI to transfer authority
  // Current authority signs this transaction directly (not via PDA)
  anchor_lang::solana_program::program::invoke(
    &set_authority_ix,
    &[
      ctx.accounts.program_data.to_account_info(),
      ctx.accounts.current_authority.to_account_info(),
      ctx.accounts.new_authority_pda.to_account_info(),
    ],
  )?;

  emit!(AuthorityTransferred {
    program_id: ctx.accounts.program_account.key(),
    old_authority: ctx.accounts.current_authority.key(),
    new_authority_pda: ctx.accounts.new_authority_pda.key(),
    transferred_at: current_time,
  });

  Ok(())
}
