import * as anchor from '@project-serum/anchor'
import {Comptoir as ComptoirDefinition, IDL} from './types/comptoir'
import {COMPTOIR_PROGRAM_ID} from './constant'
import {Keypair, PublicKey, TransactionInstruction} from '@solana/web3.js'
import {TOKEN_PROGRAM_ID} from '@solana/spl-token'
import {getAssociatedTokenAddress, getNftVaultPDA, getSellOrderPDA} from './getPDAs'
import {getMetadata} from './metaplex'
import {programs} from '@metaplex/js'
import * as idl from './types/comptoir.json'
import {IdlAccounts, web3} from "@project-serum/anchor";

const {Metadata} =
    programs.metadata

export class Collection {
    program: anchor.Program<ComptoirDefinition>
    comptoirPDA: PublicKey
    collectionPDA: PublicKey

    private collectionCache?: IdlAccounts<ComptoirDefinition>["collection"]

    constructor(
        provider: anchor.Provider,
        comptoirPDA: PublicKey,
        collectionPDA: PublicKey,
    ) {
        // @ts-ignore
        this.program = new anchor.Program(idl, COMPTOIR_PROGRAM_ID, provider)

        this.comptoirPDA = comptoirPDA
        this.collectionPDA = collectionPDA
    }

    async sellAssetInstruction(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellerDestination: PublicKey,
        price: anchor.BN,
        amount: anchor.BN,
        seller: PublicKey,
    ): Promise<TransactionInstruction> {
        let programNftVaultPDA = await getNftVaultPDA(nftMint)
        let sellOrderPDA = await getSellOrderPDA(sellerNftAccount, price)

        let metadataPDA = await Metadata.getPDA(nftMint)
        return await this.program.methods.createSellOrder(price, amount, sellerDestination).accounts(
            {
                payer: seller,
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
            }
        ).instruction()
    }

    async sellAsset(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellerDestination: PublicKey,
        price: anchor.BN,
        amount: anchor.BN,
        seller: Keypair
    ): Promise<string> {
        let ix = await this.sellAssetInstruction(
            nftMint, sellerNftAccount, sellerDestination,
            price, amount, seller.publicKey,
        )
        return this._sendInstruction(ix, [seller])
    }

    async removeSellOrderInstruction(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellOrderPDA: PublicKey,
        amount: anchor.BN,
        seller: PublicKey,
    ): Promise<TransactionInstruction> {
        let programNftVaultPDA = await getNftVaultPDA(nftMint)
        return await this.program.methods.removeSellOrder(amount).accounts({
            authority: seller,
            sellerNftTokenAccount: sellerNftAccount,
            vault: programNftVaultPDA,
            sellOrder: sellOrderPDA,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        }).instruction()
    }

    async removeSellOrder(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellOrderPDA: PublicKey,
        amount: anchor.BN,
        seller: Keypair,
    ): Promise<string> {
        let ix = await this.removeSellOrderInstruction(
            nftMint,
            sellerNftAccount,
            sellOrderPDA,
            amount,
            seller.publicKey,
        )
        return this._sendInstruction(ix, [seller])
    }

    async addToSellOrderInstruction(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellOrderPDA: PublicKey,
        amount: anchor.BN,
        seller: PublicKey,
    ): Promise<TransactionInstruction> {
        let programNftVaultPDA = await getNftVaultPDA(nftMint)
        return await this.program.methods.addQuantityToSellOrder(amount).accounts({
            authority: seller,
            sellerNftTokenAccount: sellerNftAccount,
            vault: programNftVaultPDA,
            sellOrder: sellOrderPDA,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        }).instruction()
    }

    async addToSellOrder(
        nftMint: PublicKey,
        sellerNftAccount: PublicKey,
        sellOrderPDA: PublicKey,
        amount: anchor.BN,
        seller: Keypair,
    ): Promise<string> {
        let ix = await this.addToSellOrderInstruction(
            nftMint,
            sellerNftAccount,
            sellOrderPDA,
            amount,
            seller.publicKey,
        )
        return this._sendInstruction(ix, [seller])
    }

    async buyInstruction(
        nftMint: PublicKey,
        sellOrdersPDA: PublicKey[],
        buyerNftAccount: PublicKey,
        buyerPayingAccount: PublicKey,
        wanted_quantity: anchor.BN,
        buyer: PublicKey,
    ): Promise<TransactionInstruction> {
        let programNftVaultPDA = await getNftVaultPDA(nftMint)
        let comptoirAccount = await this.program.account.comptoir.fetch(this.comptoirPDA)

        let metadata = await getMetadata(
            anchor.getProvider().connection,
            nftMint,
        )

        let collection = await this.getCollection()
        let creatorsAccounts = []

        if (!collection.ignoreCreatorFee) {
            for (let creator of metadata.data.creators) {
                let creatorAddress = new PublicKey(creator.address)
                let creatorATA = await getAssociatedTokenAddress(creatorAddress, comptoirAccount.mint)

                creatorsAccounts.push(
                    {pubkey: creatorATA, isWritable: true, isSigner: false},
                )
            }
        }

        let sellOrders = []
        for (let sellOrderPDA of sellOrdersPDA) {
            let so = await this.program.account.sellOrder.fetch(sellOrderPDA)
            sellOrders.push({pubkey: sellOrderPDA, isWritable: true, isSigner: false})
            sellOrders.push({pubkey: so.destination, isWritable: true, isSigner: false})
        }

        return await this.program.methods.buy(wanted_quantity).accounts({
            buyer: buyer,
            buyerNftTokenAccount: buyerNftAccount,
            buyerPayingTokenAccount: buyerPayingAccount,
            comptoir: this.comptoirPDA,
            comptoirDestAccount: comptoirAccount.feesDestination,
            collection: this.collectionPDA,
            metadata: await Metadata.getPDA(metadata.mint),
            vault: programNftVaultPDA,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
        }).remainingAccounts([
            ...creatorsAccounts,
            ...sellOrders,
        ]).instruction()
    }

    async buy(
        nftMint: PublicKey,
        sellOrdersPDA: PublicKey[],
        buyerNftAccount: PublicKey,
        buyerPayingAccount: PublicKey,
        wanted_quantity: anchor.BN,
        buyer: Keypair,
    ): Promise<string> {
        let ix = await this.buyInstruction(
            nftMint,
            sellOrdersPDA,
            buyerNftAccount,
            buyerPayingAccount,
            wanted_quantity,
            buyer.publicKey,
        )
        return this._sendInstruction(ix, [buyer])
    }

    async getCollection(): Promise<IdlAccounts<ComptoirDefinition>["collection"]> {
        if (this.collectionCache) {
            return this.collectionCache
        }
        this.collectionCache = await this.program.account.collection.fetch(this.collectionPDA)
        return this.collectionCache
    }

    _sendInstruction(ix: TransactionInstruction, signers: Keypair[]): Promise<string> {
        let tx = new web3.Transaction()
        tx.add(ix)
        return this.program.provider.send(tx, signers)
    }
}