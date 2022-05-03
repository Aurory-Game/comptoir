import * as anchor from '@project-serum/anchor';
import { Comptoir as ComptoirDefinition } from './types/comptoir';
import { COMPTOIR_PROGRAM_ID } from './constant';
import * as idl from './types/comptoir.json';

import { Keypair, PublicKey } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { getCollectionPDA, getComptoirPDA, getEscrowPDA } from './getPDAs';
import { IdlAccounts } from '@project-serum/anchor';

export class Comptoir {
  program: anchor.Program<ComptoirDefinition>;
  comptoirPDA: PublicKey | null;
  programID: PublicKey;

  private comptoirCache?: IdlAccounts<ComptoirDefinition>['comptoir'];

  constructor(provider: anchor.Provider, comptoirPDA?: PublicKey, programID?: PublicKey) {
    this.programID = programID ? programID : COMPTOIR_PROGRAM_ID
    // @ts-ignore
    this.program = new anchor.Program(idl, this.programID, provider);

    this.comptoirPDA = comptoirPDA ?? null;
  }

  async createComptoir(
    owner: Keypair,
    mint: PublicKey,
    fees: number,
    feesDestination: PublicKey
  ): Promise<string> {
    let comptoirPDA = await getComptoirPDA(owner.publicKey, this.programID);

    let escrowPDA = await getEscrowPDA(comptoirPDA, mint, this.programID);

    this.comptoirPDA = comptoirPDA;

    return await this.program.methods
      .createComptoir(mint, fees, feesDestination, owner.publicKey)
      .accounts({
        payer: owner.publicKey,
        comptoir: comptoirPDA,
        mint: mint,
        escrow: escrowPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([owner])
      .rpc();
  }

  async createCollection(
    authority: Keypair,
    name: string,
    required_metadata_signer: PublicKey,
    collection_symbol: string,
    ignore_creators: boolean,
    fee?: number
  ): Promise<string> {
    if (!this.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }
    let collectionPDA = await getCollectionPDA(
      this.comptoirPDA,
        name,
        this.programID
    );

    return await this.program.methods
      .createCollection(
        name,
        collection_symbol,
        required_metadata_signer,
        fee ? fee : null,
        ignore_creators
      )
      .accounts({
        authority: authority.publicKey,
        comptoir: this.comptoirPDA,
        collection: collectionPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([authority])
      .rpc();
  }

  async getComptoir(): Promise<IdlAccounts<ComptoirDefinition>['comptoir']> {
    if (this.comptoirCache) {
      return this.comptoirCache;
    }
    if (!this.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }
    this.comptoirCache = await this.program.account.comptoir.fetch(
      this.comptoirPDA
    );
    return this.comptoirCache;
  }
}
