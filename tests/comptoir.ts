import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Comptoir } from '../target/types/comptoir';

describe('comptoir', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Comptoir as Program<Comptoir>;

  it('Is initialized!', async () => {
    // Add your test here.
    const tx = await program.rpc.initialize({});
    console.log("Your transaction signature", tx);
  });
});
