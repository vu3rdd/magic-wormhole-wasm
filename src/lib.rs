use magic_wormhole::transfer;
use wasm_bindgen::prelude::*;

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, magic-wormhole-wasm!");
    let (server_welcome, connector) = magic_wormhole::Wormhole::connect_without_code(
        transfer::APP_CONFIG.rendezvous_url(rendezvous_server.into()),
        2,
    ).await?;
    let wormhole = connector.await?;
    println!(wormhole);
    println!(server_welcome.code);
}
