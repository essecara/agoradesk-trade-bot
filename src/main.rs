pub mod coins;
use std::{collections::HashMap, fs::File, rc::Rc, io::Read, thread, time::Duration};
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};
use clap::Parser;
use colored::Colorize;
use crate::coins::*;
use crate::coins::Btc;

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
    id: String,
    coin: String
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
    address: String,
    addr_sent: bool,
    coin: Rc<dyn Coin>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    msg: String
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

struct Engine {
    password: String,
    trades: HashMap<String, Entry>,
    agoradesk: Client,
    coins: HashMap<String, Rc<dyn Coin>>,
    ads: Vec<Ad>
}

impl Engine {
    
    fn new(config: &Cnf) -> Result<Engine, Box<dyn std::error::Error>> {
        
        let mut headers: reqwest::header::HeaderMap = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", config.apikey.parse()?);
        headers.insert("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.".parse()?);

        let http_client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;

        let mut coins: HashMap<String, Rc<dyn Coin>> = HashMap::new();
        for ad in &config.ads {
            match ad.coin.as_str() {
                "btc" => {
                    let btc = Btc::new(
                        config.mpk.clone(),
                        config.address_index,
                        Some(config.testnet),
                        config.electrum.clone()
                    )?;
                   
                    let t: Rc<Btc> = Rc::new(btc);
                    coins.insert("btc".to_owned(), t);
                },
                _ => panic!("Couldn't parse coin type for Ad. ")
            }
        }

        let engine: Engine = Engine { 
            trades: HashMap::new(),
            password: config.password.clone(),
            agoradesk: http_client,
            coins: coins,
            ads: config.ads.clone()
        };

        Ok(engine)
    }

    fn fetch_trades(& mut self) -> Result<Vec<String>, Box<dyn std::error::Error>> {

        let mut new_trades: Vec<String> = Vec::new();
        let url = "https://agoradesk.com/api/v1/dashboard/seller";

        match self.agoradesk.request(reqwest::Method::GET, url).send() {
            Ok(response) => 'success: {
                if !(response.status().is_success()) {
                    println!("{}", "Failed to load open offers".to_owned().red().bold());
                    break 'success;
                }

                let text = response.text()?;
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

                    let index = self.ads.iter().position(|v| v.id == *adid); 
                    if index.is_none() { continue; }

                    match self.trades.get_mut(key.as_str()) {
                        Some(e) => {
                            e.data = t.data.clone();

                            /* .. would send address here 
                                but can't borrow self as immutable
                                can't borrow mutable self as mutable...
                             */
                        }, 
                        _ => {

                            let mut entry: Entry;
                            let coin_id = self.ads.get(index.unwrap()).unwrap().coin.as_str();              
                            let address: String = match coin_id {
                                "btc" => {
                                    let btc = self.coins.get(coin_id)
                                        .unwrap()
                                        .as_any()
                                        .downcast_ref::<Btc>()
                                        .unwrap();

                                    btc.get_address(Some(0))
                                        .unwrap()
                                        .address
                                        .to_string()
                                },
                                other => {
                                    println!("{}: {}", 
                                        "failed to get address for".red().bold(),
                                        other.to_string().green().bold()
                                    );
                                    continue;
                                }
                            };

                            let ptr_coin: Rc<dyn Coin> = self.coins.get(coin_id).unwrap().clone();
                            entry = Entry {
                                data: t.data.clone(),
                                address: address,
                                addr_sent: false,
                                coin: ptr_coin
                            };
                            self.trades.insert(key.clone(), entry);
                            new_trades.push(key.clone());

                            println!("{}: {}", 
                                "New offer with ID".yellow(),
                                key.yellow().bold()
                            );
                        }
                    }
                } 
            }, 
            Err(e) => {
                    println!("{}", e.to_string().red().bold());
            }
        }

        return Ok(new_trades);

    }

    fn send_address(&self, id: &String, address: &String) -> Result<(), Box<dyn std::error::Error>> {
        let text: String = format!("Hello! This is an automatic BOT.

- transfer BTC onchain to address at the bottom of this message
- transfer must be same amount as shown in this offer / trade
- this bot is in beta currently but don't worry, you are protected by arbitrage bond
- if bot won't finalize automatically, I'll handle it manually

{address}").to_owned();

        let mut url: String = "https://agoradesk.com/api/v1/contact_message_post/".to_owned();
        url.push_str(id);

        self.agoradesk.request(reqwest::Method::POST, url)
            .json(&Message {msg: text})
            .send()?;

        Ok(())
    }

    fn monitor_trade_status(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {

        let mut payed_trades: Vec<String> = Vec::new();

        for key in self.trades.keys() {

            let trade = self.trades.get(key).unwrap();
            let amount = trade.data.amount.clone();
            let address = &trade.address;
            let expect = match amount.parse::<f64>() {
                Ok(val) => {val},
                Err(_) => continue
            };
             
            let coin: Rc<dyn Coin> = trade.coin.clone();  
            match coin.assert_eq(&address, expect) {
                Ok(true) => {
                    payed_trades.push(key.clone());
                },
                _ => continue
            }
        }

        Ok(payed_trades)
    }

    fn finalize_trade(&self, trade_id: &str) -> Result<(), Box<dyn std::error::Error>> {

        let mut url: String = "https://agoradesk.com/api/v1/contact_release/".to_owned();
        url.push_str(&trade_id);

        let mut map = HashMap::new();
        map.insert("password", &self.password);

        self.agoradesk.post(url)
            .json(&map)
            .send()?;

        Ok(())
    }

    fn remove_trade(& mut self, key: &String) {
            self.trades.remove(key);
    }

}


//#[tokio::main] @TODO
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {

    let args = Cli::parse();
    let mut cnf: Cnf = load_conf(&args.conf)?;
    let mut engine: Engine = Engine::new(&cnf)?;

    //let btc = engine.coins.get("btc").unwrap();
    //println!("btc address index: {}", btc.get_address(Some(0)).unwrap().address);


    println!("{}", "Configuration loadig, starting event loop".green().bold());


    let btc: Btc = Btc::new(
        "vpub5VRCxnBjHyAqbxcB1ioSxVX1jSyZV39P44EdsAHrCz6DET7HuYnkXTnfs7frcGKi5TTPLsiWYdga1DXcMLWX7C8mXNpnR8d1t7GURzX2vVM".to_string(),
        11,
        Some(true),
        "tcp://testnet.aranguren.org:51001".to_string()
    )?;


    btc.assert_eq(&"tb1qrz3drs0ur4am05c7jks9dxt8p53ch7rrwwecyr".to_string(), 0.00006f64);


    if 1 == 1 {
        return Ok(());
    }


    loop {

        let new_trades = match engine.fetch_trades() {
            Ok(new_trades) => new_trades,
            Err(_) => continue
        };

        for trade_id in new_trades {

            let address = engine.trades.get(&trade_id)
                .unwrap()
                .address
                .clone();
    
            match engine.send_address(&trade_id, &address) {
                Ok(()) => {
                    let trade = engine.trades.get_mut(&trade_id)
                        .unwrap();
                    trade.addr_sent = true;
                },
                Err(_) => {
                    println!("{}: {}",
                        "failed to send address for".red().bold(),
                        trade_id
                    );
                }
            }
        }
        
        let payed_trades: Vec<String> = match engine.monitor_trade_status() {
            Ok(trades) => trades,
            Err(err) => {continue;}
        };

        for payed_trade in payed_trades {
            match engine.finalize_trade(&payed_trade) {
                Ok(_) => {
                    engine.remove_trade(&payed_trade);
                }
                Err(err) => println!("{}: {}", "failed to finalize".red().bold(), payed_trade.green().bold())
            };
        }


        thread::sleep(Duration::from_secs(30));
    }

    Ok(())
}
