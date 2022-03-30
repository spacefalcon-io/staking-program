import * as anchor from '@project-serum/anchor';
import { TOKEN_PROGRAM_ID, Token } from '@solana/spl-token';

export const createMint = async (
  provider: anchor.Provider,
  decimals: number,
): Promise<Token> => {
  const mint = await Token.createMint(
    provider.connection,
    (provider.wallet as anchor.Wallet).payer,
    provider.wallet.publicKey,
    null,
    decimals,
    TOKEN_PROGRAM_ID,
  );
  return mint;
};
