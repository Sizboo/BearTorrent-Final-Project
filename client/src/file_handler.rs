use std::fs::{DirEntry, File, read_dir, create_dir, exists};
use sha1::{Sha1, Digest};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct InfoHash{
    name: String, // Name of the file
    file_length: u64, // Size of the file in bytes
    piece_length: u64, // Number of bytes per piece
    pieces: Vec<[u8;20]>, // hash list of the pieces
}

impl InfoHash {
    // Generate the info hash struct given a file
    pub fn new(file: DirEntry) -> std::io::Result<Self> {
        let path = file.path(); // PathBuf of the file

        // Name of the file
        let name = path.file_name().unwrap().to_str().unwrap().to_string(); // TODO less unwraps?
        // Byte length of the file
        let file_length = file.path().metadata()?.len();
        // Size of the pieces
        let piece_length = Self::get_piece_length(file_length);
        // Vector of piece hashes
        let pieces = Self::get_piece_hashes(path, piece_length as usize)?;

        println!("File length: {}", file_length);
        println!("Piece length: {}", piece_length);
        println!("Pieces: {:?}", pieces);
        println!("File name: {}", name);

        Ok(InfoHash{
            name,
            file_length,
            piece_length,
            pieces
        })

    }

    // Generates a vector containing 20-byte SHA1 hash of each piece from a file
    fn get_piece_hashes<P: AsRef<Path>>(path: P, piece_length: usize) -> std::io::Result<Vec<[u8;20]>>{
        let mut file_reader = BufReader::new(File::open(path)?);
        let mut buf = vec![0u8;piece_length];
        let mut pieces: Vec<[u8;20]> = Vec::new();

        // Loop through the file, reading in chunks from bytes_read to piece_length
        loop {

            let mut bytes_read = 0; // total bytes read

            // Read whole file
            while bytes_read < piece_length{
                // read segments of the file as pieces
                let n = file_reader.read(&mut buf[bytes_read..piece_length])?;
                if n == 0 { // EOF
                    break;
                }
                bytes_read += n;
            }
            // If nothing or EOF was read, no need to hash, break loop
            if bytes_read == 0 {
                break;
            }

            // Hash the piece of data that was read
            let mut hasher = Sha1::new();
            hasher.update(&buf[..]); // update hash with any data in current buffer

            // Finalize result of the hash, append 20-byte result to the pieces vector
            let result = hasher.finalize();
            let bytes: [u8; 20] = result.try_into().unwrap();
            pieces.push(bytes.into());
        }
        Ok(pieces)


    }

    // Determines the length of the pieces based on the length of the file
    fn get_piece_length(length: u64) -> u64 {
        match length {
            0..=67_108_864 => 65_536,             // ≤ 64 MiB → 64 KiB
            67_108_865..=536_870_912 => 262_144,  // ≤ 512 MiB → 256 KiB
            536_870_913..=1_073_741_824 => 524_288, // ≤ 1 GiB → 512 KiB
            _ => 1_048_576, // ≤ 4 GiB → 1 MiB
        }
    }

    // Generate the 20-byte SHA1 hash of the InfoHash
    pub fn get_hashed_info_hash(&self) -> [u8; 20] {
        // Hash all members of the info hash
        let mut hasher = Sha1::new();
        hasher.update(self.file_length.to_be_bytes());
        hasher.update(self.piece_length.to_be_bytes());
        hasher.update(self.name.as_bytes());
        for piece in &self.pieces {
            hasher.update(piece);
        }

        // Finalize the hash and generate the 20-byte value
        let result = hasher.finalize();
        let bytes: [u8; 20] = result.try_into().unwrap();
        bytes
    }
}

// This function goes through the client's resource directory
// to generate info hashes for each file
// Returns: Vec<InfoHash>
pub(crate) fn get_info_hashes() -> std::io::Result<Vec<InfoHash>> {
    let mut results: Vec<InfoHash> = Vec::new();
    
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

        // If the entry is a file, create InfoHash and append
        if path.is_file() {
            results.push(InfoHash::new(file)?)
        }
    }

    // Return the list of hashes
    Ok(results)

}