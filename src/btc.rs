#![allow(dead_code)]
#![warn(unreachable_code)]

use bdk::bitcoin::util::bip32::ExtendedPubKey;
use bdk::bitcoin::Network;
use bdk::template::{Bip84Public, DescriptorTemplate};
use bdk::database::memory::MemoryDatabase;
use bdk::Wallet;
use bdk::wallet::{AddressIndex, AddressInfo};
use slip132::FromSlip132;
use electrum_client::{Client, ElectrumApi};
use std::str::FromStr;
use std::io::{Error, ErrorKind};


/*
* Not implemented, preparation for polymorphism to support multiple 
*/
trait Req {
    
    fn x_get_address(&self, index: Option<u32>) -> Result<String, Box<dyn std::error::Error>>;
    fn x_get_balance(&self, address: &String) -> Result<u64, Box<dyn std::error::Error>>;

}

pub struct Btc {
    pub wallet: Wallet<MemoryDatabase>,
    pub electrum: String,
    mpk: String,
    network: Network,
}

impl Btc {

    pub fn get_address(&self, index: Option<u32>) -> Result<AddressInfo, String> {

        let i = index.unwrap_or(0);
        let address_index: AddressIndex = if i == 0 { AddressIndex::New } else { AddressIndex::Peek(i) };
        let address = self.wallet.get_address(address_index).unwrap();

        return Ok(address);
    }

    pub fn get_balance(&self, address: &String) -> Result<u64, String> {
        
        // @TODO for sure some repair required here
        let network: bitcoin::Network = if self.network == Network::Bitcoin {
            bitcoin::Network::Bitcoin
        } else {
            bitcoin::Network::Testnet
        };

        let addrobj: bitcoin::address::Address = bitcoin::address::Address::from_str(address).unwrap()
            .require_network(network).unwrap();

        let spk = addrobj.script_pubkey();


        let client = Client::new(&self.electrum)
            .unwrap();

        let balance = client
            .script_get_balance(spk.as_script())
            .unwrap();

        // 

        let latest_tx_option = client
            .script_get_history(spk.as_script())
            .unwrap();
            //.last();
            //.unwrap().
            //.tx_hash;

 
        let latest_tx_hash = match latest_tx_option.last() {
            Some(val) => val.tx_hash.clone(),
            None => {
                return Err("Couldn't fetch balance xx".to_owned());
            }
        };
        

        let tx = client
            .transaction_get(&latest_tx_hash)
            .unwrap();

        let tx_height = match tx.lock_time {
            bitcoin::absolute::LockTime::Blocks(b) => {
                b.to_consensus_u32()  
            },
            _ => 0
        };

        let current_height = client
            .block_headers_subscribe()
            .unwrap()
            .height;    

        // has balance AND has confirmations ?
        let confirmations = current_height - usize::try_from(tx_height)
            .unwrap();
        /*
            @TODO: dirty fix as some transactions with many inputs && many outputs return huge confirmation
            count even tho are new .. 
        */
        if (confirmations > 3 && confirmations <= 6) && balance.confirmed > 0 {
            return Ok(balance.confirmed);
        } 
        
        Err("Couldn't fetch balance".to_owned())
    }

    // expect je string bitcoin
    // balance je u64 satoshi
    pub fn assert_eq(&self, address: &String, expect: f64) -> Result<bool, String> {

        let balance: u64 = self.get_balance(address)?;

        let sat: f64 = 100000000f64;
        let expect_sats = expect * sat;

        if balance >= expect_sats as u64 {
            return Ok(true);
        }

        Ok(false)
    }

}

impl Req for Btc {

    fn x_get_address(&self, index: Option<u32>) -> Result<String, Box<dyn std::error::Error>> {
        Ok("x".to_owned())
    }

    fn x_get_balance(&self, address: &String) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(715)
    }
}

pub fn get_wallet(mpk: String, index: u32, testnet: Option<bool>, server: String) -> Result<Btc, String> {

    let network: Network;
    
    let t: bool = testnet.unwrap_or(false);

    if t {
        network = Network::Testnet
    } else {
        network = Network::Bitcoin
    }

    let slip132_xpub = ExtendedPubKey::from_slip132_str(mpk.as_str()).unwrap();
    let fingerprint = slip132_xpub.parent_fingerprint;
    let descriptor_bip84_public = Bip84Public(slip132_xpub.clone(), fingerprint, bdk::KeychainKind::External).build(network).unwrap();
    let descriptor_bip84_public_internal = Bip84Public(slip132_xpub.clone(), fingerprint, bdk::KeychainKind::Internal).build(network).unwrap();
     
    let wallet = Wallet::new(
        descriptor_bip84_public,
        Some(descriptor_bip84_public_internal),
        network,
        MemoryDatabase::default()
    ).unwrap(); 

    if 0 < index {
        match wallet.get_address(AddressIndex::Reset(index)) {
            Ok(_) => {},
            Err(err) => {
                panic!("{:?}", err)
            }
        }
    }

    Ok(Btc { wallet: wallet, mpk: mpk, network: network , electrum: server })
    
}