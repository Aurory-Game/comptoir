import * as anchor from "@project-serum/anchor";
import {Comptoir as ComptoirDefinition, IDL} from './types/comptoir';
import {COMPTOIR_PROGRAM_ID} from './constant'
import {PublicKey} from "@solana/web3.js";
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getCollectionPDA, getComptoirPDA, getEscrowPDA} from "./getPDAs";

export class Comptoir {
    program: anchor.Program<ComptoirDefinition>
    comptoirPDA: PublicKey
    comptoir: Comptoir


    constructor(provider: anchor.Provider, comptoirPDA?: PublicKey) {
        this.program = new anchor.Program<ComptoirDefinition>(
            IDL,
            COMPTOIR_PROGRAM_ID,
            provider,
        )
        this.comptoirPDA = comptoirPDA;
    }

    async createComptoir(
        owner: PublicKey,
        mint: PublicKey,
        fees: number,
        feesDestination: PublicKey,
    ): Promise<string> {
        let [comptoirPDA, comptoirNounce] = await getComptoirPDA(owner)
        let [escrowPDA, escrowNounce] = await getEscrowPDA(comptoirPDA, mint)

        return await this.program.rpc.createComptoir(
            comptoirNounce, escrowNounce, mint, fees, feesDestination, anchor.Wallet.payer.publicKey, {
                accounts: {
                    payer: anchor.Wallet.payer.publicKey,
                    comptoir: comptoirPDA,
                    mint: mint,
                    escrow: escrowPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
            });
    }

    async createCollection(
        name: string,
        fee?: number
    ): Promise<string>  {
        let [collectionPDA, collectionNounce] = await getCollectionPDA(this.comptoirPDA, name)
        return await this.program.rpc.createCollection(
            collectionNounce, collectionPDA, anchor.Wallet.payer.publicKey, fee, {
                accounts: {
                    authority: anchor.Wallet.payer.publicKey,
                    comptoir: this.comptoirPDA,
                    collection: collectionPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
            });
    }


}
