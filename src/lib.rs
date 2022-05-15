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
            let (server_welcome, mut wormhole) = magic_wormhole::Wormhole::connect_with_code(
                transfer::APP_CONFIG.rendezvous_url("ws://relay.magic-wormhole.io:4000/v1".into()),
                Code(code),
            ).await?;
            console_log!("{:?}", server_welcome.code);

            let data = wormhole.receive().await?;
            let data = String::from_utf8(data).unwrap();
            console_log!("{}", data);

            Ok((data, wormhole))
        }

        None => {
            let (server_welcome, connector) = magic_wormhole::Wormhole::connect_without_code(
                transfer::APP_CONFIG.rendezvous_url("ws://relay.magic-wormhole.io:4000/v1".into()),
                2,
            ).await?;
            console_log!("{:?}", server_welcome.code);

            let mut wormhole = connector.await?;

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

    let file = filelist.get(0).expect("Failed to get File from filelist!");

    let file_reader: web_sys::FileReader = web_sys::FileReader::new().expect("failed to create a file reader");

    let onloadend_cb = Closure::wrap(Box::new(move |event: web_sys::ProgressEvent| {
        let file_reader_target = event.target().unwrap().dyn_into::<web_sys::FileReader>().unwrap();
        let array = js_sys::Uint8Array::new(&file_reader_target.result().unwrap());
        let len = array.byte_length() as u64;
        let data_to_send: Vec<u8> = array.to_vec();
        log(&format!("Raw data ({} bytes): {:?}", len, data_to_send));

        wasm_bindgen_futures::spawn_local(send_via_wormhole(data_to_send, len))
    }) as Box<dyn Fn(web_sys::ProgressEvent)>);

    file_reader.set_onloadend(Some(onloadend_cb.as_ref().unchecked_ref()));
    file_reader.read_as_array_buffer(&file).expect("blob not readable");
    onloadend_cb.forget();
}

async fn send_via_wormhole(file: Vec<u8>, file_size: u64) {
    match connect(None).await {
        Ok((data, wormhole)) => {
            //output.set_inner_text(&data);
            log(&format!("Connected. Code: {}", data));

            /*let transfer_result = transfer::send_file(
                wormhole,
                url::Url::parse("ws://relay.magic-wormhole.io:4000/v1").unwrap(),
                &mut &file[..],
                PathBuf::from("file"),
                file_size,
                transit::Abilities::FORCE_RELAY,
                move |_sent, _total| {
                    // if sent == 0 {
                    //     pb2.reset_elapsed();
                    //     pb2.enable_steady_tick(250);
                    // }
                    // pb2.set_position(sent);
                },
                |_cur, _total| {},
                NoOpFuture {},
            ).await;
            */

            let transfer_result: Result<(), ()> = Ok(());

            match transfer_result {
                Ok(_) => {
                    log(&format!("Data sent"));
                }
                Err(_) => {
                    log(&format!("Error in data transfer"));
                }
            }
        }
        Err(_) => {
            //output.set_inner_text(&"Error in connection".to_string());
            log(&format!("Error in connection"));
        }
    };
}

#[wasm_bindgen]
pub async fn receive(code: String, output: web_sys::HtmlElement) {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    match connect(Some(code)).await {
        Ok((data, wormhole)) => {
            output.set_inner_text(&data);
        }
        Err(_) => {
            output.set_inner_text(&"Error in connection".to_string());
        }
    }
}
