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


describe('comptoir with mint', () => {
    let admin: web3.Keypair;
    let adminTokenAccount: splToken.AccountInfo;
    let buyer: web3.Keypair;
    let buyerTokenAccount: splToken.AccountInfo;
    let buyerNftTokenAccount: web3.PublicKey;
    let creator: web3.Keypair;
    let creatorTokenAccount: splToken.AccountInfo;
    let seller: web3.Keypair;
    let sellerTokenAccount: splToken.AccountInfo;
    let sellerNftAssociatedTokenAccount: web3.PublicKey;
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
    let escrowPDA: web3.PublicKey;
    let escrowDump: number;
    let buyOfferPDA: web3.PublicKey;
    let buyOfferDump: number;

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

        buyer = anchor.web3.Keypair.generate()
         fromAirdropSignature = await provider.connection.requestAirdrop(
            buyer.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);


        [comptoirPDA, comptoirDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("COMPTOIR"), admin.publicKey.toBuffer()],
            program.programId,
        );

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
        creatorTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            creator.publicKey,
        );
        sellerTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            seller.publicKey,
        );
        buyerTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            buyer.publicKey,
        );

        await comptoirMint.mintTo(buyerTokenAccount.address, admin, [], 1000);

        [collectionPDA, collectionDump] = await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from("COMPTOIR"), Buffer.from(collectionName), comptoirPDA.toBuffer()],
            program.programId,
        );

        [escrowPDA, escrowDump] = await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from("COMPTOIR"),
                comptoirPDA.toBuffer(),
                comptoirMint.publicKey.toBuffer(),
                Buffer.from("ESCROW"),
            ],
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

        buyerNftTokenAccount = await Token.getAssociatedTokenAddress(
            ASSOCIATED_TOKEN_PROGRAM_ID,
            TOKEN_PROGRAM_ID,
            nftMint.publicKey,
            buyer.publicKey
        );

       [buyOfferPDA, buyOfferDump] = await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from("COMPTOIR"),
                comptoirPDA.toBuffer(),
                buyer.publicKey.toBuffer(),
                Buffer.from("1000"),
                Buffer.from("ESCROW"),
            ],
            program.programId,
        );

        await program.rpc.createComptoir(
            comptoirDump, escrowDump, comptoirMint.publicKey, fee, adminTokenAccount.address, admin.publicKey, {
                accounts: {
                    payer: admin.publicKey,
                    comptoir: comptoirPDA,
                    mint: comptoirMint.publicKey,
                    escrow: escrowPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            });

        await program.rpc.createCollection(
            collectionDump, collectionName, creator.publicKey, collectionFee, {
                accounts: {
                    authority: admin.publicKey,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [admin]
            });
        });

    it('remove nft offer', async () => {
        await program.rpc.createBuyOffer(
            escrowDump, buyOfferDump, new anchor.BN(1000), {
                accounts: {
                    payer: buyer.publicKey,
                    nftMint: nftMint.publicKey,
                    metadata: metadataPDA,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                    escrow: escrowPDA,
                    buyerPayingAccount: buyerTokenAccount.address,
                    buyerNftAccount: buyerNftTokenAccount,
                    buyOffer: buyOfferPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [buyer]
            }
        );

        await program.rpc.removeBuyOffer(
            escrowDump, {
                accounts: {
                    buyer: buyer.publicKey,
                    buyerPayingAccount: buyerTokenAccount.address,
                    comptoir: comptoirPDA,
                    escrow: escrowPDA,
                    buyOffer: buyOfferPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [buyer]
            }
        );

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 0);
        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount.address)
        assert.equal(updatedBuyerAccount.amount, 1000);

        let closedBuyOffer = await provider.connection.getAccountInfo(buyOfferPDA);
        assert.equal(closedBuyOffer, null);

    });

    it('create nft offer', async () => {
        await program.rpc.createBuyOffer(
            escrowDump, buyOfferDump, new anchor.BN(1000), {
                accounts: {
                    payer: buyer.publicKey,
                    nftMint: nftMint.publicKey,
                    metadata: metadataPDA,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                    escrow: escrowPDA,
                    buyerPayingAccount: buyerTokenAccount.address,
                    buyerNftAccount: buyerNftTokenAccount,
                    buyOffer: buyOfferPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                signers: [buyer]
            }
        );

        let buyOffer = await program.account.buyOffer.fetch(buyOfferPDA)
        assert.equal(buyOffer.comptoir.toString(), comptoirPDA.toString());
        assert.equal(buyOffer.metadata.toString(), metadataPDA.toString());
        assert.equal(buyOffer.proposedPrice.toString(), "1000");
        assert.equal(buyOffer.authority.toString(), buyer.publicKey.toString());
        assert.equal(buyOffer.destination.toString(), buyerNftTokenAccount.toString());

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 1000);

        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount.address)
        assert.equal(updatedBuyerAccount.amount, 0);
    });

    it('execute nft offer', async () => {
        await program.rpc.executeOffer(
            escrowDump, {
                accounts: {
                    seller: seller.publicKey,
                    buyer: buyer.publicKey,
                    comptoir: comptoirPDA,
                    collection: collectionPDA,
                    comptoirDestAccount: adminTokenAccount.address,
                    escrow: escrowPDA,
                    sellerFundsDestAccount: sellerTokenAccount.address,
                    destination: buyerNftTokenAccount,
                    sellerNftAccount: sellerNftAssociatedTokenAccount,
                    buyOffer: buyOfferPDA,
                    metadata: metadataPDA,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                },
                remainingAccounts: [
                    { pubkey: creatorTokenAccount.address, isWritable: true, isSigner: false },
                ],
                signers: [seller]
            }
        );

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 0);

        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount.address)
        assert.equal(updatedBuyerAccount.amount, 0);

        let updatedBuyerNftAccount = await nftMint.getAccountInfo(buyerNftTokenAccount)
        assert.equal(updatedBuyerNftAccount.amount, 1);

        let updatedSellerAccount = await comptoirMint.getAccountInfo(sellerTokenAccount.address)
        assert.equal(updatedSellerAccount.amount, 850);

        let updatedSellerNftAccount = await nftMint.getAccountInfo(sellerNftAssociatedTokenAccount)
        assert.equal(updatedSellerNftAccount.amount.toNumber(), 4);

        let updatedComptoirDestinationAccount = await comptoirMint.getAccountInfo(adminTokenAccount.address)
        assert.equal(updatedComptoirDestinationAccount.amount.toNumber(), 50);

        let updatedCreator = await comptoirMint.getAccountInfo(creatorTokenAccount.address)
        assert.equal(updatedCreator.amount.toNumber(), 100);

        let closedBuyOffer = await provider.connection.getAccountInfo(buyOfferPDA);
        assert.equal(closedBuyOffer, null);
    });
});
