use solana_program::pubkey::Pubkey;
use anyhow::Result;

const COIN_VAULT_OFFSET: usize = 336; // coinVault/tokenVaultA
const PC_VAULT_OFFSET: usize = 368; // pcVault/tokenVaultB
const COIN_MINT_OFFSET: usize = 400; // coinMint/tokenMintA
const PC_MINT_OFFSET: usize = 432; // pcMint/tokenMintB

#[derive(Debug)]
pub struct RaydiumAmmInfo {
    pub coin_mint: Pubkey,
    pub pc_mint: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
}

impl RaydiumAmmInfo {
    fn slice_to_pubkey(data: &[u8], start: usize, end: usize) -> Pubkey {
        Pubkey::new_from_array(
            data[start..end]
                .try_into()
                .expect(&format!("Failed to convert slice [{}..{}] to 32-byte array", start, end))
        )
    }

    pub fn load_checked(data: &[u8]) -> Result<Self> {
        if data.len() < PC_MINT_OFFSET + 32 {
            return Err(anyhow::anyhow!("Invalid data length for RaydiumAmmInfo"));
        }
        
        let coin_vault = Self::slice_to_pubkey(&data, COIN_VAULT_OFFSET, COIN_VAULT_OFFSET + 32);
        let pc_vault = Self::slice_to_pubkey(&data, PC_VAULT_OFFSET, PC_VAULT_OFFSET + 32);
        let coin_mint = Self::slice_to_pubkey(&data, COIN_MINT_OFFSET, COIN_MINT_OFFSET + 32);
        let pc_mint = Self::slice_to_pubkey(&data, PC_MINT_OFFSET, PC_MINT_OFFSET + 32);
        
        Ok(Self {
            coin_mint,
            pc_mint,
            coin_vault,
            pc_vault,
        })
    }
}
