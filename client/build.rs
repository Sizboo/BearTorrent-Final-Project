fn main () -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .type_attribute("connection.FileHash", "#[derive(Hash, Eq)]")
        .type_attribute("connection.PieceHash", "#[derive(Hash, Eq)]")
        .type_attribute("connection.ClientId", "#[derive(Hash, Eq)]")
        .type_attribute("connection.PeerId", "#[derive(Hash, Eq)]")
        .compile_protos(&["../protos/connection.proto"], &["../protos"])?;
    Ok(())
}