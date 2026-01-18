use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

pub struct MeteoraDAmmV2Info {
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
}

impl MeteoraDAmmV2Info {
    fn slice_to_pubkey(data: &[u8], start: usize, end: usize) -> Pubkey {
        Pubkey::new_from_array(
            data[start..end]
                .try_into()
                .expect(&format!("Failed to convert slice [{}..{}] to 32-byte array", start, end))
        )
    }

    pub fn load_checked(data: &[u8]) -> Result<Self> {
        let base_mint = Self::slice_to_pubkey(&data, 168, 200);
        let quote_mint = Self::slice_to_pubkey(&data, 200, 232);
        let base_vault = Self::slice_to_pubkey(&data, 232, 264);
        let quote_vault = Self::slice_to_pubkey(&data, 264, 296);
        Ok(Self {
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
        })
    }
}
