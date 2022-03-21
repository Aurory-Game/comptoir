import * as anchor from "@project-serum/anchor";
import {Comptoir as ComptoirDefinition} from './types/comptoir';
import {COMPTOIR_PROGRAM_ID} from './constant'
import * as idl from './types/comptoir.json';

import {Keypair, PublicKey} from "@solana/web3.js";
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getCollectionPDA, getComptoirPDA, getEscrowPDA} from "./getPDAs";

export class Comptoir {
    program: anchor.Program<ComptoirDefinition>
    comptoirPDA: PublicKey
    comptoir: Comptoir

    constructor(provider: anchor.Provider, comptoirPDA?: PublicKey) {
        // @ts-ignore
        this.program = new anchor.Program(idl, COMPTOIR_PROGRAM_ID, provider,)
        this.comptoirPDA = comptoirPDA;
    }

    async createComptoir(
        owner: PublicKey,
        mint: PublicKey,
        fees: number,
        feesDestination: PublicKey,
        signers?: Keypair[]
    ): Promise<string> {
        let [comptoirPDA, comptoirNounce] = await getComptoirPDA(owner)

        let [escrowPDA, escrowNounce] = await getEscrowPDA(comptoirPDA, mint)

        this.comptoirPDA = comptoirPDA;

        return await this.program.rpc.createComptoir(
            comptoirNounce, escrowNounce, mint, fees, feesDestination, anchor.Wallet.local().payer.publicKey, {
                accounts: {
                    payer: anchor.Wallet.local().payer.publicKey,
                    comptoir: comptoirPDA,
                    mint: mint,
                    escrow: escrowPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: signers,
            });
    }

    async createCollection(
        name: string,
        required_metadata_signer: PublicKey,
        collection_symbol: string,
        fee?: number,
        signers?: Keypair[]
    ): Promise<string>  {
        let [collectionPDA, collectionNounce] = await getCollectionPDA(this.comptoirPDA, collection_symbol)
        if (!fee) {
            fee = null
        }
        return await this.program.rpc.createCollection(
            collectionNounce, collection_symbol, required_metadata_signer, fee, {
                accounts: {
                    authority: anchor.Wallet.local().publicKey,
                    comptoir: this.comptoirPDA,
                    collection: collectionPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: signers,
            });
    }
}
