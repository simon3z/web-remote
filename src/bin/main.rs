//! `web-remote` binary.
//!
//! Wires the CLI (clap), the chosen sink, privilege drop (evdev only),
//! the QR to the terminal, and the server.

use std::{net::SocketAddr, sync::Arc};

use clap::Parser;
use web_remote::{auth, host, server, sink::Sink};

#[cfg(feature = "evdev-sink")]
use web_remote::sink_evdev::EvdevSink;
use web_remote::sink_null::NullSink;
#[cfg(feature = "wayland-sink")]
use web_remote::sink_wayland::WaylandSink;

#[cfg(feature = "evdev-sink")]
use web_remote::privdrop::DropUser;

#[derive(Parser)]
#[command(
    name = "web-remote",
    about = "Serve a token-guarded web UI that sends multimedia keys to this host."
)]
struct Args {
    /// Input backend: `wayland` (no sudo, default) or `evdev` (sudo, any compositor).
    #[arg(long, default_value = "wayland")]
    input: String,

    /// Bind address (default `0.0.0.0`).
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,

    /// Port (default `40443`).
    #[arg(long, default_value_t = 40443)]
    port: u16,

    /// Serve over plain HTTP (debugging). Default is HTTPS (self-signed).
    #[arg(long)]
    http: bool,

    /// Host IP to print in the QR (default: auto-detected).
    #[arg(long)]
    host: Option<String>,

    /// evdev sink: who to drop to after opening uinput (name or uid).
    #[arg(long)]
    user: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // enigo logs through `log` (via the `tracing-log` bridge, which is in
    // the dep tree via tracing-subscriber). The WARN it emits when the
    // Wayland virtual-keyboard protocol is unavailable is expected on some
    // GNOME sessions — the x11rb fallback handles that case, so we silence
    // enigo's platform module by default. `RUST_LOG=enigo=debug` overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,enigo=error")),
        )
        .init();

    let args = Args::parse();
    let token = auth::generate_token();
    let scheme = if args.http { "http" } else { "https" };
    let host_ip = args
        .host
        .clone()
        .unwrap_or_else(|| host::detect_host_ip().to_string());
    let bind: SocketAddr = host::bind_addr(&args.bind, args.port);

    // Build the sink. For evdev this happens while root (under sudo); the
    // privilege drop happens right after, keyed on the chosen name.
    let sink: Box<dyn Sink> = match args.input.as_str() {
        #[cfg(feature = "wayland-sink")]
        "wayland" => Box::new(WaylandSink::new()?) as Box<dyn Sink>,
        "null" => Box::new(NullSink::new()?) as Box<dyn Sink>,
        #[cfg(feature = "evdev-sink")]
        "evdev" => Box::new(EvdevSink::new()?) as Box<dyn Sink>,
        other => {
            eprintln!("unknown --input {other:?}; use `wayland` or `evdev`");
            std::process::exit(2);
        }
    };

    // Evdev: drop privileges now that the uinput device is registered (R7,
    // R8), before any socket is bound.
    #[cfg(feature = "evdev-sink")]
    if args.input == "evdev" {
        let drop_user = DropUser::resolve(args.user.as_ref());
        let (uid, gid) = drop_user.ids()?;
        web_remote::privdrop::drop_privileges(uid, gid)?;
    }

    let sink = Arc::new(tokio::sync::Mutex::new(sink));

    let url = auth::secret_url(scheme, &host_ip, args.port, &token);
    println!("web-remote: {url}");
    println!("(scan the QR, or open the URL above; this token is valid until the process exits)");
    println!("{}", web_remote::qrgen::qr_to_ansi_text(&url));
    println!();

    let state = server::AppState { token, sink };
    server::serve(Arc::new(state), bind, scheme)
        .await
        .map_err(|e| anyhow::anyhow!("serve: {e}"))?;
    Ok(())
}
