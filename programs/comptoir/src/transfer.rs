use anchor_lang::prelude::{AccountInfo, CpiContext, Result};
use anchor_spl::token;
use anchor_spl::token::Transfer;


pub fn pay<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let cpi_accounts = Transfer {
        from: payer,
        to: dest,
        authority,
    };
    let cpi_ctx = CpiContext::new(token_program, cpi_accounts);
    token::transfer(cpi_ctx, amount)
}

pub fn pay_with_signer<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
    signer: &[&[&[u8]]]
) -> Result<()> {
    let cpi_accounts = Transfer {
        from: payer,
        to: dest,
        authority,
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, amount)
}