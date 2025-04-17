use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .compile_protos(&["./protos/connection.proto"], &["./protos"])?;
    Ok(())
}
