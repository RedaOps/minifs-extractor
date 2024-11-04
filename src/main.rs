mod minifs;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::exit,
    str::FromStr,
};

use clap::Parser;
use minifs::MiniFs;

#[derive(Debug)]
enum ParseError {
    InvalidHeader,
    UnsupportedVersion,
}

#[derive(Parser)]
#[command(author = "Tudor Gheorghiu")]
#[command(about = "A simple CLI tool to extract files from a minifs binary.")]
#[command(version)]
#[command(
    help_template = "{about-section}{author}\nVersion: {version} \n {usage-heading} {usage} \n {all-args} {tab}"
)]
struct Args {
    /// The binary file containing the minifs filesystem
    binary: String,
}

fn main() {
    let args = Args::parse();
    let path = Path::new(&args.binary);
    let original_file_name = path.file_name().unwrap().to_string_lossy();
    let output_dir = format!("_{}.extracted", original_file_name);

    let mut fd = File::open(path).expect("File not found");
    let mut content: Vec<u8> = Vec::new();
    fd.read_to_end(&mut content).expect("Unsupported file");

    match MiniFs::parse(content) {
        Err(e) => {
            match e {
                ParseError::InvalidHeader => {
                    println!("[-] Invalid minifs header");
                }
                ParseError::UnsupportedVersion => {
                    println!("[-] Unsupported minifs version");
                }
            };
            exit(1);
        }
        Ok(minifs) => {
            println!(
                "[+] Found minifs header at {:#x}",
                minifs.get_header_start()
            );
            println!(
                "[+] Found {} files in minifs. Extracing...",
                minifs.get_files_no()
            );

            let files = minifs.extract();
            std::fs::create_dir_all(output_dir.clone()).expect("Couldn't create output directory");
            for file in files.into_iter() {
                let path = PathBuf::from_str(&format!("{output_dir}/{}", file.path))
                    .expect("Invalid path");
                let mut file_path = path.clone();
                file_path.push(file.filename);

                if file_path.components().any(|x| x == Component::ParentDir)
                    || file_path.starts_with("/")
                {
                    panic!("This is not dangerous");
                }

                println!("[+] {}", file_path.clone().to_string_lossy());
                let _ = std::fs::create_dir_all(path);

                let mut output_file =
                    File::create(file_path).expect("Couldn't create file in output directory");
                output_file
                    .write_all(&file.data)
                    .expect("Couldn't write to file");
            }
            println!("[+] Extracted into {}", output_dir);
        }
    }
}
