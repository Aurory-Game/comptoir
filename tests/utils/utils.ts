import { programs } from '@metaplex/js';
import { Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import {
    ASSOCIATED_TOKEN_PROGRAM_ID,
    MintLayout,
    TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import { Token } from '@solana/spl-token';

const { Metadata, MetadataDataData, CreateMetadata, Creator } =
    programs.metadata;
const Transaction = programs.Transaction;

export async function createMint(
    fee_payer: PublicKey,
    dest_owner: PublicKey,
    lamports,
    data: any,
    json_url: string
): Promise<[Keypair, PublicKey, programs.Transaction]> {
    const mint = Keypair.generate();
    console.log(`https://solscan.io/token/${mint.publicKey.toString()}`);
    const tx_mint = new Transaction({ feePayer: fee_payer });
    let ata = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID, // always associated token program id
        TOKEN_PROGRAM_ID, // always token program id
        mint.publicKey, // mint
        dest_owner // token account authority,
    );
    tx_mint.add(
        // create mint
        SystemProgram.createAccount({
            fromPubkey: fee_payer,
            newAccountPubkey: mint.publicKey,
            space: MintLayout.span,
            lamports: lamports,
            programId: TOKEN_PROGRAM_ID,
        }),
        Token.createInitMintInstruction(
            TOKEN_PROGRAM_ID,
            mint.publicKey,
            1,
            fee_payer,
            fee_payer
        ),
        // create token account
        Token.createAssociatedTokenAccountInstruction(
            ASSOCIATED_TOKEN_PROGRAM_ID,
            TOKEN_PROGRAM_ID,
            mint.publicKey,
            ata,
            dest_owner,
            fee_payer
        ),
        // mint to token account
        Token.createMintToInstruction(
            TOKEN_PROGRAM_ID,
            mint.publicKey,
            ata,
            fee_payer,
            [],
            5
        )
    );

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
    const tx_metadata = new CreateMetadata(
        {
            feePayer: fee_payer,
        },
        {
            metadata: metadataPDA,
            metadataData,
            updateAuthority: fee_payer,
            mint: mint.publicKey,
            mintAuthority: fee_payer,
        }
    );

    const tx = Transaction.fromCombined([tx_mint, tx_metadata]);
    return [mint, metadataPDA, tx];
}