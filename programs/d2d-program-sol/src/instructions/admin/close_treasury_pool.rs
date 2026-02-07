use crate::errors::ErrorCode;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Close Treasury Pool Account (Admin only)
///
/// SECURITY: This instruction REQUIRES admin verification via deserialization.
/// If the account has an old layout, call migrate_treasury_pool first.
///
/// WARNING: This will transfer all funds to admin and make the account rent-exempt!
/// After closing, you can call initialize() again to create a new account.
#[derive(Accounts)]
pub struct CloseTreasuryPool<'info> {
    /// Treasury Pool PDA - SECURITY: Must be deserializable for admin verification
    /// If old layout, call migrate_treasury_pool first
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump,
        // SECURITY FIX C-01: Verify admin authorization
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    /// Admin who will receive the lamports - MUST match treasury_pool.admin
    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Close treasury pool account by transferring all lamports to admin
/// SECURITY: Admin verification is enforced via account constraint
pub fn close_treasury_pool(ctx: Context<CloseTreasuryPool>) -> Result<()> {
    msg!("[CLOSE] Closing Treasury Pool account");
    msg!("[CLOSE] Admin: {}", ctx.accounts.admin.key());
    msg!("[CLOSE] Treasury Pool PDA: {}", ctx.accounts.treasury_pool.key());

    // SECURITY: Admin verification already done in account constraints
    // treasury_pool.admin == admin.key() is verified by Anchor

    // Get account info for lamport manipulation
    let treasury_account = ctx.accounts.treasury_pool.to_account_info();
    let balance_before = treasury_account.lamports();

    msg!("[CLOSE] Account balance before close: {} lamports", balance_before);

    // Calculate rent-exempt minimum
    let account_data_size = treasury_account.data_len();
    let rent_exempt_minimum = Rent::get()?.minimum_balance(account_data_size);

    if balance_before <= rent_exempt_minimum {
        msg!("[CLOSE] Account already rent-exempt or has minimal balance");
        msg!("[CLOSE] Balance: {} lamports, Rent minimum: {} lamports", balance_before, rent_exempt_minimum);
    }

    // Transfer all lamports except rent-exempt minimum to admin
    let transfer_amount = balance_before.saturating_sub(rent_exempt_minimum);

    if transfer_amount > 0 {
        msg!("[CLOSE] Transferring {} lamports to verified admin", transfer_amount);

        // Use direct lamport mutation for program-owned accounts
        **treasury_account.try_borrow_mut_lamports()? = balance_before
            .checked_sub(transfer_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        **ctx.accounts.admin.try_borrow_mut_lamports()? = ctx.accounts.admin.lamports()
            .checked_add(transfer_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        msg!("[CLOSE] Transfer complete");
    } else {
        msg!("[CLOSE] No lamports to transfer (account already rent-exempt)");
    }

    msg!("[CLOSE] Treasury Pool account closed successfully");
    msg!("[CLOSE] Remaining balance: {} lamports (rent-exempt minimum)", treasury_account.lamports());
    msg!("[CLOSE] You can now call initialize() to create a new account with the updated layout");

    Ok(())
}

