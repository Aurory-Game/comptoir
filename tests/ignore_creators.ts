import * as anchor from '@project-serum/anchor';
import {Program, web3} from '@project-serum/anchor';
import {Comptoir as ComptoirProgramType} from '../target/types/comptoir';
import * as splToken from '@solana/spl-token';
import {ASSOCIATED_TOKEN_PROGRAM_ID, Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";
import assert from "assert";
import {nft_data, nft_json_url} from "./data";
import {createMint} from "./utils/utils";
import {Comptoir } from '../js/comptoir';
import {Collection} from "../js/collection";
import {getCollectionPDA, getEscrowPDA, getNftVaultPDA, getSellOrderPDA} from "../js/getPDAs";

let provider = anchor.Provider.env()
anchor.setProvider(provider);

const program = anchor.workspace.Comptoir as Program<ComptoirProgramType>;

describe('ignore creators tests', () => {
    let creator: web3.Keypair;
    let creatorTokenAccount: splToken.AccountInfo;
    let seller: web3.Keypair;
    let sellerTokenAccount: splToken.AccountInfo;
    let comptoirMint: splToken.Token;
    let nftMint: splToken.Token;
    let metadataPDA: web3.PublicKey;
    let sellerNftAssociatedTokenAccount: web3.PublicKey;

    let comptoir: Comptoir;
    let collection: Collection;


    it('Prepare tests variables', async () => {
        creator = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
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

        comptoirMint = await splToken.Token.createMint(
            provider.connection,
            seller,
            seller.publicKey,
            null,
            6,
            splToken.TOKEN_PROGRAM_ID,
        );

        creatorTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            creator.publicKey,
        );
        sellerTokenAccount = await comptoirMint.getOrCreateAssociatedAccountInfo(
            seller.publicKey,
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
        nftMint = new Token(provider.connection, mint.publicKey, TOKEN_PROGRAM_ID, seller)

        sellerNftAssociatedTokenAccount = (await nftMint.getOrCreateAssociatedAccountInfo(seller.publicKey)).address

        comptoir = new Comptoir(provider)
        await comptoir.createComptoir(seller, comptoirMint.publicKey, 5, sellerTokenAccount.address)
        await comptoir.createCollection(seller, "AURY", creator.publicKey, "AURY", true)

        let collectionPDA = await getCollectionPDA(comptoir.comptoirPDA, "AURY")
        collection = new Collection(provider, comptoir.comptoirPDA, collectionPDA)
    });

    it('sell order ignore creators', async function () {
        await collection.sellAsset(
            nftMint.publicKey,
            sellerNftAssociatedTokenAccount,
            sellerTokenAccount.address,
            new anchor.BN(2000),
            new anchor.BN(2),
            seller
        )

        let buyer = anchor.web3.Keypair.generate()
        let fromAirdropSignature = await provider.connection.requestAirdrop(
            buyer.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await provider.connection.confirmTransaction(fromAirdropSignature);

        let buyerTokenATA = await comptoirMint.createAssociatedTokenAccount(buyer.publicKey)
        await comptoirMint.mintTo(buyerTokenATA, seller, [], 4000)

        let buyerNftATA = await nftMint.createAssociatedTokenAccount(buyer.publicKey)

        await collection.buy(
            nftMint.publicKey,
            [
                await getSellOrderPDA(sellerNftAssociatedTokenAccount, new anchor.BN(2000)),
            ],
            buyerNftATA,
            buyerTokenATA,
            new anchor.BN(2),
            buyer,
        )

        let buyerNftAccountAfterSell = await nftMint.getAccountInfo(buyerNftATA)
        assert.equal(buyerNftAccountAfterSell.amount.toNumber(), 2)

        let buyerTokenAccountAfterSell = await comptoirMint.getAccountInfo(buyerTokenATA)
        assert.equal(buyerTokenAccountAfterSell.amount.toNumber(), 0)

        let creatorTokenAccountAfterSell = await comptoirMint.getAccountInfo(creatorTokenAccount.address)
        assert.equal(creatorTokenAccountAfterSell.amount.toNumber(), 0)
    });
});
