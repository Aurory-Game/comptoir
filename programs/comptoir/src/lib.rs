mod transfer;

use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use metaplex_token_metadata::state::PREFIX as METAPLEX_PREFIX;
use metaplex_token_metadata::state::{Creator, Metadata};
use std::str::FromStr;
use anchor_spl::associated_token::AssociatedToken;
use metaplex_token_metadata::utils::{assert_derivation};
use crate::constant::{ASSOCIATED_TOKEN_PROGRAM};
use crate::constant::{PREFIX, ESCROW};

declare_id!("FCoMPzD3cihsM7EBSbXtorF2yHL4jJ6vrbWtdVaN7qZc");

#[program]
pub mod comptoir {
    use crate::transfer::{pay, pay_with_signer};
    use super::*;

    pub fn create_comptoir(
        ctx: Context<CreateComptoir>, mint: Pubkey, fees: u16, fees_destination: Pubkey, authority: Pubkey,
    ) -> Result<()> {
        let comptoir = &mut ctx.accounts.comptoir;

        comptoir.fees = fees;
        comptoir.fees_destination = fees_destination;
        comptoir.authority = authority;
        comptoir.mint = mint;

        comptoir.validate()?;
        Ok(())
    }

    pub fn update_comptoir(
        ctx: Context<UpdateComptoir>,
        optional_fees: Option<u16>,
        optional_fees_destination: Option<Pubkey>,
        optional_authority: Option<Pubkey>,
    ) -> Result<()> {
        let comptoir = &mut ctx.accounts.comptoir;

        if let Some(fees) = optional_fees {
            comptoir.fees = fees;
        }
        if let Some(fees_destination) = optional_fees_destination {
            comptoir.fees_destination = fees_destination;
        }
        if let Some(authority) = optional_authority {
            comptoir.authority = authority;
        }
        comptoir.validate()?;
        Ok(())
    }

    pub fn update_comptoir_mint(
        ctx: Context<UpdateComptoirMint>,
        mint: Pubkey,
        fees_destination: Pubkey,
    ) -> Result<()> {
        let comptoir = &mut ctx.accounts.comptoir;
        comptoir.mint = mint;
        comptoir.fees_destination = fees_destination;
        comptoir.validate()?;
        Ok(())
    }

    pub fn create_collection(
        ctx: Context<CreateCollection>, name : String, symbol: String, required_verifier: Pubkey, fee: Option<u16>, ignore_fee: bool,
    ) -> Result<()> {
        let collection = &mut ctx.accounts.collection;

        collection.comptoir_key = ctx.accounts.comptoir.key();
        collection.required_verifier = required_verifier;
        collection.name = name;
        collection.symbol = symbol;
        collection.fees = fee;
        collection.ignore_creator_fee = ignore_fee;

        collection.validate()?;
        Ok(())
    }

    pub fn update_collection(
        ctx: Context<UpdateCollection>,
        optional_fee: Option<u16>,
        optional_symbol: Option<String>,
        optional_required_verifier: Option<Pubkey>,
        optional_ignore_creator_fee: Option<bool>,
    ) -> Result<()> {
        let collection = &mut ctx.accounts.collection;

        if let Some(fee_share) = optional_fee {
            collection.fees = Some(fee_share);
        }
        if let Some(symbol) = optional_symbol {
            collection.symbol = symbol;
        }
        if let Some(required_verifier) = optional_required_verifier {
            collection.required_verifier = required_verifier;
        }
        if let Some(ignore_creator_fee) = optional_ignore_creator_fee {
            collection.ignore_creator_fee = ignore_creator_fee;
        }

        collection.validate()?;
        Ok(())
    }

    pub fn create_sell_order(ctx: Context<CreateSellOrder>, price: u64, quantity: u64, destination: Pubkey) -> Result<()> {
        verify_metadata_and_derivation(
            ctx.accounts.metadata.as_ref(),
            &ctx.accounts.seller_nft_token_account.mint.key(),
            &ctx.accounts.collection,
        )?;

        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_nft_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.payer.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, quantity)?;

        let sell_order = &mut ctx.accounts.sell_order;
        sell_order.comptoir = ctx.accounts.comptoir.key();
        sell_order.price = price;
        sell_order.quantity = quantity;
        sell_order.mint = ctx.accounts.seller_nft_token_account.mint;
        sell_order.authority = ctx.accounts.payer.key();
        sell_order.destination = destination;
        Ok(())
    }

    pub fn remove_sell_order(ctx: Context<RemoveSellOrder>, quantity_to_unlist: u64) -> Result<()> {
        if ctx.accounts.sell_order.quantity < quantity_to_unlist {
            return Err(error!(ErrorCode::ErrTryingToUnlistMoreThanOwned));
        }

        let seeds = &[
            PREFIX.as_bytes(),
            "vault".as_bytes(),
            ctx.accounts.seller_nft_token_account.mint.as_ref(),
            &[*ctx.bumps.get("vault").unwrap()], ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.seller_nft_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
        token::transfer(cpi_ctx, quantity_to_unlist)?;

        let sell_order = &mut ctx.accounts.sell_order;
        sell_order.quantity = sell_order.quantity.checked_sub(quantity_to_unlist).unwrap();

        if ctx.accounts.sell_order.quantity == 0 {
            ctx.accounts.sell_order.close(ctx.accounts.authority.to_account_info())?;
        }
        Ok(())
    }

    pub fn add_quantity_to_sell_order(ctx: Context<SellOrderAddQuantity>, quantity_to_add: u64) -> Result<()> {
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_nft_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, quantity_to_add)?;

        let sell_order = &mut ctx.accounts.sell_order;
        sell_order.quantity = sell_order.quantity.checked_add(quantity_to_add).unwrap();

        Ok(())
    }

    pub fn buy<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, Buy<'info>>, ask_quantity: u64) -> Result<()> {
        let mut total_spent_ed: u64 = 0;

        let metadata = verify_metadata_and_derivation(
            ctx.accounts.metadata.as_ref(),
            &ctx.accounts.buyer_nft_token_account.mint.key(),
            &ctx.accounts.collection,
        )?;
        let mut index = 0;

        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if !ctx.accounts.collection.ignore_creator_fee {
            if let Some(creators)  = metadata.data.creators {
                index = creators.len();
                let creators_distributions = verify_and_get_creators(creators, ctx.remaining_accounts, ctx.accounts.comptoir.mint);
                creators_distributions_option = Some(creators_distributions);
            }
        }

        let mut comptoir_fee = ctx.accounts.comptoir.fees;
        if let Some(collection_share) = ctx.accounts.collection.fees {
            comptoir_fee = collection_share;
        }

        let seeds = &[
            PREFIX.as_bytes(),
            "vault".as_bytes(),
            ctx.accounts.buyer_nft_token_account.mint.as_ref(),
            &[*ctx.bumps.get("vault").unwrap()], ];
        let signer = &[&seeds[..]];

        let mut remaining_to_buy = ask_quantity;

        while index < ctx.remaining_accounts.len() {
            let sell_order_result= Account::<'info, SellOrder>::try_from(&ctx.remaining_accounts[index]);
            if sell_order_result.is_err() {
                index = index + 2;
                continue
            }

            let mut sell_order = sell_order_result.unwrap();
            assert_eq!(sell_order.comptoir, ctx.accounts.comptoir.key());
            assert_eq!(sell_order.mint, ctx.accounts.buyer_nft_token_account.mint.key());

            index = index + 1;

            let mut to_buy = remaining_to_buy;
            if sell_order.quantity < to_buy {
                to_buy = sell_order.quantity;
            }

            pay_with_signer(
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.buyer_nft_token_account.to_account_info(),
                ctx.accounts.vault.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
                to_buy,
                signer,
            )?;

            let seller_token_account = &ctx.remaining_accounts[index];
            index = index + 1;
            assert_eq!(seller_token_account.key(), sell_order.destination);
            let total_amount = sell_order.price.checked_mul(to_buy).unwrap();
            let mut creators_share: u64 = 0;
            if !ctx.accounts.collection.ignore_creator_fee {
                creators_share = calculate_fee(total_amount, metadata.data.seller_fee_basis_points, 10000);
            }
            let comptoir_share = calculate_fee(total_amount, comptoir_fee, 10000);
            let seller_share = total_amount.checked_sub(creators_share).unwrap().checked_sub(comptoir_share).unwrap();

            pay(
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                seller_token_account.to_account_info(),
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                seller_share,
            )?;
            pay(
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                ctx.accounts.comptoir_dest_account.to_account_info(),
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                comptoir_share,
            )?;

            if let Some(creators) = creators_distributions_option.as_ref() {
                for creator in creators {
                    let creator_share = calculate_fee(creators_share, creator.1 as u16, 100);
                    pay(
                        ctx.accounts.buyer_paying_token_account.to_account_info(),
                        creator.0.to_account_info(),
                        ctx.accounts.buyer.to_account_info(),
                        ctx.accounts.token_program.to_account_info(),
                        creator_share,
                    )?;
                }
            }

            emit!(BoughtSellOrderEvent{
                sell_order: sell_order.key(),
                quantity: to_buy,
                buyer: ctx.accounts.buyer.key(),
            });

            total_spent_ed = total_spent_ed + total_amount;

            sell_order.quantity = sell_order.quantity - to_buy;
            sell_order.exit(ctx.program_id)?;

            remaining_to_buy = remaining_to_buy - to_buy;
            if remaining_to_buy == 0 {
                break;
            }
        }

        if remaining_to_buy != 0 {
            return Err(error!(ErrorCode::ErrCouldNotBuyEnoughItem));
        }

        Ok(())
    }

    pub fn create_buy_offer(ctx: Context<CreateBuyOffer>, price_proposition: u64) -> Result<()> {
        verify_metadata_and_derivation(
            ctx.accounts.metadata.as_ref(),
            &ctx.accounts.nft_mint.key(),
            &ctx.accounts.collection,
        )?;

        let buy_offer = &mut ctx.accounts.buy_offer;
        buy_offer.mint = ctx.accounts.nft_mint.key();
        buy_offer.authority = ctx.accounts.payer.key();
        buy_offer.proposed_price = price_proposition;
        buy_offer.comptoir = ctx.accounts.comptoir.key();
        buy_offer.destination = ctx.accounts.buyer_nft_account.key();

        pay(
            ctx.accounts.buyer_paying_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            price_proposition,
        )?;

        Ok(())
    }

    pub fn remove_buy_offer(ctx: Context<RemoveBuyOffer>) -> Result<()> {
        let seeds = &[
            PREFIX.as_bytes(),
            ctx.accounts.comptoir.to_account_info().key.as_ref(),
            ctx.accounts.comptoir.mint.as_ref(),
            ESCROW.as_bytes(),
            &[*ctx.bumps.get("escrow").unwrap()], ];

        let signer: &[&[&[u8]]] = &[&seeds[..]];
        pay_with_signer(
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.buyer_paying_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.buy_offer.proposed_price,
            signer,
        )?;
        Ok(())
    }

    pub fn execute_offer<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, ExecuteOffer<'info>>) -> Result<()> {
        let metadata = verify_metadata_and_derivation(
            &ctx.accounts.metadata,
            &ctx.accounts.seller_nft_account.mint,
            &ctx.accounts.collection,
        )?;

        //Transfer NFT to buyer
        pay(
            ctx.accounts.seller_nft_account.to_account_info(),
        ctx.accounts.destination.to_account_info(),
            ctx.accounts.seller.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            1
        )?;

        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if !ctx.accounts.collection.ignore_creator_fee {
            if let Some(creators) = metadata.data.creators {
                let creators_distributions = verify_and_get_creators(creators, ctx.remaining_accounts, ctx.accounts.comptoir.mint);
                creators_distributions_option = Some(creators_distributions);
            }
        }

        let mut comptoir_fee = ctx.accounts.comptoir.fees;
        msg!("lulz");
        if let Some(collection_share) = ctx.accounts.collection.fees {
            msg!("lalla");
            comptoir_fee = collection_share;
        }
        msg!(comptoir_fee.to_string().as_str());
        let total_amount = ctx.accounts.buy_offer.proposed_price;
        let mut creators_share = 0;
        if !ctx.accounts.collection.ignore_creator_fee {
            creators_share = calculate_fee(total_amount, metadata.data.seller_fee_basis_points, 10000);
        }
        let comptoir_share = calculate_fee(total_amount, comptoir_fee, 10000);
        let seller_share = total_amount.checked_sub(creators_share).unwrap().checked_sub(comptoir_share).unwrap();

        let seeds = &[
            PREFIX.as_bytes(),
            ctx.accounts.comptoir.to_account_info().key.as_ref(),
            ctx.accounts.comptoir.mint.as_ref(),
            ESCROW.as_bytes(),
            &[*ctx.bumps.get("escrow").unwrap()], ];
        let signer: &[&[&[u8]]] = &[&seeds[..]];

        if let Some(creators) = creators_distributions_option.as_ref() {
            for creator in creators {
                let creator_share = calculate_fee(creators_share, creator.1 as u16, 100);
                pay_with_signer(
                    ctx.accounts.escrow.to_account_info(),
                    creator.0.to_account_info(),
                    ctx.accounts.escrow.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    creator_share,
                    signer
                )?;
            }
        }

        msg!(comptoir_share.to_string().as_str());

        pay_with_signer(
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.comptoir_dest_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            comptoir_share,
            signer,
        )?;

        pay_with_signer(
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.seller_funds_dest_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            seller_share,
            signer,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(price_proposition: u64)]
pub struct CreateBuyOffer<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    nft_mint: Account<'info, Mint>,
    /// CHECK: This is not dangerous because check it all the time using the verify_metadata_and_derivation func
    metadata: UncheckedAccount<'info>,

    comptoir: Box<Account<'info, Comptoir>>,
    #[account(mut, constraint = collection.comptoir_key == comptoir.key())]
    collection: Box<Account<'info, Collection>>,
    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump,
    )]
    escrow: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    buyer_paying_account: Box<Account<'info, TokenAccount>>,
    #[account(
    init_if_needed,
    payer = payer,
    associated_token::mint = nft_mint,
    associated_token::authority = payer,
    )]
    buyer_nft_account: Account<'info, TokenAccount>,

    #[account(
    init,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    payer.key.as_ref(),
    nft_mint.key().as_ref(),
    price_proposition.to_string().as_bytes(),
    ESCROW.as_bytes(),
    ],
    bump,
    payer = payer,
    space = 144,
    )]
    buy_offer: Account<'info, BuyOffer>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    associated_token_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RemoveBuyOffer<'info> {
    #[account(mut)]
    buyer: Signer<'info>,

    #[account(mut)]
    buyer_paying_account: Account<'info, TokenAccount>,

    comptoir: Account<'info, Comptoir>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump,
    )]
    escrow: Account<'info, TokenAccount>,

    #[account(
    mut,
    close = buyer,
    has_one = comptoir,
    constraint = buy_offer.authority == buyer.key(),
    )]
    buy_offer: Account<'info, BuyOffer>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ExecuteOffer<'info> {
    seller: Signer<'info>,

    #[account(mut)]
    buyer: SystemAccount<'info>,

    comptoir: Box<Account<'info, Comptoir>>,
    #[account(mut, constraint = collection.comptoir_key == comptoir.key())]
    collection: Box<Account<'info, Collection>>,

    #[account(mut, constraint = comptoir_dest_account.key() == comptoir.fees_destination)]
    comptoir_dest_account: Box<Account<'info, TokenAccount>>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump,
    )]
    escrow: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    seller_funds_dest_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    destination: Account<'info, TokenAccount>,
    #[account(mut)]
    seller_nft_account: Account<'info, TokenAccount>,


    /// CHECK: This is not dangerous because check it all the time using the verify_metadata_and_derivation func
    metadata: UncheckedAccount<'info>,

    #[account(
    mut,
    close = buyer,
    constraint = buy_offer.authority == buyer.key(),
    constraint = seller_nft_account.mint.key() == buy_offer.mint,
    has_one = destination,
    has_one = comptoir,
    )]
    buy_offer: Account<'info, BuyOffer>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(comptoir_mint: Pubkey)]
pub struct CreateComptoir<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    #[account(
    init,
    seeds = [
        PREFIX.as_bytes(),
        payer.key.as_ref()
    ],
    bump,
    payer = payer,
    space = 112,
    )]
    comptoir: Account<'info, Comptoir>,

    mint: Account<'info, Mint>,

    #[account(
    init,
    token::mint = mint,
    token::authority = escrow,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir_mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump,
    payer = payer,
    )]
    escrow: Account<'info, TokenAccount>,


    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateComptoir<'info> {
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,
}

#[derive(Accounts)]
#[instruction(new_comptoir_mint: Pubkey)]
pub struct UpdateComptoirMint<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,

    #[account(constraint = new_comptoir_mint == mint.key())]
    mint: Account<'info, Mint>,

    #[account(
    init_if_needed,
    token::mint = mint,
    token::authority = escrow,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    new_comptoir_mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump,
    payer = authority,
    )]
    escrow: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,
    #[account(
    init,
    seeds = [
    PREFIX.as_bytes(),
    name.as_bytes(),
    comptoir.key().as_ref(),
    ],
    bump,
    payer = authority,
    space = 110,
    )]
    collection: Account<'info, Collection>,

    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateCollection<'info> {
    authority: Signer<'info>,
    #[account(has_one = authority)]
    comptoir: Account<'info, Comptoir>,

    #[account(mut, constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,
}

#[derive(Accounts)]
#[instruction(price: u64)]
pub struct CreateSellOrder<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    #[account(mut)]
    seller_nft_token_account: Box<Account<'info, TokenAccount>>,

    comptoir: Box<Account<'info, Comptoir>>,
    #[account(constraint = collection.comptoir_key == comptoir.key())]
    collection: Box<Account<'info, Collection>>,

    #[account(constraint = mint.key() == seller_nft_token_account.mint)]
    mint: Account<'info, Mint>,
    /// CHECK: This is not dangerous because check it all the time using the verify_metadata_and_derivation func
    metadata: UncheckedAccount<'info>,

    #[account(
    init_if_needed,
    token::mint = mint,
    token::authority = vault,
    seeds = [
    PREFIX.as_bytes(),
    "vault".as_bytes(),
    seller_nft_token_account.mint.as_ref(),
    ],
    bump,
    payer = payer,
    )]
    vault: Account<'info, TokenAccount>,

    #[account(
    init,
    seeds = [
    PREFIX.as_bytes(),
    seller_nft_token_account.key().as_ref(),
    price.to_string().as_bytes(),
    ],
    bump,
    payer = payer,
    space = 152,
    )]
    sell_order: Account<'info, SellOrder>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RemoveSellOrder<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, constraint = authority.key() == seller_nft_token_account.owner)]
    seller_nft_token_account: Account<'info, TokenAccount>,
    #[account(mut, has_one = authority, constraint = seller_nft_token_account.mint == sell_order.mint)]
    sell_order: Account<'info, SellOrder>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    "vault".as_bytes(),
    seller_nft_token_account.mint.as_ref(),
    ],
    bump,
    )]
    vault: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SellOrderAddQuantity<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, constraint = authority.key() == seller_nft_token_account.owner)]
    seller_nft_token_account: Account<'info, TokenAccount>,
    #[account(mut, has_one = authority, constraint = seller_nft_token_account.mint == sell_order.mint)]
    sell_order: Account<'info, SellOrder>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    "vault".as_bytes(),
    seller_nft_token_account.mint.as_ref(),
    ],
    bump,
    )]
    vault: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Buy<'info> {
    buyer: Signer<'info>,
    #[account(mut)]
    buyer_nft_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    buyer_paying_token_account: Account<'info, TokenAccount>,

    comptoir: Account<'info, Comptoir>,
    #[account(mut, constraint = comptoir_dest_account.key() == comptoir.fees_destination)]
    comptoir_dest_account: Account<'info, TokenAccount>,
    #[account(constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,

    /// CHECK: This is not dangerous because check it all the time using the verify_metadata_and_derivation func
    metadata: UncheckedAccount<'info>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    "vault".as_bytes(),
    buyer_nft_token_account.mint.as_ref()
    ],
    bump,
    )]
    vault: Box<Account<'info, TokenAccount>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[account]
pub struct Comptoir {
    fees: u16,
    fees_destination: Pubkey,
    authority: Pubkey,
    mint: Pubkey,
}

#[account]
pub struct SellOrder {
    comptoir: Pubkey,
    price: u64,
    quantity: u64,
    mint: Pubkey,
    authority: Pubkey,
    destination: Pubkey,
}

#[account]
pub struct Collection {
    comptoir_key: Pubkey,
    name: String,
    symbol: String,
    required_verifier: Pubkey,
    fees: Option<u16>, //Takes priority over comptoir fees
    ignore_creator_fee: bool,
}

#[account]
pub struct BuyOffer {
    comptoir: Pubkey,
    mint: Pubkey,
    proposed_price: u64,
    authority: Pubkey,
    destination: Pubkey,
}

impl Collection {
    pub fn is_part_of_collection(&self, metadata: &Metadata) -> bool {
        return if let Some(creators) = metadata.data.creators.as_ref() {
            metadata.data.symbol.starts_with(&self.symbol.to_string())
                && creators.iter().any(|c| c.address == self.required_verifier && c.verified)
        } else {
            false
        };
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(fee) = self.fees {
            if fee > 10000 {
                return Err(error!(ErrorCode::ErrFeeShouldLowerOrEqualThan10000));
            }
        }
        Ok(())
    }
}

impl Comptoir {
    pub fn validate(&self) -> Result<()> {
        if self.fees > 10000 {
            return Err(error!(ErrorCode::ErrFeeShouldLowerOrEqualThan10000));
        }
        Ok(())
    }
}

fn verify_metadata_and_derivation(unverified_metadata: &AccountInfo, nft_mint: &Pubkey, collection: &Collection) -> Result<Metadata> {
    if unverified_metadata.data_is_empty() {
        return Err(error!(ErrorCode::NotInitialized));
    };
    assert_derivation(
        &metaplex_token_metadata::id(),
        unverified_metadata,
        &[
            METAPLEX_PREFIX.as_bytes(),
            metaplex_token_metadata::id().as_ref(),
            nft_mint.as_ref(),
        ],
    )?;
    let metadata = Metadata::from_account_info(unverified_metadata)?;
    if !collection.is_part_of_collection(&metadata) {
        return Err(error!(ErrorCode::ErrNftNotPartOfCollection));
    }
    return Ok(metadata);
}

pub mod constant {
    pub const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
    pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    pub const PREFIX: &str = "COMPTOIR";
    pub const ESCROW: &str = "ESCROW";
}


fn calculate_fee(amount: u64, fee_share: u16, basis: u64) -> u64 {
    let fee = amount
        .checked_mul(fee_share as u64)
        .unwrap()
        .checked_div(basis)
        .unwrap();

    return fee;
}

fn verify_and_get_creators<'a, 'b, 'c, 'info>(creators: Vec<Creator>, remaining_accounts: &'c [AccountInfo<'info>], comptoir_mint: Pubkey) -> Vec<(&'c AccountInfo<'info>, u8)> {
    let is_native = comptoir_mint == spl_token::native_mint::id();
    let mut creators_distributions = Vec::new();
    for i in 0..creators.len() {
        let remaining_account_creator = &remaining_accounts[i];
        if is_native {
            assert_eq!(remaining_account_creator.key(), creators[i].address);
            creators_distributions.push((remaining_account_creator, creators[i].share));
        } else {
            let ata_seeds: &[&[u8]] = &[
                creators[i].address.as_ref(),
                spl_token::ID.as_ref(),
                comptoir_mint.as_ref(),
            ];
            let atp = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
            let creator_associated_token_addr = Pubkey::find_program_address(&ata_seeds, &atp);
            assert_eq!(remaining_account_creator.key(), creator_associated_token_addr.0);
            creators_distributions.push((remaining_account_creator, creators[i].share));
        }
    }
    return creators_distributions;
}

#[event]
pub struct BoughtSellOrderEvent {
    pub sell_order: Pubkey,
    pub quantity: u64,
    pub buyer: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Fee should be <= 10000")]
    ErrFeeShouldLowerOrEqualThan10000,
    #[msg("Trying to unlist more than owned")]
    ErrTryingToUnlistMoreThanOwned,
    #[msg("Could not buy the required quantity of items")]
    ErrCouldNotBuyEnoughItem,
    #[msg("metadata mint does not match item mint")]
    ErrMetaDataMintDoesNotMatchItemMint,
    #[msg("nft not part of collection")]
    ErrNftNotPartOfCollection,
    #[msg("Derived key invalid")]
    DerivedKeyInvalid,
    #[msg("AccountNotInitialized")]
    NotInitialized
}
