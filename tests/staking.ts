import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { TOKEN_PROGRAM_ID, Token } from '@solana/spl-token';
import assert from 'assert';
import { Staking } from '../target/types/staking';
import { createMint } from './utils';

describe('staking', () => {
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);

  const stakingProgram = anchor.workspace.Staking as Program<Staking>;
  let stakingMint: Token;
  let stakingVault: anchor.web3.PublicKey;
  let rewardMint: Token;
  let rewardVault: anchor.web3.PublicKey;
  let pool: anchor.web3.Keypair;
  let poolSigner: anchor.web3.PublicKey;
  let nonce: number;
  let user: anchor.web3.PublicKey;
  let ownerTokenAccount: anchor.web3.PublicKey;
  const lockPeriod = new anchor.BN(0);
  const rewardDuration = new anchor.BN(86400 * 7);
  let wallet: anchor.Wallet = provider.wallet as anchor.Wallet;

  before(async () => {
    stakingMint = await createMint(provider, 4);
    rewardMint = await createMint(provider, 4);
  });

  beforeEach(async () => {
    pool = anchor.web3.Keypair.generate();

    let [_poolSigner, _nonce] = await anchor.web3.PublicKey.findProgramAddress(
      [pool.publicKey.toBuffer()],
      stakingProgram.programId,
    );
    poolSigner = _poolSigner;
    nonce = _nonce;

    stakingVault = await stakingMint.createAccount(poolSigner);
    rewardVault = await rewardMint.createAccount(poolSigner);
  });

  describe('initialize pool', () => {
    it('check initialized pool values', async () => {
      await initializePool(false);

      const poolAccount = await stakingProgram.account.pool.fetch(
        pool.publicKey,
      );
      assert.equal(
        poolAccount.authority.toString(),
        wallet.publicKey.toString(),
      );
      assert.equal(poolAccount.nonce, nonce);
      assert.equal(poolAccount.paused, false);
      assert.equal(poolAccount.stakingMint.toString(), stakingMint.publicKey);
      assert.equal(poolAccount.stakingVault.toString(), stakingVault);
      assert.equal(poolAccount.rewardMint.toString(), rewardMint.publicKey);
      assert.equal(poolAccount.rewardVault.toString(), rewardVault);
      assert.equal(
        poolAccount.rewardDuration.toString(),
        rewardDuration.toString(),
      );
      assert.equal(poolAccount.rewardDurationEnd.toString(), '0');
      assert.equal(poolAccount.lockPeriod.toString(), lockPeriod.toString());
      assert.equal(poolAccount.lastUpdateTime.toString(), '0');
      assert.equal(poolAccount.rewardRate.toString(), '0');
      assert.equal(poolAccount.rewardPerTokenStored.toString(), '0');
      assert.equal(poolAccount.userStakeCount.toString(), '0');
      assert.equal(poolAccount.funders.length, 5);
      assert.equal(poolAccount.noTier, false);
    });
  });

  describe('stake', () => {
    it('update tier', async () => {
      await initializePool(false);
      await createUser();

      const amount = new anchor.BN(2_000_000_000);

      await stake(amount);

      const userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.pool.toString(), pool.publicKey.toString());
      assert.equal(userAccount.owner.toString(), wallet.publicKey.toString());
      assert.equal(userAccount.rewardPerTokenComplete.toString(), '0');
      assert.equal(userAccount.rewardPerTokenPending.toString(), '0');
      assert.equal(userAccount.balanceStaked.toString(), amount.toString());
      assert.equal(userAccount.tier.toString(), '1');
    });

    it('increase tier', async () => {
      await initializePool(false);
      await createUser();

      const amount = new anchor.BN(2_000_000_000);

      await stake(amount);
      await stake(amount);

      let userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.tier.toString(), '1');

      await stake(amount);

      userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.tier.toString(), '2');
    });

    it('do not update tier if no allocation', async () => {
      await initializePool(true);
      await createUser();

      const amount = new anchor.BN(2_000_000_000);

      await stake(amount);

      const userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.tier.toString(), '0');
    });
  });

  describe('unstake', () => {
    it('update tier', async () => {
      await initializePool(false);
      await createUser();
      await stake(new anchor.BN(6_000_000_000));

      await unstake(new anchor.BN(5_000_000_000));

      const userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.pool.toString(), pool.publicKey.toString());
      assert.equal(userAccount.owner.toString(), wallet.publicKey.toString());
      assert.equal(userAccount.rewardPerTokenComplete.toString(), '0');
      assert.equal(userAccount.rewardPerTokenPending.toString(), '0');
      assert.equal(userAccount.tier.toString(), '0');
    });

    it('do not update tier if no allocation', async () => {
      await initializePool(true);
      await createUser();
      await stake(new anchor.BN(6_000_000_000));

      await unstake(new anchor.BN(1_000_000_000));

      const userAccount = await stakingProgram.account.user.fetch(user);
      assert.equal(userAccount.tier.toString(), '0');
    });
  });

  const initializePool = async (noTier: boolean) => {
    await stakingProgram.rpc.initializePool(
      nonce,
      rewardDuration,
      lockPeriod,
      noTier,
      {
        accounts: {
          authority: wallet.publicKey,
          stakingMint: stakingMint.publicKey,
          stakingVault,
          rewardMint: rewardMint.publicKey,
          rewardVault,
          poolSigner: poolSigner,
          pool: pool.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [pool],
        instructions: [
          await stakingProgram.account.pool.createInstruction(pool),
        ],
      },
    );
  };

  const createUser = async () => {
    ownerTokenAccount = await stakingMint.createAccount(wallet.publicKey);
    await stakingMint.mintTo(
      ownerTokenAccount,
      wallet.payer,
      [],
      100000000000000,
    );

    let [_user, nonce] = await anchor.web3.PublicKey.findProgramAddress(
      [wallet.publicKey.toBuffer(), pool.publicKey.toBuffer()],
      stakingProgram.programId,
    );
    user = _user;

    await stakingProgram.rpc.createUser({
      accounts: {
        pool: pool.publicKey,
        user,
        owner: wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
    });
  };

  const stake = async (amount: anchor.BN) => {
    await stakingProgram.rpc.stake(amount, {
      accounts: {
        pool: pool.publicKey,
        stakingVault,
        user: user,
        owner: wallet.publicKey,
        stakeFromAccount: ownerTokenAccount,
        poolSigner: poolSigner,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });
  };

  const unstake = async (amount: anchor.BN) => {
    await stakingProgram.rpc.unstake(amount, {
      accounts: {
        pool: pool.publicKey,
        stakingVault,
        user: user,
        owner: wallet.publicKey,
        stakeFromAccount: ownerTokenAccount,
        poolSigner: poolSigner,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });
  };
});
