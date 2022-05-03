use super::storage::Storage;
use ethers::types::{Address, Signature, U256};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap,};

#[derive(Serialize, Deserialize)]
pub struct Receipt {
    a_address: Address,
    b_address: Address,
    a_owes: U256,
    b_owes: U256,
    expires_by: U256,
}

#[derive(Serialize, Deserialize)]
enum Status {
    Active,
    Posted,
    Expired,
}

#[derive(Serialize, Deserialize)]
pub struct ReceiptWithSignatures {
    receipt: Receipt,
    a_signature: Option<Signature>,
    b_signature: Option<Signature>,
    /// Status tracks whether this receipt
    /// has been posted on-chain or still
    /// in use
    status: Status,
}

impl ReceiptWithSignatures {
    fn validate_signatures(&self) -> bool {
        todo!();
        true
    }

    fn increase_owed_amount_by(&mut self, amount: U256, owed_to: Address) {
        if owed_to == self.receipt.a_address {
            self.receipt.b_owes += amount;
        } else {
            self.receipt.a_owes += amount;
        }

        // past signatures are not more valid
        self.a_signature = None;
        self.b_signature = None;
    }
}

pub struct Wallet {
    storage: Storage,
    balance_owes: HashMap<Address, U256>,
    balance_owed: HashMap<Address, U256>,
    total_owes: U256,
    total_owed: U256,
    total_balance: U256,
    //TODO: Shift this to somewhere appropriate
    self_address: Address,
}

impl Wallet {
    pub fn can_pay(&self, amount: U256) -> bool {
        self.total_balance.gt(&(self.total_owes + amount))
    }

    /// Updates/creates receipt shared with `user` to reflect "pay" `amount`
    /// for the outgoing "rfp", signs updated receipts, and returns
    pub fn process_outgoing_rfp(
        &self,
        user: Address,
        amount: U256,
    ) -> anyhow::Result<ReceiptWithSignatures> {
        // Get active receipt shared with user
        let mut receipt = {
            match self.storage.find_active_receipt(&user) {
                Ok(r) => r,
                Err(_) => {
                    ReceiptWithSignatures {
                        // TODO - who decides the order a_address & b_address
                        receipt: Receipt {
                            a_address: user,
                            b_address: self.self_address,
                            a_owes: U256::zero(),
                            b_owes: U256::zero(),
                            expires_by: U256::zero(),
                        },
                        a_signature: None,
                        b_signature: None,
                        status: Status::Active,
                    }
                }
            }
        };

        receipt.increase_owed_amount_by(amount, self.self_address);

        self.storage.store_active_receipt(&user, &receipt)?;

        // TODO sign the receipt
        Ok(receipt)
    }

    /// Validates updated `receipts` correspoinding to rfp
    /// incoming from `user` for `pay_amount`, then signs it, and returns
    pub fn process_incoming_rfp(
        &mut self,
        user: Address,
        pay_amount: U256,
        new_receipt: ReceiptWithSignatures,
    ) -> anyhow::Result<ReceiptWithSignatures> {
        if !self.can_pay(pay_amount) {
            return Err(anyhow::anyhow!("Insufficient Balance!"));
        }

        let old_receipt = {
            match self.storage.find_active_receipt(&user) {
                Ok(r) => r,
                Err(_) => {
                    ReceiptWithSignatures {
                        // TODO - who decides the order a_address & b_address
                        receipt: Receipt {
                            a_address: user,
                            b_address: self.self_address,
                            a_owes: U256::zero(),
                            b_owes: U256::zero(),
                            expires_by: U256::zero(),
                        },
                        a_signature: None,
                        b_signature: None,
                        status: Status::Active,
                    }
                }
            }
        };

        // Validate that receipt addresses match
        // TODO: we might be missing some validation cases here rn
        if old_receipt.receipt.a_address != new_receipt.receipt.a_address
            || old_receipt.receipt.b_address != new_receipt.receipt.b_address
        {
            return Err(anyhow::anyhow!("Invalid receipt update!"));
        }

        if
        // If self is a[b]_address then `a[b]_owes` in new_receipt should be
        // `pay_amount` + `a[b]_owes` in old_receipt AND `b[a]_owes` should remain same.
        (new_receipt.receipt.a_address == self.self_address
            && ((old_receipt.receipt.a_owes + pay_amount) != new_receipt.receipt.a_owes
                || old_receipt.receipt.b_owes != new_receipt.receipt.b_owes))
            || (new_receipt.receipt.b_address == self.self_address
                && ((old_receipt.receipt.b_owes + pay_amount) != new_receipt.receipt.b_owes
                    || old_receipt.receipt.a_owes != new_receipt.receipt.a_owes))
        {
            return Err(anyhow::anyhow!("Invalid receipt update!"));
        }

        // new receipt seems valid, so apply necessary updates
        self.storage.store_active_receipt(&user, &new_receipt)?;

        self.total_owes += pay_amount;

        // TODO sign the receipt
        // TODO validate the signatures
        Ok(new_receipt)
    }
}

// 1. Wallet updates/creates receipts on the basis of pay request
// received.

// Unrelated
// Think about to roll up receipts and post them onchain
