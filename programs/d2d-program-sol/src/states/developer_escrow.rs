use anchor_lang::prelude::*;

/// Token type for escrow deposits and payments
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum TokenType {
  SOL,
  USDC,
  USDT,
}

impl Default for TokenType {
  fn default() -> Self {
    TokenType::SOL
  }
}

/// Developer Escrow Account
/// Stores pre-funded balance for automatic subscription renewals
#[account]
#[derive(InitSpace)]
pub struct DeveloperEscrow {
  /// Developer wallet public key (owner)
  pub developer: Pubkey,

  /// SOL balance in lamports
  pub sol_balance: u64,

  /// USDC balance (6 decimals, in smallest unit)
  pub usdc_balance: u64,

  /// USDT balance (6 decimals, in smallest unit)
  pub usdt_balance: u64,

  /// Whether auto-renewal is enabled globally for this developer
  pub auto_renew_enabled: bool,

  /// Preferred token type for auto-renewal payments
  pub preferred_token: TokenType,

  /// Minimum balance alert threshold (in lamports for SOL, or smallest unit for SPL)
  pub min_balance_alert: u64,

  /// Lifetime total SOL deposited
  pub total_deposited_sol: u64,

  /// Lifetime total USDC deposited
  pub total_deposited_usdc: u64,

  /// Lifetime total USDT deposited
  pub total_deposited_usdt: u64,

  /// Lifetime total auto-deducted (in SOL equivalent lamports)
  pub total_auto_deducted: u64,

  /// Account creation timestamp
  pub created_at: i64,

  /// Last deposit timestamp
  pub last_deposit_at: i64,

  /// Last auto-deduction timestamp
  pub last_auto_deduct_at: i64,

  /// PDA bump seed
  pub bump: u8,
}

impl DeveloperEscrow {
  pub const PREFIX_SEED: &'static [u8] = b"developer_escrow";

  /// Check if escrow can cover an auto-deduction for the given amount and token type
  pub fn can_auto_deduct(&self, amount: u64, token_type: TokenType) -> bool {
    if !self.auto_renew_enabled {
      return false;
    }

    match token_type {
      TokenType::SOL => self.sol_balance >= amount,
      TokenType::USDC => self.usdc_balance >= amount,
      TokenType::USDT => self.usdt_balance >= amount,
    }
  }

  /// Get balance for a specific token type
  pub fn get_balance(&self, token_type: TokenType) -> u64 {
    match token_type {
      TokenType::SOL => self.sol_balance,
      TokenType::USDC => self.usdc_balance,
      TokenType::USDT => self.usdt_balance,
    }
  }

  /// Deduct from balance (returns error if insufficient)
  pub fn deduct_balance(&mut self, amount: u64, token_type: TokenType) -> Result<()> {
    match token_type {
      TokenType::SOL => {
        require!(
          self.sol_balance >= amount,
          ErrorCode::InsufficientEscrowBalance
        );
        self.sol_balance = self
          .sol_balance
          .checked_sub(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
      TokenType::USDC => {
        require!(
          self.usdc_balance >= amount,
          ErrorCode::InsufficientEscrowBalance
        );
        self.usdc_balance = self
          .usdc_balance
          .checked_sub(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
      TokenType::USDT => {
        require!(
          self.usdt_balance >= amount,
          ErrorCode::InsufficientEscrowBalance
        );
        self.usdt_balance = self
          .usdt_balance
          .checked_sub(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
    }

    self.total_auto_deducted = self
      .total_auto_deducted
      .checked_add(amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
    self.last_auto_deduct_at = Clock::get()?.unix_timestamp;

    Ok(())
  }

  /// Add to balance
  pub fn add_balance(&mut self, amount: u64, token_type: TokenType) -> Result<()> {
    match token_type {
      TokenType::SOL => {
        self.sol_balance = self
          .sol_balance
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
        self.total_deposited_sol = self
          .total_deposited_sol
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
      TokenType::USDC => {
        self.usdc_balance = self
          .usdc_balance
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
        self.total_deposited_usdc = self
          .total_deposited_usdc
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
      TokenType::USDT => {
        self.usdt_balance = self
          .usdt_balance
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
        self.total_deposited_usdt = self
          .total_deposited_usdt
          .checked_add(amount)
          .ok_or(ErrorCode::CalculationOverflow)?;
      }
    }

    self.last_deposit_at = Clock::get()?.unix_timestamp;

    Ok(())
  }

  /// Check if balance is below alert threshold
  pub fn is_below_alert_threshold(&self) -> bool {
    match self.preferred_token {
      TokenType::SOL => self.sol_balance < self.min_balance_alert,
      TokenType::USDC => self.usdc_balance < self.min_balance_alert,
      TokenType::USDT => self.usdt_balance < self.min_balance_alert,
    }
  }
}

use crate::errors::ErrorCode;
