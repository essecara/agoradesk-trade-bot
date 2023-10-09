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
use std::any::Any;

pub trait Coin {
    fn get_address(&self, index: Option<u32>) -> Result<AddressInfo, Box<dyn std::error::Error>>;
    fn get_balance(&self, address: &String) -> Result<u64, Box<dyn std::error::Error>>;
    fn assert_eq(&self, address: &String, expect: f64) -> Result<bool, Box<dyn std::error::Error>>;
    fn as_any(&self) -> &dyn Any;
}

pub struct Btc {
    pub wallet: Wallet<MemoryDatabase>,
    pub electrum: String,
    mpk: String,
    network: Network,
    pub address_index: u32 // @TODO => temporarily save it here just to keep tract, see later for other solutions
}

impl Btc {
    pub fn new(mpk: String, index: u32, testnet: Option<bool>, server: String) -> Result<Btc, Box<dyn std::error::Error>> {
        let network: Network = match testnet.unwrap_or(false) {
            true => Network::Testnet,
            false => Network::Bitcoin
        };

        let slip132_xpub = ExtendedPubKey::from_slip132_str(mpk.as_str())?;       
        
        let descriptor_bip84_public = Bip84Public(
            slip132_xpub.clone(),
            slip132_xpub.parent_fingerprint,
            bdk::KeychainKind::External
        ).build(network)?;
        
        let descriptor_bip84_public_internal = Bip84Public(
            slip132_xpub.clone(),
            slip132_xpub.parent_fingerprint,
            bdk::KeychainKind::Internal
        ).build(network)?;
     
        let wallet = Wallet::new(
            descriptor_bip84_public,
            Some(descriptor_bip84_public_internal),
            network,
            MemoryDatabase::default()
        )?;

        if 0 < index {
            match wallet.get_address(AddressIndex::Reset(index)) {
                Ok(_) => {},
                Err(err) => {
                    panic!("{:?}", err)
                }
            }
        }

        Ok(Btc { wallet: wallet, mpk: mpk, network: network , electrum: server, address_index: index})    
    }
}

impl Coin for Btc {
    
    fn get_address(&self, index: Option<u32>) -> Result<AddressInfo, Box<dyn std::error::Error>>{

        let i = index.unwrap_or(0);
        let address_index: AddressIndex = if i == 0 { AddressIndex::New } else { AddressIndex::Peek(i) };
        let address = self.wallet.get_address(address_index)?;

        return Ok(address);
    }

    fn get_balance(&self, address: &String) -> Result<u64, Box<dyn std::error::Error>>{
        
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

        let latest_tx_option = client
            .script_get_history(spk.as_script())
            .unwrap();
 
        let latest_tx_hash = match latest_tx_option.last() {
            Some(val) => val.tx_hash.clone(),
            None => {
                return Err("Couldn't fetch balance xx".into());
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

        println!("confirmations: {}", confirmations);
        println!("balance: {}", balance.confirmed);

        if (confirmations > 3 && confirmations <= 6) && balance.confirmed > 0 {
            return Ok(balance.confirmed);
        } 
        
        return Err("Couldn't fetch balance".into());
    }
 
    fn assert_eq(&self, address: &String, expect: f64) -> Result<bool, Box<dyn std::error::Error>> {

        let balance: u64 = self.get_balance(address)?;

        let sat: f64 = 100000000f64;
        let expect_sats = expect * sat;

        if balance >= expect_sats as u64 {
            return Ok(true);
        }

        Ok(false)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}