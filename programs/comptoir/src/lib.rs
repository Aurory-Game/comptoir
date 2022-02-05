use anchor_lang::solana_program::system_instruction::transfer;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use anchor_lang::solana_program::program::invoke;
use metaplex_token_metadata::state::{Metadata, PREFIX};
use std::str::FromStr;
use metaplex_token_metadata::utils::assert_derivation;
use crate::constant::{ASSOCIATED_TOKEN_PROGRAM};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod comptoir {
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

        let metadata = verify_metadata(
            ctx.accounts.mint_metadata.as_ref(),
            &ctx.accounts.buyer_nft_token_account.mint.key(),
            &ctx.accounts.collection,
        )?;
        let mut index = 0;

        //verify creators and use associated token account if mint isnt native
        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if let Some(creators) = metadata.data.creators {
            index = creators.len();
            let mut creators_distributions = Vec::new();
            for i in 0..creators.len() {
                let remaining_account_creator = &ctx.remaining_accounts[i];
                if is_native {
                    assert_eq!(remaining_account_creator.key(), creators[i].address);
                    creators_distributions.push((remaining_account_creator, creators[i].share));
                } else {
                    let ata_seeds = &[
                        creators[i].address.as_ref(),
                        ctx.accounts.token_program.key.as_ref(),
                        ctx.accounts.comptoir.mint.as_ref(),
                    ];
                    let atp = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
                    let creator_associated_token_addr = Pubkey::find_program_address(ata_seeds, &atp);
                    assert_eq!(remaining_account_creator.key(), creator_associated_token_addr.0);
                    creators_distributions.push((remaining_account_creator, creators[i].share));
                }
            }
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
            assert_eq!(sell_order.mint,  ctx.accounts.buyer_nft_token_account.mint.key());

            if sell_order.price > max_price {
                return Err(ErrorCode::ErrItemPriceHigherThanMaxPrice.into());
            }

            let mut to_buy = remaining_to_buy;
            if sell_order.quantity < to_buy {
                to_buy = sell_order.quantity;
            }

            //Transfer Item from vault to buyer
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.buyer_nft_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            };

            let cpi_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
            token::transfer(cpi_ctx, to_buy)?;

            let seller_token_account = &ctx.remaining_accounts[index];
            index = index + 1;
            assert_eq!(seller_token_account.key(), sell_order.destination);
            let total_amount = sell_order.price.checked_mul(to_buy).unwrap();
            let creators_share = calculate_fee(total_amount, metadata.data.seller_fee_basis_points, 10000);
            let comptoir_share = calculate_fee(total_amount, comptoir_fee, 100);
            let seller_share = total_amount.checked_sub(creators_share).unwrap().checked_sub(comptoir_share).unwrap();

            if is_native {
                pay_native(
                    ctx.accounts.buyer_paying_token_account.to_account_info(),
                    seller_token_account.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    seller_share,
                )?;

                pay_native(
                    ctx.accounts.buyer_paying_token_account.to_account_info(),
                    ctx.accounts.comptoir_dest_account.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    comptoir_share,
                )?;
            } else {
                pay_spl(
                    ctx.accounts.buyer_paying_token_account.to_account_info(),
                    seller_token_account.to_account_info(),
                    ctx.accounts.buyer.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    seller_share,
                )?;

                pay_spl(
                    ctx.accounts.buyer_paying_token_account.to_account_info(),
                    ctx.accounts.comptoir_dest_account.to_account_info(),
                    ctx.accounts.buyer.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    comptoir_share,
                )?;
            }

            if let Some(creators) = creators_distributions_option.as_ref(){
                for creator in creators {
                    let creator_share = calculate_fee(creators_share, creator.1 as u16, 100);
                    if is_native {
                        pay_native(
                            ctx.accounts.buyer_paying_token_account.to_account_info(),
                            creator.0.to_account_info(),
                            ctx.accounts.system_program.to_account_info(),
                            creator_share,
                        )?;
                    } else {
                        pay_spl(
                            ctx.accounts.buyer_paying_token_account.to_account_info(),
                            creator.0.to_account_info(),
                            ctx.accounts.buyer.to_account_info(),
                            ctx.accounts.token_program.to_account_info(),
                            creator_share,
                        )?;
                    }
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
}


fn pay_native<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    amount: u64,
) -> ProgramResult {
    let transfer_instruction = transfer(payer.key, dest.key, amount);
    invoke(&transfer_instruction, &[payer, dest, system_program])
}

fn pay_spl<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
) -> ProgramResult {
    let cpi_accounts = Transfer {
        from: payer,
        to: dest,
        authority,
    };
    let cpi_ctx = CpiContext::new(token_program, cpi_accounts);
    token::transfer(cpi_ctx, amount)
}

fn calculate_fee(amount: u64, fee_share: u16, basis: u64) -> u64 {
    let fee = amount
        .checked_mul(fee_share as u64)
        .unwrap()
        .checked_div(basis)
        .unwrap();

    return fee;
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

    mint_metadata: AccountInfo<'info>,

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
    symbol: String, // max size of 11
    required_verifier: Pubkey,
    fees: Option<u16>, //Takes priority over comptoir fees
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
}

fn verify_metadata(metadata_key: &AccountInfo, nft_mint: &Pubkey, collection: &Collection) -> Result<Metadata> {
    assert_derivation(
        &metaplex_token_metadata::id(),
        metadata_key,
        &[
            PREFIX.as_bytes(),
            metaplex_token_metadata::id().as_ref(),
            nft_mint.as_ref(),
        ],
    )?;
    let metadata = Metadata::from_account_info(metadata_key)?;
    if !collection.is_part_of_collection(&metadata) {
        return Err(ErrorCode::ErrNftNotPartOfCollection.into());
    }
    return Ok(metadata)
}


pub mod constant {
    pub const TOKEN_METADATA_PROGRAM: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
    pub const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
}