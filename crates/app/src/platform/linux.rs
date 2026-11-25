pub(super) use super::unix::{say_without_window, zone_rules};

struct Watch;

static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WATCH: Watch = Watch;

impl log::Log for Watch {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.target().starts_with("rfd") && m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        match r.level() {
            log::Level::Error => FAILED.store(true, std::sync::atomic::Ordering::SeqCst),
            _ => FAILED.store(false, std::sync::atomic::Ordering::SeqCst),
        }
    }
    fn flush(&self) {}
}

pub(super) fn ask_path(kind: super::Pick) -> Result<super::Wait, crate::fault::Fault> {
    Ok(Box::new(move || {
        if log::set_logger(&WATCH).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
        FAILED.store(false, std::sync::atomic::Ordering::SeqCst);
        let d = rfd::FileDialog::new();
        let got = match kind {
            super::Pick::Folder => d.pick_folder(),
            super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
        };
        if got.is_none() && FAILED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(crate::fault::Fault::known(crate::fault::Known::DialogUnavailable, String::new()));
        }
        Ok(got.map(|p| p.display().to_string()))
    }))
}

pub(super) fn user_temp_dir() -> Option<std::path::PathBuf> {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(v) => Some(std::path::PathBuf::from(v)),
        None => Some(std::path::PathBuf::from("/tmp")),
    }
}

pub(super) fn dialog_brings_folders() -> bool {
    true
}

pub(super) fn asks_file_or_folder() -> bool {
    true
}

pub(super) fn window_backend() -> super::Backend {
    super::backend_of(wayland::devices_now(), std::env::var_os("DISPLAY").as_deref())
}

pub(super) fn watch_drops(window: super::drops::Window, wake: super::drops::Wake) -> Option<super::drops::Watcher> {
    match window {
        super::drops::Window::Wayland { display, surface } if super::drops::taken_here(window) => wayland::watch(display, surface as usize, wake),
        _ => None,
    }
}

mod wayland {
    use super::super::drops::{binds_beside_clipboard, devices_of_globals, Devices, Drops, Wake, Watcher, URI_LIST};
    use smithay_client_toolkit::data_device_manager::data_device::{DataDeviceData, DataDeviceHandler};
    use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
    use smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler;
    use smithay_client_toolkit::data_device_manager::data_device::DataDevice;
    use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
    use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
    use wayland_client::protocol::wl_callback::{self, WlCallback};
    use wayland_client::protocol::wl_data_device::WlDataDevice;
    use wayland_client::protocol::wl_data_device_manager::DndAction;
    use wayland_client::protocol::wl_data_source::WlDataSource;
    use wayland_client::protocol::wl_registry::{self, WlRegistry};
    use wayland_client::protocol::wl_seat::{self, WlSeat};
    use wayland_client::protocol::wl_surface::WlSurface;
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    struct State {
        shared: Arc<Mutex<Drops>>,
        surface: usize,
        wake: Wake,
        conn: Connection,
        live: Arc<Mutex<bool>>,
        _devices: Vec<DataDevice>,
    }

    const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

    pub(super) fn devices_now() -> Option<Devices> {
        if std::env::var_os("WAYLAND_SOCKET").is_some() || std::env::var_os("WAYLAND_DISPLAY").filter(|v| !v.is_empty()).is_none() {
            return None;
        }
        let conn = Connection::connect_to_env().ok()?;
        let (globals, _queue) = registry_queue_init::<Probe>(&conn).ok()?;
        Some(devices_of(&globals))
    }

    fn devices_of(globals: &GlobalList) -> Devices {
        globals.contents().with_list(|list| devices_of_globals(list.iter().map(|g| g.interface.as_str())))
    }

    struct Probe;

    impl Dispatch<WlRegistry, GlobalListContents> for Probe {
        fn event(_: &mut Self, _: &WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
    }

    pub(super) fn watch(display: *mut std::ffi::c_void, surface: usize, wake: Wake) -> Option<Watcher> {
        // SAFETY: `display` is the window's live `wl_display` (from the window's own display handle); the window stops
        // this piece ([`Watcher::stop`]) before it lets that connection go, and nothing here touches it after.
        let backend = unsafe { wayland_client::backend::Backend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
        if !binds_beside_clipboard(devices_of(&globals)) {
            return None;
        }
        let qh = queue.handle();
        let manager = DataDeviceManagerState::bind(&globals, &qh).ok()?;
        let seats: Vec<WlSeat> = globals.contents().with_list(|list| {
            list.iter().filter(|g| g.interface == WlSeat::interface().name).map(|g| globals.registry().bind::<WlSeat, _, State>(g.name, g.version.min(1), &qh, ())).collect()
        });
        if seats.is_empty() {
            return None;
        }
        let devices = seats.iter().map(|s| manager.get_data_device(&qh, s)).collect();
        let shared = Arc::new(Mutex::new(Drops::default()));
        let live = Arc::new(Mutex::new(true));
        let mut state = State { shared: shared.clone(), surface, wake, conn: conn.clone(), live: live.clone(), _devices: devices };
        let (done, ended) = std::sync::mpsc::channel::<()>();
        let on = live.clone();
        crate::task::start_named("drops", move || {
            let _keep = (manager, seats);
            while on.lock().map(|g| *g).unwrap_or(false) && queue.blocking_dispatch(&mut state).is_ok() {}
            let _ = done.send(());
        })
        .ok()?;
        let ended = Mutex::new(ended);
        let stop = move || {
            match live.lock() {
                Ok(mut on) if *on => {
                    *on = false;
                    let _ = conn.display().sync(&qh, ());
                    let _ = conn.flush();
                }
                _ => return,
            }
            if let Ok(rx) = ended.lock() {
                let _ = rx.recv_timeout(STOP_WAIT);
            }
        };
        Some(Watcher::stopped_by(shared, Arc::new(stop)))
    }

    impl State {
        fn change(&self, f: impl FnOnce(&mut Drops)) {
            if let Ok(mut d) = self.shared.lock() {
                f(&mut d);
            }
            (self.wake)();
        }

        fn offer_of(device: &WlDataDevice) -> Option<DragOffer> {
            device.data::<DataDeviceData>().and_then(|d| d.drag_offer())
        }
    }

    impl DataDeviceHandler for State {
        fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice, _x: f64, _y: f64, surface: &WlSurface) {
            let ours = surface.id().as_ptr() as usize == self.surface;
            let offer = Self::offer_of(device);
            let files = offer.as_ref().is_some_and(|o| o.with_mime_types(|m| m.iter().any(|t| t == URI_LIST)));
            if let Some(o) = offer.as_ref() {
                if ours && files {
                    o.accept_mime_type(o.serial, Some(URI_LIST.to_string()));
                    o.set_actions(DndAction::Copy, DndAction::Copy);
                } else {
                    o.accept_mime_type(o.serial, None);
                }
            }
            self.change(|d| d.entered(ours, files));
        }

        fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
            self.change(|d| d.left());
        }

        fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _x: f64, _y: f64) {}

        fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

        fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
            let Some(offer) = Self::offer_of(device) else {
                self.change(|d| d.left());
                return;
            };
            let ours = offer.surface.id().as_ptr() as usize == self.surface && offer.with_mime_types(|m| m.iter().any(|t| t == URI_LIST));
            if !ours {
                offer.destroy();
                self.change(|d| d.left());
                return;
            }
            let pipe = offer.receive(URI_LIST.to_string());
            let _ = self.conn.flush();
            let (shared, wake, conn, live) = (self.shared.clone(), self.wake.clone(), self.conn.clone(), self.live.clone());
            let read = move || {
                let mut list = Vec::new();
                if let Ok(mut p) = pipe {
                    let _ = p.read_to_end(&mut list);
                }
                match live.lock() {
                    Ok(on) if *on => {
                        offer.finish();
                        offer.destroy();
                        let _ = conn.flush();
                    }
                    _ => return,
                }
                if let Ok(mut d) = shared.lock() {
                    d.delivered(&list);
                }
                wake();
            };
            if crate::task::start_named("drop-read", read).is_err() {
                self.change(|d| d.left());
            }
        }
    }

    impl DataOfferHandler for State {
        fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, offer: &mut DragOffer, _: DndAction) {
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        }

        fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    }

    impl DataSourceHandler for State {
        fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
        fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: String, _: WritePipe) {}
        fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
        fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
        fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
        fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
    }

    impl Dispatch<WlRegistry, GlobalListContents> for State {
        fn event(_: &mut Self, _: &WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
    }

    impl Dispatch<WlSeat, ()> for State {
        fn event(_: &mut Self, _: &WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }

    impl Dispatch<WlCallback, ()> for State {
        fn event(_: &mut Self, _: &WlCallback, _: wl_callback::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }

    smithay_client_toolkit::delegate_data_device!(State);
}

pub(super) fn menu_bar(_words: &super::menubar::Words) {}

pub(super) fn menu_takes_key(_chars: &str) -> bool {
    false
}

pub(super) fn menu_bar_read() -> Vec<(String, Vec<(String, String, bool)>)> {
    Vec::new()
}
