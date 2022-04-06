pub mod account;
pub mod constants;
pub mod context;
pub mod error;
pub mod utils;

use account::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;
use anchor_spl::token::{self};
use context::*;
use error::ErrorCode;
use std::convert::Into;
use std::convert::TryFrom;
use std::convert::TryInto;
use utils::*;

declare_id!("5dAQP2JtgJ3vFKMi3McnXkut51PXfHuyXRJhFCofd13J");

pub const PRECISION: u128 = u64::MAX as u128;
pub const MIN_DURATION: u64 = 86400;

pub fn update_rewards(
    pool: &mut Account<Pool>,
    user: Option<&mut Box<Account<User>>>,
    total_staked: u64,
) -> Result<()> {
    let clock = clock::Clock::get().unwrap();
    let last_time_reward_applicable =
        last_time_reward_applicable(pool.reward_duration_end, clock.unix_timestamp);

    pool.reward_per_token_stored = reward_per_token(
        total_staked,
        pool.reward_per_token_stored,
        last_time_reward_applicable,
        pool.last_update_time,
        pool.reward_rate,
    );

    pool.last_update_time = last_time_reward_applicable;

    if let Some(u) = user {
        u.reward_per_token_pending = earned(
            u.balance_staked,
            pool.reward_per_token_stored,
            u.reward_per_token_complete,
            u.reward_per_token_pending,
        );
        u.reward_per_token_complete = pool.reward_per_token_stored;
    }
    Ok(())
}

pub fn last_time_reward_applicable(reward_duration_end: u64, unix_timestamp: i64) -> u64 {
    return std::cmp::min(unix_timestamp.try_into().unwrap(), reward_duration_end);
}

pub fn reward_per_token(
    total_staked: u64,
    reward_per_token_stored: u128,
    last_time_reward_applicable: u64,
    last_update_time: u64,
    reward_rate: u64,
) -> u128 {
    if total_staked == 0 {
        return reward_per_token_stored;
    }

    return reward_per_token_stored
        .checked_add(
            (last_time_reward_applicable as u128)
                .checked_sub(last_update_time as u128)
                .unwrap()
                .checked_mul(reward_rate as u128)
                .unwrap()
                .checked_mul(PRECISION)
                .unwrap()
                .checked_div(total_staked as u128)
                .unwrap(),
        )
        .unwrap();
}

pub fn earned(
    balance_staked: u64,
    reward_per_token: u128,
    user_reward_per_token_paid: u128,
    user_reward_pending: u64,
) -> u64 {
    return (balance_staked as u128)
        .checked_mul(
            (reward_per_token as u128)
                .checked_sub(user_reward_per_token_paid as u128)
                .unwrap(),
        )
        .unwrap()
        .checked_div(PRECISION)
        .unwrap()
        .checked_add(user_reward_pending as u128)
        .unwrap()
        .try_into()
        .unwrap();
}

#[program]
pub mod staking {
    use super::*;

    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        pool_nonce: u8,
        reward_duration: u64,
        lock_period: u64,
        no_tier: bool,
    ) -> Result<()> {
        if reward_duration < MIN_DURATION {
            return Err(ErrorCode::DurationTooShort.into());
        }

        let pool = &mut ctx.accounts.pool;

        pool.authority = ctx.accounts.authority.key();
        pool.nonce = pool_nonce;
        pool.paused = false;
        pool.staking_mint = ctx.accounts.staking_mint.key();
        pool.staking_vault = ctx.accounts.staking_vault.key();
        pool.reward_mint = ctx.accounts.reward_mint.key();
        pool.reward_vault = ctx.accounts.reward_vault.key();
        pool.reward_duration = reward_duration;
        pool.reward_duration_end = 0;
        pool.lock_period = lock_period;
        pool.last_update_time = 0;
        pool.reward_rate = 0;
        pool.reward_per_token_stored = 0;
        pool.user_stake_count = 0;
        pool.total_staked = 0;
        pool.no_tier = no_tier;

        Ok(())
    }

    pub fn create_user(ctx: Context<CreateUser>) -> Result<()> {
        let user = &mut ctx.accounts.user;
        user.pool = *ctx.accounts.pool.to_account_info().key;
        user.owner = *ctx.accounts.owner.key;
        user.reward_per_token_complete = 0;
        user.reward_per_token_pending = 0;
        user.balance_staked = 0;
        user.maturity_time = 0;
        user.tier = 0;
        user.nonce = *ctx.bumps.get("user").unwrap();

        let pool = &mut ctx.accounts.pool;
        pool.user_stake_count = pool.user_stake_count.checked_add(1).unwrap();

        Ok(())
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.paused = true;

        Ok(())
    }

    pub fn unpause(ctx: Context<Unpause>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.paused = false;
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        if amount == 0 {
            return Err(ErrorCode::AmountMustBeGreaterThanZero.into());
        }

        let pool = &mut ctx.accounts.pool;
        if pool.paused {
            return Err(ErrorCode::PoolPaused.into());
        }

        let total_staked = pool.total_staked;

        let user_opt = Some(&mut ctx.accounts.user);
        update_rewards(pool, user_opt, total_staked).unwrap();
        let clock = clock::Clock::get().unwrap();
        ctx.accounts.user.balance_staked = ctx
            .accounts
            .user
            .balance_staked
            .checked_add(amount)
            .unwrap();
        ctx.accounts.user.maturity_time = u64::try_from(clock.unix_timestamp)
            .unwrap()
            .checked_add(pool.lock_period)
            .unwrap();

        if pool.no_tier == false {
            ctx.accounts.user.tier = get_tier(ctx.accounts.user.balance_staked);
        }

        // Transfer tokens into the stake vault.
        {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.stake_from_account.to_account_info(),
                    to: ctx.accounts.staking_vault.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            );
            token::transfer(cpi_ctx, amount)?;
        }

        pool.total_staked += amount;

        Ok(())
    }

    pub fn unstake(ctx: Context<Stake>, spt_amount: u64) -> Result<()> {
        if spt_amount == 0 {
            return Err(ErrorCode::AmountMustBeGreaterThanZero.into());
        }

        let clock = clock::Clock::get().unwrap();
        if ctx.accounts.user.maturity_time > u64::try_from(clock.unix_timestamp).unwrap() {
            return Err(ErrorCode::CannotStakeOrClaimBeforeMaturity.into());
        }

        if ctx.accounts.user.balance_staked < spt_amount {
            return Err(ErrorCode::InsufficientFundUnstake.into());
        }

        let pool = &mut ctx.accounts.pool;
        let total_staked = pool.total_staked;

        let user_opt = Some(&mut ctx.accounts.user);
        update_rewards(pool, user_opt, total_staked).unwrap();
        ctx.accounts.user.balance_staked = ctx
            .accounts
            .user
            .balance_staked
            .checked_sub(spt_amount)
            .unwrap();

        if pool.no_tier == false {
            ctx.accounts.user.tier = get_tier(ctx.accounts.user.balance_staked);
        }

        pool.total_staked -= spt_amount;

        // Transfer tokens from the pool vault to user vault.
        {
            let seeds = &[pool.to_account_info().key.as_ref(), &[pool.nonce]];
            let pool_signer = &[&seeds[..]];

            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.staking_vault.to_account_info(),
                    to: ctx.accounts.stake_from_account.to_account_info(),
                    authority: ctx.accounts.pool_signer.to_account_info(),
                },
                pool_signer,
            );
            token::transfer(cpi_ctx, spt_amount.try_into().unwrap())?;
        }

        Ok(())
    }

    pub fn authorize_funder(ctx: Context<FunderChange>, funder_to_add: Pubkey) -> Result<()> {
        if funder_to_add == ctx.accounts.pool.authority {
            return Err(ErrorCode::FunderAlreadyAuthorized.into());
        }
        let funders = &mut ctx.accounts.pool.funders;
        if funders.iter().any(|x| *x == funder_to_add) {
            return Err(ErrorCode::FunderAlreadyAuthorized.into());
        }
        let default_pubkey = Pubkey::default();
        if let Some(idx) = funders.iter().position(|x| *x == default_pubkey) {
            funders[idx] = funder_to_add;
        } else {
            return Err(ErrorCode::MaxFunders.into());
        }
        Ok(())
    }

    pub fn deauthorize_funder(ctx: Context<FunderChange>, funder_to_remove: Pubkey) -> Result<()> {
        if funder_to_remove == ctx.accounts.pool.authority {
            return Err(ErrorCode::CannotDeauthorizePoolAuthority.into());
        }
        let funders = &mut ctx.accounts.pool.funders;
        if let Some(idx) = funders.iter().position(|x| *x == funder_to_remove) {
            funders[idx] = Pubkey::default();
        } else {
            return Err(ErrorCode::CannotDeauthorizeMissingAuthority.into());
        }
        Ok(())
    }

    pub fn fund(ctx: Context<Fund>, amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let total_staked = pool.total_staked;

        update_rewards(pool, None, total_staked).unwrap();

        let current_time = clock::Clock::get()
            .unwrap()
            .unix_timestamp
            .try_into()
            .unwrap();
        let reward_period_end = pool.reward_duration_end;

        if current_time >= reward_period_end {
            pool.reward_rate = amount.checked_div(pool.reward_duration).unwrap();
        } else {
            let remaining = pool.reward_duration_end.checked_sub(current_time).unwrap();
            let leftover = remaining.checked_mul(pool.reward_rate).unwrap();

            pool.reward_rate = amount
                .checked_add(leftover)
                .unwrap()
                .checked_div(pool.reward_duration)
                .unwrap();
        }

        // Transfer reward A tokens into the A vault.
        if amount > 0 {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.from.to_account_info(),
                    to: ctx.accounts.reward_vault.to_account_info(),
                    authority: ctx.accounts.funder.to_account_info(),
                },
            );

            token::transfer(cpi_ctx, amount)?;
        }

        pool.last_update_time = current_time;
        pool.reward_duration_end = current_time.checked_add(pool.reward_duration).unwrap();

        Ok(())
    }

    pub fn claim(ctx: Context<ClaimReward>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let total_staked = pool.total_staked;

        let clock = clock::Clock::get().unwrap();
        if ctx.accounts.user.maturity_time > u64::try_from(clock.unix_timestamp).unwrap() {
            return Err(ErrorCode::CannotStakeOrClaimBeforeMaturity.into());
        }

        let user_opt = Some(&mut ctx.accounts.user);
        update_rewards(pool, user_opt, total_staked).unwrap();

        let seeds = &[pool.to_account_info().key.as_ref(), &[pool.nonce]];
        let pool_signer = &[&seeds[..]];

        if ctx.accounts.user.reward_per_token_pending > 0 {
            let mut reward_amount = ctx.accounts.user.reward_per_token_pending;
            let vault_balance = ctx.accounts.reward_vault.amount;

            ctx.accounts.user.reward_per_token_pending = 0;
            if vault_balance < reward_amount {
                reward_amount = vault_balance;
            }

            if reward_amount > 0 {
                let cpi_ctx = CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.reward_vault.to_account_info(),
                        to: ctx.accounts.reward_account.to_account_info(),
                        authority: ctx.accounts.pool_signer.to_account_info(),
                    },
                    pool_signer,
                );
                token::transfer(cpi_ctx, reward_amount)?;
            }
        }
        Ok(())
    }

    pub fn close_user(ctx: Context<CloseUser>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.user_stake_count = pool.user_stake_count.checked_sub(1).unwrap();
        Ok(())
    }

    pub fn close_pool<'info>(ctx: Context<ClosePool>) -> Result<()> {
        let pool = &ctx.accounts.pool;

        let signer_seeds = &[
            pool.to_account_info().key.as_ref(),
            &[ctx.accounts.pool.nonce],
        ];

        //instead of closing these vaults, we could technically just
        //set_authority on them. it's not very ata clean, but it'd work
        //if size of tx is an issue, thats an approach

        //close staking vault
        let staking_vault_balance = ctx.accounts.staking_vault.amount;

        if staking_vault_balance > 0 {
            let ix = spl_token::instruction::transfer(
                &spl_token::ID,
                ctx.accounts.staking_vault.to_account_info().key,
                ctx.accounts.staking_refundee.to_account_info().key,
                ctx.accounts.pool_signer.key,
                &[ctx.accounts.pool_signer.key],
                staking_vault_balance,
            )?;
            solana_program::program::invoke_signed(
                &ix,
                &[
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.staking_vault.to_account_info(),
                    ctx.accounts.staking_refundee.to_account_info(),
                    ctx.accounts.pool_signer.to_account_info(),
                ],
                &[signer_seeds],
            )?;
        }

        let ix = spl_token::instruction::close_account(
            &spl_token::ID,
            ctx.accounts.staking_vault.to_account_info().key,
            ctx.accounts.refundee.key,
            ctx.accounts.pool_signer.key,
            &[ctx.accounts.pool_signer.key],
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.staking_vault.to_account_info(),
                ctx.accounts.refundee.to_account_info(),
                ctx.accounts.pool_signer.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        //close token a vault
        let reward_vault_balance = ctx.accounts.reward_vault.amount;

        if reward_vault_balance > 0 {
            let ix = spl_token::instruction::transfer(
                &spl_token::ID,
                ctx.accounts.reward_vault.to_account_info().key,
                ctx.accounts.reward_refundee.to_account_info().key,
                ctx.accounts.pool_signer.key,
                &[ctx.accounts.pool_signer.key],
                reward_vault_balance,
            )?;
            solana_program::program::invoke_signed(
                &ix,
                &[
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.reward_vault.to_account_info(),
                    ctx.accounts.reward_refundee.to_account_info(),
                    ctx.accounts.pool_signer.to_account_info(),
                ],
                &[signer_seeds],
            )?;
        }
        let ix = spl_token::instruction::close_account(
            &spl_token::ID,
            ctx.accounts.reward_vault.to_account_info().key,
            ctx.accounts.refundee.key,
            ctx.accounts.pool_signer.key,
            &[ctx.accounts.pool_signer.key],
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.reward_vault.to_account_info(),
                ctx.accounts.refundee.to_account_info(),
                ctx.accounts.pool_signer.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        Ok(())
    }
}
