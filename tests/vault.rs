use starframe_vault::*;

#[cfg(test)]
mod tests {
    // bring program type into scope only when IDL generation test is enabled
    #[cfg(feature = "idl")]
    use starframe_vault::VaultProgram;

    #[test]
    fn test_vault_initialization() {
        // Test logic will be implemented here
        // This is a placeholder for Star Frame testing utilities
        println!("Vault initialization test");
    }

    #[test]
    fn test_vault_deposit() {
        // Test logic will be implemented here
        println!("Vault deposit test");
    }

    #[test]
    fn test_vault_withdraw() {
        // Test logic will be implemented here
        println!("Vault withdraw test");
    }

    #[test]
    fn test_vault_close() {
        // Test vault close functionality
        println!("Vault close test");
    }

    #[test]
    fn test_authority_validation() {
        // Test authority validation
        println!("Authority validation test");
    }

    #[cfg(feature = "idl")]
    #[test]
    fn generate_idl() -> anyhow::Result<()> {
        use star_frame::prelude::*;
        let idl = VaultProgram::program_to_idl()?;
        let codama_idl: ProgramNode = idl.try_into()?;
        let idl_json = codama_idl.to_json()?;
        std::fs::write("idl.json", &idl_json)?;
        Ok(())
    }
}
