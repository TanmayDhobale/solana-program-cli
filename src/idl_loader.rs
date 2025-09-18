use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: [u8; 8],
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlAccount {
    pub name: String,
    #[serde(default)]
    pub writable: bool,
    #[serde(default)]
    pub signer: bool,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramIdl {
    pub address: String,
    pub instructions: Vec<IdlInstruction>,
    pub errors: Option<Vec<IdlError>>,
}

pub struct IdlLoader {
    idls: HashMap<String, ProgramIdl>,
}

impl IdlLoader {
    pub fn new() -> Self {
        Self {
            idls: HashMap::new(),
        }
    }

   
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P, program_id: &str) -> Result<()> {
        let content = fs::read_to_string(path)?;
        let idl: ProgramIdl = serde_json::from_str(&content)?;
        self.idls.insert(program_id.to_string(), idl);
        Ok(())
    }

   
    pub fn get_instruction(&self, program_id: &str, instruction_name: &str) -> Result<&IdlInstruction> {
        let idl = self.idls.get(program_id)
            .ok_or_else(|| anyhow::anyhow!("IDL not found for program: {}", program_id))?;
        
        idl.instructions.iter()
            .find(|inst| inst.name == instruction_name)
            .ok_or_else(|| anyhow::anyhow!("Instruction '{}' not found in IDL", instruction_name))
    }

   
    pub fn get_discriminator(&self, program_id: &str, instruction_name: &str) -> Result<[u8; 8]> {
        let instruction = self.get_instruction(program_id, instruction_name)?;
        Ok(instruction.discriminator)
    }

   
    pub fn get_instructions(&self, program_id: &str) -> Result<&Vec<IdlInstruction>> {
        let idl = self.idls.get(program_id)
            .ok_or_else(|| anyhow::anyhow!("IDL not found for program: {}", program_id))?;
        Ok(&idl.instructions)
    }

   
    pub fn decode_error(&self, program_id: &str, error_code: u32) -> Option<String> {
        if let Some(idl) = self.idls.get(program_id) {
            if let Some(errors) = &idl.errors {
                for error in errors {
                    if error.code == error_code {
                        return Some(format!("{}: {}", error.name, error.msg));
                    }
                }
            }
        }
        None
    }

    
    pub fn list_programs(&self) -> Vec<&String> {
        self.idls.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idl_loader() {
        let mut loader = IdlLoader::new();
        
       
        assert_eq!(loader.list_programs().len(), 0);
    }
}
