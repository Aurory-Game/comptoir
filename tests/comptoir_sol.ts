import * as anchor from '@project-serum/anchor';
import {Program, web3} from '@project-serum/anchor';
import {Comptoir} from '../target/types/comptoir';
import * as splToken from '@solana/spl-token';
import {ASSOCIATED_TOKEN_PROGRAM_ID, Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";
import assert from "assert";
import {nft_data, nft_json_url} from "./data";
import {createMint} from "./utils/utils";

let provider = anchor.Provider.env()
anchor.setProvider(provider);

const program = anchor.workspace.Comptoir as Program<Comptoir>;

describe('comptoir with sol', () => {
    let admin: web3.Keypair;
    let creator: web3.Keypair;
    let seller: web3.Keypair;
    let comptoirPDA: web3.PublicKey;
    let comptoirDump: number;
    let comptoirMint: web3.PublicKey;
    let fee = new anchor.BN(20);
    let collectionName = "AURY"
    let collectionPDA: web3.PublicKey;
    let collectionDump: number
    let collectionFee = new anchor.BN(5);
    let nftMint: splToken.Token;
    let metadataPDA: web3.PublicKey;
    let sellerNftAssociatedTokenAccount: web3.PublicKey;
    let programNftVaultPDA: web3.PublicKey;
    let programNftVaultDump: number
    let sellOrderPDA: web3.PublicKey;
    let sellOrderDump: number;

    it('Prepare tests variables', async () => {
        admin = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            admin.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        creator = anchor.web3.Keypair.generate()
        fromAirdropSignature = await provider.connection.requestAirdrop(
            creator.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        seller = anchor.web3.Keypair.generate()
        fromAirdropSignature = await provider.connection.requestAirdrop(
            seller.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        [comptoirPDA, comptoirDump] = await anchor.web3.PublicKey.findProgramAddress(
            [admin.publicKey.toBuffer()],
            program.programId,
        );
        comptoirMint = splToken.NATIVE_MINT;

        [collectionPDA, collectionDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(collectionName), comptoirPDA.toBuffer()],
            program.programId,
        );

        const data = nft_data(creator.publicKey);
        const json_url = nft_json_url;
        const lamports = await Token.getMinBalanceRentForExemptMint(
            provider.connection
        );
        const [mint, metadataAddr, tx] = await createMint(
            creator.publicKey,
            seller.publicKey,
            lamports,
            data,
            json_url
        );
        const signers = [mint, creator];
        await provider.send(tx, signers);

        metadataPDA = metadataAddr
        nftMint = new Token(provider.connection, mint.publicKey, TOKEN_PROGRAM_ID, admin)

        sellerNftAssociatedTokenAccount = await Token.getAssociatedTokenAddress(
            ASSOCIATED_TOKEN_PROGRAM_ID,
            TOKEN_PROGRAM_ID,
            nftMint.publicKey,
            seller.publicKey
        );

        [programNftVaultPDA, programNftVaultDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("vault"), nftMint.publicKey.toBuffer()],
            program.programId,
        );
        [sellOrderPDA, sellOrderDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("salt"), sellerNftAssociatedTokenAccount.toBuffer()],
            program.programId,
        );
    })

    it('create comptoir sol', async () => {
        await program.rpc.createComptoir(
            comptoirDump, fee, admin.publicKey, admin.publicKey, comptoirMint, {
                accounts: {
                    payer: admin.publicKey,
                    comptoir: comptoirPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            });
        let createdComptoir = await program.account.comptoir.fetch(comptoirPDA)
        assert.equal(createdComptoir.fees.toString(), fee.toString());
        assert.equal(createdComptoir.mint.toString(), comptoirMint.toString());
        assert.equal(createdComptoir.authority.toString(), admin.publicKey.toString());
        assert.equal(createdComptoir.feesDestination.toString(), admin.publicKey.toString());
    });

    it('create collection sol', async () => {
        await program.rpc.createCollection(
            collectionDump, collectionName, creator.publicKey, collectionFee, {
                accounts: {
                    authority: admin.publicKey,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            });
        let createdCollection = await program.account.collection.fetch(collectionPDA)
        assert.equal(createdCollection.comptoirKey.toString(), comptoirPDA.toString());
        assert.equal(createdCollection.requiredVerifier.toString(), creator.publicKey.toString());
        assert.equal(createdCollection.symbol.toString(), collectionName);
        assert.equal(createdCollection.fees.toString(), collectionFee.toString());
    });

    it('create sell order sol', async () => {
        let price = new anchor.BN(1000);
        let quantity = new anchor.BN(4);

        await program.rpc.createSellOrder(
            programNftVaultDump, "salt", sellOrderDump, price, quantity, seller.publicKey, {
                accounts: {
                    payer: seller.publicKey,
                    sellerNftTokenAccount: sellerNftAssociatedTokenAccount,
                    comptoir: comptoirPDA,
                    mint: nftMint.publicKey,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [seller]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.price.toString(), price.toString());
        assert.equal(sellOrder.quantity.toString(), quantity.toString());
        assert.equal(sellOrder.mint.toString(), nftMint.publicKey.toString());
        assert.equal(sellOrder.authority.toString(), seller.publicKey.toString());
        assert.equal(sellOrder.destination.toString(), seller.publicKey.toString());
        let accountAfterSellOrderCreate = await nftMint.getAccountInfo(sellerNftAssociatedTokenAccount)
        assert.equal(accountAfterSellOrderCreate.amount, 1);
    });

    it('remove one item from sell order sol', async () => {
        let quantity = new anchor.BN(1);

        await program.rpc.removeSellOrder(
            programNftVaultDump, quantity, {
                accounts: {
                    authority: seller.publicKey,
                    sellerNftTokenAccount: sellerNftAssociatedTokenAccount,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [seller]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.quantity.toNumber(), 3);
        let updatedAccount = await nftMint.getAccountInfo(sellerNftAssociatedTokenAccount)
        assert.equal(updatedAccount.amount, 2);
    });

    it('buy the nft', async () => {
        let buyer = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            buyer.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        )
        await provider.connection.confirmTransaction(fromAirdropSignature);
        let buyerNftAta = await nftMint.getOrCreateAssociatedAccountInfo(buyer.publicKey)

        let quantity_to_buy = new anchor.BN(1)
        let max_price = new anchor.BN(1000)

        let comptoirDestBeforeBalance = (await provider.connection.getAccountInfo(admin.publicKey)).lamports
        let sellerDestBeforeBalance = (await provider.connection.getAccountInfo(seller.publicKey)).lamports
        let creatorDestBeforeBalance = (await provider.connection.getAccountInfo(creator.publicKey)).lamports
        let buyerDestBeforeBalance = (await provider.connection.getAccountInfo(buyer.publicKey)).lamports

        await program.rpc.buy(
            programNftVaultDump, quantity_to_buy, max_price, {
                accounts: {
                    buyer: buyer.publicKey,
                    buyerNftTokenAccount: buyerNftAta.address,
                    buyerPayingTokenAccount: buyer.publicKey,
                    comptoir: comptoirPDA,
                    comptoirDestAccount: admin.publicKey,
                    collection: collectionPDA,
                    mintMetadata: metadataPDA,
                    vault: programNftVaultPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                },
                signers: [buyer],
                remainingAccounts: [
                    { pubkey: creator.publicKey, isWritable: true, isSigner: false },
                    { pubkey: sellOrderPDA, isWritable: true, isSigner: false },
                    { pubkey: seller.publicKey, isWritable: true, isSigner: false },
                ]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.quantity.toNumber(), 2);

        let updatedAdmin = await provider.connection.getAccountInfo(admin.publicKey)
        assert.equal(updatedAdmin.lamports - comptoirDestBeforeBalance, 50);

        let updatedSeller = await provider.connection.getAccountInfo(seller.publicKey)
        assert.equal(updatedSeller.lamports - sellerDestBeforeBalance, 850);

        let updatedCreator = await provider.connection.getAccountInfo(creator.publicKey)
        assert.equal(updatedCreator.lamports - creatorDestBeforeBalance, 100);

        let updatedBuyer = await provider.connection.getAccountInfo(buyer.publicKey)
        assert.equal(buyerDestBeforeBalance - updatedBuyer.lamports, 1000);
    });
})