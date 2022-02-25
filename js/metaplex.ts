import {Connection, PublicKey} from "@solana/web3.js";
import {Metadata, MetadataData} from "@metaplex/js/lib/programs/metadata";

export const getMetadataData = async (
    connection: Connection,
    mint: PublicKey,
) : Promise<Metadata> => {
    let metadaPDA = await Metadata.getPDA(mint)
    const metadataAccount = await connection.getAccountInfo(metadaPDA);
    return MetadataData.deserialize(metadataAccount.data);
}