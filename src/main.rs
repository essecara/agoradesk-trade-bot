//#![allow(dead_code)]
#![warn(unreachable_code)]
pub mod btc;

use std::collections::HashMap;
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};
use std::{thread, time::Duration};
use clap::Parser;
use std::fs::File;
use std::io::Read;
use colored::Colorize;
use crate::btc::Btc;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    conf: String
}

#[derive(Debug, Serialize, Deserialize)]
struct Cnf {
    password: String,
    apikey: String,
    testnet: bool,
    mpk: String,
    electrum: String,
    address_index: u32,
    ads: Vec<Ad>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Ad {
    id: String
}

#[derive(Debug, Serialize, Deserialize)]
struct Trades { 
    contact_list: Vec<Trade>,
    contact_count: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct Trade {
    data: Data,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Data {
    buyer: Buyer,
    amount: String,
    amount_xmr: String,
    fee_xmr: String,
    advertisement: Advertisement,
    contact_id: String,
    currency: String,
    account_info: String,
    price_equation: String,
    is_buying: bool,
    created_at: Option<String>,
    payment_completed_at: Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Buyer {
    username: String,
    feedback_score: u8,
    trade_count: String,
    last_online: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Advertisement {
    id: String,
    asset: String,
    trade_type: String,
}

struct Entry {
    data: Data,
    bitcoin_addr: String,
    addr_sent: bool
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    msg: String
}

fn finalize_trade(lm_client: &Client, trade_id: &String, password: &String) -> Result<(), ()> {

    let mut url: String = "https://agoradesk.com/api/v1/contact_release/".to_owned();
    url.push_str(&trade_id);

    let mut map = HashMap::new();
    map.insert("password", password);

    let response = lm_client.request(reqwest::Method::POST, url)
        .json(&map)
        .send();

    match response {
        Ok(_) => { return Ok(()) },
        Err(_) => { return Err(()) } 
    };
}


fn send_btc_address(lm_client: &Client, address: &String, id: &String) {

    let text: String = format!("Hello! This is an automatic BOT.

- transfer BTC onchain to address at the bottom of this message
- transfer must be same amount as shown in this offer / trade
- this bot is in beta currently but don't worry, you are protected by arbitrage bond
- if bot won't finalize automatically, I'll handle it manually

{address}").to_owned();

    let mut url: String = "https://agoradesk.com/api/v1/contact_message_post/".to_owned();
    url.push_str(id);

    let message: Message = Message { msg: text };
    
    let _response = lm_client.request( reqwest::Method::POST, url)
        .json(&message)
        .send(); 
}

fn load_conf(path: &String) -> Result<Cnf, std::io::Error> {
    let mut file = File::open(path).expect("Configuration file doesn't exists!");
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    
    let cnf = serde_json::from_str(&data).unwrap_or_else(|error| {
        panic!("Configuration not well formatted: \n{:?}", error)
    });

    Ok(cnf)
}

fn up_conf(path: &String, cnf: &Cnf) -> Result<(), std::io::Error> {

    let mut file = std::fs::File::create(path)?;

    match serde_json::to_writer_pretty(&mut file, &cnf) {
       Ok(_) => { return Ok(()); },
       Err(err) => { return Err(std::io::Error::from(err)); } 
    };
}

fn remove_keys(hmap: &mut HashMap<String, Entry>, keys: &mut Vec<String>) {

        let mut i = keys.iter();
        
        while let Some(value) = i.next() {
            println!("fn: remove_keys => removing {}", value);
            hmap.remove(value);
        }

        keys.clear();
}

//#[tokio::main] @TODO
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {

    let args = Cli::parse();

    let mut cnf: Cnf = load_conf(&args.conf)?;
    let mut address_index = cnf.address_index;
    
    let btc: Btc = btc::get_wallet(
        cnf.mpk.clone(),
        cnf.address_index, 
        Some(cnf.testnet), 
        cnf.electrum.clone()
    )?;

    let mut trades_db: HashMap::<String,Entry> = HashMap::new();
    
    let mut headers: reqwest::header::HeaderMap = reqwest::header::HeaderMap::new();
    headers.insert("Authorization", cnf.apikey.parse()?);
    headers.insert("User-Agent", "PostmanRuntime/7.32.2".parse()?);

    let lm_client: Client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()?;
    
    loop { 

        // 
        let url = "https://agoradesk.com/api/v1/dashboard/seller";
        match lm_client.request(reqwest::Method::GET, url).send() {

            Ok(response) => 'success: {

                if !(response.status().is_success()) {
                    println!("{}", "Failed to load open offers".to_owned().red().bold());
                    break 'success;
                }

                let text = response.text().unwrap();
                let json: HashMap<String, Trades> = match serde_json::from_str(&text.as_str()) {
                    Result::Ok(j) => { j },
                    Result::Err(err) => {
                        println!("{}", "Failed to convert open offers to json".to_owned().red().bold());
                        break 'success;
                    }
                };
                
                for t in &json["data"].contact_list {

                    let key = t.data.contact_id.clone();
                    let adid: &String = &t.data.advertisement.id;

                    // check if the offer is interesting for us
                    let index = cnf.ads.iter().position(|v| v.id == *adid); 
                    if index.is_none() { continue; }

                    match trades_db.get_mut(key.as_str()) {

                        Some(e) => {
                            e.data = t.data.clone();
                        },
                        _ => {

                            let mut entry: Entry;
                            //let address_info = btc.get_address(Some(0));
                            // address_index = address_info.unwrap().index;


                            match btc.get_address(Some(0)) {
                                Ok(v) => {

                                    address_index = v.index;
                                    entry = Entry {
                                        data: t.data.clone(),
                                        bitcoin_addr: v.address.to_string(),
                                        addr_sent: false 
                                    };

                                    send_btc_address(&lm_client, &entry.bitcoin_addr, &key);
                                    entry.addr_sent = true;
                                    trades_db.insert(key.clone(), entry);

                                    println!("{}: {}", 
                                        "New offer with ID".yellow(),
                                        key.yellow().bold()
                                    );
                                },

                                Err(err) => {
                                    continue;
                                }
                            }
                        }
                    }
                }
                
                        
                if address_index > cnf.address_index {

                    cnf.address_index = address_index;

                    match up_conf(&args.conf, &cnf) {
                        Ok(_) => {
                            println!("Update address index in config file");
                        },
                        Err(err) => {
                            println!("Failed to update configuration with new address index\n{}", err);
                        }
                    };
                }                
            }

            Err(e) => {
                println!("{}", e.to_string().red().bold());
            }
        }
        
        let mut remove_keys_vec: Vec<String> = Vec::new();

        for key in trades_db.keys() {

            let tmp = trades_db.get(key).unwrap();
            
            let amount= tmp.data.amount.clone();
            let _amount_xmr = tmp.data.amount.clone();

            let address = &tmp.bitcoin_addr;
            let expect = match amount.parse::<f64>() {
                Ok(val) => {val},
                Err(_) => continue
            };
                
            let payed = btc.assert_eq(address, expect).unwrap_or(false);

            //@TODO Watch out, this can't fail
            if payed {

                println!("{} {}",
                    "Trade finalized ID:".green(),
                    key.green().bold(),
                );
                
                match finalize_trade(&lm_client, key, &cnf.password) {
                    Ok(_) => {
                        println!("removin offer from db");
                        remove_keys_vec.push(key.clone());
                    },
                    Err(_) => { println!("Trade {} failed to finalize", key) }
                }; 
            } 
        }

        remove_keys(&mut trades_db, &mut remove_keys_vec);

        thread::sleep(Duration::from_secs(60));
    }

    Ok(())
}
