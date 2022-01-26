use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;
use metaplex_token_metadata::state::{Metadata, PREFIX};
use crate::constant::TOKEN_METADATA_PROGRAM;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod comptoir {
    use super::*;

    pub fn create_comptoir(
        ctx: Context<CreateComptoir>,
        _nounce: u8,fees: u16, fees_destination: Pubkey, authority: Pubkey, mint: Pubkey,
    ) -> ProgramResult {
        if fees > 100 {
            return Err(ErrorCode::ErrFeeShouldLowerOrEqualThan100.into());
        }
        let comptoir = &mut ctx.accounts.comptoir;

        comptoir.fees = fees;
        comptoir.fees_destination = fees_destination;
        comptoir.authority = authority;
        comptoir.mint = mint;

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

        Ok(())
    }


    pub fn create_collection(
        ctx: Context<CreateCollection>,
        _nounce: u8, name: String, required_verifier: Pubkey, fee_share: Option<u16>,
    ) -> ProgramResult {
        let collection = &mut ctx.accounts.collection;

        collection.comptoir_key = ctx.accounts.comptoir.key();
        collection.required_verifier = required_verifier;
        collection.symbol = name;
        collection.fee_share = fee_share;

        Ok(())
    }

    pub fn update_collection(
        ctx: Context<UpdateCollection>,
        optional_fee_share: Option<u16>,
        optional_name: Option<String>,
        optional_required_verifier: Option<Pubkey>,
    ) -> ProgramResult {
        let collection = &mut ctx.accounts.collection;

        if let Some(fee_share) = optional_fee_share {
            collection.fee_share = Some(fee_share);
        }
        if let Some(name) = optional_name {
            collection.symbol = name;
        }
        if let Some(required_verifier) = optional_required_verifier {
            collection.required_verifier = required_verifier;
        }

        Ok(())
    }

    pub fn list_item(ctx: Context<ListItem>, price: u64, quantity: u64, destination: Pubkey) -> ProgramResult {
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

    pub fn unlist_item(ctx: Context<UnlistItem>, nounce: u8, quantity_to_unlist: u64) -> ProgramResult {
        if ctx.accounts.sell_order.quantity < quantity_to_unlist {
            return Err(ErrorCode::ErrTryingToUnlistMoreThanOwned.into());
        }

        if quantity_to_unlist > 0 {
            let seeds = &[
                "vaut".as_bytes(),
                ctx.accounts.sell_order.mint.as_ref(),
                &[nounce], ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.seller_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            };

            let cpi_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
            token::transfer(cpi_ctx, quantity_to_unlist)?;
        }

        if ctx.accounts.sell_order.quantity == 0 {
            ctx.accounts.sell_order.close(ctx.accounts.authority.to_account_info())?;
        }
        Ok(())
    }

    pub fn buy_item_with_mint<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, BuyItemWithMint<'info>>,
        nounce: u8, mint: Pubkey, ask_quantity: u64,  max_price: u64
    ) -> ProgramResult {
        verify_metadata_mint(ctx.accounts.mint_metadata.key(), mint)?;
        let metadata = Metadata::from_account_info(ctx.accounts.mint_metadata.as_ref())?;
        ctx.accounts.collection.is_part_of_collection(&metadata);

        let account_iter = &mut ctx.remaining_accounts.iter();

        let mut creators_distributions_option: Option<Vec<(&AccountInfo, u8)>> = None;
        if let Some(creators) = metadata.data.creators {
            let mut creators_distributions = Vec::with_capacity(creators.len());

            for i in 0..creators.len() {
                let creator = next_account_info(account_iter)?;
                creators_distributions[i] = (creator, creators[i].share);
            }

            creators_distributions_option = Some(creators_distributions);
        }

        let mut comptoir_fee = ctx.accounts.comptoir.fees;
        if let Some(collection_share) = ctx.accounts.collection.fee_share {
            comptoir_fee = collection_share;
        }

        let seeds = &[
            "vaut".as_bytes(),
            mint.as_ref(),
            &[nounce], ];
        let signer = &[&seeds[..]];

        let mut remaining_to_buy = ask_quantity;
        for account in &mut ctx.remaining_accounts.iter() {
            let mut sell_order: Account<'info, SellOrder> = Account::<'info, SellOrder>::try_from(account)?;
            assert_eq!(sell_order.mint, mint);

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

            let seller_token_account = next_account_info(account_iter)?;
            assert_eq!(seller_token_account.key(), sell_order.destination);
            let total_amount = sell_order.price.checked_mul(to_buy).unwrap();
            let creators_share = calculate_fee(total_amount, metadata.data.seller_fee_basis_points, 10000);
            let comptoir_share = calculate_fee(total_amount, comptoir_fee, 10000);
            let seller_share = total_amount.checked_sub(creators_share).unwrap().checked_sub(comptoir_share).unwrap();

            transfer_if_not_zero(
              ctx.accounts.buyer_paying_token_account.to_account_info(),
              seller_token_account.to_account_info(),
              ctx.accounts.buyer.to_account_info(),
              ctx.accounts.token_program.to_account_info(),
              seller_share,
            )?;
            transfer_if_not_zero(
                ctx.accounts.buyer_paying_token_account.to_account_info(),
                ctx.accounts.comptoir_paying_token_account.to_account_info(),
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                comptoir_share,
            )?;
            if let Some(creators) = creators_distributions_option.as_ref() {
                for creator in creators {
                    let creator_share = calculate_fee(creators_share, creator.1 as u16, 100);

                    transfer_if_not_zero(
                        ctx.accounts.buyer_paying_token_account.to_account_info(),
                        creator.0.to_account_info(),
                        ctx.accounts.buyer.to_account_info(),
                        ctx.accounts.token_program.to_account_info(),
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
}

fn transfer_if_not_zero<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(())
    }
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
    rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
pub struct UpdateComptoir<'info> {
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,
}

#[derive(Accounts)]
#[instruction(_nounce: u8, name: String)]
pub struct CreateCollection<'info> {
    authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    comptoir: Account<'info, Comptoir>,
    #[account(
    init,
    seeds = [
    name.as_bytes(),
    comptoir.key().as_ref(),
    ],
    bump = _nounce,
    payer = authority,
    )]
    collection: Account<'info, Collection>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
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
#[instruction(price: u64, quantity: u64, _nounce: u8)]
pub struct ListItem<'info> {
    payer: Signer<'info>,
    #[account(owner = seller_nft_token_account.key())]
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
    #[account(init, payer = payer)]
    sell_order: Account<'info, SellOrder>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
#[instruction(_nounce: u8)]
pub struct UnlistItem<'info> {
    authority: Signer<'info>,
    #[account(constraint = authority.key() == seller_token_account.owner)]
    seller_token_account: Account<'info, TokenAccount>,
    #[account(mut, has_one = authority)]
    sell_order: Account<'info, SellOrder>,

    #[account(
    seeds = [
    "vault".as_bytes(),
    sell_order.mint.as_ref()
    ],
    bump = _nounce,
    )]
    vault: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
#[instruction(_nounce: u8, mint: Pubkey)]
pub struct BuyItemWithMint<'info> {
    buyer: Signer<'info>,
    #[account(owner = buyer.key())] //are they even useful since the transfer would make the transaction fail ??
    buyer_nft_token_account: Account<'info, TokenAccount>,
    #[account(owner = buyer.key())]
    buyer_paying_token_account: Account<'info, TokenAccount>,

    comptoir: Account<'info, Comptoir>,
    #[account(constraint = comptoir_paying_token_account.key() == comptoir.fees_destination)]
    comptoir_paying_token_account: Account<'info, TokenAccount>,
    #[account(constraint = collection.comptoir_key == comptoir.key())]
    collection: Account<'info, Collection>,

    mint_metadata: AccountInfo<'info>,

    #[account(
    seeds = [
    "vault".as_bytes(),
    mint.as_ref()
    ],
    bump = _nounce,
    )]
    vault: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
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
#[derive(Default)]
pub struct Collection {
    comptoir_key: Pubkey,
    symbol: String,
    required_verifier: Pubkey,
    fee_share: Option<u16>, //Takes priority over comptoir fees
}

impl Collection {
    pub fn is_part_of_collection(&self, metadata: &Metadata) -> bool {
        return if let Some(creators) = metadata.data.creators.as_ref() {
            metadata.data.symbol == self.symbol && creators.iter().any(|c| c.address == self.required_verifier && c.verified)
        } else {
            false
        }
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
}

fn verify_metadata_mint(user_input_metadata_key: Pubkey, item_mint: Pubkey) -> Result<()> {
    let metadata_seeds = &[
        PREFIX.as_bytes(),
        TOKEN_METADATA_PROGRAM,
        item_mint.as_ref(),
    ];
    let (metadata_key, _bump_seed) = Pubkey::find_program_address(metadata_seeds, &Pubkey::new(TOKEN_METADATA_PROGRAM));
    if user_input_metadata_key != metadata_key {
        return Err(ErrorCode::ErrMetaDataMintDoesNotMatchItemMint.into());
    }

    return Ok(())
}

pub mod constant {
    pub const TOKEN_METADATA_PROGRAM: &[u8] = b"metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
}