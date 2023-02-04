use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use serde_json;
use futures::io::{AsyncRead, Error};

use magic_wormhole::{Code, transfer, transit, Wormhole, WormholeError, AppID, AppConfig, transfer::AppVersion, rendezvous};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
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

    pub async fn send(&self, file: web_sys::File, output: web_sys::HtmlElement) {
        let name = file.name();
        let mut file_wrapper = FileWrapper::new(file);
        let size = file_wrapper.size;

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
                    &self.transit_server_url,
                    &mut file_wrapper,
                    size as u64,
                    name,
                ).await
            }
            Err(_) => {
                console_log!("Error waiting for connection");
            }
        }
    }

    pub async fn receive(&self, code: String, output: web_sys::HtmlElement) -> Option<JsValue> {
        let rendezvous = Box::new(self.rendezvous_url.as_str());
        let connect = Wormhole::connect_with_code(
            transfer::APP_CONFIG.rendezvous_url(Cow::Owned(rendezvous.to_string())),
            Code(code),
        );

        return match connect.await {
            Ok((_, wormhole)) => {
                let req = transfer::request_file(
                    wormhole,
                    url::Url::parse(&self.transit_server_url).unwrap(),
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
}

async fn send_via_wormhole(config: &AppConfig<impl serde::Serialize + Send + Sync + 'static>,
                           code: Code,
                           server: rendezvous::RendezvousServer,
                           transit_server_url: &str,
                           mut file: &mut FileWrapper,
                           file_size: u64,
                           file_name: String) {

    let versions = serde_json::to_value({}).unwrap();
    let connector = Wormhole::connect_custom(server, config.id.clone(), code.0, versions);

    match connector.await {
        Ok(wormhole) => {
            let transfer_result = transfer::send_file(
                wormhole,
                url::Url::parse(transit_server_url).unwrap(),
                &mut file,
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

struct FileWrapper {
    file: web_sys::File,
    size: i32,
    index: i32,
    f: Box<Option<JsFuture>>
}

impl FileWrapper {
    fn new(file: web_sys::File) -> Self {
        let size = file.size();
        FileWrapper {
            file: file,
            size: size as i32,
            index: 0,
            f: Box::new(None),
        }
    }
}

impl AsyncRead for FileWrapper {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {

        // use File::slice to read into buf
        // Poll::Ready(io::Read::read(&mut *self, buf))
        let start = self.index;
        let end = i32::min(start + buf.len() as i32, self.size);

        if let Some(f) = &mut *self.f {
            //let Some(f) = &mut *self.f;

            let p = Pin::new(&mut *f);
            match p.poll(cx) {
                Poll::Pending => {
                    Poll::Pending
                },
                Poll::Ready(array_buffer) => {
                    let abuf: js_sys::ArrayBuffer = array_buffer.unwrap().into();
                    js_sys::Uint8Array::new(&abuf).copy_to(buf);
                    self.f = Box::new(None);
                    let size = end - start;
                    // let size = abuf.byte_length() as i32;
                    self.index += size;
                    Poll::Ready(Ok(size as usize))
                }
            }
        } else {
            let blob = self.file.slice_with_i32_and_i32(start, end).unwrap();

            let mut array_buffer_future: JsFuture = blob.array_buffer().into();
            let p = Pin::new(&mut array_buffer_future);
            match p.poll(cx) {
                Poll::Pending => {
                    self.f = Box::new(Some(array_buffer_future));
                    Poll::Pending
                },
                Poll::Ready(array_buffer) => {
                    let abuf: js_sys::ArrayBuffer = array_buffer.unwrap().into();
                    js_sys::Uint8Array::new(&abuf).copy_to(buf);
                    self.f = Box::new(None);
                    // let abuf: js_sys::ArrayBuffer = array_buffer.unwrap().into();
                    // let size = abuf.byte_length() as i32;
                    let size = end - start;
                    self.index += size;
                    Poll::Ready(Ok((end - start) as usize))
                }
            }
        }
    }
}
