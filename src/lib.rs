use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use serde_json;

use magic_wormhole::{Code, transfer, transit, Wormhole, WormholeError, AppID, AppConfig, transfer::AppVersion, rendezvous};
use wasm_bindgen::prelude::*;
use std::{borrow::Cow, alloc::*};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn init() {
    wasm_logger::init(wasm_logger::Config::default());
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

struct NoOpFuture {}

impl Future for NoOpFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

#[wasm_bindgen]
pub struct ClientConfig {
    appid:                    AppID,
    rendezvous_url:           String,
    transit_server_url:       String,
    passphrase_component_len: usize,
}

#[wasm_bindgen]
impl ClientConfig {
    pub fn client_init(appid: &str, rendezvous_url: &str, transit_server_url: &str, passphrase_component_len: usize) -> Self {
        Self {
            appid: appid.to_string().into(),
            rendezvous_url: rendezvous_url.to_string(),
            transit_server_url: transit_server_url.to_string(),
            passphrase_component_len: passphrase_component_len,
        }
    }

    pub async fn send(&self, file_input: web_sys::HtmlInputElement, output: web_sys::HtmlElement) {
        let file_list = file_input.files().expect("Failed to get filelist from File Input!");
        if file_list.length() < 1 || file_list.get(0) == None {
            alert("Please select at least one valid file.");
            return;
        }

        let file: web_sys::File = file_list.get(0).expect("Failed to get File from filelist!");

        match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
            Ok(file_content) => {
                let array = js_sys::Uint8Array::new(&file_content);
                let len = array.byte_length() as u64;
                let data_to_send: Vec<u8> = array.to_vec();
                console_log!("Read raw data ({} bytes)", len);

                output.set_inner_text("connecting...");

                let rendezvous = Box::new(self.rendezvous_url.as_str());
                let config = transfer::APP_CONFIG.rendezvous_url(Cow::Owned(rendezvous.to_string()));
                let connect = Wormhole::connect_and_get_code(&config.id, rendezvous.to_string(), 2);

                match connect.await {
                    Ok((server_welcome, server)) => {
                        console_log!("{}", server_welcome.code);
                        output.set_inner_text(&format!("wormhole code:  {}", server_welcome.code));

                        send_via_wormhole(
                            &config,
                            server_welcome.code,
                            server,
                            data_to_send,
                            len,
                            file.name(),
                        ).await
                    }
                    Err(_) => {
                        console_log!("Error waiting for connection");
                    }
                }

            }
            Err(_) => {
                console_log!("Error reading file");
            }
        }
    }

}


#[wasm_bindgen]
pub async fn send(file_input: web_sys::HtmlInputElement, output: web_sys::HtmlElement) {
    let file_list = file_input.files().expect("Failed to get filelist from File Input!");
    if file_list.length() < 1 || file_list.get(0) == None {
        alert("Please select at least one valid file.");
        return;
    }

    let file: web_sys::File = file_list.get(0).expect("Failed to get File from filelist!");

    match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
        Ok(file_content) => {
            let array = js_sys::Uint8Array::new(&file_content);
            let len = array.byte_length() as u64;
            let data_to_send: Vec<u8> = array.to_vec();
            console_log!("Read raw data ({} bytes)", len);

            output.set_inner_text("connecting...");

            let rendezvous_url_str = "wss://mailbox.mw.leastauthority.com/v1";
            let config = transfer::APP_CONFIG.rendezvous_url(rendezvous_url_str.into());
            let connect = Wormhole::connect_and_get_code(&config.id, rendezvous_url_str.to_string(), 2);

            match connect.await {
                Ok((server_welcome, server)) => {
                    console_log!("{}", server_welcome.code);
                    output.set_inner_text(&format!("wormhole code:  {}", server_welcome.code));

                    send_via_wormhole(
                        &config,
                        server_welcome.code,
                        server,
                        data_to_send,
                        len,
                        file.name(),
                    ).await
                }
                Err(_) => {
                    console_log!("Error waiting for connection");
                }
            }

        }
        Err(_) => {
            console_log!("Error reading file");
        }
    }
}

async fn send_via_wormhole(config: &AppConfig<impl serde::Serialize + Send + Sync + 'static>,
                           code: Code,
                           server: rendezvous::RendezvousServer,
                           file: Vec<u8>,
                           file_size: u64,
                           file_name: String) {
    let transit_server_url = "wss://relay.winden.app/";

    let versions = serde_json::to_value({}).unwrap();
    let connector = Wormhole::connect_custom(server, config.id.clone(), code.0, versions);

    match connector.await {
        Ok(wormhole) => {
            let transfer_result = transfer::send_file(
                wormhole,
                url::Url::parse(transit_server_url).unwrap(),
                &mut &file[..],
                PathBuf::from(file_name),
                file_size,
                transit::Abilities::FORCE_RELAY,
                |info, address| {
                    console_log!("Connected to '{:?}' on address {:?}", info, address);
                },
                |cur, total| {
                    console_log!("Progress: {}/{}", cur, total);
                },
                NoOpFuture {},
            ).await;

            match transfer_result {
                Ok(_) => {
                    console_log!("Data sent");
                }
                Err(e) => {
                    console_log!("Error in data transfer: {:?}", e);
                }
            }
        }
        Err(_) => {
            console_log!("Error waiting for connection");
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ReceiveResult {
    data: Vec<u8>,
    filename: String,
    filesize: u64,
}

#[wasm_bindgen]
pub async fn receive(code: String, output: web_sys::HtmlElement) -> Option<JsValue> {
    let connect = Wormhole::connect_with_code(
        transfer::APP_CONFIG.rendezvous_url("ws://relay.magic-wormhole.io:4000/v1".into()),
        Code(code),
    );

    return match connect.await {
        Ok((_, wormhole)) => {
            let req = transfer::request_file(
                wormhole,
                url::Url::parse("ws://piegames.de:4002").unwrap(),
                transit::Abilities::FORCE_RELAY,
                NoOpFuture {},
            ).await;

            let mut file: Vec<u8> = Vec::new();

            match req {
                Ok(Some(req)) => {
                    let filename = req.filename.clone();
                    let filesize = req.filesize;
                    console_log!("File name: {:?}, size: {}", filename, filesize);
                    let file_accept = req.accept(
                        |info, address| {
                            console_log!("Connected to '{:?}' on address {:?}", info, address);
                        },
                        |cur, total| {
                            console_log!("Progress: {}/{}", cur, total);
                        },
                        &mut file,
                        NoOpFuture {},
                    );

                    match file_accept.await {
                        Ok(_) => {
                            console_log!("Data received, length: {}", file.len());
                            //let array: js_sys::Array = file.into_iter().map(JsValue::from).collect();
                            //data: js_sys::Uint8Array::new(&array),
                            let result = ReceiveResult {
                                data: file,
                                filename: filename.to_str().unwrap_or_default().into(),
                                filesize,
                            };
                            return Some(JsValue::from_serde(&result).unwrap());
                        }
                        Err(e) => {
                            console_log!("Error in data transfer: {:?}", e);
                            None
                        }
                    }
                }
                _ => {
                    console_log!("No ReceiveRequest");
                    None
                }
            }
        }
        Err(_) => {
            output.set_inner_text("Error in connection");
            None
        }
    };
}
