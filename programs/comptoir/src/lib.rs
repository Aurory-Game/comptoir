mod transfer;

use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use metaplex_token_metadata::state::PREFIX as METAPLEX_PREFIX;
use metaplex_token_metadata::state::{Creator, Metadata};
use std::str::FromStr;
use metaplex_token_metadata::utils::{assert_derivation, assert_owned_by};
use crate::constant::{ASSOCIATED_TOKEN_PROGRAM};
use crate::constant::{PREFIX, ESCROW};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod comptoir {
    use crate::transfer::{pay, pay_with_signer};
    use super::*;

    pub fn create_comptoir(
        ctx: Context<CreateComptoir>,
        _nounce: u8, fees: u16, fees_destination: Pubkey, authority: Pubkey, mint: Pubkey,
    ) -> ProgramResult {
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
        optional_mint: Option<Pubkey>,
    ) -> ProgramResult {
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
        if let Some(mint) = optional_mint {
            comptoir.mint = mint;
        }

        comptoir.validate()?;
        Ok(())
    }


    pub fn create_collection(
        ctx: Context<CreateCollection>,
        _nounce: u8, symbol: String, required_verifier: Pubkey, fee: Option<u16>,
    ) -> ProgramResult {
        let collection = &mut ctx.accounts.collection;

        collection.comptoir_key = ctx.accounts.comptoir.key();
        collection.required_verifier = required_verifier;
        collection.symbol = symbol;
        collection.fees = fee;

        collection.validate()?;
        Ok(())
    }

    pub fn update_collection(
        ctx: Context<UpdateCollection>,
        optional_fee: Option<u16>,
        optional_symbol: Option<String>,
        optional_required_verifier: Option<Pubkey>,
    ) -> ProgramResult {
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
        collection.validate()?;
        Ok(())
    }

    pub fn create_sell_order(ctx: Context<CreateSellOrder>, _nounce: u8, _salt: String, _sell_order_nounce: u8, price: u64, quantity: u64, destination: Pubkey) -> ProgramResult {
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_nft_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.payer.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, quantity)?;

        let sell_order = &mut ctx.accounts.sell_order;
        sell_order.price = price;
        sell_order.quantity = quantity;
        sell_order.mint = ctx.accounts.seller_nft_token_account.mint;
        sell_order.authority = ctx.accounts.payer.key();
        sell_order.destination = destination;
        Ok(())
    }

    pub fn remove_sell_order(ctx: Context<RemoveSellOrder>, nounce: u8, quantity_to_unlist: u64) -> ProgramResult {
        if ctx.accounts.sell_order.quantity < quantity_to_unlist {
            return Err(ErrorCode::ErrTryingToUnlistMoreThanOwned.into());
        }

        let seeds = &[
            "vault".as_bytes(),
            ctx.accounts.sell_order.mint.as_ref(),
            &[nounce], ];
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

    pub fn buy<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, Buy<'info>>,
        nounce: u8, ask_quantity: u64, max_price: u64,
    ) -> ProgramResult {
        let is_native = ctx.accounts.comptoir.mint.key() == spl_token::native_mint::id();
        let transfer_program = if is_native {
            ctx.accounts.system_program.to_account_info()
        } else {
            ctx.accounts.token_program.to_account_info()
        };

        let metadata = verify_metadata_and_derivation(
            ctx.accounts.mint_metadata.as_ref(),
            &ctx.accounts.buyer_nft_token_account.mint.key(),
            &ctx.accounts.collection,
        )?;
        let mut index = 0;

        //verify creators and use associated token account if mint isnt native
        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if let Some(creators) = metadata.data.creators {
            index = creators.len();
            let creators_distributions = verify_and_get_creators(creators, ctx.remaining_accounts, ctx.accounts.comptoir.mint);
            creators_distributions_option = Some(creators_distributions);
        }

        let mut comptoir_fee = ctx.accounts.comptoir.fees;
        if let Some(collection_share) = ctx.accounts.collection.fees {
            comptoir_fee = collection_share;
        }

        let seeds = &[
            "vault".as_bytes(),
            ctx.accounts.buyer_nft_token_account.mint.as_ref(),
            &[nounce], ];
        let signer = &[&seeds[..]];

        let mut remaining_to_buy = ask_quantity;

        while index < ctx.remaining_accounts.len() {
            let mut sell_order: Account<'info, SellOrder> = Account::<'info, SellOrder>::try_from(&ctx.remaining_accounts[index])?;
            index = index + 1;
            assert_eq!(sell_order.mint, ctx.accounts.buyer_nft_token_account.mint.key());

            if sell_order.price > max_price {
                return Err(ErrorCode::ErrItemPriceHigherThanMaxPrice.into());
            }

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
            let creators_share = calculate_fee(total_amount, metadata.data.seller_fee_basis_points, 10000);
            let comptoir_share = calculate_fee(total_amount, comptoir_fee, 100);
            let seller_share = total_amount.checked_sub(creators_share).unwrap().checked_sub(comptoir_share).unwrap();

            pay(
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                seller_token_account.to_account_info(),
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                transfer_program.to_account_info(),
                seller_share,
            )?;
            pay(
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                ctx.accounts.comptoir_dest_account.to_account_info(),
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                transfer_program.to_account_info(),
                comptoir_share,
            )?;

            if let Some(creators) = creators_distributions_option.as_ref() {
                for creator in creators {
                    let creator_share = calculate_fee(creators_share, creator.1 as u16, 100);
                    pay(
                        ctx.accounts.buyer_paying_token_account.to_account_info(),
                        creator.0.to_account_info(),
                        ctx.accounts.buyer_paying_token_account.to_account_info(),
                        transfer_program.to_account_info(),
                        creator_share,
                    )?;
                }
            }

            sell_order.quantity = sell_order.quantity - to_buy;
            sell_order.exit(ctx.program_id)?;

            remaining_to_buy = remaining_to_buy - to_buy;
            if remaining_to_buy == 0 {
                break;
            }
        }

        if remaining_to_buy != 0 {
            return Err(ErrorCode::ErrCouldNotBuyEnoughItem.into());
        }
        Ok(())
    }

    pub fn create_buy_offer(ctx: Context<CreateBuyOffer>, _nounce: u8, _buy_offer_nounce: u8, price_proposition: u64) -> ProgramResult {
        let is_native = ctx.accounts.comptoir.mint.key() == spl_token::native_mint::id();
        let transfer_program = if is_native {
            ctx.accounts.system_program.to_account_info()
        } else {
            ctx.accounts.token_program.to_account_info()
        };

        verify_metadata(
            ctx.accounts.mint_metadata.as_ref(),
            &ctx.accounts.collection,
        )?;

        let buy_offer = &mut ctx.accounts.buy_offer;
        buy_offer.metadata_key = ctx.accounts.mint_metadata.key();
        buy_offer.authority = ctx.accounts.payer.key();
        buy_offer.proposed_price = price_proposition;
        buy_offer.comptoir_key = ctx.accounts.comptoir.key();

        pay(
            ctx.accounts.buyer_paying_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.buyer_paying_account.to_account_info(),
            transfer_program.to_account_info(),
            price_proposition,
        )?;

        Ok(())
    }

    pub fn remove_buy_offer(ctx: Context<RemoveBuyOffer>, nounce: u8) -> ProgramResult {
        let is_native = ctx.accounts.comptoir.mint.key() == spl_token::native_mint::id();
        let transfer_program = if is_native {
            ctx.accounts.system_program.to_account_info()
        } else {
            ctx.accounts.token_program.to_account_info()
        };

        let seeds = &[
            PREFIX.as_bytes(),
            ctx.accounts.comptoir.to_account_info().key.as_ref(),
            ctx.accounts.comptoir.mint.as_ref(),
            ESCROW.as_bytes(),
            &[nounce], ];

        let signer: &[&[&[u8]]] = &[&seeds[..]];
        pay_with_signer(
            ctx.accounts.buyer_paying_account.to_account_info(),
            ctx.accounts.buyer_paying_account.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            transfer_program.to_account_info(),
            ctx.accounts.buy_offer.proposed_price,
            signer,
        )?;
        Ok(())
    }

    pub fn execute_offer<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, ExecuteOffer<'info>>, _nounce: u8) -> ProgramResult {
        let is_native = ctx.accounts.comptoir.mint.key() == spl_token::native_mint::id();

        let metadata = verify_metadata_and_derivation(
            ctx.accounts.mint_metadata.as_ref(),
            &ctx.accounts.seller_nft_account.mint.key(),
            &ctx.accounts.collection,
        )?;
        let mut index = 0;

        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if let Some(creators) = metadata.data.creators {
            index = creators.len();
            let creators_distributions = verify_and_get_creators(creators, ctx.remaining_accounts, ctx.accounts.comptoir.mint);
            creators_distributions_option = Some(creators_distributions);
        }

        let mut comptoir_fee = ctx.accounts.comptoir.fees;
        if let Some(collection_share) = ctx.accounts.collection.fees {
            comptoir_fee = collection_share;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(nounce: u8, buy_offer_nounce: u8, price_proposition: u64)]
pub struct CreateBuyOffer<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    mint_metadata: UncheckedAccount<'info>,

    comptoir: Account<'info, Comptoir>,
    #[account(mut, constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,
    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump = nounce,
    )]
    escrow: UncheckedAccount<'info>,

    #[account(mut, owner = payer.key())]
    buyer_paying_account: UncheckedAccount<'info>,
    #[account(
    init,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    payer.key.as_ref(),
    & price_proposition.to_ne_bytes(),
    mint_metadata.key.as_ref(),
    ],
    bump = buy_offer_nounce,
    payer = payer,
    )]
    buy_offer: Account<'info, BuyOffer>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(nounce: u8)]
pub struct RemoveBuyOffer<'info> {
    #[account(mut)]
    buyer: Signer<'info>,

    #[account(mut, owner = buyer.key())]
    buyer_paying_account: UncheckedAccount<'info>,

    comptoir: Account<'info, Comptoir>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump = nounce,
    )]
    escrow: UncheckedAccount<'info>,

    #[account(
    mut,
    close = buyer,
    constraint = buy_offer.authority == buyer.key(),
    constraint = buy_offer.comptoir_key == comptoir.key()
    )]
    buy_offer: Account<'info, BuyOffer>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(nounce: u8)]
pub struct ExecuteOffer<'info> {
    seller: Signer<'info>,

    #[account(mut)]
    buyer: SystemAccount<'info>,

    comptoir: Account<'info, Comptoir>,
    #[account(mut, constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,

    #[account(
    mut,
    seeds = [
    PREFIX.as_bytes(),
    comptoir.key().as_ref(),
    comptoir.mint.as_ref(),
    ESCROW.as_bytes()
    ],
    bump = nounce,
    )]
    escrow: UncheckedAccount<'info>,

    #[account(mut, owner = buyer.key())]
    buyer_paying_account: UncheckedAccount<'info>,
    seller_dest_account: UncheckedAccount<'info>,

    #[account(
    mut,
    close = buyer,
    constraint = buy_offer.authority == buyer.key(),
    )]
    buy_offer: Account<'info, BuyOffer>,

    #[account(mut)]
    seller_nft_account: Account<'info, TokenAccount>,
    #[account(mut, owner = buyer.key())]
    buyer_nft_account: Account<'info, TokenAccount>,

    mint_metadata: UncheckedAccount<'info>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(nounce: u8)]
pub struct CreateComptoir<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    #[account(
    init,
    seeds = [ payer.key.as_ref()],
    bump = nounce,
    payer = payer,
    )]
    comptoir: Account<'info, Comptoir>,

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
#[instruction(_nounce: u8, symbol: String)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,
    #[account(
    init,
    seeds = [
    symbol.as_bytes(),
    comptoir.key().as_ref(),
    ],
    bump = _nounce,
    payer = authority,
    space = 90,
    )]
    collection: Account<'info, Collection>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
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
#[instruction(_nounce: u8, _salt: String, _sell_order_nounce: u8)]
pub struct CreateSellOrder<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    #[account(mut)]
    seller_nft_token_account: Account<'info, TokenAccount>,

    comptoir: Account<'info, Comptoir>,
    mint: Account<'info, Mint>,

    #[account(
    init_if_needed,
    token::mint = mint,
    token::authority = vault,
    seeds = [
    "vault".as_bytes(),
    seller_nft_token_account.mint.as_ref(),
    ],
    bump = _nounce,
    payer = payer,
    )]
    vault: Account<'info, TokenAccount>,
    #[account(
    init,
    seeds = [
    _salt.as_bytes(),
    seller_nft_token_account.key().as_ref(),
    ],
    bump = _sell_order_nounce,
    payer = payer,
    )]
    sell_order: Account<'info, SellOrder>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(_nounce: u8)]
pub struct RemoveSellOrder<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    #[account(mut, constraint = authority.key() == seller_nft_token_account.owner)]
    seller_nft_token_account: Account<'info, TokenAccount>,
    #[account(mut, has_one = authority)]
    sell_order: Account<'info, SellOrder>,

    #[account(
    mut,
    seeds = [
    "vault".as_bytes(),
    seller_nft_token_account.mint.as_ref(),
    ],
    bump = _nounce,
    )]
    vault: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(_nounce: u8)]
pub struct Buy<'info> {
    buyer: Signer<'info>,
    #[account(mut)]
    buyer_nft_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    buyer_paying_token_account: UncheckedAccount<'info>,

    comptoir: Account<'info, Comptoir>,
    #[account(mut, constraint = comptoir_dest_account.key() == comptoir.fees_destination)]
    comptoir_dest_account: UncheckedAccount<'info>,
    #[account(constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,

    mint_metadata: UncheckedAccount<'info>,

    #[account(
    mut,
    seeds = [
    "vault".as_bytes(),
    buyer_nft_token_account.mint.as_ref()
    ],
    bump = _nounce,
    )]
    vault: Box<Account<'info, TokenAccount>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
pub struct Comptoir {
    fees: u16,
    fees_destination: Pubkey,
    authority: Pubkey,
    mint: Pubkey,
}

#[account]
#[derive(Default)]
pub struct SellOrder {
    price: u64,
    quantity: u64,
    mint: Pubkey,
    authority: Pubkey,
    destination: Pubkey,
}

#[account]
pub struct Collection {
    comptoir_key: Pubkey,
    symbol: String,
    // max size of 11
    required_verifier: Pubkey,
    fees: Option<u16>, //Takes priority over comptoir fees
}

#[account]
#[derive(Default)]
pub struct BuyOffer {
    comptoir_key: Pubkey,
    metadata_key: Pubkey,
    proposed_price: u64,
    authority: Pubkey,
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

    pub fn validate(&self) -> ProgramResult {
        if let Some(fee) = self.fees {
            if fee > 100 {
                return Err(ErrorCode::ErrFeeShouldLowerOrEqualThan100.into());
            }
        }
        Ok(())
    }
}

impl Comptoir {
    pub fn validate(&self) -> ProgramResult {
        if self.fees > 100 {
            return Err(ErrorCode::ErrFeeShouldLowerOrEqualThan100.into());
        }
        Ok(())
    }
}

fn verify_metadata_and_derivation(metadata_key: &AccountInfo, nft_mint: &Pubkey, collection: &Collection) -> core::result::Result<Metadata, ProgramError> {
    assert_derivation(
        &metaplex_token_metadata::id(),
        metadata_key,
        &[
            METAPLEX_PREFIX.as_bytes(),
            metaplex_token_metadata::id().as_ref(),
            nft_mint.as_ref(),
        ],
    )?;
    let metadata = Metadata::from_account_info(metadata_key)?;
    if !collection.is_part_of_collection(&metadata) {
        return Err(ErrorCode::ErrNftNotPartOfCollection.into());
    }
    return Ok(metadata);
}

fn verify_metadata(metadata_key: &AccountInfo, collection: &Collection) -> core::result::Result<Metadata, ProgramError> {
    assert_owned_by(metadata_key.as_ref(), &metaplex_token_metadata::id())?;

    let metadata: Metadata = Metadata::from_account_info(metadata_key)?;
    if !collection.is_part_of_collection(&metadata) {
        return Err(ErrorCode::ErrNftNotPartOfCollection.into());
    }
    return Ok(metadata);
}

fn assert_derivation_key(mint: Pubkey, metadata_key: Pubkey) -> ProgramResult {
    let path: &[&[u8]] = &[
        METAPLEX_PREFIX.as_bytes(),
        metaplex_token_metadata::ID.as_ref(),
        mint.as_ref(),
    ];
    let (key, _) = Pubkey::find_program_address(&path, &metaplex_token_metadata::id());
    if key != metadata_key {
        return Err(ErrorCode::DerivedKeyInvalid.into());
    }
    Ok(())
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


#[error]
pub enum ErrorCode {
    #[msg("Fee should be <= 100")]
    ErrFeeShouldLowerOrEqualThan100,
    #[msg("Trying to unlist more than owned")]
    ErrTryingToUnlistMoreThanOwned,
    #[msg("Item price got higher than max price")]
    ErrItemPriceHigherThanMaxPrice,
    #[msg("Could not buy the required quantity of items")]
    ErrCouldNotBuyEnoughItem,
    #[msg("Trying to buy item with mint but only accepts sol")]
    ErrComptoirDoesNotHaveMint,
    #[msg("Sol is not the right currency for this item")]
    ErrComptoirDoesNotAcceptSol,
    #[msg("metadata mint does not match item mint")]
    ErrMetaDataMintDoesNotMatchItemMint,
    #[msg("nft not part of collection")]
    ErrNftNotPartOfCollection,
    #[msg("Derived key invalid")]
    DerivedKeyInvalid,
    #[msg("Wrong transfer program")]
    ErrWrongTransferProgram,
}