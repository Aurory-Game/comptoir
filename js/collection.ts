import * as anchor from "@project-serum/anchor";
import {Comptoir as ComptoirDefinition, IDL} from './types/comptoir';
import {COMPTOIR_PROGRAM_ID} from './constant'
import {PublicKey} from "@solana/web3.js";
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getAssociatedTokenAddress, getNftVaultPDA} from "./getPDAs";
import {getMetadata} from "./metaplex";
import {programs} from "@metaplex/js";
import * as idl from './types/comptoir.json';
const { Metadata } =
    programs.metadata;

export class Collection {
    program: anchor.Program<ComptoirDefinition>
    comptoirPDA: PublicKey
    collectionPDA: PublicKey
    constructor(
        provider: anchor.Provider,
        comptoirPDA: PublicKey,
        collectionPDA: PublicKey,
    ) {
        // @ts-ignore
        this.program = new anchor.Program(idl, COMPTOIR_PROGRAM_ID, provider,)

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
        let [sellOrderPDA, sellOrderDump] = await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from("COMPTOIR"),
                sellerNftAccount.toBuffer(),
                Buffer.from(price.toString()),
            ],
            this.program.programId,
        );

        let metadataPDA = await Metadata.getPDA(nftMint)
        return await this.program.rpc.createSellOrder(
            programNftVaultDump, sellOrderDump, price, amount, sellerDestination, {
                accounts: {
                    payer: anchor.Wallet.local().payer.publicKey,
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
                    authority: anchor.Wallet.local().payer.publicKey,
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
        sellOrdersPDA: [PublicKey],
        buyerNftAccount: PublicKey,
        buyerPayingAccount: PublicKey,
        max_price: anchor.BN,
        wanted_quantity: anchor.BN,
    ) : Promise<string> {
        let [programNftVaultPDA, programNftVaultDump] = await getNftVaultPDA(nftMint)
        let comptoirAccount = await this.program.account.comptoir.fetch(this.comptoirPDA)

        let metadata = await getMetadata(
                anchor.getProvider().connection,
                nftMint,
        )

        let creatorsAccounts = []
        for (let creator of metadata.data.creators) {
            let creatorAddress = new PublicKey(creator.address)
            let creatorATA = await getAssociatedTokenAddress(creatorAddress, comptoirAccount.mint)

            creatorsAccounts.push (
                { pubkey: creatorATA, isWritable: true, isSigner: false },
            )
        }

        let sellOrders = []
        for (let sellOrderPDA of sellOrdersPDA) {
            let so = await this.program.account.sellOrder.fetch(sellOrderPDA)
            sellOrders.push({pubkey: sellOrderPDA, isWritable: true, isSigner: false})
            sellOrders.push({pubkey: so.destination, isWritable: true, isSigner: false})
        }

        return await this.program.rpc.buy(
            programNftVaultDump, wanted_quantity, max_price, {
                accounts: {
                    buyer: anchor.Wallet.local().payer.publicKey,
                    buyerNftTokenAccount: buyerNftAccount,
                    buyerPayingTokenAccount: buyerPayingAccount,
                    comptoir: this.comptoirPDA,
                    comptoirDestAccount: comptoirAccount.feesDestination,
                    collection: this.collectionPDA,
                    mintMetadata: await Metadata.getPDA(metadata.mint),
                    vault: programNftVaultPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                },
                remainingAccounts: [
                    ...creatorsAccounts,
                    ...sellOrders,
                ]
            }
        );
    }
}