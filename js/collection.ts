import * as anchor from "@project-serum/anchor";
import {Comptoir as ComptoirDefinition, IDL} from './types/comptoir';
import {COMPTOIR_PROGRAM_ID} from './constant'
import {PublicKey} from "@solana/web3.js";
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getAssociatedTokenAddress, getNftVaultPDA} from "./getPDAs";
import {getMetadataData} from "./metaplex";
import {Program} from "@project-serum/anchor";


export class Collection {
    program: anchor.Program<ComptoirDefinition>
    comptoirPDA: PublicKey
    collectionPDA: PublicKey
    constructor(
        provider: anchor.Provider,
        comptoirPDA: PublicKey,
        collectionPDA: PublicKey,
    ) {
        this.program = new anchor.Program<ComptoirDefinition>(
            IDL,
            COMPTOIR_PROGRAM_ID,
            provider,
        )

        this.comptoirPDA = comptoirPDA;
        this.collectionPDA = collectionPDA;
    }

    async sellAsset(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellerDestination: PublicKey,
        price: anchor.BN,
        amount: anchor.BN
    ): Promise<string> {
        let [programNftVaultPDA, programNftVaultDump] = await getNftVaultPDA(nftMint)
        let salt = (new Date()).getTime().toString()
        let [sellOrderPDA, sellOrderDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(salt), sellerNftAccount.toBuffer()],
            this.program.programId,
        );

        let metadataPDA = await getMetadataData(anchor.getProvider().connection, nftMint)
        return await this.program.rpc.createSellOrder(
            programNftVaultDump, salt, sellOrderDump, price, amount, sellerDestination, {
                accounts: {
                    payer: anchor.Wallet.payer.publicKey,
                    sellerNftTokenAccount: sellerNftAccount,
                    comptoir: this.comptoirPDA,
                    collection: this.collectionPDA,
                    mint: nftMint,
                    metadata: metadataPDA,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
            }
        );
    }

    async removeSellOrder(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellOrderPDA: PublicKey,
        amount: anchor.BN
    ): Promise<string> {
        let [programNftVaultPDA, programNftVaultDump] = await getNftVaultPDA(nftMint)
        return await this.program.rpc.removeSellOrder(
            programNftVaultDump, amount, {
                accounts: {
                    authority: anchor.Wallet.payer.publicKey,
                    sellerNftTokenAccount: sellerNftAccount,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
            }
        );
    }

    async buy(
        nftMint: PublicKey,
        sellOrderPDA: PublicKey,
        sellerDestinationAccount: PublicKey,
        buyerNftAccount: PublicKey,
        buyerPayingAccount: PublicKey,
        wanted_quantity: anchor.BN,
        max_price: anchor.BN,
    ) : Promise<string> {
        let [programNftVaultPDA, programNftVaultDump] = await getNftVaultPDA(nftMint)

        let comptoirAccount = await this.program.account.comptoir.fetch(this.comptoirPDA)

        let metadata = await getMetadataData(
                anchor.getProvider().connection,
                nftMint,
        )

        let creatorsAccounts = []
        for (let creator of metadata.data.data.creators) {
            let creatorAddress = new PublicKey(creator.address)
            let creatorATA = await getAssociatedTokenAddress(creatorAddress, comptoirAccount.mint)

            creatorsAccounts.push (
                { pubkey: creatorATA, isWritable: true, isSigner: false },
            )
        }

        return await this.program.rpc.buy(
            programNftVaultDump, wanted_quantity, max_price, {
                accounts: {
                    buyer: anchor.Wallet.payer.publicKey,
                    buyerNftTokenAccount: buyerNftAccount,
                    buyerPayingTokenAccount: buyerPayingAccount,
                    comptoir: this.comptoirPDA,
                    comptoirDestAccount: comptoirAccount.feesDestination,
                    collection: this.collectionPDA,
                    mintMetadata: metadata.pubkey,
                    vault: programNftVaultPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                },
                remainingAccounts: [
                    ...creatorsAccounts,
                    { pubkey: sellOrderPDA, isWritable: true, isSigner: false },
                    { pubkey: sellerDestinationAccount, isWritable: true, isSigner: false },
                ]
            }
        );
    }
}