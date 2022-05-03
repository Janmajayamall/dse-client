use super::file_seeder::Process;
use super::wallet::ReceiptWithSignatures;
use ethers::types::Address;
use rocksdb::DB;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Storage {
    /// Stores receipts actively used with other
    /// user. Stored using user's address as key
    active_receipts: Arc<Mutex<DB>>,
    /// Stores all past receipts (already posted/expireed ones)
    /// used with other user.
    old_receipts: Arc<Mutex<DB>>,
    /// Stores various cache like
    /// user_addresses
    cache: Arc<Mutex<DB>>,
}

impl Storage {
    pub fn new() -> Self {
        let active_receipts = Arc::new(Mutex::new(
            DB::open_default("./dbs/active_receipts").expect("Failed to open DB"),
        ));
        let old_receipts = Arc::new(Mutex::new(
            DB::open_default("./dbs/old_receipts").expect("Failed to open DB"),
        ));
        let cache = Arc::new(Mutex::new(
            DB::open_default("./dbs/cache").expect("Failed to open DB"),
        ));

        Self {
            active_receipts,
            old_receipts,
            cache,
        }
    }

    /// find active receipt
    pub fn find_active_receipt(&self, user: &Address) -> anyhow::Result<ReceiptWithSignatures> {
        let db = self.active_receipts.lock().unwrap();
        db.get(user.as_bytes()).map_err(|e| e.into()).and_then(|r| {
            if let Some(r) = r {
                bincode::deserialize::<ReceiptWithSignatures>(&r).map_err(|e| e.into())
            } else {
                Err(anyhow::anyhow!("Record does not exists"))
            }
        })
    }

    /// store active receipt
    pub fn store_active_receipt(
        &self,
        user: &Address,
        receipt: &ReceiptWithSignatures,
    ) -> anyhow::Result<()> {
        let r_bytes = bincode::serialize(&receipt)?;
        let db = self.active_receipts.lock().unwrap();
        db.put(user.as_bytes(), r_bytes)?;
        Ok(())
    }

    // get active `Process`es
    pub fn get_all_active_process(&self) -> anyhow::Result<HashMap<u32, Process>> {
        let db = self.cache.lock().unwrap();
        db.get(b"active-processes")
            .map_err(|e| e.into())
            .and_then(|r| {
                if let Some(r) = r {
                    bincode::deserialize::<HashMap<u32, Process>>(&r).map_err(|e| e.into())
                } else {
                    Err(anyhow::anyhow!("Record does not exists"))
                }
            })
    }

    // update active `Process`
    pub fn update_active_process(&self, process: Process) -> anyhow::Result<()> {
        let map = self.get_all_active_process()?;
        map.insert(process.id, process);
        let db = self.cache.lock().unwrap();
        db.put(b"active-processes", bincode::serialize(&map)?)?;
        Ok(())
    }
}
