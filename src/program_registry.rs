use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramRoute {
    GeneratedSendProgram,
    GeneratedHelloWorld,
    Dynamic,
}

pub struct ProgramRegistry;

impl ProgramRegistry {
    pub fn resolve(program_id: &Pubkey) -> ProgramRoute {
        match program_id.to_string().as_str() {
            "Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY" => ProgramRoute::GeneratedSendProgram,
            
            "5PiuXarsz2F7Q6NpSCtdBbK6vroQWiGSdJZW3fPkjWHw" => ProgramRoute::GeneratedHelloWorld,
            _ => ProgramRoute::Dynamic,
        }
    }
}


