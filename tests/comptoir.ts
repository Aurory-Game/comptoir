import * as anchor from '@project-serum/anchor';
import {Program, web3} from '@project-serum/anchor';
import { Comptoir } from '../target/types/comptoir';
import * as splToken from '@solana/spl-token';
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import assert from "assert";

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

  it('failed create comptoir fee > 100', async () => {
    let feeAbove100 = new anchor.BN(101)
    await assert.rejects(
        program.rpc.createComptoir(
          comptoirDump, feeAbove100, admin.publicKey, admin.publicKey, comptoirMint.publicKey, {
            accounts: {
              payer: admin.publicKey,
              comptoir: comptoirPDA,
              systemProgram: anchor.web3.SystemProgram.programId,
              tokenProgram: TOKEN_PROGRAM_ID,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
            signers: [admin]
          }),
        )
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
});
