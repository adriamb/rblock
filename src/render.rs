use handlebars::Handlebars;
use web3::types::{Address, BlockId, BlockNumber, Bytes, Transaction, TransactionId, H256, U256};
use reader::BlockchainReader;
use rustc_hex::ToHex;
use serde_derive::Serialize;
use reqwest;
use ethabi;

use db;
use reader;
use state;
use contract;

lazy_static! {
    static ref GWEI: U256 = U256::from_dec_str("1000000000").unwrap();
    static ref ETHER: U256 = U256::from_dec_str("1000000000000000000").unwrap();
}

#[derive(Debug)]
pub enum Error {
    Unexpected,
    NotFound,
    Handlebars(handlebars::RenderError),
    Reqwest(reqwest::Error),
    Reader(reader::Error),
    Io(std::io::Error),
    Db(db::Error),
    EthAbi(ethabi::Error),
    Contract(contract::Error),
}

impl From<handlebars::RenderError> for Error {
    fn from(err: handlebars::RenderError) -> Self {
        Error::Handlebars(err)
    }
}
impl From<reader::Error> for Error {
    fn from(err: reader::Error) -> Self {
        Error::Reader(err)
    }
}
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Reqwest(err)
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
impl From<db::Error> for Error {
    fn from(err: db::Error) -> Self {
        Error::Db(err)
    }
}
impl From<ethabi::Error> for Error {
    fn from(err: ethabi::Error) -> Self {
        Error::EthAbi(err)
    }
}
impl From<contract::Error> for Error {
    fn from(err: contract::Error) -> Self {
        Error::Contract(err)
    }
}

pub struct TransactionIdShort(pub TransactionId);
pub struct GWei(pub U256);
pub struct Ether(pub U256);

#[derive(Serialize)]
pub struct TextWithLink {
    pub text: String,
    pub link: Option<String>,
}

impl TextWithLink {
    fn new_link(text: String, link: String) -> Self {
        TextWithLink {
            text: text,
            link: Some(link),
        }
    }
    fn new_text(text: String) -> Self {
        TextWithLink {
            text: text,
            link: None,
        }
    }
    fn blank() -> Self {
        TextWithLink {
            text: "".to_string(),
            link: None,
        }
    }
}

pub trait HtmlRender {
    fn html(&self) -> TextWithLink;
}

impl HtmlRender for Address {
    fn html(&self) -> TextWithLink {
        TextWithLink::new_link(format!("0x{:x}", self), format!("/0x{:x}", self))
    }
}

impl HtmlRender for Bytes {
    fn html(&self) -> TextWithLink {
        TextWithLink::new_text(
            (self.0)
                .as_slice()
                .chunks(32)
                .into_iter()
                .map(|c| c.to_hex::<String>())
                .map(|c| format!("{},", c))
                .collect::<String>(),
        )
    }
}

impl HtmlRender for Option<Address> {
    fn html(&self) -> TextWithLink {
        self.map(|v| v.html())
            .unwrap_or(TextWithLink::new_text("New contract".to_string()))
    }
}

impl HtmlRender for TransactionId {
    fn html(&self) -> TextWithLink {
        match &self {
            TransactionId::Hash(h) => {
                TextWithLink::new_link(format!("0x{:x}", h), format!("/0x{:x}", h))
            }
            _ => unreachable!(),
        }
    }
}

impl HtmlRender for TransactionIdShort {
    fn html(&self) -> TextWithLink {
        match &self.0 {
            TransactionId::Hash(h) => TextWithLink::new_link(
                format!("{:x}", h).chars().take(7).collect::<String>(),
                format!("/0x{:x}", h),
            ),
            _ => unreachable!(),
        }
    }
}


impl HtmlRender for BlockId {
    fn html(&self) -> TextWithLink {
        match &self {
            BlockId::Number(BlockNumber::Number(n)) => {
                TextWithLink::new_link(format!("{}", n), format!("/{}", n))
            }
            _ => unreachable!(),
        }
    }
}

impl HtmlRender for GWei {
    fn html(&self) -> TextWithLink {
        TextWithLink::new_text(format!("{} GWei ({})", self.0 / *GWEI, self.0))
    }
}

impl HtmlRender for Ether {
    fn html(&self) -> TextWithLink {
        if self.0 == U256::zero()  {
            TextWithLink::new_text("0 Ξ".to_string())
        } else {
            let ether  = self.0 / *ETHER;
            let mut remain = self.0 % *ETHER;
            while remain > U256::zero() && remain % 10 == U256::zero() {
                remain = remain / 10; 
            }
            TextWithLink::new_text(format!("{}.{} Ξ", ether, remain))
        }
    }
}

pub fn page(innerhtml: &str) -> String {
    let mut html = String::from("");
    html.push_str("<html><style>body {font-family: Courier;}</style>");
    html.push_str(&innerhtml.replace(" ", "&nbsp;").replace("_", " "));
    html.push_str("</html>");
    html
}

fn tx_short_json(tx: &Transaction) -> serde_json::Value {
    let shortdata = tx
        .input.0.to_hex::<String>()
        .chars().take(8).collect::<String>();

    let blockid =
        BlockId::Number(BlockNumber::Number(tx.block_number.unwrap().low_u64()));

    json!({
        "blockno"       : blockid.html(),
        "tx"            : TransactionIdShort(TransactionId::Hash(tx.hash)).html(),
        "from"          : tx.from.html(),
        "tonewcontract" : tx.to.is_none(),
        "to"            : if let Some(to) = tx.to { to.html() } else { TextWithLink::blank()},
        "shortdata"     : shortdata,
        "value"         : Ether(tx.value).html()
    })
}

pub fn block_info(
    reader: &BlockchainReader,
    hb: &Handlebars,
    blockno: u64,
) -> Result<String, Error> {
    if let Some(block) = reader.block_with_txs(blockno)? {
        let mut txs = Vec::new();
        for tx in &block.transactions {
            txs.push(tx_short_json(&tx));
        }
        Ok(hb.render(
            "block.handlebars",
            &json!({
            "blockno"          : blockno,
            "parent_hash"      : block.parent_hash,
            "uncles_hash"      : block.uncles_hash,
            "author"           : block.author.html(),
            "state_root"       : block.state_root,
            "receipts_root"    : block.receipts_root,
            "gas_used"         : block.gas_used.low_u64(),
            "gas_limit"        : block.gas_limit.low_u64(),
            "extra_data"       : block.extra_data,
            "timestamp"        : block.timestamp,
            "difficulty"       : block.difficulty,
            "total_difficulty" : block.total_difficulty,
            "seal_fields"      : block.seal_fields,
            "uncles"           : block.uncles,
            "txs"              : txs
            }),
        )?)
    } else {
        Err(Error::NotFound)
    }
}

pub fn tx_info(db : &db::AppDB, reader: &BlockchainReader, hb: &Handlebars, txid: H256) -> Result<String, Error> {

    if let Some((tx, receipt)) = reader.tx(txid)? {

        let mut logs = Vec::new();
        let mut cumulative_gas_used = String::from("");
        let mut gas_used = String::from("");
        let mut contract_address = TextWithLink::blank();
        let mut status = String::from("");
        
        if let Some(receipt) = receipt {

            cumulative_gas_used = format!("{}", receipt.cumulative_gas_used.low_u64());
            gas_used = format!("{}", receipt.gas_used.low_u64());
            contract_address = receipt
                .contract_address
                .map_or_else(|| TextWithLink::blank(), |c| c.html());
            status = receipt
                .status
                .map_or_else(|| String::from(""), |s| format!("{}", s));


            for (_, log) in receipt.logs.into_iter().enumerate() {
                
                let mut txt = Vec::new();

                if let Some(contract) = db.get_contract(&log.address)? {
                    // TODO: remove clone
                    let callinfo = contract::log_to_string(&contract.abi,log.clone())?;
                    txt.extend_from_slice(&callinfo);
                    txt.push(String::from(""));
                }

                txt.push(format!("data"));
                for ll in log.data.html().text.split(',') {
                    txt.push(format!("  {}",ll));
                }
                
                txt.push(format!("topics"));
                for (t, topic) in log.topics.into_iter().enumerate() {
                    txt.push(format!("  [{}] {:?}",t,topic));
                }

                logs.push(json!({
                    "address" : log.address.html(),
                    "txt"     : txt,
                }));

            }

        }

        // log_to_string
        let mut input: Vec<String> = Vec::new();
        if let Some(to) = tx.to {
            if let Some(contract) = db.get_contract(&to)? {
                let callinfo = contract::call_to_string(&contract.abi,&tx.input.0)?;
                input.extend_from_slice(&callinfo);
                input.push(String::from(""));
            }

            let inputhtml = tx.input.html();
            let inputvec : Vec<String> = inputhtml.text.split(',').map(|x| x.to_string()).collect(); 
            input.extend_from_slice(&inputvec);
        }

        let block = tx.block_number.map_or_else(
            || TextWithLink::blank(),
            |b| BlockId::Number(BlockNumber::Number(b.low_u64())).html(),
        );

        Ok(hb.render(
            "tx.handlebars",
            &json!({
            "txhash"              : format!("0x{:x}",txid),
            "from"                : tx.from.html(),
            "to"                  : tx.to.html(),
            "value"               : Ether(tx.value).html().text,
            "block"               : block,
            "gas"                 : tx.gas.low_u64(),
            "gas_price"           : GWei(tx.gas_price).html().text,
            "cumulative_gas_used" : cumulative_gas_used,
            "gas_used"            : gas_used,
            "contract_address"    : contract_address,
            "status"              : status,
            "input"               : input,
            "logs"                : logs,
            }),
        )?)
    } else {
        Err(Error::NotFound)
    }
}

pub fn addr_info(
    cfg : &state::Config,
    reader: &BlockchainReader,
    hb: &Handlebars,
    addr: &Address,
) -> Result<String, Error> {

    let balance = reader.current_balance(addr)?;
    let code = reader.current_code(addr)?;
    let mut txs = Vec::new();

    for txhash in reader.db.iter_addr_txs(&addr).take(20) {
        if let Some(txrc) = reader.tx(txhash)? {
            txs.push(tx_short_json(&txrc.0));
        }
    }

    if &code.0.len() > &0 {

        let rawcodehtml = code.html().text;
        let rawcode = rawcodehtml.split(',').into_iter().collect::<Vec<&str>>();
        
        if let Some(contract) = reader.db.get_contract(addr)? {
            Ok(hb.render(
                "address.handlebars",
                &json!({
                    "address" : format!("0x{:x}",addr),
                    "balance" : Ether(balance).html().text,
                    "txs" : txs,
                    "hascode" : true,
                    "rawcode" : rawcode,
                    "hascontract" : true,
                    "contract_source" : contract.source,
                    "contract_name" : contract.name,
                    "contract_abi" : contract.abi,
                    "contract_compiler" : contract.compiler,
                    "contract_optimized": contract.optimized
                })
            )?)            
        } else {
            let solcversions =  contract::compilers(&cfg)?;
            Ok(hb.render(
                "address.handlebars",
                &json!({
                    "address" : format!("0x{:x}",addr),
                    "balance" : Ether(balance).html().text,
                    "txs" : txs,
                    "hascode" : true,
                    "rawcode" : rawcode,
                    "hascontract" : false,
                    "solcversions" : solcversions,
                })
            )?)
        }
    } else {    
        Ok(hb.render(
            "address.handlebars",
            &json!({
                "address" : format!("0x{:x}",addr),
                "balance" : Ether(balance).html().text,
                "txs"     : txs,
                "hascode" : false,
            })
        )?)
    }
}

pub fn home(reader: &BlockchainReader, hb: &Handlebars) -> Result<String, Error> {
    let mut last_blockno = reader.current_block_number()?;
    let mut blocks = Vec::new();

    for _ in 0..20 {
        let blockno = BlockId::Number(BlockNumber::Number(last_blockno));
        if let Some(block) = reader.block(last_blockno)? {
            blocks.push(json!({
                "block"    : blockno.html(),
                "tx_count" : block.transactions.len()
            }));
        } else {
            return Err(Error::Unexpected);
        }
        last_blockno = last_blockno - 1;
    }

    Ok(hb.render(
        "home.handlebars",
        &json!({
            "last_indexed_block" : reader.db.get_last_block().unwrap(),
            "blocks": blocks
        }),
    )?)
}