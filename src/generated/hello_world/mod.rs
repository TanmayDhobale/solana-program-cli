// Auto-generated Rust client for hello_world
// Generated from Anchor IDL using Codama

use anchor_lang::prelude::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;
use anyhow::Result;


pub const PROGRAM_ID: &str = "5PiuXarsz2F7Q6NpSCtdBbK6vroQWiGSdJZW3fPkjWHw";

pub fn program_id() -> Pubkey {
    Pubkey::from_str(PROGRAM_ID).unwrap()
}


pub const GET_MESSAGE_DISCRIMINATOR: [u8; 8] = [159, 69, 186, 171, 244, 131, 99, 223];

pub fn get_message_instruction(
    hello_world_account: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&GET_MESSAGE_DISCRIMINATOR);

    let accounts = vec![
        AccountMeta::new_readonly(hello_world_account, false), // hello_world_account
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


pub const INITIALIZE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];

pub fn initialize_instruction(
    message: String,
    hello_world_account: Pubkey,
    user: Pubkey,
    system_program: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&INITIALIZE_DISCRIMINATOR);
    

        let message_bytes = message.as_bytes();
    data.extend_from_slice(&(message_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(message_bytes);

    let accounts = vec![
        AccountMeta::new_readonly(hello_world_account, false), // hello_world_account
        AccountMeta::new_readonly(user, false), // user
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


pub const UPDATE_MESSAGE_DISCRIMINATOR: [u8; 8] = [23, 135, 34, 211, 96, 120, 107, 9];

pub fn update_message_instruction(
    new_message: String,
    hello_world_account: Pubkey,
    user: Pubkey,
) -> Result<Instruction> {
    let mut data = Vec::new();
    data.extend_from_slice(&UPDATE_MESSAGE_DISCRIMINATOR);
    
   
        let new_message_bytes = new_message.as_bytes();
    data.extend_from_slice(&(new_message_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(new_message_bytes);

    let accounts = vec![
        AccountMeta::new_readonly(hello_world_account, false), // hello_world_account
        AccountMeta::new_readonly(user, false), // user
    ];

    Ok(Instruction {
        program_id: program_id(),
        accounts,
        data,
    })
}


#[derive(Debug, Clone)]
pub struct HelloWorldAccount {
}

