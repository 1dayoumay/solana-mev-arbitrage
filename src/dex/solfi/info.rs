use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

pub struct SolfiInfo {
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
}

impl SolfiInfo {
    fn slice_to_pubkey(data: &[u8], start: usize, end: usize) -> Pubkey {
        Pubkey::new_from_array(
            data[start..end]
                .try_into()
                .expect(&format!("Failed to convert slice [{}..{}] to 32-byte array", start, end))
        )
    }

    pub fn load_checked(data: &[u8]) -> Result<Self> {
        let base_mint = Self::slice_to_pubkey(&data, 2664, 2696);
        let quote_mint = Self::slice_to_pubkey(&data, 2696, 2728);
        let base_vault = Self::slice_to_pubkey(&data, 2736, 2768);
        let quote_vault = Self::slice_to_pubkey(&data, 2768, 2800);

        Ok(Self {
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
        })
    }
}
