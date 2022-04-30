import * as anchor from '@project-serum/anchor';
import {web3} from '@project-serum/anchor';
import * as splToken from '@solana/spl-token';
import {Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";
import assert from "assert";
import {nft_data, nft_json_url} from "./data";
import {createMint} from "./utils/utils";
import {Comptoir, Collection, getBuyOfferPDA, getCollectionPDA, getEscrowPDA} from "@aurory/comptoirjs";
import {PublicKey} from "@solana/web3.js";


let provider = anchor.getProvider()
anchor.setProvider(provider);

describe('comptoir with mint', () => {
    let admin: web3.Keypair;
    let adminTokenAccount: PublicKey;
    let creator: web3.Keypair;
    let creatorTokenAccount: PublicKey;
    let buyer: web3.Keypair;
    let buyerTokenAccount: PublicKey;
    let buyerNftTokenAccount: web3.PublicKey;
    let seller: web3.Keypair;
    let sellerTokenAccount: PublicKey;
    let sellerNftTokenAccount: PublicKey;
    let comptoirMint: splToken.Token;
    let nftMint: splToken.Token;
    let metadataPDA: web3.PublicKey;

    let comptoir: Comptoir;
    let collection: Collection;


    it('Prepare tests variables', async () => {
        creator = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            creator.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        buyer = anchor.web3.Keypair.generate()
        fromAirdropSignature = await provider.connection.requestAirdrop(
            buyer.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        seller = anchor.web3.Keypair.generate()
        fromAirdropSignature = await provider.connection.requestAirdrop(
            seller.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        admin = anchor.web3.Keypair.generate()
        fromAirdropSignature = await provider.connection.requestAirdrop(
            admin.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        comptoirMint = await splToken.Token.createMint(
            provider.connection,
            admin,
            admin.publicKey,
            null,
            6,
            splToken.TOKEN_PROGRAM_ID,
        );

        creatorTokenAccount = (await comptoirMint.getOrCreateAssociatedAccountInfo(
            creator.publicKey,
        )).address;

        buyerTokenAccount = (await comptoirMint.getOrCreateAssociatedAccountInfo(
            buyer.publicKey,
        )).address;

        sellerTokenAccount = (await comptoirMint.getOrCreateAssociatedAccountInfo(
            seller.publicKey,
        )).address;

        adminTokenAccount = (await comptoirMint.getOrCreateAssociatedAccountInfo(
            admin.publicKey,
        )).address;

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
        await provider.sendAndConfirm(tx, signers);

        metadataPDA = metadataAddr
        nftMint = new Token(provider.connection, mint.publicKey, TOKEN_PROGRAM_ID, creator)

        buyerNftTokenAccount = (await nftMint.getOrCreateAssociatedAccountInfo(buyer.publicKey)).address
        sellerNftTokenAccount = (await nftMint.getOrCreateAssociatedAccountInfo(seller.publicKey)).address

        await comptoirMint.mintTo(buyerTokenAccount, admin, [], 1000)
        comptoir = new Comptoir(provider)
        await comptoir.createComptoir(admin, comptoirMint.publicKey, 500, adminTokenAccount)
        await comptoir.createCollection(admin, "AURY", creator.publicKey, "AURY", false)

        let collectionPDA = await getCollectionPDA(comptoir.comptoirPDA, "AURY")
        collection = new Collection(provider, collectionPDA, comptoir)
    });


    it('remove nft offer', async () => {
        let price = new anchor.BN(1000)
        await collection.createBuyOffer(
            nftMint.publicKey,
            price,
            buyerNftTokenAccount,
            buyerTokenAccount,
            buyer,
        )

        let escrowPDA = await getEscrowPDA(comptoir.comptoirPDA, comptoirMint.publicKey)
        let buyOfferPDA = await getBuyOfferPDA(
            comptoir.comptoirPDA,
            buyer.publicKey,
            nftMint.publicKey,
            price,
        )

        await collection.removeBuyOffer(
            nftMint.publicKey,
            buyOfferPDA,
            buyerTokenAccount,
            buyer,
        )

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 0);
        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount)
        assert.equal(updatedBuyerAccount.amount, price.toNumber());

        let closedBuyOffer = await provider.connection.getAccountInfo(buyOfferPDA);
        assert.equal(closedBuyOffer, null);
    });

    it('create nft offer', async () => {
        let price = new anchor.BN(1000)
        await collection.createBuyOffer(
            nftMint.publicKey,
            price,
            buyerNftTokenAccount,
            buyerTokenAccount,
            buyer,
        )

        let escrowPDA = await getEscrowPDA(comptoir.comptoirPDA, comptoirMint.publicKey)
        let buyOfferPDA = await getBuyOfferPDA(
            comptoir.comptoirPDA,
            buyer.publicKey,
            nftMint.publicKey,
            price,
        )

        let buyOffer = await comptoir.program.account.buyOffer.fetch(buyOfferPDA)
        assert.equal(buyOffer.comptoir.toString(), comptoir.comptoirPDA.toString());
        assert.equal(buyOffer.mint.toString(), nftMint.publicKey.toString());
        assert.equal(buyOffer.proposedPrice.toString(), "1000");
        assert.equal(buyOffer.authority.toString(), buyer.publicKey.toString());
        assert.equal(buyOffer.destination.toString(), buyerNftTokenAccount.toString());

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 1000);

        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount)
        assert.equal(updatedBuyerAccount.amount, 0);
    });

    it('execute nft offer', async () => {
        let escrowPDA = await getEscrowPDA(comptoir.comptoirPDA, comptoirMint.publicKey)
        let buyOfferPDA = await getBuyOfferPDA(
            comptoir.comptoirPDA,
            buyer.publicKey,
            nftMint.publicKey,
            new anchor.BN(1000),
        )

        await collection.executeOffer(
            nftMint.publicKey,
            buyOfferPDA,
            buyer.publicKey,
            buyerNftTokenAccount,
            sellerTokenAccount,
            sellerNftTokenAccount,
            seller,
        )

        let escrowAccount = await comptoirMint.getAccountInfo(escrowPDA)
        assert.equal(escrowAccount.amount, 0);

        let updatedBuyerAccount = await comptoirMint.getAccountInfo(buyerTokenAccount)
        assert.equal(updatedBuyerAccount.amount.toNumber(), 0);

        let updatedBuyerNftAccount = await nftMint.getAccountInfo(buyerNftTokenAccount)
        assert.equal(updatedBuyerNftAccount.amount.toNumber(), 1);

        let updatedSellerAccount = await comptoirMint.getAccountInfo(sellerTokenAccount)
        assert.equal(updatedSellerAccount.amount.toNumber(), 850);

        let updatedSellerNftAccount = await nftMint.getAccountInfo(sellerNftTokenAccount)
        assert.equal(updatedSellerNftAccount.amount.toNumber(), 4);

        let updatedComptoirDestinationAccount = await comptoirMint.getAccountInfo(adminTokenAccount)
        assert.equal(updatedComptoirDestinationAccount.amount.toNumber(), 50);

        let updatedCreator = await comptoirMint.getAccountInfo(creatorTokenAccount)
        assert.equal(updatedCreator.amount.toNumber(), 100);

        let closedBuyOffer = await provider.connection.getAccountInfo(buyOfferPDA);
        assert.equal(closedBuyOffer, null);
    });
});
