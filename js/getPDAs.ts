import * as anchor from '@project-serum/anchor';
import { COMPTOIR_PROGRAM_ID } from './constant';
import { PublicKey } from '@solana/web3.js';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  Token,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';

export const getComptoirPDA = async (owner: PublicKey, programID?: PublicKey): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from('COMPTOIR'), owner.toBuffer()],
      programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};

export const getEscrowPDA = async (
  comptoirPDA: PublicKey,
  comptoirMint: PublicKey,
  programID?: PublicKey
): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from('COMPTOIR'),
        comptoirPDA.toBuffer(),
        comptoirMint.toBuffer(),
        Buffer.from('ESCROW'),
      ],
        programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};

export const getCollectionPDA = async (
  comptoirPDA: PublicKey,
  name: string,
  programID?: PublicKey
): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from('COMPTOIR'), Buffer.from(name), comptoirPDA.toBuffer()],
        programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};

export const getNftVaultPDA = async (
  nftMint: PublicKey,
  programID?: PublicKey
): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from('COMPTOIR'), Buffer.from('vault'), nftMint.toBuffer()],
        programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};

export const getSellOrderPDA = async (
  sellerTokenAccount: PublicKey,
  price: anchor.BN,
  programID?: PublicKey
): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from('COMPTOIR'),
        sellerTokenAccount.toBuffer(),
        Buffer.from(price.toString()),
      ],
        programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};

export const getAssociatedTokenAddress = async (
  addr: PublicKey,
  mint: PublicKey,
): Promise<PublicKey> => {
  return await Token.getAssociatedTokenAddress(
    ASSOCIATED_TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
    mint,
    addr,
    false
  );
};
export const getBuyOfferPDA = async (
  comptoirPDA: PublicKey,
  buyer: PublicKey,
  mint: PublicKey,
  price: anchor.BN,
  programID?: PublicKey
): Promise<PublicKey> => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from('COMPTOIR'),
        comptoirPDA.toBuffer(),
        buyer.toBuffer(),
        mint.toBuffer(),
        Buffer.from(price.toString()),
        Buffer.from('ESCROW'),
      ],
        programID ? programID : COMPTOIR_PROGRAM_ID
    )
  )[0];
};
