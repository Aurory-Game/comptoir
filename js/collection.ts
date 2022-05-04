import * as anchor from '@project-serum/anchor';
import { Comptoir as ComptoirDefinition, IDL } from './types/comptoir';
import { COMPTOIR_PROGRAM_ID } from './constant';
import { Keypair, PublicKey, TransactionInstruction } from '@solana/web3.js';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import {
  getAssociatedTokenAddress,
  getBuyOfferPDA,
  getEscrowPDA,
  getNftVaultPDA,
  getSellOrderPDA,
} from './getPDAs';
import { getMetadata } from './metaplex';
import { programs } from '@metaplex/js';
import * as idl from './types/comptoir.json';
import { IdlAccounts, web3 } from '@project-serum/anchor';
import { Comptoir } from './comptoir';
import { MetadataData } from '@metaplex/js/lib/programs/metadata';

const { Metadata } = programs.metadata;

export class Collection {
  program: anchor.Program<ComptoirDefinition>;
  collectionPDA: PublicKey;
  comptoir: Comptoir;

  private collectionCache?: IdlAccounts<ComptoirDefinition>['collection'];

  constructor(
    provider: anchor.Provider,
    collectionPDA: PublicKey,
    comptoir: Comptoir,
  ) {
    this.comptoir = comptoir;
    this.program = new anchor.Program(
      idl as ComptoirDefinition,
      comptoir.programID,
      provider
    );
    this.collectionPDA = collectionPDA;
  }

  async sellAssetInstruction(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellerDestination: PublicKey,
    price: anchor.BN,
    amount: anchor.BN,
    seller: PublicKey
  ): Promise<TransactionInstruction> {
    if (!this.comptoir.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }

    let programNftVaultPDA = await getNftVaultPDA(nftMint, this.comptoir.programID);
    let sellOrderPDA = await getSellOrderPDA(sellerNftAccount, price, this.comptoir.programID);

    let metadataPDA = await Metadata.getPDA(nftMint);
    return await this.program.methods
      .createSellOrder(price, amount, sellerDestination)
      .accounts({
        payer: seller,
        sellerNftTokenAccount: sellerNftAccount,
        comptoir: this.comptoir.comptoirPDA,
        collection: this.collectionPDA,
        mint: nftMint,
        metadata: metadataPDA,
        vault: programNftVaultPDA,
        sellOrder: sellOrderPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();
  }

  async sellAsset(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellerDestination: PublicKey,
    price: anchor.BN,
    amount: anchor.BN,
    seller: Keypair
  ): Promise<string> {
    let ix = await this.sellAssetInstruction(
      nftMint,
      sellerNftAccount,
      sellerDestination,
      price,
      amount,
      seller.publicKey
    );
    return this._sendInstruction(ix, [seller]);
  }

  async removeSellOrderInstruction(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellOrderPDA: PublicKey,
    amount: anchor.BN,
    seller: PublicKey
  ): Promise<TransactionInstruction> {
    let programNftVaultPDA = await getNftVaultPDA(nftMint, this.comptoir.programID);
    return await this.program.methods
      .removeSellOrder(amount)
      .accounts({
        authority: seller,
        sellerNftTokenAccount: sellerNftAccount,
        vault: programNftVaultPDA,
        sellOrder: sellOrderPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();
  }

  async removeSellOrder(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellOrderPDA: PublicKey,
    amount: anchor.BN,
    seller: Keypair
  ): Promise<string> {
    let ix = await this.removeSellOrderInstruction(
      nftMint,
      sellerNftAccount,
      sellOrderPDA,
      amount,
      seller.publicKey
    );
    return this._sendInstruction(ix, [seller]);
  }

  async addToSellOrderInstruction(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellOrderPDA: PublicKey,
    amount: anchor.BN,
    seller: PublicKey
  ): Promise<TransactionInstruction> {
    let programNftVaultPDA = await getNftVaultPDA(nftMint, this.comptoir.programID);
    return await this.program.methods
      .addQuantityToSellOrder(amount)
      .accounts({
        authority: seller,
        sellerNftTokenAccount: sellerNftAccount,
        vault: programNftVaultPDA,
        sellOrder: sellOrderPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();
  }

  async addToSellOrder(
    nftMint: PublicKey,
    sellerNftAccount: PublicKey,
    sellOrderPDA: PublicKey,
    amount: anchor.BN,
    seller: Keypair
  ): Promise<string> {
    let ix = await this.addToSellOrderInstruction(
      nftMint,
      sellerNftAccount,
      sellOrderPDA,
      amount,
      seller.publicKey
    );
    return this._sendInstruction(ix, [seller]);
  }

  async buyInstruction(
    nftMint: PublicKey,
    sellOrdersPDA: PublicKey[],
    buyerNftAccount: PublicKey,
    buyerPayingAccount: PublicKey,
    wanted_quantity: anchor.BN,
    buyer: PublicKey
  ): Promise<TransactionInstruction> {
    if (!this.comptoir.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }
    let comptoirAccount = await this.program.account.comptoir.fetch(
      this.comptoir.comptoirPDA
    );

    let metadata = await getMetadata(this.program.provider.connection, nftMint);

    let collection = await this.getCollection();
    let creatorsAccounts: {
      pubkey: anchor.web3.PublicKey;
      isWritable: boolean;
      isSigner: boolean;
    }[] = [];

    if (!collection.ignoreCreatorFee) {
      creatorsAccounts = await this._extractCreatorsAsRemainingAccount(
        metadata
      );
    }

    let sellOrders = [];
    for (let sellOrderPDA of sellOrdersPDA) {
      let so = await this.program.account.sellOrder.fetch(sellOrderPDA);
      sellOrders.push({
        pubkey: sellOrderPDA,
        isWritable: true,
        isSigner: false,
      });
      sellOrders.push({
        pubkey: so.destination,
        isWritable: true,
        isSigner: false,
      });
    }

    let programNftVaultPDA = await getNftVaultPDA(nftMint, this.program.programId);

    return await this.program.methods
      .buy(wanted_quantity)
      .accounts({
        buyer: buyer,
        buyerNftTokenAccount: buyerNftAccount,
        buyerPayingTokenAccount: buyerPayingAccount,
        comptoir: this.comptoir.comptoirPDA,
        comptoirDestAccount: comptoirAccount.feesDestination,
        collection: this.collectionPDA,
        metadata: await Metadata.getPDA(metadata.mint),
        vault: programNftVaultPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([...creatorsAccounts, ...sellOrders])
      .instruction();
  }

  async buy(
    nftMint: PublicKey,
    sellOrdersPDA: PublicKey[],
    buyerNftAccount: PublicKey,
    buyerPayingAccount: PublicKey,
    wanted_quantity: anchor.BN,
    buyer: Keypair
  ): Promise<string> {
    let ix = await this.buyInstruction(
      nftMint,
      sellOrdersPDA,
      buyerNftAccount,
      buyerPayingAccount,
      wanted_quantity,
      buyer.publicKey
    );
    return this._sendInstruction(ix, [buyer]);
  }

  async createBuyOfferInstruction(
    nftMintToBuy: PublicKey,
    offerPrice: anchor.BN,
    buyerNftAccount: PublicKey,
    buyerPayingAccount: PublicKey,
    buyer: PublicKey
  ): Promise<TransactionInstruction> {
    if (!this.comptoir.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }

    let escrowPDA = await getEscrowPDA(
      this.comptoir.comptoirPDA,
      (
        await this.comptoir.getComptoir()
      ).mint,
      this.comptoir.programID
    );

    let buyOfferPDA = await getBuyOfferPDA(
      this.comptoir.comptoirPDA,
      buyer,
      nftMintToBuy,
      offerPrice,
      this.comptoir.programID
    );

    let metadataPDA = await Metadata.getPDA(nftMintToBuy);

    return await this.program.methods
      .createBuyOffer(offerPrice)
      .accounts({
        payer: buyer,
        nftMint: nftMintToBuy,
        metadata: metadataPDA,
        comptoir: this.comptoir.comptoirPDA,
        collection: this.collectionPDA,
        escrow: escrowPDA,
        buyerNftAccount: buyerNftAccount,
        buyerPayingAccount: buyerPayingAccount,
        buyOffer: buyOfferPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();
  }

  async createBuyOffer(
    nftMintToBuy: PublicKey,
    offerPrice: anchor.BN,
    buyerNftAccount: PublicKey,
    buyerPayingAccount: PublicKey,
    buyer: Keypair
  ): Promise<string> {
    let ix = await this.createBuyOfferInstruction(
      nftMintToBuy,
      offerPrice,
      buyerNftAccount,
      buyerPayingAccount,
      buyer.publicKey
    );
    return this._sendInstruction(ix, [buyer]);
  }

  async removeBuyOfferInstruction(
    nftMintToBuy: PublicKey,
    buyOfferPDA: PublicKey,
    buyerTokenAccount: PublicKey,
    buyer: PublicKey
  ): Promise<TransactionInstruction> {
    if (!this.comptoir.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }
    let escrowPDA = await getEscrowPDA(
      this.comptoir.comptoirPDA,
      (
        await this.comptoir.getComptoir()
      ).mint,
      this.comptoir.programID
    );

    return await this.program.methods
      .removeBuyOffer()
      .accounts({
        buyer: buyer,
        buyerPayingAccount: buyerTokenAccount,
        comptoir: this.comptoir.comptoirPDA,
        escrow: escrowPDA,
        buyOffer: buyOfferPDA,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();
  }

  async removeBuyOffer(
    nftMintToBuy: PublicKey,
    buyOfferPDA: PublicKey,
    buyerTokenAccount: PublicKey,
    buyer: Keypair
  ): Promise<string> {
    let ix = await this.removeBuyOfferInstruction(
      nftMintToBuy,
      buyOfferPDA,
      buyerTokenAccount,
      buyer.publicKey
    );
    return this._sendInstruction(ix, [buyer]);
  }

  async executeOfferInstruction(
    nftMint: PublicKey,
    buyOfferPDA: PublicKey,
    buyer: PublicKey,
    buyerNftTokenAccount: PublicKey,
    sellerTokenAccount: PublicKey,
    sellerNftTokenAccount: PublicKey,
    seller: PublicKey
  ): Promise<TransactionInstruction> {
    let metadata = await getMetadata(this.program.provider.connection, nftMint);

    if (!this.comptoir.comptoirPDA) {
      throw new Error('comptoirPDA is not set');
    }

    let escrowPDA = await getEscrowPDA(
      this.comptoir.comptoirPDA,
      (
        await this.comptoir.getComptoir()
      ).mint,
      this.comptoir.programID
    );

    let creatorsAccounts: {
      pubkey: anchor.web3.PublicKey;
      isWritable: boolean;
      isSigner: boolean;
    }[] = [];
    if (!(await this.getCollection()).ignoreCreatorFee) {
      creatorsAccounts = await this._extractCreatorsAsRemainingAccount(
        metadata
      );
    }

    return await this.program.methods
      .executeOffer()
      .accounts({
        seller: seller,
        buyer: buyer,
        comptoir: this.comptoir.comptoirPDA,
        collection: this.collectionPDA,
        comptoirDestAccount: (
          await this.comptoir.getComptoir()
        ).feesDestination,
        escrow: escrowPDA,
        sellerFundsDestAccount: sellerTokenAccount,
        destination: buyerNftTokenAccount,
        sellerNftAccount: sellerNftTokenAccount,
        buyOffer: buyOfferPDA,
        metadata: await Metadata.getPDA(nftMint),
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .remainingAccounts([...creatorsAccounts])
      .instruction();
  }

  async executeOffer(
    nftMint: PublicKey,
    buyOfferPDA: PublicKey,
    buyer: PublicKey,
    buyerNftTokenAccount: PublicKey,
    sellerTokenAccount: PublicKey,
    sellerNftTokenAccount: PublicKey,
    seller: Keypair
  ) {
    let ix = await this.executeOfferInstruction(
      nftMint,
      buyOfferPDA,
      buyer,
      buyerNftTokenAccount,
      sellerTokenAccount,
      sellerNftTokenAccount,
      seller.publicKey
    );
    return this._sendInstruction(ix, [seller]);
  }

  async getCollection(): Promise<
    IdlAccounts<ComptoirDefinition>['collection']
  > {
    if (this.collectionCache) {
      return this.collectionCache;
    }
    this.collectionCache = await this.program.account.collection.fetch(
      this.collectionPDA
    );
    return this.collectionCache;
  }

  _sendInstruction(
    ix: TransactionInstruction,
    signers: Keypair[]
  ): Promise<string> {
    let tx = new web3.Transaction();
    tx.add(ix);
    // @ts-ignore
    return this.program.provider.sendAll([{tx, signers}]);
  }

  async _extractCreatorsAsRemainingAccount(metadata: MetadataData) {
    let creatorsAccounts = [];
    if (metadata.data?.creators) {
      for (let creator of metadata.data.creators) {
        let creatorAddress = new PublicKey(creator.address);
        let comptoirMint = (await this.comptoir.getComptoir()).mint;

        let creatorATA = await getAssociatedTokenAddress(
          creatorAddress,
          comptoirMint
        );
        creatorsAccounts.push({
          pubkey: creatorATA,
          isWritable: true,
          isSigner: false,
        });
      }
    }
    return creatorsAccounts;
  }
}
