import { Connection, PublicKey } from '@solana/web3.js';
import { programs } from '@metaplex/js';
const { Metadata, MetadataData } = programs.metadata;

export const getMetadata = async (connection: Connection, mint: PublicKey) => {
  let metadaPDA = await Metadata.getPDA(mint);
  const metadataAccount = await connection.getAccountInfo(metadaPDA);
  return !metadataAccount
    ? null
    : MetadataData.deserialize(metadataAccount.data);
};
