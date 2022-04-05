// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Staking } from '../target/types/staking';
import { createStakingPool } from './createStakingPools';

module.exports = async function (provider: anchor.Provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  let wallet: anchor.Wallet = provider.wallet as anchor.Wallet;

  const stakingProgram = anchor.workspace.Staking as Program<Staking>;

  console.log('Deploy no lock pool...');
  await createStakingPool(
    stakingProgram,
    provider,
    wallet,
    new anchor.BN(0),
    true,
  );
  console.log('Deploy 7 days lock pool...');
  await createStakingPool(
    stakingProgram,
    provider,
    wallet,
    new anchor.BN(7 * 86400),
    false,
  );
  console.log('Deploy 2 months lock pool...');
  await createStakingPool(
    stakingProgram,
    provider,
    wallet,
    new anchor.BN(60 * 86400),
    false,
  );
};
