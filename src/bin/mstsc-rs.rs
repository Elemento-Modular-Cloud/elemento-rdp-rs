#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "linux")]
extern crate libc;
#[cfg(target_os = "macos")]
extern crate libc;
extern crate minifb;
extern crate rdp;
extern crate hex;
extern crate clap;
extern crate hmac;
extern crate websocket;
extern crate serde;
extern crate serde_json;

// use minifb::{Key, Window, WindowOptions, MouseMode, MouseButton, KeyRepeat};
use std::net::{SocketAddr, TcpStream};
use std::io::{Read, Write};
use std::ptr;
use std::mem;
use std::mem::{size_of, forget};
use std::time::Instant;
use rdp::core::client::{RdpClient, Connector};
#[cfg(target_os = "windows")]
use winapi::um::winsock2::{select, fd_set};
#[cfg(target_os = "linux")]
use libc::{select, fd_set, FD_SET};
#[cfg(target_os = "macos")]
use libc::{select, fd_set, FD_SET, FD_ZERO};
#[cfg(target_os = "windows")]
use std::os::windows::io::{AsRawSocket};
#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd};
#[cfg(target_os = "macos")]
use std::os::unix::io::{AsRawFd};
use rdp::core::event::{RdpEvent, BitmapEvent as RdpBitmapEvent, PointerEvent, PointerButton, KeyboardEvent};
// use std::convert::TryFrom;
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{JoinHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use rdp::model::error::{Error, RdpErrorKind, RdpError, RdpResult};
use clap::{Arg, App, ArgMatches};
use rdp::core::gcc::KeyboardLayout;
use std::sync::mpsc::{Sender, Receiver};
use serde::{Serialize, Deserialize};
use websocket::{Message, OwnedMessage, sync::Server};
use websocket::sync::{Writer, Reader};
use std::intrinsics::copy_nonoverlapping;
// use websocket::sync::Client;

const APPLICATION_NAME: &str = "mstsc-rs";

#[cfg(target_os = "macos")]
fn wait_for_fd(fd: usize) -> bool {
    unsafe {
        let mut raw_fds: fd_set = mem::zeroed();
        FD_ZERO(&mut raw_fds);
        FD_SET(fd as i32, &mut raw_fds);
        let result = select(fd as i32 + 1, &mut raw_fds, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
        result > 0
    }
}

/// Transmute is use to convert Vec<u8> -> Vec<u32>
/// To accelerate data convert
pub unsafe fn transmute_vec<S, T>(mut vec: Vec<S>) -> Vec<T> {
    let ptr = vec.as_mut_ptr();
    let capacity = vec.capacity() * size_of::<S>() / size_of::<T>();
    let len = vec.len() * size_of::<S>() / size_of::<T>();
    forget(vec);
    Vec::from_raw_parts(ptr as *mut T, len, capacity)
}

/// Create a tcp stream from main args
fn tcp_from_args(args: &ArgMatches) -> RdpResult<TcpStream> {
    let ip = args.value_of("target").expect("You need to provide a target argument");
    let port = args.value_of("port").unwrap_or_default();

    // TCP connection
    let addr = format!("{}:{}", ip, port).parse::<SocketAddr>().map_err( |e| {
        Error::RdpError(RdpError::new(RdpErrorKind::InvalidData, &format!("Cannot parse the IP PORT input [{}]", e)))
    })?;
    let tcp = TcpStream::connect(&addr).unwrap();
    tcp.set_nodelay(true).map_err(|e| {
        Error::RdpError(RdpError::new(RdpErrorKind::InvalidData, &format!("Unable to set no delay option [{}]", e)))
    })?;

    Ok(tcp)
}

/// Create rdp client from args
fn rdp_from_args<S: Read + Write>(args: &ArgMatches, stream: S) -> RdpResult<RdpClient<S>> {

    let width = args.value_of("width").unwrap_or_default().parse().map_err(|e| {
        Error::RdpError(RdpError::new(RdpErrorKind::UnexpectedType, &format!("Cannot parse the input width argument [{}]", e)))
    })?;
    let height = args.value_of("height").unwrap_or_default().parse().map_err(|e| {
        Error::RdpError(RdpError::new(RdpErrorKind::UnexpectedType, &format!("Cannot parse the input height argument [{}]", e)))
    })?;
    let domain = args.value_of("domain").unwrap_or_default();
    let username = args.value_of("username").unwrap_or_default();
    let password = args.value_of("password").unwrap_or_default();
    let name = args.value_of("name").unwrap_or_default();
    let ntlm_hash = args.value_of("hash");
    let restricted_admin_mode = args.is_present("admin");
    let layout = KeyboardLayout::from(args.value_of("layout").unwrap_or_default());
    let auto_logon = args.is_present("auto_logon");
    let blank_creds = args.is_present("blank_creds");
    let check_certificate = args.is_present("check_certificate");
    let use_nla = !args.is_present("disable_nla");

    let mut rdp_connector =  Connector::new()
        .screen(width, height)
        .credentials(domain.to_string(), username.to_string(), password.to_string())
        .set_restricted_admin_mode(restricted_admin_mode)
        .auto_logon(auto_logon)
        .blank_creds(blank_creds)
        .layout(layout)
        .check_certificate(check_certificate)
        .name(name.to_string())
        .use_nla(use_nla);

    if let Some(hash) = ntlm_hash {
        rdp_connector = rdp_connector.set_password_hash(hex::decode(hash).map_err(|e| {
            Error::RdpError(RdpError::new(RdpErrorKind::InvalidData, &format!("Cannot parse the input hash [{}]", e)))
        })?)
    }
    // RDP connection
    Ok(rdp_connector.connect(stream)?)
}

/// This will launch the thread in charge
/// of receiving event (mostly bitmap event)
/// And send back to the gui thread
fn launch_rdp_thread<S: 'static + Read + Write + Send>(
    handle: usize,
    rdp_client: Arc<Mutex<RdpClient<S>>>,
    sync: Arc<AtomicBool>,
    bitmap_channel: Sender<RdpBitmapEvent>) -> RdpResult<JoinHandle<()>> {
    // Create the rdp thread
    Ok(thread::spawn(move || {
        while wait_for_fd(handle as usize) && sync.load(Ordering::Relaxed) {
            let mut guard = rdp_client.lock().unwrap();
            if let Err(Error::RdpError(e)) = guard.read(|event| {
                match event {
                    RdpEvent::Bitmap(bitmap) => {
                        bitmap_channel.send(bitmap).unwrap();
                    },
                    _ => println!("{}: ignore event", APPLICATION_NAME)
                }
            }) {
                match e.kind() {
                    RdpErrorKind::Disconnect => {
                        println!("{}: Server ask for disconnect", APPLICATION_NAME);
                    },
                    _ => println!("{}: {:?}", APPLICATION_NAME, e)
                }
                break;
            }
        }
    }))
}

/// Copy a bitmap event into the buffer
/// This function use unsafe copy
/// to accelerate data transfer
fn fast_bitmap_transfer(buffer: &mut Vec<u32>, width: usize, bitmap: RdpBitmapEvent) -> RdpResult<()>{
    let bitmap_dest_left = bitmap.dest_left as usize;
    let bitmap_dest_right = bitmap.dest_right as usize;
    let bitmap_dest_bottom = bitmap.dest_bottom as usize;
    let bitmap_dest_top = bitmap.dest_top as usize;
    let bitmap_width = bitmap.width as usize;

    let data = bitmap.decompress()?;

    // Use some unsafe method to faster
    // data transfer between buffers
    unsafe {
        let data_aligned :Vec<u32> = transmute_vec(data);
        for i in 0..(bitmap_dest_bottom - bitmap_dest_top + 1) {
            let dest_i = (i + bitmap_dest_top) * width + bitmap_dest_left;
            let src_i = i * bitmap_width;
            let count = bitmap_dest_right - bitmap_dest_left + 1;
            if dest_i > buffer.len() || dest_i + count > buffer.len() || src_i > data_aligned.len() || src_i + count > data_aligned.len() {
                return Err(Error::RdpError(RdpError::new(RdpErrorKind::InvalidSize, "Image have invalide size")))
            }
            copy_nonoverlapping(data_aligned.as_ptr().offset((src_i) as isize), buffer.as_mut_ptr().offset(dest_i as isize), count)
        }
    }

    Ok(())
}

fn bitmap_loop<S: Read + Write>(
    width: usize,
    height: usize,
    rdp_client: Arc<Mutex<RdpClient<S>>>,
    sync: Arc<AtomicBool>,
    bitmap_receiver: Receiver<RdpBitmapEvent>,
    buffer: Arc<Mutex<Vec<u32>>>) -> RdpResult<()> {

    // Initialize the shared buffer if empty
    {
        let mut buf = buffer.lock().unwrap();
        if buf.is_empty() {
            *buf = vec![0; width * height];
        }
    }

    // Start the refresh loop
    while sync.load(Ordering::Relaxed) {
        let now = Instant::now();

        // Process bitmap updates at ~30 Hz
        while now.elapsed().as_micros() < 5000 {
            match bitmap_receiver.try_recv() {
                Ok(bitmap) => {
                    let mut buf = buffer.lock().unwrap();
                    fast_bitmap_transfer(&mut buf, width, bitmap)?;
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    sync.store(false, Ordering::Relaxed);
                    break
                }
            };
        }

        // Add a small sleep to prevent busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    sync.store(false, Ordering::Relaxed);
    rdp_client.lock().unwrap().shutdown()?;
    Ok(())
}

fn main() {
    // Parsing argument
    let matches = App::new(APPLICATION_NAME)
        .version("0.2.0")
        .author("Sylvain Peyrefitte <citronneur@gmail.com>, Gabriele Gaetano Fronze <gabriele.fronze@elemento.cloud>")
        .about("Secure Remote Desktop Client in RUST")
        .arg(Arg::with_name("target")
                 .long("target")
                 .takes_value(true)
                 .help("Target IP of the server"))
        .arg(Arg::with_name("port")
                 .long("port")
                 .takes_value(true)
                 .default_value("3389")
                 .help("Destination Port"))
        .arg(Arg::with_name("width")
                 .long("width")
                 .takes_value(true)
                 .default_value("1600")
                 .help("Screen width"))
        .arg(Arg::with_name("height")
                 .long("height")
                 .takes_value(true)
                 .default_value("1200")
                 .help("Screen height"))
        .arg(Arg::with_name("domain")
                 .long("dom")
                 .takes_value(true)
                 .default_value("")
                 .help("Windows domain"))
        .arg(Arg::with_name("username")
                 .long("user")
                 .takes_value(true)
                 .default_value("")
                 .help("Username"))
        .arg(Arg::with_name("password")
                 .long("pass")
                 .takes_value(true)
                 .default_value("")
                 .help("Password"))
        .arg(Arg::with_name("hash")
                 .long("hash")
                 .takes_value(true)
                 .help("NTLM Hash"))
        .arg(Arg::with_name("admin")
                 .long("admin")
                 .help("Restricted admin mode"))
        .arg(Arg::with_name("layout")
                 .long("layout")
                 .takes_value(true)
                 .default_value("us")
                 .help("Keyboard layout: us or fr"))
        .arg(Arg::with_name("auto_logon")
                 .long("auto")
                 .help("AutoLogon mode in case of SSL nego"))
        .arg(Arg::with_name("blank_creds")
                 .long("blank")
                 .help("Do not send credentials at the last CredSSP payload"))
        .arg(Arg::with_name("check_certificate")
                 .long("check")
                 .help("Check the target SSL certificate"))
        .arg(Arg::with_name("disable_nla")
                 .long("ssl")
                 .help("Disable Netwoek Level Authentication and only use SSL"))
        .arg(Arg::with_name("name")
                 .long("name")
                 .default_value("mstsc-rs")
                 .help("Name of the client send to the server"))
        .get_matches();

    // Create a tcp stream from args
    let tcp = tcp_from_args(&matches).unwrap();
    
    // Get appropriate handle based on platform before moving tcp
    #[cfg(target_os = "windows")]
    let handle = tcp.as_raw_socket() as usize;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let handle = tcp.as_raw_fd() as usize;

    // Get width and height from args
    let width: usize = matches.value_of("width").unwrap_or("1600").parse().unwrap();
    let height: usize = matches.value_of("height").unwrap_or("1200").parse().unwrap();

    // Create RDP client after getting handle
    let rdp_client_mutex = Arc::new(Mutex::new(rdp_from_args(&matches, tcp).unwrap()));
    
    // Keep track of connected WebSocket clients
    let ws_clients: Arc<Mutex<Vec<Writer<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
    let ws_clients_clone = ws_clients.clone();

    // Create channels and setup RDP client as before
    let (bitmap_sender, bitmap_receiver) = mpsc::channel();
    let sync = Arc::new(AtomicBool::new(true));
    let buffer = Arc::new(Mutex::new(Vec::new()));

    // Launch RDP thread
    let rdp_thread = launch_rdp_thread(
        handle,
        Arc::clone(&rdp_client_mutex),
        Arc::clone(&sync),
        bitmap_sender
    ).unwrap();

    // Start WebSocket server
    let server = Server::bind("127.0.0.1:9000").unwrap();
    println!("WebSocket server listening on port 9000");

    // Create a clone of the buffer for the bitmap loop
    let buffer_clone = Arc::clone(&buffer);
    let sync_clone = Arc::clone(&sync);
    let rdp_client_clone = Arc::clone(&rdp_client_mutex);

    // Launch bitmap processing in a separate thread
    let bitmap_handler = thread::spawn(move || {
        bitmap_loop(
            width,
            height,
            rdp_client_clone,
            sync_clone,
            bitmap_receiver,
            buffer_clone,
        ).unwrap();
    });

    // Create a thread to periodically send buffer updates to WebSocket clients
    let buffer_clone = Arc::clone(&buffer);
    let sync_clone = Arc::clone(&sync);
    let ws_sender = thread::spawn(move || {
        while sync_clone.load(Ordering::Relaxed) {
            // Send buffer update every 33ms (approximately 30fps)
            thread::sleep(std::time::Duration::from_millis(33));

            let buffer_data = {
                let buf = buffer_clone.lock().unwrap();
                if buf.is_empty() {
                    continue;
                }
                buf.clone()
            };

            let ws_frame = WsBufferUpdate {
                width: width as u16,
                height: height as u16,
                buffer: buffer_data,
            };

            let message = serde_json::to_string(&ws_frame).unwrap();
            
            // Send to all connected WebSocket clients
            let mut clients = ws_clients.lock().unwrap();
            clients.retain_mut(|sender| {
                sender.send_message(&Message::text(message.clone())).is_ok()
            });
        }
    });

    // Accept WebSocket connections in the main thread
    for request in server.filter_map(Result::ok) {
        let rdp_client = Arc::clone(&rdp_client_mutex);
        let ws_clients = Arc::clone(&ws_clients_clone);
        
        let client_addr = request.stream.peer_addr().map_or("Unknown".to_string(), |addr| addr.to_string());
        println!("New WebSocket client connecting from: {}", client_addr);
        
        thread::spawn(move || {
            if let Ok(client) = request.accept() {
                println!("WebSocket client connected from: {}", client_addr);
                let (receiver, sender) = client.split().unwrap();
                
                {
                    let mut clients = ws_clients.lock().unwrap();
                    clients.push(sender);
                    println!("Total connected clients: {}", clients.len());
                }
                
                handle_websocket(receiver, rdp_client, client_addr);
            } else {
                println!("WebSocket client failed to connect from: {}", client_addr);
            }
        });
    }

    // Wait for threads to complete
    bitmap_handler.join().unwrap();
    ws_sender.join().unwrap();
    rdp_thread.join().unwrap();
}

// Update handle_websocket signature to include client_addr
fn handle_websocket(
    mut receiver: Reader<TcpStream>,
    rdp_client: Arc<Mutex<RdpClient<TcpStream>>>,
    client_addr: String
) {
    for message in receiver.incoming_messages() {
        match message {
            Ok(OwnedMessage::Text(text)) => {
                println!("Received message: {}", text); // Debug logging
                if let Ok(event) = serde_json::from_str::<WsInputEvent>(&text) {
                    let mut client = rdp_client.lock().unwrap();
                    match event {
                        WsInputEvent::Mouse { x, y, button, down } => {
                            // Only process mouse events with button and down state
                            if let (Some(button), Some(down)) = (button, down) {
                                println!("[{}] Mouse event: pos=({}, {}), button={}, {}", 
                                    client_addr, x, y, button, 
                                    if down { "pressed" } else { "released" }
                                );
                                
                                let pointer_button = match button {
                                    0 => PointerButton::Left,
                                    1 => PointerButton::Middle,
                                    2 => PointerButton::Right,
                                    _ => PointerButton::None,
                                };

                                if let Err(e) = client.write(RdpEvent::Pointer(PointerEvent {
                                    x: x as u16,
                                    y: y as u16,
                                    button: pointer_button,
                                    down,
                                    wheel_delta: None,
                                })) {
                                    println!("Error sending mouse event from {}: {:?}", client_addr, e);
                                }
                            } else {
                                // Handle mouse move events (when button and down are null)
                                if let Err(e) = client.write(RdpEvent::Pointer(PointerEvent {
                                    x: x as u16,
                                    y: y as u16,
                                    button: PointerButton::None,
                                    down: false,
                                    wheel_delta: None,
                                })) {
                                    println!("Error sending mouse move event from {}: {:?}", client_addr, e);
                                }
                            }
                        }
                        WsInputEvent::Keyboard { code, down } => {
                            println!("[{}] Keyboard event: scancode=0x{:04x} ({})", 
                                client_addr, code,
                                if down { "pressed" } else { "released" }
                            );
                            
                            if let Err(e) = client.write(RdpEvent::Key(KeyboardEvent {
                                code,
                                down
                            })) {
                                println!("Error sending keyboard event from {}: {:?}", client_addr, e);
                            }
                        }
                        WsInputEvent::Wheel { x, y, delta } => {
                            println!("[{}] Wheel event: delta={}, pos=({}, {})", 
                                client_addr, delta, x, y
                            );
                            
                            if let Err(e) = client.write(RdpEvent::Pointer(PointerEvent {
                                x: x as u16,
                                y: y as u16,
                                button: PointerButton::Wheel,
                                down: true,
                                wheel_delta: Some(delta as i16),
                            })) {
                                println!("Error sending wheel event from {}: {:?}", client_addr, e);
                            }
                        }
                    }
                } else {
                    println!("Failed to parse message: {}", text);
                }
            }
            Ok(OwnedMessage::Close(_)) => {
                println!("WebSocket client disconnected: {}", client_addr);
                break;
            }
            Err(e) => {
                println!("WebSocket error for client {}: {:?}", client_addr, e);
                break;
            }
            _ => {}
        }
    }
}

// // Create a WebSocket message struct that matches RdpBitmapEvent field names
// #[derive(Serialize)]
// struct WsBitmapEvent {
//     bpp: u16,
//     width: u16,
//     height: u16,
//     dest_left: u16,
//     dest_top: u16,
//     dest_right: u16,
//     dest_bottom: u16,
//     data: Vec<u8>,
//     is_compress: bool,
// }

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum WsInputEvent {
    Mouse { x: i32, y: i32, button: Option<u8>, #[serde(rename = "is_pressed")] down: Option<bool> },
    #[serde(rename = "scancode")]
    Keyboard { #[serde(rename = "scancode")] code: u16, #[serde(rename = "is_pressed")] down: bool },
    Wheel { x: i32, y: i32, delta: i32 },
}

// Custom serialization for PointerButton
mod pointer_button_serde {
    // use super::PointerButton;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u8, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Ok(match value {
            0 => 0, // None
            1 => 1, // Left
            2 => 2, // Right
            3 => 3, // Middle
            _ => 0  // Default to None for unknown values
        })
    }
}

// Add this new struct for WebSocket buffer updates
#[derive(Serialize)]
struct WsBufferUpdate {
    width: u16,
    height: u16,
    buffer: Vec<u32>,
}
