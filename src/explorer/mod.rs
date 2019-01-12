use web3::types::{Address,H256};

mod address;
mod block;
mod error;
mod home;
mod html;
mod tx;

use super::state::GlobalState;
use super::reader::BlockchainReader;
use super::types::{hex_to_addr,hex_to_h256};
use super::contract;
use super::db;
use Response;

#[derive(Serialize)]
pub enum Id {
    Addr(Address),
    Tx(H256),
    Block(u64),
}

impl Id {
    pub fn from(id: &str) -> Option<Self> {
        if id.len() == 42 /* address */
        {
            hex_to_addr(id).map(Id::Addr).ok()
        } else if id.len() == 66 /* tx */
        {
            hex_to_h256(id).map(Id::Tx).ok()
        } else if let Ok(blockno_u64) = id.parse::<u64>() {
            Some(Id::Block(blockno_u64))
        } else {
            None
        }
    }
}

pub fn error_page(innerhtml: &str) -> String {
    let mut html = String::from("");
    html.push_str("<html><style>body {font-family: Courier;}</style>");
    html.push_str(&innerhtml.replace(" ", "&nbsp;").replace("_", " "));
    html.push_str("</html>");
    html
}

pub fn get_home(ge: &GlobalState) -> Response {
    let wc = ge.new_web3client();
    let reader = BlockchainReader::new(&wc,&ge.db);
    Response::html(
        match home::html(&reader,&ge.hb) {
            Ok(html) => html,
            Err(err) => error_page(format!("Error: {:?}", err).as_str())
        }
    )
}

pub fn get_object(ge: &GlobalState, id: &str) -> Response {
    let wc = ge.new_web3client();
    let reader = BlockchainReader::new(&wc,&ge.db);
    Response::html(
        if let Some(id) = Id::from(&id) {
            let html = match id {
                Id::Addr(addr) => address::html(&ge.cfg,&reader,&ge.hb,&addr),
                Id::Tx(txid) => tx::html(&ge.db,&reader,&ge.hb,txid),
                Id::Block(block) => block::html(&reader,&ge.hb,block)
            };
            match html {
                Ok(html) => html,
                Err(err) => error_page(format!("Error: {:?}", err).as_str())
            }
        } else {
            error_page("Not found")
        }
    )
}

pub fn post_contract(
    ge: &GlobalState,
    id: &str,
    contract_source: &str,
    contract_compiler: &str,
    contract_optimized: bool,
    contract_name: &str
) -> Response {

    if let Some(Id::Addr(addr)) = Id::from(&id) {
        let wc = ge.new_web3client();
        let reader = BlockchainReader::new(&wc,&ge.db);

        let code = reader.current_code(&addr).expect("failed to read contract code").0;
        
        let abi = contract::compile_and_verify(&ge.cfg,
            &contract_source,
            &contract_name,
            &contract_compiler,
            contract_optimized,
            &code
        ).expect("cannot verify contract code");

        // TODO remove to_string clones
        let contractentry = db::Contract{
            source : contract_source.to_string(),
            compiler : contract_compiler.to_string(),
            optimized: contract_optimized,
            name : contract_name.to_string(),
            abi ,
            constructor : Vec::new(),
        };
        ge.db.set_contract(&addr,&contractentry).expect("cannot update db");

        Response::redirect_302(format!("/{}",id))
    } else {
        Response::html(error_page("bad input"))
    }
}