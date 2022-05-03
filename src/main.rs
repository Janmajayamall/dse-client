mod network;
mod file_seeder;
mod storage;
mod wallet;

fn main() {
    // 1. Store and exchange receipts
    // 2. Maintaining account tree
    // 2. Watch on chain state updates -> And act as a watch tower!
    // 3. Oracle for deciding a good time to post updates
    // 4. And finally roll up the receipts and post them
    //    on chain.

    // Do we need propogation of receipts throughout the network?
    // I don't think so!

    // How do you store and exchange receipts?
    // Do you need a p2p network? Probably a multaddr
    // to send requests to should suffice right now.

    // How do you maintain account tree?
    // Probably maintain a record of all updates within
    // the last fraud proof period?
}
