import * as anchor from "@project-serum/anchor";
import {COMPTOIR_PROGRAM_ID} from "./constant";
import {PublicKey} from "@solana/web3.js";
import {ASSOCIATED_TOKEN_PROGRAM_ID, Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";


export const getComptoirPDA = async (owner): Promise<[PublicKey, number]> => {
    return await anchor.web3.PublicKey.findProgramAddress(
        [
            Buffer.from("COMPTOIR"),
            owner.publicKey.toBuffer()
        ],
        COMPTOIR_PROGRAM_ID,
    )
}

export const getEscrowPDA = async (comptoirPDA: PublicKey, comptoirMint: PublicKey): Promise<[PublicKey, number]> => {
    return await anchor.web3.PublicKey.findProgramAddress(
        [
            Buffer.from("COMPTOIR"),
            comptoirPDA.toBuffer(),
            comptoirMint.toBuffer(),
            Buffer.from("ESCROW"),
        ],
        COMPTOIR_PROGRAM_ID,
    );
}

export const getCollectionPDA = async (comptoirPDA: PublicKey, name: string): Promise<[PublicKey, number]> => {
    return await anchor.web3.PublicKey.findProgramAddress(
        [
            Buffer.from("COMPTOIR"),
            Buffer.from(name),
            comptoirPDA.toBuffer(),
        ],
        COMPTOIR_PROGRAM_ID,
    );
}

export const getNftVaultPDA = async (nftMint: PublicKey): Promise<[PublicKey, number]> => {
    return await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("vault"), nftMint.toBuffer()],
        COMPTOIR_PROGRAM_ID,
    );
}

export const getAssociatedTokenAddress = async (addr: PublicKey, mint: PublicKey): Promise<PublicKey> => {
    return await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint,
        addr,
        false,
    )
}