use crate::ParseError;

const HEADER_MAGIC_NUMBER: &[u8] = b"MINIFS";
const HEADER_SIZE: usize = 32;

const TOF_ENTRY_SIZE: usize = 20;
const TOC_ENTRY_SIZE: usize = 12;

const LZMA_CONFIGURATION_WORD: u32 = 0x5D000080;

struct MiniFsOffsets {
    /// Table of Names
    pub ton_offset: usize,
    /// Table of Files
    pub tof_offset: usize,
    /// Table of chunks
    pub toc_offset: usize,
    /// Raw chunks
    pub raw_chunks_offset: usize,
}

#[derive(Debug)]
pub struct ToFEntry {
    pub ton_path_offset: u32,
    pub ton_file_name_offset: u32,
    pub chunk_number: u32,
    pub offset_in_chunk: u32,
    pub file_size: u32,
}

impl ToFEntry {
    pub fn parse(data: [u8; TOF_ENTRY_SIZE]) -> Self {
        Self {
            ton_path_offset: u32::from_be_bytes(data[0..4].try_into().unwrap()),
            ton_file_name_offset: u32::from_be_bytes(data[4..8].try_into().unwrap()),
            chunk_number: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            offset_in_chunk: u32::from_be_bytes(data[12..16].try_into().unwrap()),
            file_size: u32::from_be_bytes(data[16..20].try_into().unwrap()),
        }
    }
}

#[derive(Debug)]
pub struct ToCEntry {
    pub chunk_offset: u32,
    pub chunk_size: u32,
    pub decompressed_size: u32,
}

impl ToCEntry {
    pub fn parse(data: [u8; TOC_ENTRY_SIZE]) -> Self {
        Self {
            chunk_offset: u32::from_be_bytes(data[0..4].try_into().unwrap()),
            chunk_size: u32::from_be_bytes(data[4..8].try_into().unwrap()),
            decompressed_size: u32::from_be_bytes(data[8..12].try_into().unwrap()),
        }
    }
}

pub struct DecompressedFile {
    pub path: String,
    pub filename: String,
    pub data: Vec<u8>,
}

// https://arxiv.org/html/2407.05064v1
pub struct MiniFs {
    content: Vec<u8>,
    header_start: usize,
    offsets: MiniFsOffsets,
    files: Vec<ToFEntry>,
    chunks: Vec<ToCEntry>,
}

impl MiniFs {
    pub fn parse(content: Vec<u8>) -> Result<Self, ParseError> {
        let header_start =
            find_bytes(&content, HEADER_MAGIC_NUMBER).ok_or(ParseError::InvalidHeader)?;

        let header = content
            .iter()
            .skip(header_start)
            .take(HEADER_SIZE)
            .copied()
            .collect::<Vec<u8>>();

        let files_no = u32::from_be_bytes(get_offset(&header, 0x14, 4).try_into().unwrap());
        let ton_size = u32::from_be_bytes(get_offset(&header, 0x1c, 4).try_into().unwrap());

        let content: Vec<u8> = content.into_iter().skip(header_start).collect();

        let ton_offset = HEADER_SIZE;
        let tof_offset = ton_offset + ton_size as usize;
        let toc_offset = tof_offset + (TOF_ENTRY_SIZE * files_no as usize);

        let mut offsets = MiniFsOffsets {
            ton_offset,
            tof_offset,
            toc_offset,
            // Unknown at this time
            raw_chunks_offset: 0,
        };

        let files = Self::parse_files_internal(&content, &offsets, files_no);

        let chunks_no = files.last().unwrap().chunk_number + 1;
        offsets.raw_chunks_offset = offsets.toc_offset + (TOC_ENTRY_SIZE * chunks_no as usize);
        let chunks = Self::parse_chunks_internal(&content, &offsets, chunks_no);

        // To make sure we are decompressing a minifs filesystem that matches the documentation (https://arxiv.org/html/2407.05064v1),
        // make sure the LZMA Configuration word is the same
        if u32::from_be_bytes(
            get_offset(&content, offsets.raw_chunks_offset, 4)
                .try_into()
                .unwrap(),
        ) != LZMA_CONFIGURATION_WORD
        {
            return Err(ParseError::UnsupportedVersion);
        }

        Ok(Self {
            header_start,
            content,
            offsets,
            files,
            chunks,
        })
    }

    fn parse_files_internal(
        content: &[u8],
        offsets: &MiniFsOffsets,
        files_no: u32,
    ) -> Vec<ToFEntry> {
        (0..files_no as usize)
            .map(|offset| {
                let entry_offset = offsets.tof_offset + offset * TOF_ENTRY_SIZE;
                ToFEntry::parse(
                    content
                        .iter()
                        .copied()
                        .skip(entry_offset)
                        .take(TOF_ENTRY_SIZE)
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                )
            })
            .collect()
    }

    fn parse_chunks_internal(
        content: &[u8],
        offsets: &MiniFsOffsets,
        chunks_no: u32,
    ) -> Vec<ToCEntry> {
        (0..chunks_no as usize)
            .map(|offset| {
                let entry_offset = offsets.toc_offset + offset * TOC_ENTRY_SIZE;
                ToCEntry::parse(
                    content
                        .iter()
                        .copied()
                        .skip(entry_offset)
                        .take(TOC_ENTRY_SIZE)
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                )
            })
            .collect()
    }

    pub fn get_header_start(&self) -> usize {
        self.header_start
    }

    pub fn get_files_no(&self) -> usize {
        self.files.len()
    }

    pub fn extract(&self) -> Vec<DecompressedFile> {
        let decompressed_chunks = self
            .chunks
            .iter()
            .map(|x| {
                let compressed_chunk = get_offset(
                    &self.content,
                    self.offsets.raw_chunks_offset + x.chunk_offset as usize,
                    x.chunk_size.try_into().unwrap(),
                );
                let decompressed_chunk =
                    lzma::decompress(&compressed_chunk).expect("Couldn't decompress LZMA chunk");
                if decompressed_chunk.len() != x.decompressed_size as usize {
                    panic!("LZMA decompressed chunk doesn't match size");
                }

                decompressed_chunk
            })
            .collect::<Vec<Vec<u8>>>();
        println!("[+] Decompressed {} chunks", decompressed_chunks.len());

        self.files
            .iter()
            .map(|x| {
                let path = read_string(
                    &self.content,
                    self.offsets.ton_offset + x.ton_path_offset as usize,
                );
                let filename = read_string(
                    &self.content,
                    self.offsets.ton_offset + x.ton_file_name_offset as usize,
                );

                let data = get_offset(
                    &decompressed_chunks[x.chunk_number as usize],
                    x.offset_in_chunk as usize,
                    x.file_size as usize,
                );

                DecompressedFile {
                    path,
                    filename,
                    data,
                }
            })
            .collect::<Vec<DecompressedFile>>()
    }
}

fn find_bytes(content: &[u8], pattern: &[u8]) -> Option<usize> {
    content
        .windows(pattern.len())
        .position(|window| window == pattern)
}

fn get_offset(content: &[u8], offset: usize, len: usize) -> Vec<u8> {
    content.iter().skip(offset).take(len).copied().collect()
}

fn read_string(content: &[u8], offset: usize) -> String {
    let mut data = String::new();

    for byte in content.iter().skip(offset) {
        if *byte == 0_u8 {
            return data;
        }

        data.push(*byte as char);
    }

    data
}
