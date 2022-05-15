use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};

use magic_wormhole::{Code, transfer, transit, Wormhole, WormholeError};
use wasm_bindgen::JsCast;
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

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

async fn connect(code: Option<String>) -> Result<(String, Wormhole), WormholeError> {
    match code {
        Some(code) => {
            let (server_welcome, wormhole) = magic_wormhole::Wormhole::connect_with_code(
                transfer::APP_CONFIG.rendezvous_url("ws://relay.magic-wormhole.io:4000/v1".into()),
                Code(code),
            ).await?;
            console_log!("wormhole connection opened");

            Ok(("".into(), wormhole))
        }

        None => {
            let (server_welcome, connector) = magic_wormhole::Wormhole::connect_without_code(
                transfer::APP_CONFIG.rendezvous_url("ws://relay.magic-wormhole.io:4000/v1".into()),
                2,
            ).await?;
            console_log!("{:?}", server_welcome.code);

            let wormhole = connector.await?;

            Ok((server_welcome.code.0, wormhole))
        }
    }
}

struct NoOpFuture {}

impl Future for NoOpFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

#[wasm_bindgen]
pub async fn send(file_input: web_sys::HtmlInputElement, output: web_sys::HtmlElement) {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    //Check the file list from the input
    let filelist = file_input.files().expect("Failed to get filelist from File Input!");
    //Do not allow blank inputs
    if filelist.length() < 1 {
        alert("Please select at least one file.");
        return;
    }
    if filelist.get(0) == None {
        alert("Please select a valid file");
        return;
    }

    let file: web_sys::File = filelist.get(0).expect("Failed to get File from filelist!");
    let file_name = file.name();

    let file_reader: web_sys::FileReader = web_sys::FileReader::new().expect("failed to create a file reader");

    let onloadend_cb = Closure::wrap(Box::new(move |event: web_sys::ProgressEvent| {
        let file_reader_target = event.target().unwrap().dyn_into::<web_sys::FileReader>().unwrap();
        let array = js_sys::Uint8Array::new(&file_reader_target.result().unwrap());
        let len = array.byte_length() as u64;
        let data_to_send: Vec<u8> = array.to_vec();
        console_log!("Raw data ({} bytes): {:?}", len, data_to_send);

        wasm_bindgen_futures::spawn_local(send_via_wormhole(data_to_send, len, file_name.clone()))
    }) as Box<dyn Fn(web_sys::ProgressEvent)>);

    file_reader.set_onloadend(Some(onloadend_cb.as_ref().unchecked_ref()));
    file_reader.read_as_array_buffer(&file).expect("blob not readable");
    onloadend_cb.forget();
}

async fn send_via_wormhole(file: Vec<u8>, file_size: u64, file_name: String) {
    match connect(None).await {
        Ok((data, wormhole)) => {
            //output.set_inner_text(&data);
            console_log!("Connected. Code: {}", data);

            let transfer_result = transfer::send_file(
                wormhole,
                url::Url::parse("ws://relay.magic-wormhole.io:4000/v1").unwrap(),
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
            //output.set_inner_text(&"Error in connection".to_string());
            console_log!("Error in connection");
        }
    };
}

#[wasm_bindgen]
pub async fn receive(code: String, output: web_sys::HtmlElement) {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    match connect(Some(code)).await {
        Ok((_, wormhole)) => {
            let req = transfer::request_file(
                wormhole,
                url::Url::parse("ws://relay.magic-wormhole.io:4000/v1").unwrap(),
                transit::Abilities::FORCE_RELAY,
                NoOpFuture {},
            ).await;

            let mut file: Vec<u8> = Vec::new();

            match req {
                Ok(Some(req)) => {
                    console_log!("File name: {:?}, size: {}", req.filename, req.filesize);
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
                            console_log!("Data received");
                        }
                        Err(e) => {
                            console_log!("Error in data transfer: {:?}", e);
                        }
                    }
                }
                _ => {
                    console_log!("No ReceiveRequest");
                }
            };
        }
        Err(_) => {
            output.set_inner_text("Error in connection");
        }
    }
}
