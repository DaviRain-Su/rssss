#![allow(clippy::result_large_err)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use core::mem::size_of;
use core::mem::size_of_val;
use opml::{Outline, OPML};
use std::borrow::BorrowMut;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// one month as seconds
pub const DURATION_ONE_MONTH: i64 = 60 * 60 * 24 * 30;

// Assume a maximum number of users for space allocation
pub const MAX_USERS: usize = 1000;

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

    pub fn add_item(
        ctx: Context<AddItem>,
        text: String,
        html_url: String,
        xml_url: String,
    ) -> Result<()> {
        let rss_source_account = ctx.accounts.rss_source_account.borrow_mut();
        let string = String::from_utf8(rss_source_account.document.clone()).unwrap();
        let mut opml = OPML::from_str(&string).unwrap();

        let subscription_outline = Outline {
            text: text.to_string(),
            r#type: Some("rss".to_string()),
            html_url: Some(html_url.to_string()),
            xml_url: Some(xml_url.to_string()),
            ..Default::default() // Fill other fields with default values
        };

        opml.body.outlines.push(subscription_outline);
        rss_source_account.document = opml.to_string().unwrap().as_bytes().to_vec();

        Ok(())
    }

    pub fn remove_item(ctx: Context<RemoveItem>, xml_url: String) -> Result<()> {
        let rss_source_account = ctx.accounts.rss_source_account.borrow_mut();
        let string = String::from_utf8(rss_source_account.document.clone()).unwrap();
        let mut opml = OPML::from_str(&string).unwrap();

        opml.body
            .outlines
            .retain(|subscription| subscription.xml_url.as_deref() != Some(&xml_url));

        rss_source_account.document = opml.to_string().unwrap().as_bytes().to_vec();
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

    // TODO: this is not working, must impl by frontend
    // pub fn get_subscriber_opml_data(
    //     ctx: Context<GetSubscriberOpmlData>,
    //     active_subscribers: Vec<Pubkey>,
    // ) -> Result<OPMLResponses> {
    //     let mut opml_responses = Vec::new();

    //     for subscriber in &active_subscribers {
    //         // Derive the address of the subscriber's data account based on your PDA rules.
    //         let (data_account_address, bump_seed) =
    //             Pubkey::find_program_address(&[b"rss", subscriber.as_ref()], &crate::id());

    //         // Create an AccountInfo instance for the subscriber's data account
    //         // let data_account_info = create_account_info(ctx, data_account_address)?;

    //         // Assume the data account contains a RssSource struct.
    //         // Deserialize the RssSource struct from the data account's data.
    //         // let rss_source: RssSource = anchor_lang::deserialize(&data_account_info.data.borrow())?;
    //         let rss_source: RssSource = RssSource::default();

    //         // Convert the RssSource document to OPML and extract title and html_url
    //         let string = String::from_utf8(rss_source.document.clone()).unwrap();
    //         let opml = OPML::from_str(&string).unwrap();

    //         // Assuming the first outline contains the title and html_url you want
    //         // Adjust this logic based on the actual structure of your OPML data
    //         if let Some(outline) = opml.body.outlines.first() {
    //             let response = OPMLResponse {
    //                 title: outline.text.clone(),
    //                 html_url: outline.html_url.clone().unwrap_or_default(),
    //             };
    //             opml_responses.push(response);
    //         }
    //     }

    //     Ok(OPMLResponses {
    //         res: opml_responses,
    //     })
    // }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + RssSource::SIZE,
        seeds = [b"rss", user.key().as_ref()],
        bump
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<Subscriptions>(),
        seeds = [b"subscriptions", user.key().as_ref()],
        bump
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<SubscriptionPrice>(),
        seeds = [b"subprice", user.key().as_ref()],
        bump
    )]
    pub subscription_price_acc: Account<'info, SubscriptionPrice>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddItem<'info> {
    #[account(
        mut,
        seeds = [b"rss", user.key().as_ref()],
        bump,
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct RemoveItem<'info> {
    #[account(
        mut,
        seeds = [b"rss", user.key().as_ref()],
        bump,
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct Subscribe<'info> {
    // Account for collecting platform fees
    pub fee_account: AccountInfo<'info>,
    // you want subscription account
    pub subscription_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"subscriptions", user.key().as_ref()],
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
        seeds = [b"subscriptions", user.key().as_ref()],
        bump,
    )]
    pub subscriptions_account: Account<'info, Subscriptions>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetActiveSubscriptions<'info> {
    #[account(
        seeds = [b"subscriptions", user.key().as_ref()],
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
        seeds = [b"logged-in-users"],
        bump
    )]
    pub logged_in_users_account: Account<'info, LoggedInUsers>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// #[derive(Accounts)]
// pub struct GetSubscriberOpmlData {
//     // ... other fields ...
// }

#[account]
#[derive(PartialEq, Debug, Default)]
pub struct LoggedInUsers {
    pub users: Vec<Pubkey>,
}

#[derive(Accounts)]
pub struct AddLoggedInUser<'info> {
    #[account(
        mut,
        seeds = [b"logged-in-users"],
        bump
    )]
    pub logged_in_users_account: Account<'info, LoggedInUsers>,
}

#[account]
#[derive(Debug, PartialEq)]
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
impl Default for RssSource {
    fn default() -> Self {
        let document = OPML::from_str(DEFAULT_CONFIG_FILE).expect("never failed because is valid");
        let document_str = document.to_string().expect("never failed because is valid");

        Self {
            document: document_str.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, PartialEq, borsh::BorshDeserialize, borsh::BorshSerialize)]
pub struct OPMLResponses {
    pub res: Vec<OPMLResponse>,
}

#[derive(Debug, PartialEq, borsh::BorshDeserialize, borsh::BorshSerialize)]
pub struct OPMLResponse {
    pub title: String,
    pub html_url: String,
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
    pub const SIZE: usize = 32 + 8 + 1;

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

#[test]
fn test_rss_source_size() {
    let default_size = RssSource::default_size();
    println!("default size {}", default_size);
}

#[test]
fn der_and_ser_rss_source() {
    let rss_source = RssSource::default();
    let string = String::from_utf8(rss_source.document).unwrap();
    let opml = OPML::from_str(&string).unwrap();
    println!("opml = {:#?}", opml);
}
