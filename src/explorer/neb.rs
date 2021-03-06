use super::error::{Error,Result};
use super::html::HtmlRender;
use super::utils;

use super::super::state::GlobalState;
use super::super::eth::BlockchainReader;

/// render the non-empty-blocks page
pub fn render(
    ge: &GlobalState,
    page_no : u64,
) -> Result<String> {

    let hr = HtmlRender::new(&ge); 
    let reader = BlockchainReader::new(&ge);
    let db = &ge.db;
    let hb = &ge.hb;

    let mut blocks = Vec::new();

    let count_non_empty_blocks = db.count_non_empty_blocks()?;
    let  limit = if count_non_empty_blocks > 200 {
        200
    } else {
        count_non_empty_blocks
    };
    let pg = utils::paginate(limit,15,page_no);

    if pg.from <= pg.to {
        let it = db.iter_non_empty_blocks()?.skip(pg.from as usize);
        for n in it.take((pg.to-pg.from) as usize) {
            if let Some(block) = reader.block(n)? {
                let author = utils::block_author(&ge.cfg,&block);
                let gas_used_p = (100*block.gas_used.low_u64())/block.gas_limit.low_u64();
                let gas_limit =  block.gas_limit.low_u64() / 100_000;
                blocks.push(json!({
                    "block"     : hr.blockno(n),
                    "author"    : hr.addr(&author),
                    "tx_count"  : block.transactions.len(),
                    "timestamp" : hr.timestamp(&block.timestamp),
                    "gas_used"   : format!("{}%",gas_used_p), 
                    "gas_limit"  : format!("{}.{}M",gas_limit/10,gas_limit%10)
                }));
            } else {
                return Err(Error::Unexpected);
            }
        }
    }

    Ok(hb.render(
        "neb.handlebars",
        &json!({
            "ui_title" : ge.cfg.ui_title,
            "last_indexed_block" : db.get_next_block_to_scan().unwrap(),
            "blocks": blocks,
            "has_next_page": pg.next_page.is_some(),
            "next_page": pg.next_page.unwrap_or(0),
            "has_prev_page": pg.prev_page.is_some(),
            "prev_page": pg.prev_page.unwrap_or(0),
        }),
    )?)
}
