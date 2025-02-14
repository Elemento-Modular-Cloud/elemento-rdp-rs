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
extern crate serde;
extern crate serde_json;
extern crate websocket;

// use minifb::{Key, Window, WindowOptions, MouseMode, MouseButton, KeyRepeat};
use std::net::{SocketAddr, TcpStream};
use std::io::{Read, Write};
// use std::time::{Instant};
use std::ptr;
use std::mem;
use std::mem::{size_of, forget};
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
// use std::ptr::copy_nonoverlapping;
use std::convert::TryFrom;
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{JoinHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use rdp::model::error::{Error, RdpErrorKind, RdpError, RdpResult};
use clap::{Arg, App, ArgMatches};
use rdp::core::gcc::KeyboardLayout;
use std::sync::mpsc::{Sender};
use serde::{Serialize, Deserialize};
use websocket::{Message, OwnedMessage, sync::Server};
// use websocket::sync::stream::Stream as WebSocketStream;
// use websocket::receiver::Receiver as WsReceiver;
// use websocket::sender::Sender as WsSender;
// use websocket::ws::sender::Sender as WsSenderTrait;
use websocket::sync::{Writer, Reader};
// use websocket::sync::Client;

const APPLICATION_NAME: &str = "mstsc-rs";

/// This is a function just to check if data
/// is available on socket to work only in one thread
#[cfg(target_os = "windows")]
fn wait_for_fd(fd: usize) -> bool {
    unsafe {
        let mut raw_fds: fd_set = mem::zeroed();
        raw_fds.fd_array[0] = fd;
        raw_fds.fd_count = 1;
        let result = select(0, &mut raw_fds, ptr::null_mut(), ptr::null_mut(), ptr::null());
        result == 1
    }
}

#[cfg(target_os = "linux")]
fn wait_for_fd(fd: usize) -> bool {
    unsafe {
        let mut raw_fds: fd_set = mem::zeroed();

        FD_SET(fd as i32, &mut raw_fds);
        
        let result = select(fd as i32 + 1, &mut raw_fds, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
        result == 1
    }
}

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

/// Copy a bitmap event into the buffer
/// This function use unsafe copy
/// to accelerate data transfer
// fn fast_bitmap_transfer(buffer: &mut Vec<u32>, width: usize, bitmap: RdpBitmapEvent) -> RdpResult<()>{
//     let bitmap_dest_left = bitmap.dest_left as usize;
//     let bitmap_dest_right = bitmap.dest_right as usize;
//     let bitmap_dest_bottom = bitmap.dest_bottom as usize;
//     let bitmap_dest_top = bitmap.dest_top as usize;
//     let bitmap_width = bitmap.width as usize;

//     let data = bitmap.decompress()?;

//     // Use some unsafe method to faster
//     // data transfer between buffers
//     unsafe {
//         let data_aligned :Vec<u32> = transmute_vec(data);
//         for i in 0..(bitmap_dest_bottom - bitmap_dest_top + 1) {
//             let dest_i = (i + bitmap_dest_top) * width + bitmap_dest_left;
//             let src_i = i * bitmap_width;
//             let count = bitmap_dest_right - bitmap_dest_left + 1;
//             if dest_i > buffer.len() || dest_i + count > buffer.len() || src_i > data_aligned.len() || src_i + count > data_aligned.len() {
//                 return Err(Error::RdpError(RdpError::new(RdpErrorKind::InvalidSize, "Image have invalide size")))
//             }
//             copy_nonoverlapping(data_aligned.as_ptr().offset((src_i) as isize), buffer.as_mut_ptr().offset(dest_i as isize), count)
//         }
//     }

//     Ok(())
// }

/// Translate minifb mouse to rdp-rs
// fn get_rdp_pointer_down(window: &Window) -> PointerButton {
//     if window.get_mouse_down(MouseButton::Left) {
//         PointerButton::Left
//     } else if window.get_mouse_down(MouseButton::Middle) {
//         PointerButton::Middle
//     } else if window.get_mouse_down(MouseButton::Right) {
//         PointerButton::Right
//     } else {
//         PointerButton::None
//     }
// }

/// Translate minifb key to scancode
// fn to_scancode(key: Key) -> u16 {
//     match key {
//         Key::Escape => 0x0001,
//         Key::Key1 => 0x0002,
//         Key::Key2 => 0x0003,
//         Key::Key3 => 0x0004,
//         Key::Key4 => 0x0005,
//         Key::Key5 => 0x0006,
//         Key::Key6 => 0x0007,
//         Key::Key7 => 0x0008,
//         Key::Key8 => 0x0009,
//         Key::Key9 => 0x000A,
//         Key::Key0 => 0x000B,
//         Key::Minus => 0x000C,
//         Key::Equal => 0x000D,
//         Key::Backspace => 0x000E,
//         Key::Tab => 0x000F,
//         Key::Q => 0x0010,
//         Key::W => 0x0011,
//         Key::E => 0x0012,
//         Key::R => 0x0013,
//         Key::T => 0x0014,
//         Key::Y => 0x0015,
//         Key::U => 0x0016,
//         Key::I => 0x0017,
//         Key::O => 0x0018,
//         Key::P => 0x0019,
//         Key::LeftBracket => 0x001A,
//         Key::RightBracket => 0x001B,
//         Key::Enter => 0x001C,
//         Key::LeftCtrl => 0x001D,
//         Key::A => 0x001E,
//         Key::S => 0x001F,
//         Key::D => 0x0020,
//         Key::F => 0x0021,
//         Key::G => 0x0022,
//         Key::H => 0x0023,
//         Key::J => 0x0024,
//         Key::K => 0x0025,
//         Key::L => 0x0026,
//         Key::Semicolon => 0x0027,
//         Key::Apostrophe => 0x0028,
//         Key::Backquote => 0x0029,
//         Key::LeftShift => 0x002A,
//         Key::Backslash => 0x002B,
//         Key::Z => 0x002C,
//         Key::X => 0x002D,
//         Key::C => 0x002E,
//         Key::V => 0x002F,
//         Key::B => 0x0030,
//         Key::N => 0x0031,
//         Key::M => 0x0032,
//         Key::Comma => 0x0033,
//         Key::Period => 0x0034,
//         Key::Slash => 0x0035,
//         Key::RightShift => 0x0036,
//         Key::NumPadAsterisk => 0x0037,
//         Key::LeftAlt => 0x0038,
//         Key::Space => 0x0039,
//         Key::CapsLock => 0x003A,
//         Key::F1 => 0x003B,
//         Key::F2 => 0x003C,
//         Key::F3 => 0x003D,
//         Key::F4 => 0x003E,
//         Key::F5 => 0x003F,
//         Key::F6 => 0x0040,
//         Key::F7 => 0x0041,
//         Key::F8 => 0x0042,
//         Key::F9 => 0x0043,
//         Key::F10 => 0x0044,
//         Key::Pause => 0x0045,
//         Key::ScrollLock => 0x0046,
//         Key::NumPad7 => 0x0047,
//         Key::NumPad8 => 0x0048,
//         Key::NumPad9 => 0x0049,
//         Key::NumPadMinus => 0x004A,
//         Key::NumPad4 => 0x004B,
//         Key::NumPad5 => 0x004C,
//         Key::NumPad6 => 0x004D,
//         Key::NumPadPlus => 0x004E,
//         Key::NumPad1 => 0x004F,
//         Key::NumPad2 => 0x0050,
//         Key::NumPad3 => 0x0051,
//         Key::NumPad0 => 0x0052,
//         Key::NumPadDot => 0x0053,
//         Key::F11 => 0x0057,
//         Key::F12 => 0x0058,
//         Key::F13 => 0x0064,
//         Key::F14 => 0x0065,
//         Key::F15 => 0x0066,
//         Key::NumPadEnter => 0xE01C,
//         Key::RightCtrl => 0xE01D,
//         Key::NumPadSlash => 0xE035,
//         Key::RightAlt => 0xE038,
//         Key::NumLock => 0xE045,
//         Key::Home => 0xE047,
//         Key::Up => 0xE048,
//         Key::PageUp => 0xE049,
//         Key::Left => 0xE04B,
//         Key::Right => 0xE04D,
//         Key::End => 0xE04F,
//         Key::Down => 0xE050,
//         Key::PageDown => 0xE051,
//         Key::Insert => 0xE052,
//         Key::Delete => 0xE053,
//         Key::LeftSuper => 0xE05B,
//         Key::RightSuper => 0xE05C,
//         Key::Menu => 0xE05D,
//         _ => panic!("foo")
//     }
// }

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

/// This function is in charge of the
/// window refresh loop
/// It's also in charge to send input
/// like keyboard and mouse to the
/// RDP protocol
// fn window_from_args(args: &ArgMatches) -> RdpResult<Window> {
//     let width = args.value_of("width").unwrap_or_default().parse().map_err(|e| {
//         Error::RdpError(RdpError::new(RdpErrorKind::UnexpectedType, &format!("Cannot parse the input width argument [{}]", e)))
//     })?;
//     let height = args.value_of("height").unwrap_or_default().parse().map_err(|e| {
//         Error::RdpError(RdpError::new(RdpErrorKind::UnexpectedType, &format!("Cannot parse the input height argument [{}]", e)))
//     })?;

//     let window = Window::new(
//         "Elemento Remote Desktop in Rust",
//         width,
//         height,
//         WindowOptions {
//             resize: true,
//             scale: minifb::Scale::X1,
//             ..WindowOptions::default()
//         },
//     ).map_err(|e| {
//         Error::RdpError(RdpError::new(RdpErrorKind::Unknown, &format!("Unable to create window [{}]", e)))
//     })?;

//     Ok(window)
// }

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

/// This is the main loop
/// Print Window and handle all input (mous + keyboard)
/// to RDP
// fn main_gui_loop<S: Read + Write>(
//     mut window: Window,
//     rdp_client: Arc<Mutex<RdpClient<S>>>,
//     sync: Arc<AtomicBool>,
//     bitmap_receiver: Receiver<RdpBitmapEvent>) -> RdpResult<()> {

//     let (width, height) = window.get_size();
//     // Now we continue with the graphical main thread
//     // Limit to max ~60 fps update rate
//     window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

//     // The window buffer
//     let mut buffer: Vec<u32> = vec![0; width * height];

//     // State for mouse button
//     let mut last_button = PointerButton::None;

//     // state for keyboard keys
//     let mut last_keys = vec![];

//     // Start the refresh loop
//     while window.is_open() && sync.load(Ordering::Relaxed) {
//         let now = Instant::now();

//         // Refresh loop must faster than 30 Hz
//         while now.elapsed().as_micros() < 16600 * 2 {
//             match bitmap_receiver.try_recv() {
//                 Ok(bitmap) => fast_bitmap_transfer(&mut buffer, width, bitmap)?,
//                 Err(mpsc::TryRecvError::Empty) => break,
//                 Err(mpsc::TryRecvError::Disconnected) => {
//                     sync.store(false, Ordering::Relaxed);
//                     break
//                 }
//             };
//         }

//         // Mouse position input
//         if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp) {
//             let mut rdp_client_guard = rdp_client.lock().map_err(|e| {
//                 Error::RdpError(RdpError::new(RdpErrorKind::Unknown, &format!("Thread error during access to mutex [{}]", e)))
//             })?;

//             // Button is down if not 0
//             let current_button = get_rdp_pointer_down(&window);
//             rdp_client_guard.write(RdpEvent::Pointer(
//                 PointerEvent{
//                     x: x as u16,
//                     y: y as u16,
//                     button: if last_button == current_button { PointerButton::None } else { PointerButton::try_from(last_button as u8 | current_button as u8).unwrap() },
//                     down: (last_button != current_button) && last_button == PointerButton::None
//                 })
//             )?;

//             last_button = current_button;
//         }

//         // Keyboard inputs
//         if let Some(keys) = window.get_keys() {
//             let mut rdp_client_guard = rdp_client.lock().unwrap();

//             for key in last_keys.iter() {
//                 if !keys.contains(key) {
//                     rdp_client_guard.write(RdpEvent::Key(
//                         KeyboardEvent {
//                             code: to_scancode(*key),
//                             down: false
//                         })
//                     )?
//                 }
//             }

//             for key in keys.iter() {
//                 if window.is_key_pressed(*key, KeyRepeat::Yes){
//                     rdp_client_guard.write(RdpEvent::Key(
//                         KeyboardEvent {
//                             code: to_scancode(*key),
//                             down: true
//                         })
//                     )?
//                 }
//             }

//             last_keys = keys;
//         }

//         // We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
//         window.update_with_buffer(&buffer, width, height).map_err(|e| {
//             Error::RdpError(RdpError::new(RdpErrorKind::Unknown, &format!("Unable to update screen buffer [{}]", e)))
//         })?;
//     }

//     sync.store(false, Ordering::Relaxed);
//     rdp_client.lock().unwrap().shutdown()?;
//     Ok(())
// }

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

    // Create RDP client after getting handle
    let rdp_client_mutex = Arc::new(Mutex::new(rdp_from_args(&matches, tcp).unwrap()));
    
    // Keep track of connected WebSocket clients
    let ws_clients: Arc<Mutex<Vec<Writer<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
    let ws_clients_clone = ws_clients.clone();

    // Create channels and setup RDP client as before
    let (bitmap_sender, bitmap_receiver) = mpsc::channel();
    let sync = Arc::new(AtomicBool::new(true));

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
    let mut count = 0;

    // Handle bitmap events and forward to WebSocket in a separate thread
    let bitmap_handler = thread::spawn(move || {
        while let Ok(bitmap) = bitmap_receiver.recv() {
            println!("Received bitmap event");
            println!("bitmap.bpp: {}", bitmap.bpp);
            println!("bitmap.width: {}", bitmap.width);
            println!("bitmap.height: {}", bitmap.height);
            println!("bitmap.dest_left: {}", bitmap.dest_left);
            println!("bitmap.dest_top: {}", bitmap.dest_top);
            println!("bitmap.dest_right: {}", bitmap.dest_right);
            println!("bitmap.dest_bottom: {}", bitmap.dest_bottom);
            println!("bitmap.is_compress: {}", bitmap.is_compress);
            println!("bitmap.data: {}", bitmap.data.len());
            
            let ws_bitmap = WsBitmapEvent {
                bpp: bitmap.bpp,
                width: bitmap.width,
                height: bitmap.height,
                dest_left: bitmap.dest_left,
                dest_top: bitmap.dest_top,
                dest_right: bitmap.dest_right,
                dest_bottom: bitmap.dest_bottom,
                data: bitmap.data,
                is_compress: bitmap.is_compress
            };

            let message = serde_json::to_string(&ws_bitmap).unwrap();
            
            // println!("Sending bitmap frame N: {}", count);
            count += 1;

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
        
        // Log the client IP address when connecting
        let client_addr = request.stream.peer_addr().map_or("Unknown".to_string(), |addr| addr.to_string());
        println!("New WebSocket client connecting from: {}", client_addr);
        
        thread::spawn(move || {
            if let Ok(client) = request.accept() {
                println!("WebSocket client connected from: {}", client_addr);
                let (receiver, sender) = client.split().unwrap();
                
                // Add sender to clients list
                {
                    let mut clients = ws_clients.lock().unwrap();
                    clients.push(sender);
                    println!("Total connected clients: {}", clients.len());
                }
                
                // Handle incoming messages
                handle_websocket(receiver, rdp_client, client_addr);
            } else {
                println!("WebSocket client failed to connect from: {}", client_addr);
            }
        });
    }

    // Wait for threads to complete
    bitmap_handler.join().unwrap();
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
                if let Ok(event) = serde_json::from_str::<WsInputEvent>(&text) {
                    let mut client = rdp_client.lock().unwrap();
                    match event {
                        WsInputEvent::Mouse { x, y, button, down } => {
                            // Use TryFrom instead of From
                            let pointer_button = PointerButton::try_from(button).unwrap_or(PointerButton::None);
                            if let Err(e) = client.write(RdpEvent::Pointer(PointerEvent {
                                x: x as u16,
                                y: y as u16,
                                button: pointer_button,
                                down
                            })) {
                                println!("Error sending mouse event from {}: {:?}", client_addr, e);
                            }
                        }
                        WsInputEvent::Keyboard { code, down } => {
                            if let Err(e) = client.write(RdpEvent::Key(KeyboardEvent {
                                code,
                                down
                            })) {
                                println!("Error sending keyboard event from {}: {:?}", client_addr, e);
                            }
                        }
                    }
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

// Create a WebSocket message struct that matches RdpBitmapEvent field names
#[derive(Serialize)]
struct WsBitmapEvent {
    bpp: u16,
    width: u16,
    height: u16,
    dest_left: u16,
    dest_top: u16,
    dest_right: u16,
    dest_bottom: u16,
    data: Vec<u8>,
    is_compress: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsInputEvent {
    Mouse {
        x: i32,
        y: i32,
        #[serde(deserialize_with = "pointer_button_serde::deserialize")]
        button: u8,
        down: bool,
    },
    Keyboard {
        code: u16,
        down: bool,
    },
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
