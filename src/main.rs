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
        unpack_table(path1, path2).unwrap();
    } else if command == "pack" {
        todo!();
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
