use std::net::{IpAddr, Ipv4Addr};

use anyhow::Context;
use clap::Parser;
use syncrs::path::FilePath;
use syncrs::protocol::{run_client, run_server};

/// syncrs, a local and remote file-copying tool
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Source file or directory
    #[arg(required_unless_present = "server")]
    source: Option<String>,

    /// Destination file or directory
    #[arg(required_unless_present = "server")]
    destination: Option<String>,

    /// Start in server mode.
    /// The process will communicate with the client through the custom protocol
    #[arg(long, required_unless_present = "source")]
    server: Option<String>,

    /// Increase verbosity
    #[arg(long)]
    verbose: bool,

    /// Increase verbosity to also see debug logs
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut env_builder = env_logger::Builder::from_default_env();
    if cli.debug {
        env_builder.filter_level(log::LevelFilter::Debug);
    } else if cli.verbose {
        env_builder.filter_level(log::LevelFilter::Info);
    }
    env_builder.init();

    let mut ip_addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // this only works when server is the empty string, not when it is not present
    if let Some(addr) = cli.server {
        ip_addr = addr
            .parse::<IpAddr>()
            .context("The IP address you provided is not valid. Check the syntax again.")?;

        run_server(ip_addr).await?;
    } else {
        // SAFETY: `unwrap` is fine because it is a required parameter in this case
        let src = cli.source.unwrap();
        let src: FilePath = src.as_str().into();
        let dest = cli.destination.unwrap();
        let dest: FilePath = dest.as_str().into();

        match (src, dest) {
            (FilePath::Local(src), FilePath::Local(dest)) => {
                // run both the server and client on the same machine
                let server = tokio::spawn(async move { run_server(ip_addr).await });

                let src = src.to_path_buf();
                let dest = dest.to_path_buf();

                let client = tokio::spawn(async move { run_client(ip_addr, &src, &dest).await });

                let (server_res, client_res) = tokio::try_join!(server, client)?;

                if let Err(err) = server_res {
                    log::error!("Server error. {err}");
                }
                if let Err(err) = client_res {
                    log::error!("Client error. {err}");
                }
            }

            (
                FilePath::Local(src),
                FilePath::Remote {
                    user: _,
                    host,
                    path: remote_path,
                },
            ) => {
                let ip_addr = host.parse::<IpAddr>().context(
                    "The host address you provided is not valid. Check the syntax again.",
                )?;

                run_client(ip_addr, src, remote_path).await?;
            }

            (FilePath::Remote { .. }, FilePath::Local(_)) => {
                anyhow::bail!("Remote source as a sender hasn't been implemented yet.")
            }
            (FilePath::Remote { .. }, FilePath::Remote { .. }) => {
                anyhow::bail!("Sender and receiver cannot be both remote paths at the same time.")
            }
        }
    }
    Ok(())
}
