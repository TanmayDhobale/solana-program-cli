use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_program;


pub const SEND_PROGRAM_ID: &str = "Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY";

pub struct SendClient {
    pub program_id: Pubkey,
}

impl SendClient {
    pub fn new() -> Self {
        let program_id = Pubkey::from_str_const(SEND_PROGRAM_ID);
        Self { program_id }
    }


    const DISC_INIT: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];

    const DISC_SEND_SOL: [u8; 8] = [77, 87, 52, 141, 228, 28, 116, 40];
    
    const DISC_GET_STATS: [u8; 8] = [197, 31, 142, 121, 22, 189, 77, 75];

    pub fn initialize(&self, send_account: Pubkey, user: Pubkey) -> Instruction {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&Self::DISC_INIT);
        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(send_account, true),
                AccountMeta::new(user, true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data,
        }
    }

    pub fn send_sol(&self, send_account: Pubkey, sender: Pubkey, recipient: Pubkey, amount: u64) -> Instruction {
        let mut data = Vec::with_capacity(8 + 8 + 32);
        data.extend_from_slice(&Self::DISC_SEND_SOL);
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(recipient.as_ref());
        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(send_account, false),
                AccountMeta::new(sender, true),
                AccountMeta::new(recipient, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data,
        }
    }

    pub fn get_stats(&self, send_account: Pubkey) -> Instruction {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&Self::DISC_GET_STATS);
        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new_readonly(send_account, false),
            ],
            data,
        }
    }
}
