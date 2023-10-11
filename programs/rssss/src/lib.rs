#![allow(clippy::result_large_err)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use core::mem::size_of_val;
use opml::{Outline, OPML};
use std::borrow::BorrowMut;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod rssss {

    use std::ops::Deref;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let rss_source = RssSource::default();
        ctx.accounts.rss_source_account.set_inner(rss_source);
        ctx.accounts.list_account.set_inner(RssSourceListing {
            seller: ctx.accounts.user.key(),
            price: 0,
            is_list: false,
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
    pub fn list_rss_source(ctx: Context<ListRssSource>, price: u64) -> Result<()> {
        let listing = RssSourceListing {
            seller: *ctx.accounts.user.key,
            price,
            is_list: true,
        };
        ctx.accounts.list_account.set_inner(listing);

        Ok(())
    }

    pub fn cancel_list_rss_source(ctx: Context<CancelListRssSource>) -> Result<()> {
        let listing = RssSourceListing {
            seller: *ctx.accounts.user.key,
            price: 0,
            is_list: false,
        };
        ctx.accounts.list_account.set_inner(listing);

        Ok(())
    }

    pub fn purchase_rss_source(ctx: Context<PurchaseRssSource>, amount: u64) -> Result<()> {
        let list_account = ctx.accounts.list_account.clone();
        if !list_account.is_list {
            return Err(ErrorCode::NotListed.into());
        }
        if list_account.price != amount {
            return Err(ErrorCode::IncorrectAmount.into());
        }

        // check buyer have enought balance
        if ctx.accounts.buyer.to_account_info().clone().lamports() <= amount {
            return Err(ErrorCode::InsufficientBalance.into());
        }

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info().clone(),
                to: ctx.accounts.user.to_account_info().clone(),
            },
        );
        system_program::transfer(cpi_context, amount)?;

        // Copy the RSS source to the buyer's account
        let rss_source = ctx.accounts.rss_source_account.clone();
        let buyer_rss_source = ctx.accounts.buyer_rss_source_account.borrow_mut();
        buyer_rss_source.merge(rss_source.deref().clone());

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + RssSource::SIZE,
        seeds = [b"rssss", user.key().as_ref()],
        bump
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(
        init,
        payer = user,
        space = 8 + RssSourceListing::SIZE,
        seeds = [b"rssss-list", user.key().as_ref()],
        bump
    )]
    pub list_account: Account<'info, RssSourceListing>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddItem<'info> {
    #[account(
        mut,
        seeds = [b"rssss", user.key().as_ref()],
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
        seeds = [b"rssss", user.key().as_ref()],
        bump,
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct ListRssSource<'info> {
    #[account(
        mut,
        seeds = [b"rssss-list", user.key().as_ref()],
        bump,
    )]
    pub list_account: Account<'info, RssSourceListing>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelListRssSource<'info> {
    #[account(
        mut,
        seeds = [b"rssss-list", user.key().as_ref()],
        bump,
    )]
    pub list_account: Account<'info, RssSourceListing>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct PurchaseRssSource<'info> {
    #[account(
        mut,
        seeds = [b"rssss-list", user.key().as_ref()],
        bump,
    )]
    pub list_account: Account<'info, RssSourceListing>,
    #[account(
        mut,
        seeds = [b"rssss", user.key().as_ref()],
        bump,
    )]
    pub rss_source_account: Account<'info, RssSource>,
    #[account(
        mut,
        seeds = [b"rssss", buyer.key().as_ref()],
        bump,
    )]
    pub buyer_rss_source_account: Account<'info, RssSource>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(Debug, PartialEq)]
pub struct RssSource {
    document: Vec<u8>,
}

impl RssSource {
    // for now is const set
    pub const SIZE: usize = 1024 * 10;

    pub fn merge(&mut self, right: RssSource) {
        let string = String::from_utf8(self.document.clone()).unwrap();
        let mut self_opml = OPML::from_str(&string).unwrap();

        let string = String::from_utf8(right.document.clone()).unwrap();
        let right_opml = OPML::from_str(&string).unwrap();

        // Merge
        for right_outline in right_opml.body.outlines {
            if !self_opml
                .body
                .outlines
                .iter()
                .any(|self_outline| self_outline.xml_url == right_outline.xml_url)
            {
                self_opml.body.outlines.push(right_outline);
            }
        }

        // Serialize self_opml back into self.document
        let merged_string = self_opml.to_string().unwrap();
        self.document = merged_string.as_bytes().to_vec();
    }
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

#[account]
#[derive(PartialEq, Debug)]
pub struct RssSourceListing {
    pub seller: Pubkey,
    pub price: u64,
    pub is_list: bool,
}

impl RssSourceListing {
    pub const SIZE: usize = 32 + 8 + 1;
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
