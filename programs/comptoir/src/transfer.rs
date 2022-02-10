use anchor_lang::Key;
use anchor_lang::prelude::{AccountInfo, CpiContext, ProgramResult};
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::system_instruction::transfer;
use anchor_spl::token;
use anchor_spl::token::Transfer;
use spl_token::solana_program::program::invoke_signed;
use crate::ErrorCode::ErrWrongTransferProgram;


pub fn pay<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    program: AccountInfo<'info>,
    amount: u64,
) -> ProgramResult {
    if program.key() == anchor_lang::solana_program::system_program::ID {
        pay_native(payer, dest, program, amount)?
    } else if program.key() == spl_token::ID {
        pay_spl(payer, dest, authority, program, amount)?
    }
    return Err(ErrWrongTransferProgram.into())
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


pub fn pay_with_signer<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    program: AccountInfo<'info>,
    amount: u64,
    signer: &[&[&[u8]]]
) -> ProgramResult {
    if program.key() == anchor_lang::solana_program::system_program::ID {
        pay_native_with_signer(payer, dest, program, amount, signer)?
    } else if program.key() == spl_token::ID {
        pay_spl_with_signer(payer, dest, authority, program, amount, signer)?
    }
    return Err(ErrWrongTransferProgram.into())
}

fn pay_native_with_signer<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    amount: u64,
    signer: &[&[&[u8]]]
) -> ProgramResult {
    let transfer_instruction = transfer(payer.key, dest.key, amount);
    invoke_signed(&transfer_instruction, &[payer, dest, system_program], signer)
}

fn pay_spl_with_signer<'info>(
    payer: AccountInfo<'info>,
    dest: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
    signer: &[&[&[u8]]]
) -> ProgramResult {
    let cpi_accounts = Transfer {
        from: payer,
        to: dest,
        authority,
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, amount)
}