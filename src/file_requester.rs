use ethers::types::{Address, U256};
pub struct RProcess {
    pub id: u32,
    pub sender_address: Address,
    /// Index of latest chunk of data
    /// received
    sequence_no: usize,
    /// `rfp_sequence_no` should only
    /// be 1 less than `sequence_no`
    rfp_sequence_no: usize,
}

struct FileRequester {
    // When seeder accepts the request
// start the process
}
