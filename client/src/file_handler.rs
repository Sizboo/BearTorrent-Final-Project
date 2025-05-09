use std::collections::HashMap;
use std::fs::{DirEntry, File, read_dir, exists, create_dir_all, OpenOptions, rename, remove_file};
use sha1::{Sha1, Digest};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::iter::Map;
use std::path::{Path, PathBuf};
use crate::connection::*;

// #[derive(Debug, Clone)]
// pub struct InfoHash{
//     pub(crate) name: String, // Name of the file
//     file_length: u64, // Size of the file in bytes
//     piece_length: u32, // Number of bytes per piece
//     pub(crate) pieces: Vec<[u8;20]>, // hash list of the pieces
// }

// Represents the status of the piece download
#[derive(Debug)]
pub struct Status{
    pub(crate) pieces_status: Vec<u8> // Using u8 for simplified reading and writing
}

impl Status{

    // Converts the status array to a vector of bools for easy use as needed
    pub fn to_bool_vec(&self) -> Vec<bool>{
        let mut bool_vec: Vec<bool> = vec![];
        // Convert non-zero to true, and zero to false
        for value in self.pieces_status.iter() {
            bool_vec.push(*value != 0);
        }
        bool_vec
    }

    pub fn has_all_pieces(&self) -> bool {
        if self.pieces_status.is_empty(){
            return true;
        }
        self.pieces_status.iter().all(|value| *value == 1)
    }
}

impl connection::InfoHash {
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
        println!("Pieces: {:x?}", pieces);
        println!("File name: {}", name);

        Ok(connection::InfoHash{
            name,
            file_length,
            piece_length,
            pieces
        })

    }

    // Generates a vector containing 20-byte SHA1 hash of each piece from a file
    fn get_piece_hashes<P: AsRef<Path>>(path: P, piece_length: usize) -> std::io::Result<Vec<connection::PieceHash>>{
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
        let piece_hashes = pieces.iter().map(|piece| connection::PieceHash{
            hash: piece.to_vec()
        }).collect();

        Ok(piece_hashes)

    }

    // Determines the length of the pieces based on the length of the file
    fn get_piece_length(length: u64) -> u32 {
        match length {
            0..=67_108_864 => 65_536,             // ≤ 64 MiB → 64 KiB
            67_108_865..=536_870_912 => 262_144,  // ≤ 512 MiB → 256 KiB
            536_870_913..=1_073_741_824 => 524_288, // ≤ 1 GiB → 512 KiB
            _ => 1_048_576, // ≤ 4 GiB → 1 MiB
        }
    }

    // Generate the 20-byte SHA1 hash of the InfoHash
    pub  fn get_hashed_info_hash(&self) -> [u8; 20] {
        // Hash all members of the info hash
        let mut hasher = Sha1::new();
        hasher.update(self.file_length.to_be_bytes());
        hasher.update(self.piece_length.to_be_bytes());
        hasher.update(self.name.as_bytes());
        for piece in &self.pieces {
            hasher.update(piece.hash.as_slice());
        }

        // Finalize the hash and generate the 20-byte value
        let result = hasher.finalize();
        let bytes: [u8; 20] = result.try_into().unwrap();
        bytes
    }

    // pub fn get_server_info_hash(&self) ->  connection::InfoHash {
    //
    //     let pieces = self.pieces.iter().map(|x| connection::PieceHash{
    //         hash: Vec::from(x)
    //     }).collect::<Vec<_>>();
    //
    //     connection::InfoHash {
    //         name: self.name.clone(),
    //         file_length: self.file_length.clone(),
    //         piece_length: self.piece_length.clone() as u64,
    //         pieces,
    //     }
    // }
    //
    // pub fn server_to_client_hash(server_info_hash: connection::InfoHash) -> InfoHash {
    //
    //     let pieces = server_info_hash.pieces.iter().map(|x| x.hash.clone().try_into().unwrap()).collect::<Vec<_>>();
    //
    //     InfoHash {
    //         name: server_info_hash.name,
    //         file_length: server_info_hash.file_length,
    //         piece_length: server_info_hash.piece_length as u32,
    //         pieces,
    //     }
    // }
}

// Checks if a file exists, if it doesn't then it is created.
// Returns the PathBuf to this file
fn get_temp_file(file_name: String, extension: String, src: String) -> std::io::Result<(PathBuf, bool)> {
    let temp_file_name = format!("resources/{}/{}{}", src, file_name, extension);
    let temp_file:(PathBuf, bool) = match exists(Path::new(&temp_file_name)) {
        Ok(true) => (PathBuf::from(temp_file_name), true),
        Ok(false) => {
            File::create(&temp_file_name)?;
            (PathBuf::from(temp_file_name), false)
        }
        Err(e) => return Err(e),
    };
    Ok(temp_file)
}

// Get the .part of the specified file
fn get_part_file(file_name: String) -> PathBuf {
    let (path, is_new) = get_temp_file(file_name, ".part".to_string(), "cache".to_string()).unwrap();
    path
}

fn get_info_file(file_name: String) -> (PathBuf, bool) {
    let (path, is_new) = get_temp_file(file_name, ".info".to_string(), "cache".to_string()).unwrap();
    (path, is_new)
}

fn get_file(file_name: String) -> PathBuf {
    let (path, is_new) = get_temp_file(file_name, "".to_string(), "files".to_string()).unwrap();
    path
}

// Create the files directory if it doesn't exist
fn get_client_files_dir() -> std::io::Result<(PathBuf)> {
    let dir = match create_dir_all("resources/files"){
        Ok(dir) => PathBuf::from("resources/files"),
        Err(e) => return Err(e),
    };
    Ok(dir)
}

// Create the cache directory for .part and .info files if it doesn't exist
fn get_client_cache_dir() -> std::io::Result<PathBuf> {
    let cache = match create_dir_all("resources/cache") {
        Ok(c) => PathBuf::from("resources/cache"),
        Err(e) => return Err(e),
    };
    Ok(cache)
}

// Checks the client for resources directory containing cache and files.
// If they don't exist, they are created.
fn verify_client_dir_setup() -> () {
    // Create the cache directory for .part and .info files
    let cache = get_client_cache_dir();

    // Create the files directory if it doesn't exist
    let dir = get_client_files_dir();
}

// Returns the Status struct that represents the status of the file download
fn get_info_status(info_hash: connection::InfoHash) -> Status {
    let (path, is_new) = get_info_file(info_hash.name);
    let mut info_file = OpenOptions::new().write(true).read(true).open(&path).unwrap();
    match is_new {
        // If the .info has never been generated before, construct the file
        true => {
            let mut buffer: Vec<u8> = Vec::new();
            let mut buf = [0u8; 1];
            while let Ok(n) = info_file.read(&mut buf){
                if n == 0 {
                    break;
                }
                buffer.append(&mut buf.to_vec());
            }

            Status{
                pieces_status: buffer
            }
        }
        // .info existed before, so we can read from it
        false => {
            let pieces= vec![0u8;info_hash.pieces.len()];
            info_file.write_all(&pieces).unwrap();

            Status{
                pieces_status: pieces
            }
        }
    }
}

// If the file can be completed, the .info cache file is removed and the .part
// file moves to resources/files removing the extension
pub(crate) fn build_file(info_hash: connection::InfoHash) -> Result<(), Box<dyn std::error::Error>> {
    match is_file_complete(info_hash.clone()) {
        true => {
            // Get both cached files
            let part_file = get_part_file(info_hash.name.clone());
            let (info_file, _) = get_info_file(info_hash.name.clone());

            // New target file path
            let new_file_name = format!("../files/{}", info_hash.name);

            // Check if the file already exists to prevent overwriting
            if exists(Path::new(&new_file_name))?{
                return Err("File already exists in resources/files, cannot build!".into());
            }

            // Move the .part file to resources/files
            rename(part_file, new_file_name)?;

            // remove the .info file
            remove_file(info_file)?;

            Ok(())
        },
        false => Err("Missing pieces for file, cannot build!".into())
    }
}

// Returns a bool whether all the pieces have been received
pub(crate) fn is_file_complete(info_hash: connection::InfoHash) -> bool {
    get_info_status(info_hash).has_all_pieces()
}

// This function writes a piece to a .part file
pub(crate) fn write_piece_to_part(info_hash: connection::InfoHash, piece: Vec<u8>, piece_index: u32) -> std::io::Result<()> {
    // Verify cache directory exists
    let dir = get_client_cache_dir()?;

    // Creates the .part file if it doesn't exist, returns PathBuf of this file
    let part_path = get_part_file(info_hash.name.clone());

    // Open the .part file
    let mut part_file = OpenOptions::new().write(true).open(&part_path)?;

    // Seek to the index we need to write to, write the piece, flush the buffer
    // TODO check that seeking ahead in an empty file doesn't cause issues
    part_file.seek(SeekFrom::Start(((piece_index) * info_hash.piece_length) as u64))?;
    part_file.write_all(&piece)?;
    part_file.flush()?;

    // Update the status of the piece within the .info file
    let mut info_status = get_info_status(info_hash.clone());
    // for status in info_status.pieces_status.iter_mut(){
    //     println!("Piece {}", status);
    // }
    info_status.pieces_status[piece_index as usize] = 1u8; // Sets piece at index to true

    // Get the .info file and seek to the byte that represents this piece, set it to true
    let (info_file_path, is_new_info) = get_info_file(info_hash.name);
    let mut info_file = OpenOptions::new().write(true).open(&info_file_path)?;
    info_file.seek(SeekFrom::Start(piece_index as u64))?;
    info_file.write(&[1u8])?;
    info_file.flush()?;

    Ok(())

}

// Reads piece data from a file given an index
pub(crate) fn read_piece_from_file(info_hash: connection::InfoHash, piece_index: u32) -> std::io::Result<Vec<u8>>{
    let file_path = get_file(info_hash.name.clone());
    let mut file = OpenOptions::new().read(true).open(&file_path)?;

    let piece_length = if (info_hash.pieces.len() - 1) == piece_index as usize {
        info_hash.file_length as usize - (info_hash.piece_length as usize * (info_hash.pieces.len() - 1))
    } else {
        info_hash.piece_length as usize
    };

    let mut buf= vec![0u8;piece_length];

    file.seek(SeekFrom::Start((info_hash.piece_length * piece_index) as u64))?;
    file.read_exact(&mut buf)?;
    Ok(buf)
}

// This function goes through the client's resource directory
// to generate info hashes for each file
// Returns: Vec<InfoHash>
pub(crate) fn get_info_hashes() -> std::io::Result<HashMap<[u8;20], connection::InfoHash>> {
    let mut results: HashMap<[u8;20],connection::InfoHash> = HashMap::new();

    // Verify resources are set up, fetch downloaded files PathBuf
    verify_client_dir_setup();
    let dir = get_client_files_dir()?;

    // For each file in "/resources", request the hash and append to results
    for file in read_dir(dir)? {
        let file = file?;
        let path = file.path();

        // If the entry is a file, create InfoHash and append
        if path.is_file() {
            let temp_infohash = connection::InfoHash::new(file)?;
            results.insert(temp_infohash.get_hashed_info_hash(), temp_infohash);
        }
    }
    
    // Return the list of hashes
    Ok(results)

}