import * as anchor from '@project-serum/anchor';
import {Program, web3} from '@project-serum/anchor';
import {Comptoir} from '../target/types/comptoir';
import * as splToken from '@solana/spl-token';
import {ASSOCIATED_TOKEN_PROGRAM_ID, Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";
import assert from "assert";
import fs from "fs";
import {mintNFT} from "@metaplex/js/lib/actions";
import {nft_data, nft_json_url} from "./data";
import {createMint} from "./utils/utils";
import * as dns from "dns";

describe('comptoir', () => {
    // Configure the client to use the local cluster.
    let provider = anchor.Provider.env()
    anchor.setProvider(provider);

    const program = anchor.workspace.Comptoir as Program<Comptoir>;

    let admin: web3.Keypair;
    let adminTokenAccount: splToken.AccountInfo;
    let comptoirPDA: web3.PublicKey;
    let comptoirDump: number;
    let comptoirMint: splToken.Token;
    let fee = new anchor.BN(20);
    let collectionName = "AURY"
    let collectionPDA: web3.PublicKey;
    let collectionDump: number
    let collectionFee = new anchor.BN(5);
    let nftMint: splToken.Token;
    let metadataPDA: web3.PublicKey;
    let creatorAddr: web3.PublicKey;
    let adminNftAssociatedTokenAccount: web3.PublicKey;
    let programNftVaultPDA: web3.PublicKey;
    let programNftVaultDump: number
    let sellOrderPDA: web3.PublicKey;
    let sellOrderDump: number;

    it('Prepare tests variables', async () => {
        admin = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            admin.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        )
        await provider.connection.confirmTransaction(fromAirdropSignature);

        [comptoirPDA, comptoirDump] = await anchor.web3.PublicKey.findProgramAddress(
            [admin.publicKey.toBuffer()],
            program.programId,
        )

        comptoirMint = await splToken.Token.createMint(
            provider.connection,
            admin,
            admin.publicKey,
            null,
            6,
            splToken.TOKEN_PROGRAM_ID,
        );

        adminTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            admin.publicKey,
        );

        await comptoirMint.mintTo(adminTokenAccount.address, admin, [], 10);

        [collectionPDA, collectionDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(collectionName), comptoirPDA.toBuffer()],
            program.programId,
        );

        const data = nft_data(admin.publicKey);
        const json_url = nft_json_url;
        const lamports = await Token.getMinBalanceRentForExemptMint(
            provider.connection
        );
        const [mint, metadataAddr, tx] = await createMint(
            admin.publicKey,
            admin.publicKey,
            lamports,
            data,
            json_url
        );
        const signers = [mint, admin];
        await provider.send(tx, signers);

        metadataPDA = metadataAddr
        nftMint = new Token(provider.connection, mint.publicKey, TOKEN_PROGRAM_ID, admin)

        adminNftAssociatedTokenAccount = await Token.getAssociatedTokenAddress(
            ASSOCIATED_TOKEN_PROGRAM_ID,
            TOKEN_PROGRAM_ID,
            nftMint.publicKey,
            admin.publicKey
        );

        [programNftVaultPDA, programNftVaultDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("vault"), nftMint.publicKey.toBuffer()],
            program.programId,
        );
        [sellOrderPDA, sellOrderDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("salt"), adminNftAssociatedTokenAccount.toBuffer()],
            program.programId,
        );
    });

    it('create comptoir', async () => {
        await program.rpc.createComptoir(
            comptoirDump, fee, adminTokenAccount.address, admin.publicKey, comptoirMint.publicKey, {
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
        assert.equal(createdComptoir.mint.toString(), comptoirMint.publicKey.toString());
        assert.equal(createdComptoir.authority.toString(), admin.publicKey.toString());
        assert.equal(createdComptoir.feesDestination.toString(), adminTokenAccount.address.toString());
    });

    it('fail: create comptoir fee > 100', async () => {
        let tmpAuthority = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            tmpAuthority.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        )
        await provider.connection.confirmTransaction(fromAirdropSignature);
        let tmpTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            tmpAuthority.publicKey,
        );

        let [failedComptoirPDA, failedComptoirDump] = await anchor.web3.PublicKey.findProgramAddress(
            [tmpAuthority.publicKey.toBuffer()],
            program.programId,
        )
        let feeAbove100 = new anchor.BN(101)
        await assert.rejects(
            program.rpc.createComptoir(
                failedComptoirDump, feeAbove100, tmpTokenAccount.address, tmpAuthority.publicKey, comptoirMint.publicKey, {
                    accounts: {
                        payer: tmpAuthority.publicKey,
                        comptoir: failedComptoirPDA,
                        systemProgram: anchor.web3.SystemProgram.programId,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                    },
                    signers: [tmpAuthority]
                }),
            {
                code: 6000, // Fee > 100
            })
    });

    it('update comptoir fields', async () => {
        let tmpFee = new anchor.BN(5);
        let tmpAuthority = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            admin.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        )
        await provider.connection.confirmTransaction(fromAirdropSignature);
        let tmpTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            tmpAuthority.publicKey,
        );
        let tmpMint = anchor.web3.Keypair.generate().publicKey

        await program.rpc.updateComptoir(
            tmpFee, tmpTokenAccount.address, tmpAuthority.publicKey, tmpMint, {
                accounts: {
                    authority: admin.publicKey,
                    comptoir: comptoirPDA,
                },
                signers: [admin]
            });
        let updatedComptoir = await program.account.comptoir.fetch(comptoirPDA)
        assert.equal(updatedComptoir.fees.toString(), tmpFee.toString());
        assert.equal(updatedComptoir.mint.toString(), tmpMint.toString());
        assert.equal(updatedComptoir.authority.toString(), tmpAuthority.publicKey.toString());
        assert.equal(updatedComptoir.feesDestination.toString(), tmpTokenAccount.address.toString());

        //revert
        await program.rpc.updateComptoir(
            fee, adminTokenAccount.address, admin.publicKey, comptoirMint.publicKey, {
                accounts: {
                    authority: tmpAuthority.publicKey,
                    comptoir: comptoirPDA,
                },
                signers: [tmpAuthority]
            });
    });

    it('create collection', async () => {
        await program.rpc.createCollection(
            collectionDump, collectionName, admin.publicKey, collectionFee, {
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
        assert.equal(createdCollection.requiredVerifier.toString(), admin.publicKey.toString());
        assert.equal(createdCollection.symbol.toString(), collectionName);
        assert.equal(createdCollection.fees.toString(), collectionFee.toString());
    });

    it('fail: create collection fee > 100', async () => {
        let feeAbove100 = new anchor.BN(101)
        await assert.rejects(
            program.rpc.createCollection(
                collectionDump, collectionName, admin.publicKey, feeAbove100, {
                    accounts: {
                        authority: admin.publicKey,
                        comptoir: comptoirPDA,
                        collection: collectionPDA,
                        systemProgram: anchor.web3.SystemProgram.programId,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                    },
                    signers: [admin]
                })
        );
    });

    it('update collection', async () => {
        let tmpFee = new anchor.BN(12)
        let tmpName = "some name"
        let tmpRequiredVerifier = anchor.web3.Keypair.generate().publicKey

        await program.rpc.updateCollection(
            tmpFee, tmpName, tmpRequiredVerifier, {
                accounts: {
                    authority: admin.publicKey,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                },
                signers: [admin]
            });
        let updatedCollection = await program.account.collection.fetch(collectionPDA)
        assert.equal(updatedCollection.requiredVerifier.toString(), tmpRequiredVerifier.toString());
        assert.equal(updatedCollection.symbol.toString(), tmpName);
        assert.equal(updatedCollection.fees.toString(), tmpFee.toString());

        // reset
        await program.rpc.updateCollection(
            collectionFee, collectionName, admin.publicKey, {
                accounts: {
                    authority: admin.publicKey,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                },
                signers: [admin]
            });
    });

    it('create sell order', async () => {
        let price = new anchor.BN(1000);
        let quantity = new anchor.BN(4);

        await program.rpc.createSellOrder(
            programNftVaultDump, "salt", sellOrderDump, price, quantity, adminTokenAccount.address, {
                accounts: {
                    payer: admin.publicKey,
                    sellerNftTokenAccount: adminNftAssociatedTokenAccount,
                    comptoir: comptoirPDA,
                    mint: nftMint.publicKey,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.price.toString(), price.toString());
        assert.equal(sellOrder.quantity.toString(), quantity.toString());
        assert.equal(sellOrder.mint.toString(), nftMint.publicKey.toString());
        assert.equal(sellOrder.authority.toString(), admin.publicKey.toString());
        assert.equal(sellOrder.destination.toString(), adminTokenAccount.address.toString());
        let accountAfterSellOrderCreate = await nftMint.getAccountInfo(adminNftAssociatedTokenAccount)
        assert.equal(accountAfterSellOrderCreate.amount, 1);
    });

    it('remove one item from sell order', async () => {
        let quantity = new anchor.BN(1);

        await program.rpc.removeSellOrder(
            programNftVaultDump, quantity, {
                accounts: {
                    authority: admin.publicKey,
                    sellerNftTokenAccount: adminNftAssociatedTokenAccount,
                    vault: programNftVaultPDA,
                    sellOrder: sellOrderPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.quantity.toNumber(), 3);
        let updatedAccount = await nftMint.getAccountInfo(adminNftAssociatedTokenAccount)
        assert.equal(updatedAccount.amount, 2);
    });

    it('buy the nft', async () => {
        let buyer = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            admin.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        )
        await provider.connection.confirmTransaction(fromAirdropSignature);
        let buyerNftAta = await nftMint.getOrCreateAssociatedAccountInfo(buyer.publicKey)
        let buyerComptoirAta = await comptoirMint.getOrCreateAssociatedAccountInfo(buyer.publicKey)
        await comptoirMint.mintTo(buyerComptoirAta.address, admin, [], 10000)

        let quantity_to_buy = new anchor.BN(1)
        let max_price = new anchor.BN(1000)


        await program.rpc.buy(
            programNftVaultDump, nftMint.publicKey, quantity_to_buy, max_price, {
                accounts: {
                    buyer: buyer.publicKey,
                    buyerNftTokenAccount: buyerNftAta.address,
                    buyerPayingTokenAccount: buyerComptoirAta.address,
                    comptoir: comptoirPDA,
                    comptoirPayingTokenAccount: adminTokenAccount.address,
                    collection: collectionPDA,
                    mintMetadata: metadataPDA,
                    vault: programNftVaultPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [buyer],
                remainingAccounts: [
                    { pubkey: adminTokenAccount.address, isWritable: true, isSigner: false },
                    { pubkey: sellOrderPDA, isWritable: true, isSigner: false },
                    { pubkey: adminTokenAccount.address, isWritable: true, isSigner: false },
                ]
            }
        );

        let sellOrder = await program.account.sellOrder.fetch(sellOrderPDA)
        assert.equal(sellOrder.quantity.toNumber(), 2);
    });
});