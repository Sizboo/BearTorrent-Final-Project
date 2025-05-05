use std::fs::{File, read_dir, create_dir, exists};
use sha1::{Sha1, Digest};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

// This function generates the info hash of a given file.
// Client files will be located in client/resources.
// Returns: u32 file hash
fn hash_file_u32<P: AsRef<Path>>(path: P) -> std::io::Result<u32> {
    let mut hasher = Sha1::new();
    let mut file = BufReader::new(File::open(path)?);
    let mut buf = [0u8; 4096];

    // Read through the file and update the hasher
    while let Ok(size) = file.read(&mut buf[..]) {
        if size == 0 {
            break;
        }
        hasher.update(&buf[0..size]);
    }

    // Now finish generating the hash result and return it
    let result = hasher.finalize();
    let hash = u32::from_be_bytes([result[0], result[1], result[2], result[3]]);
    Ok(hash)
}

// This function goes through the client's resource directory
// to generate info hashes for each file
// Returns: Vec<u32>
pub(crate) fn get_file_hashes() -> std::io::Result<Vec<u32>> {
    let mut results = vec![];
    
    // Get the path of the client's resources.
    // If it doesn't exist, it is created. 
    let dir = match exists("resources"){
        Ok(true) => PathBuf::from("resources"),
        Ok(false) => {
            create_dir("resources")?;
            PathBuf::from("resources")
        }
        Err(e) => Err(e)?
    };

    // For each file in "/resources", request the hash and append to results
    for file in read_dir(dir)? {
        let file = file?;
        let path = file.path();
        if path.is_file() {
            // generate the file hash
            if let Ok(hash) = hash_file_u32(&path)  {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    // TODO swap println! to debug! if needed
                    println!("Found file {:?}, with hash {:?}", file_name, hash);
                    // append the generated hash to results
                    results.push(hash);
                }
            }
        }
    }

    // Return the list of hashes
    Ok(results)

}