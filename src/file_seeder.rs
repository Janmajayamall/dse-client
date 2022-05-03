use std::collections::HashSet;

use super::storage::Storage;
use super::wallet::Wallet;
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use tokio::{select, time};

#[derive(Serialize, Deserialize)]
struct FileRFP {
    file_id: u32,
    sequence_no: u32,
}

/// Process object for tracking file transfer
#[derive(Serialize, Deserialize)]
pub struct Process {
    pub id: u32,
    requester_address: Address,
    /// Index of chunk of data being sent
    sequence_no: usize,
    /// `rfp_sequence_no` should only
    /// be 1 less than `sequence_no`
    rfp_sequence_no: usize,
}

#[derive(Serialize, Deserialize)]
struct FileRequest {
    requester_address: Address,
}

#[derive(Serialize, Deserialize)]
struct File {
    id: u32,
    chunk_size: usize,
    chunk_price: U256,
    file: Vec<u8>,
}

struct FileSeeder {
    storage: Storage,
    wallet: Wallet,
    rfp_sent: HashSet<u32>,
    main_file: File,
}

impl FileSeeder {
    pub fn process_request(request: FileRequest) {
        // Check that file exists
    }

    pub fn read_chunk_at_index(&self, index: usize) -> Option<&[u8]> {
        self.main_file
            .file
            .chunks(self.main_file.chunk_size)
            .nth(index)
    }

    pub fn send_chunk(&self, process: Process) {
        let chunk = self
            .read_chunk_at_index(process.sequence_no)
            .expect("Chunk should be present")
            .to_owned();

        // TODO:
        // 1. send the chunk to the `address` in `process`
        // 2. increase process sequence number by 1 and store
        // 3. Also check whether it was the end.
    }

    pub fn send_rfp(&self) {}

    pub async fn run(&self) {
        let mut interval = time::interval(time::Duration::from_secs(5));
        loop {
            select! {
                _ = interval.tick() => {
                    if let Ok(processes) = self.storage.get_all_active_process() {
                        for (_,p) in processes {
                            if p.sequence_no < p.rfp_sequence_no + 1 {
                                // send chunk at sequence no.
                                self.send_chunk(p);
                            }else {
                                // send RFP
                            }

                            // Check what action to take depending on the status
                            // Send RFP?
                            // Send byte?
                        }
                    };

                }
            }
        }
    }
}

// 1. Create a protocol that asks for payments after sending fixed number of bytes.
// 2. Initiate RFPs for a new payment
// 3. Handle new file requests
// 4. Handle sending new file requests
// 5.
