import * as anchor from '@project-serum/anchor'
import {Comptoir} from '../comptoir'
import {Keypair, PublicKey} from '@solana/web3.js'
import {getAssociatedTokenAddress, getCollectionPDA, getComptoirPDA, getSellOrderPDA} from '../getPDAs'
import {Token} from '@solana/spl-token'
import {nft_data, nft_json_url} from '../../tests/data'
import {createMint} from '../../tests/utils/utils'
import * as splToken from '@solana/spl-token'
import {Collection} from '../collection'

let provider = anchor.AnchorProvider.local('https://api.devnet.solana.com')
anchor.setProvider(provider)

async function workflow(comptoirMint: PublicKey, nftMint: PublicKey) {
    let comptoirPDA = await getComptoirPDA(
        anchor.Wallet.local().payer.publicKey
    )
    let comptoir = new Comptoir(provider, comptoirPDA)

    await comptoir.createComptoir(
        anchor.Wallet.local().payer,
        comptoirMint,
        5,
        await getAssociatedTokenAddress(
            anchor.Wallet.local().payer.publicKey,
            comptoirMint,
        )
    )

    await comptoir.createCollection(
        anchor.Wallet.local().payer,
        'aurorian',
        anchor.Wallet.local().payer.publicKey,
        'AURY',
        true,
        2,
    )


    let collectionPDA = await getCollectionPDA(comptoir.comptoirPDA, 'AURY')
    let userNftAccount = await getAssociatedTokenAddress(anchor.Wallet.local().payer.publicKey, nftMint)
    let userTokenAccount = await getAssociatedTokenAddress(anchor.Wallet.local().payer.publicKey, comptoirMint)

    let collection = new Collection(provider, collectionPDA, comptoir)

    let sellPrice = new anchor.BN(1000)
    let sellQuantity = new anchor.BN(1)


    await collection.sellAsset(
        nftMint,
        userNftAccount,
        userTokenAccount,
        sellPrice,
        sellQuantity,
        anchor.Wallet.local().payer
    )

    //We buy our own asset just for demonstration
    await collection.buy(
        nftMint,
        [await getSellOrderPDA(userNftAccount, sellPrice)],
        userNftAccount,
        userTokenAccount,
        sellQuantity,
        anchor.Wallet.local().payer
    )
}

async function mintMeNft(): Promise<PublicKey> {
    const data = nft_data(anchor.Wallet.local().payer.publicKey)
    const json_url = nft_json_url
    const lamports = await Token.getMinBalanceRentForExemptMint(
        provider.connection
    )
    const [mint, metadataAddr, tx] = await createMint(
        anchor.Wallet.local().payer.publicKey,
        anchor.Wallet.local().payer.publicKey,
        lamports,
        data,
        json_url
    )
    const signers = [mint, anchor.Wallet.local().payer]
    await provider.sendAndConfirm(tx, signers)

    return mint.publicKey
}

async function setup() {
    let fromAirdropSignature = await provider.connection.requestAirdrop(
        anchor.Wallet.local().payer.publicKey,
        anchor.web3.LAMPORTS_PER_SOL,
    )
    await provider.connection.confirmTransaction(fromAirdropSignature)

    let nftMint: PublicKey = await mintMeNft()
    let comptoirMint = await splToken.Token.createMint(
        provider.connection,
        anchor.Wallet.local().payer,
        anchor.Wallet.local().payer.publicKey,
        null,
        6,
        splToken.TOKEN_PROGRAM_ID,
    )

    let ata = await comptoirMint.getOrCreateAssociatedAccountInfo(
        anchor.Wallet.local().payer.publicKey
    )
    await comptoirMint.mintTo(ata.address, anchor.Wallet.local().payer, [], 1000)
    return [comptoirMint.publicKey, nftMint]
}

(async () => {
    let [comptoirMint, nftMint] = await setup()

    await workflow(
        comptoirMint,
        nftMint
    )
})()
