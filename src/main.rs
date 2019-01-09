#[macro_use]
extern crate rust_embed;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate rouille;

extern crate ctrlc;
extern crate handlebars;
extern crate rand;
extern crate rocksdb;
extern crate rustc_hex;
extern crate serde;
extern crate serde_cbor;
extern crate toml;
extern crate web3;
extern crate ethabi;
extern crate reqwest;
extern crate tiny_keccak;

mod db;
mod reader;
mod render;
mod scanner;
mod state;
mod types;
mod contract;

use std::env;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use rouille::Response;

use types::Id;

// it turns out that is not possible to put an Arc inside a rocket::State,
//  rocket internally crashes when unreferencing, so it can be solved by
//  wrapping it inside a one-element tuple
pub struct SharedGlobalState(Arc<state::GlobalState>);

fn main() {
    let args: Vec<String> = env::args().collect();
    let cfg = if args.len() > 1 {
        state::Config::read(args[1].as_str())
    } else {
        state::Config::read_default()
    }
    .expect("cannot read config");

    let shared_ge = SharedGlobalState(Arc::new(state::GlobalState::new(cfg)));

    if shared_ge.0.cfg.scan {
        let shared_ge_scan = shared_ge.0.clone();
        thread::spawn(move || scanner::scan(&shared_ge_scan));
    }

    let shared_ge_controlc = shared_ge.0.clone();
    ctrlc::set_handler(move || {
        println!("Got Ctrl-C handler signal. Stopping...");
        shared_ge_controlc.stop_signal.store(true, Ordering::SeqCst);
        if !shared_ge_controlc.cfg.scan {
            std::process::exit(0);
        }
    })
    .expect("Error setting Ctrl-C handler");

    println!("Lisening to {}...", &shared_ge.0.cfg.bind.clone()); // TODO: remove clones
    rouille::start_server(&shared_ge.0.cfg.bind.clone(), move |request| {
        router!(request,
            (GET) (/) => {
                 let wc = shared_ge.0.new_web3client();
                 let reader = reader::BlockchainReader::new(&wc,&shared_ge.0.db);
                 Response::html(
                     match render::home(&reader,&shared_ge.0.hb) {
                         Ok(html) => html,
                         Err(err) => render::page(format!("Error: {:?}", err).as_str())
                     }
                 )
            },
            (GET) (/{id: String}) => {
                 let wc = shared_ge.0.new_web3client();
                 let reader = reader::BlockchainReader::new(&wc,&shared_ge.0.db);
                 Response::html(
                     if let Some(id) = Id::from(&id) {
                         let html = match id {
                             Id::Addr(addr) => render::addr_info(&shared_ge.0.cfg,&reader,&shared_ge.0.hb,&addr),
                             Id::Tx(txid) => render::tx_info(&shared_ge.0.db,&reader,&shared_ge.0.hb,txid),
                             Id::Block(block) => render::block_info(&reader,&shared_ge.0.hb,block)
                         };
                         match html {
                             Ok(html) => html,
                             Err(err) => render::page(format!("Error: {:?}", err).as_str())
                         }
                     } else {
                         render::page("Not found")
                     }
                 )
            },
           (POST) (/{id: String}/contract) => {
                if let Some(Id::Addr(addr)) = Id::from(&id) {
                    
                    let data = try_or_400!(post_input!(request, {
                        contract_source: String,
                        contract_compiler: String,
                        contract_optimized: bool,
                        contract_name: String,
                    }));

                    let contract = contract::compile(&shared_ge.0.cfg,
                        &data.contract_source,
                        &data.contract_name,
                        &data.contract_compiler,
                        data.contract_optimized).expect("Error compiling");

                    let wc = shared_ge.0.new_web3client();
                    let reader = reader::BlockchainReader::new(&wc,&shared_ge.0.db);    
                    let code = reader.current_code(&addr).expect("failed to read code").0;
                    
                    // TODO: remove expect
                    if contract::codeequals(&contract,&code).expect("failed to check equals") {
                        let contractentry = db::Contract{
                            source : data.contract_source,
                            compiler : data.contract_compiler,
                            optimized: data.contract_optimized,
                            name : data.contract_name,
                            abi : contract.abi.clone(),
                            constructor : Vec::new(),
                        };

                        &shared_ge.0.db.set_contract(&addr,&contractentry).expect("cannot update db");
                        Response::redirect_302(format!("/{}",id))
                    } else {
                        Response::html("Code does not match")
                    }
                } else {
                    Response::html("bad input")
                }
            },
            _ => rouille::Response::empty_404()
        )
    })
}
