use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::AutoRenewSettingsChanged;
use crate::states::{DeveloperEscrow, TreasuryPool};

#[derive(Accounts)]
pub struct ToggleAutoRenew<'info> {
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        mut,
        seeds = [DeveloperEscrow::PREFIX_SEED, developer.key().as_ref()],
        bump = developer_escrow.bump,
        constraint = developer_escrow.developer == developer.key() @ ErrorCode::Unauthorized
    )]
    pub developer_escrow: Account<'info, DeveloperEscrow>,

    #[account(mut)]
    pub developer: Signer<'info>,
}

pub fn toggle_auto_renew(ctx: Context<ToggleAutoRenew>, enabled: bool) -> Result<()> {
    let treasury_pool = &ctx.accounts.treasury_pool;
    let developer_escrow = &mut ctx.accounts.developer_escrow;
    let developer = &ctx.accounts.developer;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    developer_escrow.auto_renew_enabled = enabled;

    emit!(AutoRenewSettingsChanged {
        developer: developer.key(),
        auto_renew_enabled: enabled,
        preferred_token: developer_escrow.preferred_token as u8,
        changed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
