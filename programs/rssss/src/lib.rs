#![allow(clippy::result_large_err)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use std::borrow::BorrowMut;
use std::mem::size_of;
use std::mem::size_of_val;

declare_id!("3QKjPj2ajRsyRpZfsyqpcDMyN2SF866j5jcRZ6cx4PK6");

/// one month as seconds
pub const DURATION_ONE_MONTH: i64 = 60 * 60 * 24 * 30;

// Assume a maximum number of users for space allocation
pub const MAX_USERS: usize = 1000;

pub const LOGGED_IN_USERS: &[u8] = b"logged-in-users";
pub const RSS_SUBSCRIPTIONS: &[u8] = b"subscriptions";
pub const RSS: &[u8] = b"rss";
pub const SUB_PRICE: &[u8] = b"sub-price";

#[program]
pub mod rssss {
    use super::*;

    pub fn initialize_logged_in_users(ctx: Context<InitializeLoggedInUsers>) -> Result<()> {
        let logged_in_users = LoggedInUsers::default();
        ctx.accounts
            .logged_in_users_account
            .set_inner(logged_in_users);
        Ok(())
    }

    pub fn add_logged_in_user(ctx: Context<AddLoggedInUser>, user_pubkey: Pubkey) -> Result<()> {
        let logged_in_users_account = ctx.accounts.logged_in_users_account.borrow_mut();
        if logged_in_users_account.users.len() < MAX_USERS {
            logged_in_users_account.users.push(user_pubkey);
            Ok(())
        } else {
            Err(ErrorCode::MaxUsersReached.into())
        }
    }

    pub fn initialize(ctx: Context<Initialize>, price: u64) -> Result<()> {
        let rss_source = RssSource::default();
        ctx.accounts.rss_source_account.set_inner(rss_source);
        ctx.accounts
            .subscriptions_account
            .set_inner(Subscriptions::default());
        ctx.accounts
            .subscription_price_acc
            .set_inner(SubscriptionPrice {
                price_one_month: price,
            });
        Ok(())
    }

    pub fn change_sub_price(ctx: Context<ChangeSubPrice>, price: u64) -> Result<()> {
        let subscription_price_acc = ctx.accounts.subscription_price_acc.borrow_mut();
        subscription_price_acc.price_one_month = price;
        Ok(())
    }

    pub fn update_item(ctx: Context<UpdateOutline>, new_document: Vec<u8>) -> Result<()> {
        let rss_source_account = ctx.accounts.rss_source_account.borrow_mut();
        rss_source_account.document = new_document;

        Ok(())
    }

    // list rss source you want to sell
    pub fn subscribe(ctx: Context<Subscribe>, price: u64) -> Result<()> {
        let subscription_account = ctx.accounts.subscription_account.borrow_mut();

        // Calculate 5% fee
        let fee = price / 20; // 5%
        let net_price = price - fee; // Amount after fee is deducted

        // first: transfer lamports to subscription_account
        // check buyer have enought balance
        if ctx.accounts.user.to_account_info().clone().lamports() <= price {
            return Err(ErrorCode::InsufficientBalance.into());
        }

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info().clone(),
                to: subscription_account.to_account_info().clone(),
            },
        );
        system_program::transfer(cpi_context, net_price)?;

        // Transfer fee to the platform fee account
        let cpi_context_fee = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info().clone(),
                to: ctx.accounts.fee_account.to_account_info().clone(),
            },
        );
        system_program::transfer(cpi_context_fee, fee)?;

        // Get the current time
        let current_time = Clock::get()?.unix_timestamp;

        // Check if this subscription already exists
        let subscription_accounts = ctx.accounts.subscriptions_account.borrow_mut();
        if let Some(subscription) = subscription_accounts
            .subscriptions
            .iter_mut()
            .find(|s| s.seller == subscription_account.key())
        {
            // Subscription exists, update it
            subscription.last_payment_time = current_time;
            subscription.duration += DURATION_ONE_MONTH; // Assuming DURATION_ONE_MONTH is defined
        } else {
            // Subscription doesn't exist, create a new one
            let subscription = Subscription {
                seller: subscription_account.key(),
                start_time: current_time,
                duration: DURATION_ONE_MONTH,
                last_payment_time: current_time + DURATION_ONE_MONTH,
            };
            subscription_accounts.subscriptions.push(subscription);
        }

        Ok(())
    }

    pub fn get_active_subscriptions(
        ctx: Context<GetActiveSubscriptions>,
        current_time: i64,
    ) -> Result<Vec<Pubkey>> {
        let subscriptions_account = &ctx.accounts.subscriptions_account;
        let mut active_subscribers = Vec::new();

        for subscription in &subscriptions_account.subscriptions {
            if subscription.is_active(current_time) {
                active_subscribers.push(subscription.seller);
            }
        }

        Ok(active_subscribers)
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + RssSource::SIZE,
        seeds = [RSS, user.key().as_ref()],
        bump
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<Subscriptions>(),
        seeds = [RSS_SUBSCRIPTIONS, user.key().as_ref()],
        bump
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<SubscriptionPrice>(),
        seeds = [SUB_PRICE, user.key().as_ref()],
        bump
    )]
    pub subscription_price_acc: Account<'info, SubscriptionPrice>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeSubPrice<'info> {
    #[account(
        mut,
        seeds = [SUB_PRICE, user.key().as_ref()],
        bump
    )]
    pub subscription_price_acc: Account<'info, SubscriptionPrice>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateOutline<'info> {
    #[account(
        mut,
        seeds = [RSS, user.key().as_ref()],
        bump,
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct Subscribe<'info> {
    /// CHECK: It's Ok!
    // Account for collecting platform fees
    pub fee_account: AccountInfo<'info>,
    /// CHECK: It's OK!
    // you want subscription account
    pub subscription_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [RSS_SUBSCRIPTIONS, user.key().as_ref()],
        bump,
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelSubscribe<'info> {
    #[account(
        mut,
        seeds = [RSS_SUBSCRIPTIONS, user.key().as_ref()],
        bump,
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetActiveSubscriptions<'info> {
    #[account(
        seeds = [RSS_SUBSCRIPTIONS, user.key().as_ref()],
        bump,
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    pub user: Signer<'info>,
}

/// this acccount init by platform
#[derive(Accounts)]
pub struct InitializeLoggedInUsers<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + (32 * MAX_USERS),  // Assume a maximum number of users for space allocation
        seeds = [LOGGED_IN_USERS],
        bump
    )]
    pub logged_in_users_account: Account<'info, LoggedInUsers>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLoggedInUser<'info> {
    #[account(
        mut,
        seeds = [LOGGED_IN_USERS],
        bump
    )]
    pub logged_in_users_account: Account<'info, LoggedInUsers>,
}

#[account]
#[derive(PartialEq, Debug, Default)]
pub struct LoggedInUsers {
    pub users: Vec<Pubkey>,
}

#[account]
#[derive(Debug, PartialEq, Default)]
pub struct RssSource {
    document: Vec<u8>,
}

#[account]
#[derive(Debug, PartialEq)]
pub struct SubscriptionPrice {
    price_one_month: u64,
}

impl RssSource {
    // for now is const set
    pub const SIZE: usize = 1024 * 10;
}

impl RssSource {
    pub fn default_size() -> usize {
        let default_value = RssSource::default();
        size_of_val(&default_value)
    }
}

#[account]
#[derive(PartialEq, Debug, Default)]
pub struct Subscriptions {
    subscriptions: Vec<Subscription>,
}

#[account]
#[derive(PartialEq, Debug)]
pub struct Subscription {
    pub seller: Pubkey,
    pub start_time: i64,        // Subscription start time in Unix timestamp
    pub duration: i64,          // Subscription duration in months
    pub last_payment_time: i64, // Last payment time in Unix timestamp
}

impl Subscription {
    pub const SIZE: usize = 32 + 8 + 8 + 8;

    // Checks if the subscription is active based on the current timestamp.
    pub fn is_active(&self, current_time: i64) -> bool {
        let elapsed_time_in_months = (current_time - self.start_time) / (30 * 24 * 3600);
        elapsed_time_in_months < self.duration
    }

    // Updates the subscription duration based on a new payment.
    pub fn update_duration(&mut self, additional_months: i64, payment_time: i64) {
        self.duration += additional_months;
        self.last_payment_time = payment_time;
    }

    // Checks if the subscription needs renewal based on the current timestamp.
    pub fn needs_renewal(&self, current_time: i64) -> bool {
        !self.is_active(current_time)
    }
}

pub const DEFAULT_CONFIG_FILE: &str = r#"
<opml version="2.0">
    <head>
        <title>Your Subscription List</title>
    </head>
    <body>
        <outline text="24 ways" htmlUrl="http://24ways.org/" type="rss" xmlUrl="http://feeds.feedburner.com/24ways"/>
    </body>
</opml>
"#;

#[error_code]
pub enum ErrorCode {
    #[msg("not listed")]
    NotListed,
    #[msg("incorrect amount")]
    IncorrectAmount,
    #[msg("Insufficient balance.")]
    InsufficientBalance,
    #[msg("Max users reached.")]
    MaxUsersReached,
}
