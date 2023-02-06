use std::{time::Duration, path::{PathBuf, Path}, str::FromStr, sync::Arc, io, thread, process::exit, fmt::Display};

use clap::Parser;
use configfs::{BasicConfigHook, async_trait, Result, Mount, Configuration, FS};
use tokio::{sync::{RwLock, oneshot, Mutex}, fs::{File, OpenOptions}, io::AsyncWriteExt};
use virt_hid::HID;

#[derive(clap::Parser)]
struct Args {
    /// Mount point
    mount: String,
    /// Keyboard Interface Path (Default: /dev/hidg1)
    keyboard: Option<String>,
    /// Mouse Interface Path (Default: /dev/hidg0)
    mouse: Option<String>,
}

pub struct LEDState {
    packet: Arc<Mutex<u8>>,
}

impl LEDState {
    pub async fn new(mouse: String, keyboard: String, led: String) -> io::Result<Configuration> {
        let (new_tx, new_rx) = oneshot::channel();
        let packet = Arc::new(Mutex::new(0));

        {
            let packet = packet.clone();
            tokio::task::spawn_blocking(move || {
                let mut hid = match HID::new(&mouse, &keyboard, &led) {
                    Ok(hid) => {new_tx.send(Ok(())).ok(); hid},
                    Err(e) => {new_tx.send(Err(e)).ok(); return;},
                };

                loop {
                    let Ok(new_packet) = hid.receive_states_packet(Duration::from_millis(10)) else {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    };

                    let Some(new_packet) = new_packet else {
                        continue;
                    };

                    *packet.blocking_lock() = new_packet;
                }
            });
        }

        match new_rx.await {
            Ok(res) => res.map(|_| Configuration::Basic(Arc::new(RwLock::new(LEDState{ packet })))),
            Err(_) => Err(io::Error::from_raw_os_error(libc::ENOSYS)),
        }
    }
}

#[async_trait]
impl BasicConfigHook for LEDState {
    async fn fetch(&mut self) -> Result<Vec<u8>> {
        Ok(vec![*self.packet.lock().await])
    }

    async fn size(&mut self) -> Result<u64> {
        Ok(1)
    }

    async fn update(&mut self, _data: Vec<u8>) -> Result<()> {
        Err(libc::ENOSYS.into())
    }
     
    async fn tick(&mut self) {
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_secs(0)
    }
}

struct Writable {
    file: Arc<Mutex<File>>,
}

impl Writable {
    pub async fn new(path: PathBuf) -> io::Result<Configuration> {
        Ok(Configuration::Basic(Arc::new(RwLock::new(Writable { file: Arc::new(Mutex::new(OpenOptions::new().write(true).open(path).await?)) }))))
    }
}

#[async_trait]
impl BasicConfigHook for Writable {
    async fn fetch(&mut self) -> Result<Vec<u8>> {
        Ok(vec![])
    }

    async fn size(&mut self) -> Result<u64> {
        Ok(0)
    }

    async fn update(&mut self, data: Vec<u8>) -> Result<()> {
        self.file.lock().await
            .write_all(&data).await
            .map_err(|e| e.raw_os_error().unwrap_or(libc::ENOSYS).into())
    }
    
    async fn tick(&mut self) {
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_secs(0)
    }
}

pub trait OrExit<T> {
    fn or_exit(self, msg: &str) -> T;
}

impl<T, E> OrExit<T> for std::result::Result<T,E> 
where
    E: Display
{
    fn or_exit(self, msg: &str) -> T {
        match self {
            Ok(res) => res,
            Err(e) => {
                println!("{}, {}", msg, e);
                exit(1);
            },
        }
    }
}


#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mouse_str = args.mouse.unwrap_or("/dev/hidg1".to_string());
    let keyboard_str = args.keyboard.unwrap_or("/dev/hidg0".to_string());

    let mouse_path = PathBuf::from_str(&mouse_str).or_exit("Invalid mouse interface path");
    let keyboard_path = PathBuf::from_str(&keyboard_str).or_exit("Invalid keyboard interface path");

    let mouse = Writable::new(mouse_path).await.or_exit("Unable to open mouse interface");
    let keyboard = Writable::new(keyboard_path.clone()).await.or_exit("Unable to open keyboard interface");
    let Ok(led) = LEDState::new(mouse_str, keyboard_str.clone(), keyboard_str).await
        .map_err(|e| if e.raw_os_error() == Some(libc::ENOSYS) {
            println!("Channel error");
        } else {
            println!("Unable to open LED interface, {}", e);
        }) else {
            return;
        };

    let mount = Mount::new();
    {
        let mut mount = mount.write().await;
        mount.mount("/mouse", mouse).unwrap();
        mount.mount("/keyboard", keyboard).unwrap();
        mount.mount("/led", led).unwrap();
    }

    let fs_thread = FS::mount("Virtual HID Interface", &args.mount, mount)
        .await
        .or_exit("Error creating mount");
    
    let mouse_path = Path::new(&args.mount).join("mouse");
    loop {
        if !mouse_path.exists() {
            println!("Hid was unexpectedly unmounted");
            fs_thread.abort();
            std::process::abort();
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

}
