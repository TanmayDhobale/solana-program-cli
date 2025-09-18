use anyhow::Result;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use crate::idl_loader::IdlLoader;

pub struct BorshEncoder;

impl BorshEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_instruction(
        &self,
        idl_loader: &IdlLoader,
        program_id: &str,
        instruction_name: &str,
        args: HashMap<String, Value>,
    ) -> Result<Vec<u8>> {
       
        let discriminator = idl_loader.get_discriminator(program_id, instruction_name)?;
        let mut instruction_data = discriminator.to_vec();

       
        let instruction = idl_loader.get_instruction(program_id, instruction_name)?;

       
        for arg_def in &instruction.args {
            if let Some(value) = args.get(&arg_def.name) {
                let encoded_arg = self.encode_value(value, &arg_def.field_type)?;
                instruction_data.extend_from_slice(&encoded_arg);
            } else {
                return Err(anyhow::anyhow!("Missing required argument: {}", arg_def.name));
            }
        }

        Ok(instruction_data)
    }

   
    fn encode_value(&self, value: &Value, field_type: &str) -> Result<Vec<u8>> {
        match field_type {
            "u8" => {
                let val = value.as_u64().ok_or_else(|| anyhow::anyhow!("Expected u8"))? as u8;
                Ok(val.to_le_bytes().to_vec())
            }
            "u16" => {
                let val = value.as_u64().ok_or_else(|| anyhow::anyhow!("Expected u16"))? as u16;
                Ok(val.to_le_bytes().to_vec())
            }
            "u32" => {
                let val = value.as_u64().ok_or_else(|| anyhow::anyhow!("Expected u32"))? as u32;
                Ok(val.to_le_bytes().to_vec())
            }
            "u64" => {
                let val = value.as_u64().ok_or_else(|| anyhow::anyhow!("Expected u64"))?;
                Ok(val.to_le_bytes().to_vec())
            }
            "i8" => {
                let val = value.as_i64().ok_or_else(|| anyhow::anyhow!("Expected i8"))? as i8;
                Ok(val.to_le_bytes().to_vec())
            }
            "i16" => {
                let val = value.as_i64().ok_or_else(|| anyhow::anyhow!("Expected i16"))? as i16;
                Ok(val.to_le_bytes().to_vec())
            }
            "i32" => {
                let val = value.as_i64().ok_or_else(|| anyhow::anyhow!("Expected i32"))? as i32;
                Ok(val.to_le_bytes().to_vec())
            }
            "i64" => {
                let val = value.as_i64().ok_or_else(|| anyhow::anyhow!("Expected i64"))?;
                Ok(val.to_le_bytes().to_vec())
            }
            "f32" => {
                let val = value.as_f64().ok_or_else(|| anyhow::anyhow!("Expected f32"))? as f32;
                Ok(val.to_le_bytes().to_vec())
            }
            "f64" => {
                let val = value.as_f64().ok_or_else(|| anyhow::anyhow!("Expected f64"))?;
                Ok(val.to_le_bytes().to_vec())
            }
            "bool" => {
                let val = value.as_bool().ok_or_else(|| anyhow::anyhow!("Expected bool"))?;
                Ok(vec![if val { 1u8 } else { 0u8 }])
            }
            "string" => {
                let string_val = value.as_str().ok_or_else(|| anyhow::anyhow!("Expected string"))?;
                let mut result = Vec::new();
               
                result.extend_from_slice(&(string_val.len() as u32).to_le_bytes());
                result.extend_from_slice(string_val.as_bytes());
                Ok(result)
            }
            "pubkey" => {
                let pubkey_str = value.as_str().ok_or_else(|| anyhow::anyhow!("Expected pubkey string"))?;
                let pubkey = Pubkey::try_from(pubkey_str)
                    .map_err(|_| anyhow::anyhow!("Invalid pubkey: {}", pubkey_str))?;
                Ok(pubkey.to_bytes().to_vec())
            }
            _ => {
               
                Err(anyhow::anyhow!("Unsupported type: {}", field_type))
            }
        }
    }
}


#[macro_export]
macro_rules! args {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut map = std::collections::HashMap::new();
            $(
                map.insert($key.to_string(), serde_json::to_value($value).unwrap());
            )*
            map
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_value() {
        let encoder = BorshEncoder::new();
        
       
        let result = encoder.encode_value(&json!(1000000), "u64").unwrap();
        assert_eq!(result, 1000000u64.to_le_bytes().to_vec());
        
       
        let result = encoder.encode_value(&json!("hello"), "string").unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(&5u32.to_le_bytes()); 
        expected.extend_from_slice(b"hello");
        assert_eq!(result, expected);
        
       
        let result = encoder.encode_value(&json!(true), "bool").unwrap();
        assert_eq!(result, vec![1u8]);
    }
}
