use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;
use anchor_lang::AccountsClose;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod comptoir {
    use super::*;

    pub fn create_comptoir(
        ctx: Context<CreateComptoir>,
        fees: u8, fees_destination: Pubkey, authority: Pubkey, mint: Option<Pubkey>,
    ) -> ProgramResult {
        let comptoir = &mut ctx.accounts.comptoir;

        comptoir.fees = fees;
        comptoir.fees_destination = fees_destination;
        comptoir.authority = authority;
        comptoir.mint = mint;

        Ok(())
    }

    pub fn update_comptoir(
        ctx: Context<UpdateComptoir>,
        optional_fees: Option<u8>,
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
            comptoir.mint = Some(mint);
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

        let item = &mut ctx.accounts.item;
        item.price = price;
        item.quantity = quantity;
        item.mint = ctx.accounts.seller_nft_token_account.mint;
        item.authority = ctx.accounts.payer.key();
        item.comptoir_key = ctx.accounts.comptoir.to_account_info().key();
        item.destination = destination;
        Ok(())
    }

    pub fn unlist_item(ctx: Context<UnlistItem>, nounce: u8, quantity_to_unlist: u64) -> ProgramResult {
        if ctx.accounts.item.quantity < quantity_to_unlist {
            return Err(ErrorCode::ErrTryingToUnlistMoreThanOwned.into());
        }

        if quantity_to_unlist > 0 {
            let seeds = &[
                "vaut".as_bytes(),
                ctx.accounts.item.mint.as_ref(),
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

        if ctx.accounts.item.quantity == 0 {
            ctx.accounts.item.close(ctx.accounts.authority.to_account_info())?;
        }
        Ok(())
    }

    #[access_control(check_comptoir_has_mint(& ctx.accounts.comptoir))]
    pub fn buy_item_with_mint<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, BuyItemWithMint<'info>>,
                                                 nounce: u8, mint: Pubkey, ask_quantity: u64,  max_price: u64
    ) -> ProgramResult {
        let account_iter = &mut ctx.remaining_accounts.iter();
        let mut remaining_to_buy = ask_quantity;

        let seeds = &[
            "vaut".as_bytes(),
            mint.as_ref(),
            &[nounce], ];
        let signer = &[&seeds[..]];

        for account in &mut ctx.remaining_accounts.iter() {
            let mut item: Account<'info, Item> = Account::<'info, Item>::try_from(account)?;
            assert_eq!(ctx.accounts.comptoir.key(), item.comptoir_key);

            if item.price > max_price {
               return Err(ErrorCode::ErrItemPriceHigherThanMaxPrice.into());
            }

            let mut to_buy = remaining_to_buy;
            if item.quantity < to_buy {
                to_buy = item.quantity;
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
            assert_eq!(seller_token_account.key(), item.destination);
            let amount = item.price.checked_mul(to_buy).unwrap();

            //Pay the seller
            transfer(
              ctx.accounts.buyer_paying_token_account.to_account_info(),
              seller_token_account.to_account_info(),
              ctx.accounts.buyer.to_account_info(),
              ctx.accounts.token_program.to_account_info(),
                amount,
            )?;
            item.quantity = item.quantity - to_buy;
            item.exit(ctx.program_id)?;

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

fn transfer<'info>(
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

#[derive(Accounts)]
pub struct CreateComptoir<'info> {
    payer: Signer<'info>,
    #[account(
    init,
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
    item: Account<'info, Item>,

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
    item: Account<'info, Item>,

    #[account(
    seeds = [
    "vault".as_bytes(),
    item.mint.as_ref()
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
    fees: u8,
    fees_destination: Pubkey,
    authority: Pubkey,
    mint: Option<Pubkey>,
}

#[account]
#[repr(C)]
#[derive(Default)]
pub struct Item {
    comptoir_key: Pubkey,
    price: u64,
    quantity: u64,
    mint: Pubkey,
    authority: Pubkey,
    destination: Pubkey,
}

#[error]
pub enum ErrorCode {
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
}

fn check_comptoir_has_mint(comptoir: &Comptoir) -> Result<()> {
    if let Some(_mint) = comptoir.mint {
        return Ok(())
    }

    return Err(ErrorCode::ErrComptoirDoesNotAcceptSol.into());
}