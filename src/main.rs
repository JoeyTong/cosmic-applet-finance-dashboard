mod analog;
mod app;
mod flags;
mod panel_time;

use cosmic_applet_finance_dashboard::{config, market, network};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,cosmic_applet_finance_dashboard=info".into()),
        )
        .init();
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.iter().any(|a| a == "--help") {
        println!(
            "Finance Dashboard COSMIC applet\n\n  (no arguments)  Run as a COSMIC panel applet\n  --preview       Open the native dashboard in a regular window\n  --diagnose      Check public time, market and IP services without Google sign-in\n  --help          Show this help"
        );
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--diagnose") {
        return tokio::runtime::Runtime::new()?.block_on(async {
            let client = network::client();
            let pairs = config::Settings::load()?.forex;
            let (time, quotes, ip) = tokio::join!(
                network::sync_time(&client),
                market::fetch(&client, &pairs),
                network::public_ip(&client)
            );
            let mut failures = 0;
            match time {
                Ok(time) => println!(
                    "Time: {} \u{b7} OS offset {:+} ms",
                    time.source, time.offset_ms
                ),
                Err(e) => {
                    failures += 1;
                    eprintln!("Time: {e:#}");
                }
            }
            match quotes {
                Ok(snapshot) => {
                    for (symbol, quote) in snapshot.quotes {
                        println!(
                            "{symbol}: {:.*} \u{b7} daily {:+.2}% \u{b7} {}",
                            quote.decimals,
                            quote.price,
                            quote.percent.unwrap_or_default(),
                            quote.latency()
                        );
                    }
                    if !snapshot.unavailable.is_empty() {
                        failures += 1;
                        eprintln!("Unavailable: {}", snapshot.unavailable.join(", "));
                    }
                }
                Err(e) => {
                    failures += 1;
                    eprintln!("Markets: {e:#}");
                }
            }
            match ip {
                Ok(ip) => println!(
                    "Public IP: {} \u{b7} {}, {} ({})",
                    ip.ip, ip.city, ip.country, ip.source
                ),
                Err(e) => {
                    failures += 1;
                    eprintln!("Public IP: {e:#}");
                }
            }
            anyhow::ensure!(failures == 0, "{failures} public service checks failed");
            Ok(())
        });
    }
    if arguments.iter().any(|a| a != "--preview") {
        anyhow::bail!("Unknown argument; use --help");
    }
    let settings = config::Settings::load()?;
    let preview = arguments.iter().any(|a| a == "--preview");
    if preview {
        cosmic::app::run::<app::FinanceDashboard>(
            cosmic::app::Settings::default()
                .size(cosmic::iced::Size::new(app::WIDTH, app::HEIGHT))
                .transparent(false),
            (settings, true),
        )?;
    } else {
        cosmic::applet::run::<app::FinanceDashboard>((settings, false))?;
    }
    Ok(())
}
