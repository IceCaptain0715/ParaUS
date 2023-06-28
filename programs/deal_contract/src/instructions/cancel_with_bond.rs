use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, TokenAccount, Transfer};

use crate::{constants::*, errors::ErrorCode, state::DealState};

#[derive(Accounts)]
#[instruction(id: Vec<u8>)]
pub struct CancelWithBond<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut, signer)]
    pub initializer: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        constraint = *authority.to_account_info().key == deal_state.authority_key
    )]
    pub authority: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        mut,
        constraint = *deposit_account.to_account_info().key == deal_state.deposit_key
    )]
    pub deposit_account: Account<'info, TokenAccount>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        mut,
        constraint = *client_token_account.to_account_info().key == deal_state.client_token_account_key
    )]
    pub client_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = *client_bond_account.to_account_info().key == deal_state.client_bond_token_account_key
    )]
    pub client_bond_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = *executor_bond_account.to_account_info().key == deal_state.executor_bond_token_account_key
    )]
    pub executor_bond_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = *deposit_client_bond_account.to_account_info().key == deal_state.client_bond_deposit_key
    )]
    pub deposit_client_bond_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = *deposit_executor_bond_account.to_account_info().key == deal_state.executor_bond_deposit_key
    )]
    pub deposit_executor_bond_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [&id, b"state".as_ref(), deal_state.client_key.as_ref(), deal_state.executor_key.as_ref()],
        bump = deal_state.bump,
        constraint = (*initializer.to_account_info().key == deal_state.client_key || *initializer.to_account_info().key == deal_state.executor_key || *initializer.to_account_info().key == deal_state.checker_key 
      || *initializer.to_account_info().key == SERVICE_ACCOUNT_ADDRESS),
        close = initializer
    )]
    pub deal_state: Box<Account<'info, DealState>>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: AccountInfo<'info>,
}

// Cancel With Bond
impl<'info> CancelWithBond<'info> {
    fn into_transfer_to_client_token_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.deposit_account.to_account_info(),
            to: self.client_token_account.to_account_info(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_transfer_to_bond_client_token_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.deposit_client_bond_account.to_account_info(),
            to: self.client_bond_account.to_account_info(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_transfer_to_bond_executor_token_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.deposit_executor_bond_account.to_account_info(),
            to: self.executor_bond_account.to_account_info(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.deposit_account.to_account_info(),
            destination: self.initializer.to_account_info(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

pub fn handle(ctx: Context<CancelWithBond>, id: Vec<u8>) -> Result<()> {
    if !ctx.accounts.deal_state.is_started {
        return Err(ErrorCode::NotStarted.into());
    }

    if !ctx.accounts.deal_state.with_bond {
        return Err(ErrorCode::NeedCancelWithoutBond.into());
    }

    if ctx.accounts.deal_state.deadline_ts > 0 {
        let clock = Clock::get()?;
        let current_ts = clock.unix_timestamp;
        if ctx.accounts.deal_state.deadline_ts > current_ts {
            return Err(ErrorCode::DeadlineNotCome.into());
        }
    }

    let seeds = &[
        &id,
        &AUTHORITY_SEED[..],
        ctx.accounts.deal_state.client_key.as_ref(),
        ctx.accounts.deal_state.executor_key.as_ref(),
        &[ctx.accounts.deal_state.authority_bump],
    ];

    let amount = ctx.accounts.deal_state.amount + ctx.accounts.deal_state.checker_fee;

    token::transfer(
        ctx.accounts
            .into_transfer_to_client_token_account_context()
            .with_signer(&[&seeds[..]]),
        amount,
    )?;

    if ctx.accounts.deal_state.client_bond_amount > 0 {
        token::transfer(
            ctx.accounts
                .into_transfer_to_bond_client_token_account_context()
                .with_signer(&[&seeds[..]]),
            ctx.accounts.deal_state.client_bond_amount,
        )?;
    }

    if ctx.accounts.deal_state.executor_bond_amount > 0 {
        token::transfer(
            ctx.accounts
                .into_transfer_to_bond_executor_token_account_context()
                .with_signer(&[&seeds[..]]),
            ctx.accounts.deal_state.executor_bond_amount,
        )?;
    }

    token::close_account(ctx.accounts.into_close_context().with_signer(&[&seeds[..]]))?;

    Ok(())
}
