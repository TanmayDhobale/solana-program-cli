

use anchor_lang::prelude::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;
use anyhow::Result;

pub const PROGRAM_ID: &str = "Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY";

pub fn program_id() -> Pubkey {
    Pubkey::from_str(PROGRAM_ID).unwrap()
}


pub fn decode_error(code: u32) -> Option<&'static str> {
    match code {
        6000 => Some("Amount must be at least 0.001 SOL (1,000,000 lamports)"),
        6001 => Some("Unauthorized: sender does not own the send account"),
        _ => None,
    }
}


pub const GET_STATS_DISCRIMINATOR: [u8; 8] = [241, 65, 112, 185, 230, 140, 139, 177];

pub fn get_stats_instruction(
    send_account: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&GET_STATS_DISCRIMINATOR);

    let accounts = vec![
        AccountMeta::new_readonly(send_account, false), // send_account
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


pub const INITIALIZE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];

pub fn initialize_instruction(
    send_account: Pubkey,
    user: Pubkey,
    system_program: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&INITIALIZE_DISCRIMINATOR);

    let accounts = vec![
        AccountMeta::new(send_account, false), // send_account (writable, PDA)
        AccountMeta::new(user, true), // user (writable, signer)
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


pub const SEND_SOL_DISCRIMINATOR: [u8; 8] = [214, 24, 219, 18, 3, 205, 201, 179];

pub fn send_sol_instruction(
    amount: u64,
    recipient: Pubkey,
    send_account: Pubkey,
    sender: Pubkey,
    recipient_account: Pubkey,
    system_program: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&SEND_SOL_DISCRIMINATOR);
    
    // Serialize arguments
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(recipient.as_ref());

    let accounts = vec![
        AccountMeta::new(send_account, false), // send_account (writable, PDA)
        AccountMeta::new(sender, true), // sender (writable, signer)
        AccountMeta::new(recipient_account, false), // recipient (writable)
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


#[derive(Debug, Clone)]
pub struct SendAccount {
    pub owner: Pubkey,
    pub total_sent: u64,
    pub transactions_count: u64,
}

