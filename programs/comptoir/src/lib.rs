use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod comptoir {
    use anchor_lang::AccountsClose;
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

    pub fn unlist_item(ctx: Context<UnlistItem>, nounce: u8, quantity: u64) -> ProgramResult {
        if ctx.accounts.item.quantity < quantity {
            return Err(ErrorCode::ErrTryingToUnlistMoreThanOwned.into());
        }

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
        token::transfer(cpi_ctx, quantity)?;

        ctx.accounts.item.close(ctx.accounts.authority.to_account_info())?;
        Ok(())
    }

    #[access_control(check_comptoir_has_mint(& ctx.accounts.comptoir))]
    pub fn buy_item_with_mint(ctx: Context<BuyItemWithMint>, nounce: u8, ask_quantity: u64) -> ProgramResult {
        let mut used_quantity = ask_quantity;
        if ask_quantity > ctx.accounts.item.quantity {
            used_quantity = ctx.accounts.item.quantity;
        }

        let seeds = &[
            "vaut".as_bytes(),
            ctx.accounts.item.mint.as_ref(),
            &[nounce], ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.buyer_nft_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
        token::transfer(cpi_ctx, used_quantity)?;

        let cpi_accounts = Transfer {
            from: ctx.accounts.buyer_paying_token_account.to_account_info(),
            to: ctx.accounts.destination_token_account.to_account_info(),
            authority: ctx.accounts.buyer.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);

        let amount = ctx.accounts.item.price.checked_mul(used_quantity).unwrap();
        token::transfer(cpi_ctx, amount)?;

        if used_quantity == ctx.accounts.item.quantity {
            ctx.accounts.item.close(ctx.accounts.authority.to_account_info())?;
        } else {
            let item = &mut ctx.accounts.item;
            item.quantity = item.quantity - used_quantity;
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateComptoir<'info> {
    payer: Signer<'info>,
    #[account(
    init,
    payer = payer,
    space = 103,
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
    #[account(init, payer = payer, space = 212)]
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
#[instruction(_nounce: u8)]
pub struct BuyItemWithMint<'info> {
    buyer: Signer<'info>,
    #[account(owner = buyer.key())] //are they even useful since the transfer would make the transaction fail ??
    buyer_nft_token_account: Account<'info, TokenAccount>,
    #[account(owner = buyer.key())]
    buyer_paying_token_account: Account<'info, TokenAccount>,

    #[account(address = item.destination)]
    destination_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    authority: AccountInfo<'info>,

    #[account(mut, has_one = authority)]
    item: Account<'info, Item>,

    #[account(mut, address = item.comptoir_key)]
    comptoir: Account<'info, Comptoir>,

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

#[account]
pub struct Comptoir {
    fees: u8,
    fees_destination: Pubkey,
    authority: Pubkey,
    mint: Option<Pubkey>,
}

#[account]
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