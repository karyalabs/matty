//! The idle time on Linux. Under Wayland, X11 only sees input to X apps,
//! so the desktop's own idle service answers there: GNOME's Mutter idle
//! monitor, or the freedesktop screen saver KDE Plasma serves. Under X11,
//! the MIT-SCREEN-SAVER extension knows.

use std::sync::OnceLock;
use std::time::Duration;

pub(super) async fn idle() -> Option<Duration> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return desktop_service().await;
    }
    match x11() {
        Some(idle) => Some(idle),
        None => desktop_service().await,
    }
}

fn x11() -> Option<Duration> {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::screensaver::ConnectionExt as _;

    // One connection for the life of the app; an idle check every 30
    // seconds needn't dial the X server each time.
    static CONNECTION: OnceLock<Option<(x11rb::rust_connection::RustConnection, u32)>> =
        OnceLock::new();
    let (connection, root) = CONNECTION
        .get_or_init(|| {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            Some((connection, root))
        })
        .as_ref()?;
    let info = connection
        .screensaver_query_info(*root)
        .ok()?
        .reply()
        .ok()?;
    Some(Duration::from_millis(u64::from(info.ms_since_user_input)))
}

async fn desktop_service() -> Option<Duration> {
    let connection = session_bus().await?;
    let gnome = connection
        .call_method(
            Some("org.gnome.Mutter.IdleMonitor"),
            "/org/gnome/Mutter/IdleMonitor/Core",
            Some("org.gnome.Mutter.IdleMonitor"),
            "GetIdletime",
            &(),
        )
        .await
        .ok()
        .and_then(|reply| reply.body().deserialize::<u64>().ok());
    if let Some(milliseconds) = gnome {
        return Some(Duration::from_millis(milliseconds));
    }
    connection
        .call_method(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            Some("org.freedesktop.ScreenSaver"),
            "GetSessionIdleTime",
            &(),
        )
        .await
        .ok()
        .and_then(|reply| reply.body().deserialize::<u32>().ok())
        .map(|milliseconds| Duration::from_millis(u64::from(milliseconds)))
}

async fn session_bus() -> Option<zbus::Connection> {
    static BUS: OnceLock<zbus::Connection> = OnceLock::new();
    if let Some(bus) = BUS.get() {
        return Some(bus.clone());
    }
    let bus = zbus::Connection::session().await.ok()?;
    Some(BUS.get_or_init(|| bus).clone())
}
