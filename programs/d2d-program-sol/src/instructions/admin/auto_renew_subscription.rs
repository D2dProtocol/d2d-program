use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::{AutoRenewalExecuted, AutoRenewalFailed},
  states::{DeployRequest, DeployRequestStatus, DeveloperEscrow, TokenType, TreasuryPool},
};

#[derive(Accounts)]
#[instruction(request_id: [u8; 32])]
pub struct AutoRenewSubscription<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump,
        constraint = deploy_request.request_id == request_id @ ErrorCode::InvalidRequestId
    )]
  pub deploy_request: Account<'info, DeployRequest>,

  #[account(
        mut,
        seeds = [DeveloperEscrow::PREFIX_SEED, deploy_request.developer.as_ref()],
        bump = developer_escrow.bump,
        constraint = developer_escrow.developer == deploy_request.developer @ ErrorCode::Unauthorized
    )]
  pub developer_escrow: Account<'info, DeveloperEscrow>,

  /// CHECK: Dev wallet address - validated against treasury_pool
  #[account(
        mut,
        constraint = dev_wallet.key() == treasury_pool.dev_wallet @ ErrorCode::InvalidTreasuryWallet
    )]
  pub dev_wallet: UncheckedAccount<'info>,

  #[account(
        constraint = treasury_pool.is_admin(&admin.key()) @ ErrorCode::Unauthorized
    )]
  pub admin: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn auto_renew_subscription(
  ctx: Context<AutoRenewSubscription>,
  request_id: [u8; 32],
  months: u32,
) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let deploy_request = &mut ctx.accounts.deploy_request;
  let developer_escrow = &mut ctx.accounts.developer_escrow;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(months > 0, ErrorCode::InvalidAmount);

  // Verify subscription is active or expired (not in grace period or closed)
  require!(
    deploy_request.status == DeployRequestStatus::Active
      || deploy_request.status == DeployRequestStatus::SubscriptionExpired
      || deploy_request.status == DeployRequestStatus::InGracePeriod,
    ErrorCode::InvalidRequestStatus
  );

  // Check if auto-renewal is enabled on both escrow and deploy request
  require!(
    developer_escrow.auto_renew_enabled && deploy_request.auto_renewal_enabled,
    ErrorCode::AutoRenewalDisabled
  );

  // Calculate payment amount
  let payment_amount = deploy_request.monthly_fee * months as u64;

  // Get preferred token type from escrow
  let token_type = developer_escrow.preferred_token;

  // Check if escrow has sufficient balance
  if !developer_escrow.can_auto_deduct(payment_amount, token_type) {
    // Auto-renewal failed due to insufficient funds
    deploy_request.increment_auto_renewal_failed();

    emit!(AutoRenewalFailed {
      request_id,
      developer: deploy_request.developer,
      reason: "Insufficient escrow balance".to_string(),
      escrow_balance: developer_escrow.get_balance(token_type),
      required_amount: payment_amount,
      failed_at: Clock::get()?.unix_timestamp,
    });

    return Err(ErrorCode::InsufficientEscrowBalance.into());
  }

  // Deduct from escrow
  developer_escrow.deduct_balance(payment_amount, token_type)?;

  // For SOL payments, transfer from escrow PDA to dev_wallet
  if token_type == TokenType::SOL {
    let escrow_account_info = developer_escrow.to_account_info();
    let dev_wallet_account_info = ctx.accounts.dev_wallet.to_account_info();

    **escrow_account_info.try_borrow_mut_lamports()? = escrow_account_info
      .lamports()
      .checked_sub(payment_amount)
      .ok_or(ErrorCode::CalculationOverflow)?;

    **dev_wallet_account_info.try_borrow_mut_lamports()? = dev_wallet_account_info
      .lamports()
      .checked_add(payment_amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
  }
  // Note: SPL token transfers would require additional accounts and logic
  // For USDC/USDT, the transfer would use token program CPI

  // Extend subscription (with overflow protection)
  deploy_request.extend_subscription(months)?;

  // Update status to active
  deploy_request.status = DeployRequestStatus::Active;

  // Credit payment to treasury reward pool
  treasury_pool.credit_reward_pool(payment_amount as u128)?;

  let current_time = Clock::get()?.unix_timestamp;

  emit!(AutoRenewalExecuted {
    request_id,
    developer: deploy_request.developer,
    token_type: token_type as u8,
    amount_deducted: payment_amount,
    months_renewed: months,
    new_expiry: deploy_request.subscription_paid_until,
    escrow_remaining: developer_escrow.get_balance(token_type),
    renewed_at: current_time,
  });

  Ok(())
}
