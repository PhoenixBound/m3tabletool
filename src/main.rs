use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

fn main() {
    let mut args = std::env::args_os();
    if args.len() != 4 {
        println!("m3tabletool by phoenixbound");

        let program_name = args.next().unwrap_or_else(|| {
            OsString::from_str("m3tabletool").unwrap()
        });
        let program_name = if program_name.is_empty() { "m3tabletool".into() } else { program_name.to_string_lossy().into_owned() };
        println!("Usage:");
        println!("    {} unpack <extracted-table.bin> <output-directory>", program_name);
        println!("    {} pack <input-directory> <out-table.bin>", program_name);
        std::process::exit(1);
    }
    
    let command = args.nth(1).unwrap();
    let path1 = PathBuf::from(args.next().unwrap());
    let path2 = PathBuf::from(args.next().unwrap());
    
    if command == "unpack" {
        if let Err(e) = unpack_table(path1, path2) {
            eprintln!("Error while unpacking: {}", e);
            std::process::exit(1);
        }
    } else if command == "pack" {
        if let Err(e) = pack_table(path1, path2) {
            eprintln!("Error while packing: {}", e);
            std::process::exit(1);
        }
    } else {
        eprintln!("error: Unknown command {:?}", command);
        std::process::exit(1);
    }
}

fn offset_of_end(i: u16, entry_count: u16, table: &[u8]) -> usize {
    for j in i+1..entry_count+1 {
        let ofs = (4 + j * 4) as usize;
        let end = u32::from_le_bytes(table[ofs..ofs+4].try_into().unwrap()).try_into().unwrap();
        if end != 0 {
            return end;
        }
    }
    unreachable!();
}

fn unpack_table(table_path: PathBuf, mut out_directory: PathBuf) -> Result<(), std::io::Error> {
    let table = fs::read(table_path)?;
    let entry_count = u32::from_le_bytes(table[0..4].try_into().unwrap());
    let entry_count: u16 = entry_count.try_into().unwrap();
    let size_start: usize = ((entry_count + 1) * 4).try_into().unwrap();
    let expected_file_size = u32::from_le_bytes(table[size_start..size_start+4].try_into().unwrap());
    assert!(expected_file_size == table.len().try_into().unwrap());
    
    fs::create_dir_all(&out_directory)?;
    
    for i in 0u16..entry_count {
        // After all those try_into()s and unwrap()s up there I think there's no way this could possibly truncate
        let offset_start = (4 + i * 4) as usize;
        let start: usize = u32::from_le_bytes(table[offset_start..(offset_start+4)].try_into().unwrap())
                                             .try_into().unwrap();
        if start == 0 {
            // Output `i`.ignore file
            out_directory.push(format!("{}.ignore", i));
            fs::File::create(&out_directory).unwrap();
            out_directory.pop();
        } else {
            // Figure out size by seeing where the next entry's offset is
            let end: usize = offset_of_end(i, entry_count, &table);
            assert!(end > start);
            // Output i.bin
            out_directory.push(format!("{}.bin", i));
            fs::write(&out_directory, &table[start..end]).unwrap();
            out_directory.pop();
        }
    }
    
    Ok(())
}

fn pack_table(input_directory: PathBuf, table_path: PathBuf) -> Result<(), Box<dyn Error>> {
    // Read all files in the folder, or error out if we can't
    let mut entries = fs::read_dir(&input_directory)?.collect::<Vec<_>>();
    for entry in &entries {
        if entry.is_err() {
            eprintln!("IO error occurred while reading {}", input_directory.to_string_lossy());
            panic!();
        }
    }
    
    let mut files_and_numbers =
        // First extract the path from the DirEntry
        entries.iter().map(|e| (e.as_ref().unwrap().path(), e.as_ref().unwrap().path()))
        // Then try to turn the filename without the extension into a u16
        .map(|(p1, p2)| (p1.file_stem().and_then(|q|
            q.to_str().and_then(|r|
                u16::from_str(r).ok()
            )
        ), p2))
        // Only keep the files for which that succeeded
        .filter(|(o1, _)| o1.is_some())
        .map(|(o, p)| (o.unwrap(), p))
        // And put the result in a Vec
        .collect::<Vec<(u16, PathBuf)>>();

    // There should be files with numerical names in this folder, otherwise something probably went wrong
    assert!(files_and_numbers.len() != 0);

    // Sort it in numerical order and verify that there are no duplicates or missing files
    files_and_numbers.sort_unstable_by(|(num1, _), (num2, _)| num1.cmp(num2));
    for (i, tup) in files_and_numbers.iter().enumerate() {
        if usize::from(tup.0) != i {
            return Err(format!("While looking for file number {}, found file '{}'. Do you have two files named '{}' (without the extension)? Is the file named '{}' missing?",
                i, tup.1.to_string_lossy(), i - 1, i).into());
            // ...Hey wait this isn't how you return an error
            // panic!();
        }
    }
    
    // Now for the actual interesting part!
    let mut table_bytes = vec![0u8; files_and_numbers.len() * 4 + 8];
    table_bytes.splice(0..4, (files_and_numbers.len() as u32).to_le_bytes());
    // Using the number in the tuple is safe, because we verified in the previous loop that
    // it's equal to what you'd get from a normal .iter().enumerate() call
    for (i, filename) in &files_and_numbers {
        let offset_bytes = if filename.extension().unwrap_or_default().to_str().unwrap() == "ignore" {
            0u32.to_le_bytes()
        } else {
            let mut file_bytes = fs::read(filename)?;
            let file_offset: u32 = match table_bytes.len().try_into() {
                Ok(n) => n,
                Err(err) => return Err(err.into())
            };
            table_bytes.append(&mut file_bytes);
            file_offset.to_le_bytes()
        };
        table_bytes.splice((usize::from(*i) * 4 + 4)..(usize::from(*i) * 4 + 8), offset_bytes);
    }
    let length: u32 = match table_bytes.len().try_into() {
        Ok(n) => n,
        Err(err) => return Err(err.into())
    };
    table_bytes.splice((files_and_numbers.len() * 4 + 4)..(files_and_numbers.len() * 4 + 8), length.to_le_bytes());
    fs::write(table_path, table_bytes)?;
    
    Ok(())
}
