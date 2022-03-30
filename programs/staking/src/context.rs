use crate::account::*;
use crate::error::ErrorCode;
use anchor_lang::solana_program::{clock, program_option::COption, sysvar};
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use std::convert::TryInto;

#[derive(Accounts)]
#[instruction(pool_nonce: u8)]
pub struct InitializePool<'info> {
    /// CHECK: nothing to check.
    pub authority: AccountInfo<'info>,

    pub staking_mint: Box<Account<'info, Mint>>,
    #[account(
        constraint = staking_vault.mint == staking_mint.key(),
        constraint = staking_vault.owner == pool_signer.key(),
        //strangely, spl maintains this on owner reassignment for non-native accounts
        //we don't want to be given an account that someone else could close when empty
        //because in our "pool close" operation we want to assert it is still open
        constraint = staking_vault.close_authority == COption::None,
    )]
    pub staking_vault: Box<Account<'info, TokenAccount>>,

    pub reward_mint: Box<Account<'info, Mint>>,
    #[account(
        constraint = reward_vault.mint == reward_mint.key(),
        constraint = reward_vault.owner == pool_signer.key(),
        constraint = reward_vault.close_authority == COption::None,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [
            pool.to_account_info().key.as_ref()
        ],
        bump = pool_nonce,
    )]
    /// CHECK: nothing to check.
    pub pool_signer: AccountInfo<'info>,

    #[account(
        zero,
    )]
    pub pool: Box<Account<'info, Pool>>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateUser<'info> {
    // Stake instance.
    #[account(
        mut,
        constraint = !pool.paused @ ErrorCode::PoolPaused,
    )]
    pub pool: Box<Account<'info, Pool>>,
    // Member.
    #[account(
        init,
        payer=owner,
        seeds = [
            owner.key.as_ref(), 
            pool.to_account_info().key.as_ref()
        ],
        bump
    )]
    pub user: Box<Account<'info, User>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    // Misc.
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut, 
        has_one = authority,
        constraint = !pool.paused @ ErrorCode::PoolPaused,
        constraint = pool.reward_duration_end < clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap(),
    )]
    pub pool: Box<Account<'info, Pool>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Unpause<'info> {
    #[account(
        mut, 
        has_one = authority,
        constraint = pool.paused,
    )]
    pub pool: Box<Account<'info, Pool>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    // Global accounts for the staking instance.
    #[account(
        mut, 
        has_one = staking_vault,
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(
        mut,
        constraint = staking_vault.owner == *pool_signer.key,
    )]
    pub staking_vault: Box<Account<'info, TokenAccount>>,

    // User.
    #[account(
        mut, 
        has_one = owner, 
        has_one = pool,
        seeds = [
            owner.key.as_ref(), 
            pool.to_account_info().key.as_ref()
        ],
        bump = user.nonce,
    )]
    pub user: Box<Account<'info, User>>,
    pub owner: Signer<'info>,
    #[account(mut)]
    pub stake_from_account: Box<Account<'info, TokenAccount>>,

    // Program signers.
    #[account(
        seeds = [
            pool.to_account_info().key.as_ref()
        ],
        bump = pool.nonce,
    )]
    /// CHECK: nothing to check.
    pub pool_signer: AccountInfo<'info>,

    // Misc.
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct FunderChange<'info> {
    // Global accounts for the staking instance.
    #[account(
        mut, 
        has_one = authority,
    )]
    pub pool: Box<Account<'info, Pool>>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Fund<'info> {
    // Global accounts for the staking instance.
    #[account(
        mut,
        has_one = reward_vault,
        constraint = !pool.paused @ ErrorCode::PoolPaused,
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(mut)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        //require signed funder auth - otherwise constant micro fund could hold funds hostage
        constraint = funder.key() == pool.authority || pool.funders.iter().any(|x| *x == funder.key()),
    )]
    pub funder: Signer<'info>,
    #[account(mut)]
    pub from: Box<Account<'info, TokenAccount>>,

    // Program signers.
    #[account(
        seeds = [
            pool.to_account_info().key.as_ref()
        ],
        bump = pool.nonce,
    )]
    /// CHECK: nothing to check.
    pub pool_signer: AccountInfo<'info>,

    // Misc.
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    // Global accounts for the staking instance.
    #[account(
        mut, 
        has_one = staking_vault,
        has_one = reward_vault,
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(mut)]
    pub staking_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    // User.
    #[account(
        mut,
        has_one = owner,
        has_one = pool,
        seeds = [
            owner.key.as_ref(), 
            pool.to_account_info().key.as_ref()
        ],
        bump = user.nonce,
    )]
    pub user: Box<Account<'info, User>>,
    pub owner: Signer<'info>,
    #[account(mut)]
    pub reward_account: Box<Account<'info, TokenAccount>>,

    // Program signers.
    #[account(
        seeds = [
            pool.to_account_info().key.as_ref()
        ],
        bump = pool.nonce,
    )]
    /// CHECK: nothing to check.
    pub pool_signer: AccountInfo<'info>,

    // Misc.
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseUser<'info> {
    #[account(mut)]
    pub pool: Box<Account<'info, Pool>>,
    #[account(
        mut,
        close = owner,
        has_one = owner,
        has_one = pool,
        constraint = user.balance_staked == 0,
        constraint = user.reward_per_token_pending == 0,
        seeds = [
            owner.key.as_ref(), 
            pool.to_account_info().key.as_ref()
        ],
        bump = user.nonce,
    )]
    pub user: Account<'info, User>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClosePool<'info> {
    #[account(mut)]
    /// CHECK: nothing to check.
    pub refundee: AccountInfo<'info>,
    #[account(mut)]
    pub staking_refundee: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub reward_refundee: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        close = refundee,
        has_one = authority,
        has_one = staking_vault,
        has_one = reward_vault,
        constraint = pool.paused,
        constraint = pool.reward_duration_end > 0,
        constraint = pool.reward_duration_end < sysvar::clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap(),
        constraint = pool.user_stake_count == 0,
        constraint = pool.total_staked == 0,
    )]
    pub pool: Account<'info, Pool>,
    pub authority: Signer<'info>,
    #[account(mut)]
    pub staking_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        seeds = [
            pool.to_account_info().key.as_ref()
        ],
        bump = pool.nonce,
    )]
    /// CHECK: nothing to check.
    pub pool_signer: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}