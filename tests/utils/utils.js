"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.createMint = void 0;
const js_1 = require("@metaplex/js");
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const spl_token_2 = require("@solana/spl-token");
const { Metadata, MetadataDataData, CreateMetadata, Creator } = js_1.programs.metadata;
const Transaction = js_1.programs.Transaction;
async function createMint(fee_payer, dest_owner, lamports, data, json_url) {
    const mint = web3_js_1.Keypair.generate();
    console.log(`https://solscan.io/token/${mint.publicKey.toString()}`);
    const tx_mint = new Transaction({ feePayer: fee_payer });
    let ata = await spl_token_2.Token.getAssociatedTokenAddress(spl_token_1.ASSOCIATED_TOKEN_PROGRAM_ID, // always associated token program id
    spl_token_1.TOKEN_PROGRAM_ID, // always token program id
    mint.publicKey, // mint
    dest_owner // token account authority,
    );
    tx_mint.add(
    // create mint
    web3_js_1.SystemProgram.createAccount({
        fromPubkey: fee_payer,
        newAccountPubkey: mint.publicKey,
        space: spl_token_1.MintLayout.span,
        lamports: lamports,
        programId: spl_token_1.TOKEN_PROGRAM_ID,
    }), spl_token_2.Token.createInitMintInstruction(spl_token_1.TOKEN_PROGRAM_ID, mint.publicKey, 1, fee_payer, fee_payer), 
    // create token account
    spl_token_2.Token.createAssociatedTokenAccountInstruction(spl_token_1.ASSOCIATED_TOKEN_PROGRAM_ID, spl_token_1.TOKEN_PROGRAM_ID, mint.publicKey, ata, dest_owner, fee_payer), 
    // mint to token account
    spl_token_2.Token.createMintToInstruction(spl_token_1.TOKEN_PROGRAM_ID, mint.publicKey, ata, fee_payer, [], 5));
    const metadataPDA = await Metadata.getPDA(mint.publicKey);
    const metadataData = new MetadataDataData({
        name: data.name,
        symbol: 'AURY',
        uri: json_url,
        sellerFeeBasisPoints: data.seller_fee_basis_points,
        creators: [
            new Creator({
                address: fee_payer.toString(),
                verified: true,
                share: 100,
            }),
        ],
    });
    const tx_metadata = new CreateMetadata({
        feePayer: fee_payer,
    }, {
        metadata: metadataPDA,
        metadataData,
        updateAuthority: fee_payer,
        mint: mint.publicKey,
        mintAuthority: fee_payer,
    });
    const tx = Transaction.fromCombined([tx_mint, tx_metadata]);
    return [mint, metadataPDA, tx];
}
exports.createMint = createMint;
