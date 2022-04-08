use std::future::Future;
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

#[wasm_bindgen]
pub async fn send(file_input: web_sys::HtmlInputElement, output: web_sys::HtmlElement) {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    match connect(None).await {
        Ok((data, wormhole)) => {
            output.set_inner_text(&data);

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

            let file_reader: web_sys::FileReader = match web_sys::FileReader::new() {
                Ok(f) => f,
                Err(e) => {
                    alert("There was an error creating a file reader");
                    log(&JsValue::as_string(&e).expect("error converting jsvalue to string."));
                    web_sys::FileReader::new().expect("")
                }
            };

            let fr_c = file_reader.clone();
            // create onLoadEnd callback
            let onloadend_cb = Closure::wrap(Box::new(move |_e: web_sys::ProgressEvent| {
                let mut array = js_sys::Uint8Array::new(&fr_c.result().unwrap());
                let len = array.byte_length() as u64;
                log(&format!("Raw data ({} bytes): {:?}", len, array.to_vec()));
                // here you can for example use the received image/png data

                struct NoOpFuture {}
                impl Future for NoOpFuture {
                    type Output = ();

                    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                        Poll::Pending
                    }
                }
                
                let relay_url = url::Url::parse("ws://relay.magic-wormhole.io:4000/v1").unwrap();

                let data = array.to_vec();
                
                transfer::send_file(
                    wormhole,
                    relay_url,
                    &mut &data[..],
                    "file".into(),
                    len,
                    transit::Abilities::FORCE_RELAY,
                    move |_sent, _total| {
                        // if sent == 0 {
                        //     pb2.reset_elapsed();
                        //     pb2.enable_steady_tick(250);
                        // }
                        // pb2.set_position(sent);
                    },
                    NoOpFuture {},
                )
            }) as Box<dyn Fn(web_sys::ProgressEvent)>);

            file_reader.set_onloadend(Some(onloadend_cb.as_ref().unchecked_ref()));
            file_reader.read_as_array_buffer(&file).expect("blob not readable");
            onloadend_cb.forget();
        },
        Err(_) => {
            output.set_inner_text(&"Error in connection".to_string());
        }
    }
}

#[wasm_bindgen]
pub async fn receive(code: String, output: web_sys::HtmlElement) {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    match connect(Some(code)).await {
        Ok((data, wormhole)) => {
            output.set_inner_text(&data);
        },
        Err(_) => {
            output.set_inner_text(&"Error in connection".to_string());
        }
    }
}
