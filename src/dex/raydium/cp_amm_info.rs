use solana_program::pubkey::Pubkey;
use anyhow::Result;

const AMM_CONFIG_OFFSET: usize = 8; // amm_config
const POOL_CREATOR_OFFSET: usize = 40; // pool_creator
const TOKEN_0_VAULT_OFFSET: usize = 72; // token_0_vault
const TOKEN_1_VAULT_OFFSET: usize = 104; // token_1_vault
const LP_MINT_OFFSET: usize = 136; // lp_mint
const TOKEN_0_MINT_OFFSET: usize = 168; // token_0_mint
const TOKEN_1_MINT_OFFSET: usize = 200; // token_1_mint
const TOKEN_0_PROGRAM_OFFSET: usize = 232; // token_0_program
const TOKEN_1_PROGRAM_OFFSET: usize = 264; // token_1_program
const OBSERVATION_KEY_OFFSET: usize = 296; // observation_key

#[derive(Debug)]
pub struct RaydiumCpAmmInfo {
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub amm_config: Pubkey,
    pub observation_key: Pubkey,
}

impl RaydiumCpAmmInfo {
    fn slice_to_pubkey(data: &[u8], start: usize, end: usize) -> Pubkey {
        Pubkey::new_from_array(
            data[start..end]
                .try_into()
                .expect(&format!("Failed to convert slice [{}..{}] to 32-byte array", start, end))
        )
    }

    pub fn load_checked(data: &[u8]) -> Result<Self> {
        if data.len() < OBSERVATION_KEY_OFFSET + 32 {
            return Err(anyhow::anyhow!("Invalid data length for RaydiumCpAmmInfo"));
        }
        
        let token_0_vault = Self::slice_to_pubkey(&data, TOKEN_0_VAULT_OFFSET, TOKEN_0_VAULT_OFFSET + 32);
        let token_1_vault = Self::slice_to_pubkey(&data, TOKEN_1_VAULT_OFFSET, TOKEN_1_VAULT_OFFSET + 32);
        let token_0_mint = Self::slice_to_pubkey(&data, TOKEN_0_MINT_OFFSET, TOKEN_0_MINT_OFFSET + 32);
        let token_1_mint = Self::slice_to_pubkey(&data, TOKEN_1_MINT_OFFSET, TOKEN_1_MINT_OFFSET + 32);
        let amm_config = Self::slice_to_pubkey(&data, AMM_CONFIG_OFFSET, AMM_CONFIG_OFFSET + 32);
        let observation_key = Self::slice_to_pubkey(&data, OBSERVATION_KEY_OFFSET, OBSERVATION_KEY_OFFSET + 32);
        
        Ok(Self {
            token_0_mint,
            token_1_mint,
            token_0_vault,
            token_1_vault,
            amm_config,
            observation_key,
        })
    }
}
