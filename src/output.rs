//! Destinations d'impression : périphérique USB, TCP (imprimante réseau, émulateur) ou fichier.

use std::fs::OpenOptions;
use std::path::PathBuf;

use anyhow::{Context, Result};
use escpos::driver::{Driver, FileDriver, NetworkDriver};
use escpos::printer::Printer;
use escpos::printer_options::PrinterOptions;
use escpos::utils::{PageCode, Protocol};

use crate::doc::Doc;
use crate::i18n::t;

pub enum Target {
    Device(PathBuf),
    Tcp(String),
    Dump(PathBuf),
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Device(path) => write!(f, "{}", path.display()),
            Target::Tcp(addr) => write!(f, "tcp://{addr}"),
            Target::Dump(path) => write!(f, "{} ({})", path.display(), t("fichier", "file")),
        }
    }
}

impl Target {
    /// Ouvre la destination, envoie le document puis la referme.
    pub fn send(&self, doc: &Doc, cut: bool) -> Result<()> {
        match self {
            Target::Tcp(addr) => {
                let (host, port) =
                    addr.rsplit_once(':').context(t("--tcp attend hôte:port", "--tcp expects host:port"))?;
                let port = port.parse().context(t("port TCP invalide", "invalid TCP port"))?;
                let driver = NetworkDriver::open(host, port, None)
                    .with_context(|| tr!("impossible de se connecter à {addr}", "cannot connect to {addr}"))?;
                send(driver, doc, cut)
            }
            Target::Dump(path) => {
                let driver =
                    FileDriver::open_with_options(path, OpenOptions::new().create(true).write(true).truncate(true))
                        .with_context(|| tr!("impossible de créer {}", "cannot create {}", path.display()))?;
                send(driver, doc, cut)
            }
            Target::Device(path) => {
                let driver =
                    FileDriver::open_with_options(path, OpenOptions::new().write(true)).with_context(|| {
                        tr!(
                            "impossible d'ouvrir {} (imprimante branchée ? droits udev ?)",
                            "cannot open {} (printer plugged in? udev permissions?)",
                            path.display()
                        )
                    })?;
                send(driver, doc, cut)
            }
        }
    }
}

fn send<D: Driver>(driver: D, doc: &Doc, cut: bool) -> Result<()> {
    // PC858 = Europe de l'Ouest avec le symbole € ; le texte est encodé par `cp858`.
    let options = PrinterOptions::new(Some(PageCode::PC858), None, 42);
    let mut printer = Printer::new(driver, Protocol::default(), Some(options));
    printer.init()?.page_code(PageCode::PC858)?;
    doc.render(&mut printer)?;
    if cut {
        printer.feeds(3)?.print_cut()?;
    } else {
        printer.print()?;
    }
    Ok(())
}
