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
mod explorer;
mod scanner;
mod state;
mod types;
mod contract;

use std::env;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use rouille::Response;

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
            (GET)  (/) => {
                explorer::get_home(&shared_ge.0)
            },
            (GET)  (/{id: String}) => {
                explorer::get_object(&shared_ge.0,&id)
            },
            (POST) (/{id: String}/contract) => {
                let data = try_or_400!(post_input!(request, {
                    contract_source: String,
                    contract_compiler: String,
                    contract_optimized: bool,
                    contract_name: String,
                }));
                explorer::post_contract(&shared_ge.0, &id,
                    &data.contract_source, &data.contract_compiler,
                    data.contract_optimized, &data.contract_name
                )
            },
            _ => rouille::Response::empty_404()
        )
    })
}
