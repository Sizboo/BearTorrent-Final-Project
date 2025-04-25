fn main () -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .type_attribute("connection.ClientId", "#[derive(Hash, Eq)]")
        .compile_protos(&["protos/connection.proto"], &["protos"])?;
    Ok(())
}