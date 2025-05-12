use std::collections::HashMap;
use std::fs::{DirEntry, File, read_dir, exists, create_dir_all, OpenOptions, rename, remove_file, copy};
use sha1::{Sha1, Digest};
use std::io::{read_to_string, BufReader, Read, Seek, SeekFrom, Write};
use std::iter::Map;
use std::path::{Path, PathBuf};
use crate::connection::*;
use hex::encode;
// #[derive(Debug, Clone)]
// pub struct InfoHash{
//     pub(crate) name: String, // Name of the file
//     file_length: u64, // Size of the file in bytes
//     piece_length: u32, // Number of bytes per piece
//     pub(crate) pieces: Vec<[u8;20]>, // hash list of the pieces
// }

// Represents the status of the piece download inside a vector.
// This vector represents the status of each piece, and whether
// it has been fully received or not.
#[derive(Debug)]
pub struct Status{
    pub(crate) pieces_status: Vec<u8> // Using u8 for simplified reading and writing
}

// This struct is used to track the status of the pieces, whether they have been built or not
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

    // Checks whether all the pieces have been assembled
    pub fn has_all_pieces(&self) -> bool {
        if self.pieces_status.is_empty(){
            return true;
        }
        // Go through all piece values and return whether they are all set to true
        self.pieces_status.iter().all(|value| *value == 1)
    }
}

// The InfoHash struct stores necessary information for requesting and advertising
// a client's file. It closely resembles a .torrent file
impl connection::InfoHash {
    // Generate the info hash struct given a file from resources/files
    pub fn new(file: DirEntry) -> std::io::Result<Self> {
        let path = file.path(); // PathBuf of the file

        // Name of the file
        let name = path.file_name().unwrap().to_str().unwrap().to_string();

        let (file_cache, is_new) = get_file_cache(name.clone());
        match is_new {

            // Generate the missing .fileinfo file
            false =>{
                // Byte length of the file
                let file_length = file.path().metadata()?.len();
                // Size of the pieces
                let piece_length = Self::get_piece_length(file_length);
                // Vector of piece hashes
                let pieces = Self::get_piece_hashes(path, piece_length as usize, file_length as usize)?;
                
                // Create the new cache file to improve load time
                let mut file = OpenOptions::new().write(true).open(file_cache)?;

                // Write each field as newlines, this helps since we have 2 variable length fields
                writeln!(file, "name: {}", name.clone())?;
                writeln!(file, "file_length: {}", file_length.clone())?;
                writeln!(file, "piece_length: {}", piece_length.clone())?;
                writeln!(file, "pieces:")?;

                // Write each piece on a newline
                for piece in &pieces {
                    let hex_hash = hex::encode(&piece.hash); // converts to hex string
                    writeln!(file, "{}", hex_hash)?;
                }
                
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
            // A cached file was identified, load it to save time
            true =>{
                let mut file = OpenOptions::new().read(true).open(file_cache)?;
                
                // Load content from the cache file
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let mut lines = contents.lines();

                // Gets the name entry
                let name = lines.next().ok_or("Missing name").unwrap().strip_prefix("name: ").ok_or("Invalid name line").unwrap().to_string();

                // Gets the file_length entry
                let file_length = lines.next().ok_or("Missing file_length").unwrap().strip_prefix("file_length: ").ok_or("Invalid file_length").unwrap().parse::<u64>().unwrap();

                // Gets the piece_length entry
                let piece_length = lines.next().ok_or("Missing piece_length").unwrap().strip_prefix("piece_length: ").ok_or("Invalid piece_length").unwrap().parse::<u32>().unwrap();

                // Gets the piece entry
                let header = lines.next().ok_or("Missing 'pieces:' line").unwrap();
                if header.trim() != "pieces:" {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid header"));
                }

                // Read pieces
                let mut pieces = Vec::new();
                for line in lines {
                    let bytes = hex::decode(line).unwrap();
                    pieces.push(connection::PieceHash { hash: bytes });
                }

                println!("Loaded {:?} from cache", name.clone());

                Ok(connection::InfoHash{
                    name,
                    file_length,
                    piece_length,
                    pieces
                })
            }
        }

    }

    // Generates a vector containing 20-byte SHA1 hash of each piece from a file
    fn get_piece_hashes<P: AsRef<Path>>(path: P, piece_length: usize, file_length: usize) -> std::io::Result<Vec<connection::PieceHash>>{
        let mut file_reader = BufReader::new(File::open(path)?);
        let mut buf = vec![0u8;piece_length];
        let mut pieces: Vec<[u8;20]> = Vec::new();

        // Loop through the file, reading in chunks from bytes_read to piece_length
        loop {

            let mut bytes_read = 0; // total bytes read
            
            // Read whole file
            while bytes_read < piece_length{
                // read segments of the file as pieces
                buf = vec![0u8;piece_length];
                let n = file_reader.read(&mut buf[bytes_read..piece_length])?;
                if n == 0 || buf.iter().all(|value| *value == 0) { // EOF
                    bytes_read = 0;
                    break;
                }
                bytes_read += n;
            }
            // If nothing or EOF was read, no need to hash, break loop
            if bytes_read == 0 {
                break;
            }

            // Hash the piece of data that was read
            pieces.push(hash_piece_data(buf.to_vec()));
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

// Gets the .info file
fn get_info_file(file_name: String) -> (PathBuf, bool) {
    let (path, is_new) = get_temp_file(file_name, ".info".to_string(), "cache".to_string()).unwrap();
    (path, is_new)
}

// Gets the .infofile file
fn get_file_cache(file_name: String) -> (PathBuf, bool) {
    let (path, is_new) = get_temp_file(file_name, ".filecache".to_string(), "cache".to_string()).unwrap();
    (path, is_new)
}

// Gets the actual file
fn get_file(file_name: String) -> PathBuf {
    let (path, is_new) = get_temp_file(file_name, "".to_string(), "files".to_string()).unwrap();
    path
}

// Create the files directory if it doesn't exist
fn get_client_files_dir() -> std::io::Result<(PathBuf)> {
    let dir = Path::new("resources/files");
    if !dir.exists(){
        create_dir_all(dir)?;
    }
    Ok(dir.to_path_buf())
}

// Create the cache directory for .part and .info files if it doesn't exist
fn get_client_cache_dir() -> std::io::Result<(PathBuf)> {
    let dir = Path::new("resources/cache");
    if !dir.exists(){
        create_dir_all(dir)?;
    }
    Ok(dir.to_path_buf())
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
        // .info existed before, so we can read from it
        true => {
            let mut buffer: Vec<u8> = Vec::new();
            let mut buf = [0u8; 1];
            // Read each u8 into the buffer
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
        // If the .info has never been generated before, construct the file
        false => {
            let pieces= vec![0u8;info_hash.pieces.len()];
            info_file.write_all(&pieces).unwrap();

            Status{
                pieces_status: pieces
            }
        }
    }
}

// This function generates a 20-btye SHA1 has of a vector of u8's
pub(crate) fn hash_piece_data(buf: Vec<u8>) -> [u8;20]{
    let mut hasher = Sha1::new();
    hasher.update(&buf[..]); // update hash with any data in current buffer

    // Finalize result of the hash, append 20-byte result to the pieces vector
    let result = hasher.finalize();
    let bytes: [u8; 20] = result.try_into().unwrap();
    println!("{:?}", buf.clone());
    bytes.into()
}

// Deletes a file and its associated cache files from resources, if any exist
pub(crate) fn delete_file(file_name: String) -> std::io::Result<()> {
    let file_path = format!("resources/files/{}", file_name);
    let (info_path, _) = get_info_file(file_name.clone());
    let part_path = get_part_file(file_name.clone());
    let (cache_path,_) = get_file_cache(file_name);
    if exists(Path::new(&file_path))? {
        match remove_file(file_path){
            Err(e) => eprintln!("{}", e),
            _ => {}
        };
        match remove_file(part_path){
            Err(e) => eprintln!("{}", e),
            _ => {}
        };
        match remove_file(cache_path){
            Err(e) => eprintln!("{}", e),
            _ => {}
        };
        match remove_file(info_path){
            Err(e) => eprintln!("{}", e),
            _ => {}
        };
    }
    Ok(())
}

// Copies a file from the frontend to the resource directory, given a direct path
pub(crate) fn add_file(path: String) -> std::io::Result<()> {
    let file_path = Path::new(&path);
    let dest_folder = Path::new("resources/files");
    let dest = dest_folder.join(file_path.file_name().unwrap());

    println!("Copying file into: {:?}", dest);
    match copy(file_path, dest){
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }

}

// If the file can be completed, the .info cache file is removed and the .part
// file moves to resources/files removing its extension
pub(crate) fn build_file(info_hash: connection::InfoHash) -> Result<(), Box<dyn std::error::Error>> {
    match is_file_complete(info_hash.clone()) {
        true => {
            // Get both cached files
            println!("Printing: {:?}", info_hash.name.clone());
            let part_file = get_part_file(info_hash.name.clone());
            let (info_file, _) = get_info_file(info_hash.name.clone());

            // New target file path
            let new_file_name = format!("resources/files/{}", info_hash.name);

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