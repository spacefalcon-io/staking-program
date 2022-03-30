import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Staking } from '../target/types/staking';
import { Token, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { PublicKey } from '@solana/web3.js';
import FCON_ADDRESS from './token.json';

export const createStakingPool = async (
  stakingProgram: Program<Staking>,
  provider: anchor.Provider,
  wallet: anchor.Wallet,
  lockPeriod: anchor.BN,
  noTier: boolean,
) => {
  const tokenMint = new Token(
    provider.connection,
    new PublicKey(FCON_ADDRESS.devnet),
    TOKEN_PROGRAM_ID,
    wallet.payer,
  );

  const pool = anchor.web3.Keypair.generate();
  console.log('Pool: ', pool.publicKey.toString());

  let [poolSigner, nonce] = await anchor.web3.PublicKey.findProgramAddress(
    [pool.publicKey.toBuffer()],
    stakingProgram.programId,
  );

  console.log('Pool signer: ', poolSigner.toString());
  console.log('Nonce: ', nonce);

  const stakingVault = await tokenMint.createAccount(poolSigner);
  const rewardVault = await tokenMint.createAccount(poolSigner);

  console.log('Staking vault: ', stakingVault.toString());
  console.log('Reward vault: ', rewardVault.toString());

  const rewardDuration = new anchor.BN(86400 * 30);

  await stakingProgram.rpc.initializePool(
    nonce,
    rewardDuration,
    lockPeriod,
    noTier,
    {
      accounts: {
        authority: wallet.publicKey,
        stakingMint: tokenMint.publicKey,
        stakingVault,
        rewardMint: tokenMint.publicKey,
        rewardVault,
        poolSigner: poolSigner,
        pool: pool.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      signers: [pool],
      instructions: [await stakingProgram.account.pool.createInstruction(pool)],
    },
  );
};
