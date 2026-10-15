//! Destinations d'impression : périphérique USB, TCP (imprimante réseau, émulateur) ou fichier.

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use escpos::driver::Driver;
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
        self.write(&render(doc, cut)?)
    }

    /// Envoie des octets ESC/POS déjà prêts (ticket mis en file d'attente, par exemple).
    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        match self {
            Target::Tcp(addr) => {
                let mut stream = connect(addr, Duration::from_secs(5))?;
                stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
                stream
                    .write_all(bytes)
                    .with_context(|| tr!("envoi vers {addr} interrompu", "sending to {addr} interrupted"))?;
                stream.flush()?;
                Ok(())
            }
            Target::Dump(path) => std::fs::write(path, bytes)
                .with_context(|| tr!("impossible de créer {}", "cannot create {}", path.display())),
            Target::Device(path) => write_device(path, bytes),
        }
    }

    /// L'imprimante est-elle là ? USB : le périphérique existe (udev le crée quand l'imprimante est
    /// branchée et allumée) ; réseau : elle accepte une connexion ; fichier : toujours.
    pub fn available(&self) -> bool {
        match self {
            Target::Device(path) => path.exists(),
            Target::Tcp(addr) => connect(addr, Duration::from_millis(1500)).is_ok(),
            Target::Dump(_) => true,
        }
    }
}

/// Au-delà, une écriture est considérée comme bloquée (imprimante allumée mais en défaut).
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

fn connect(addr: &str, timeout: Duration) -> Result<TcpStream> {
    let target = addr
        .to_socket_addrs()
        .with_context(|| {
            tr!("adresse invalide : {addr} (hôte:port attendu)", "invalid address: {addr} (host:port expected)")
        })?
        .next()
        .with_context(|| tr!("adresse introuvable : {addr}", "address not found: {addr}"))?;
    TcpStream::connect_timeout(&target, timeout)
        .with_context(|| tr!("impossible de se connecter à {addr}", "cannot connect to {addr}"))
}

/// Écrit sur le périphérique USB dans un fil à part : si l'imprimante bloque l'écriture (plus de
/// papier, capot ouvert), on abandonne au bout de `WRITE_TIMEOUT` au lieu de rester coincé.
fn write_device(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).open(path).with_context(|| {
        tr!(
            "impossible d'ouvrir {} (imprimante branchée ? droits udev ?)",
            "cannot open {} (printer plugged in? udev permissions?)",
            path.display()
        )
    })?;
    let bytes = bytes.to_vec();
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(file.write_all(&bytes).and_then(|()| file.flush()));
    });
    match wait.recv_timeout(WRITE_TIMEOUT) {
        Ok(result) => result.with_context(|| tr!("écriture sur {} impossible", "cannot write to {}", path.display())),
        Err(_) => anyhow::bail!(
            "{}",
            tr!(
                "l'imprimante ne répond plus ({}) : papier, capot ?",
                "the printer is not responding ({}): paper, cover?",
                path.display()
            )
        ),
    }
}

/// Octets ESC/POS d'un ticket, fabriqués en mémoire.
pub fn render(doc: &Doc, cut: bool) -> Result<Vec<u8>> {
    let buffer = Rc::new(RefCell::new(Vec::new()));
    let driver = MemoryDriver(Rc::clone(&buffer));
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
    drop(printer);
    Ok(Rc::try_unwrap(buffer).map(RefCell::into_inner).unwrap_or_default())
}

/// Pilote escpos qui garde les octets en mémoire au lieu de les envoyer.
struct MemoryDriver(Rc<RefCell<Vec<u8>>>);

impl Driver for MemoryDriver {
    fn name(&self) -> String {
        "memory".to_owned()
    }

    fn write(&self, data: &[u8]) -> escpos::errors::Result<()> {
        self.0.borrow_mut().extend_from_slice(data);
        Ok(())
    }

    fn read(&self, _buf: &mut [u8]) -> escpos::errors::Result<usize> {
        Ok(0)
    }

    fn flush(&self) -> escpos::errors::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Style;

    #[test]
    fn rendered_bytes_start_with_init_and_end_with_cut() {
        let mut doc = Doc::new();
        doc.text("Bonjour", Style::default());
        let bytes = render(&doc, true).unwrap();
        assert!(bytes.starts_with(&[0x1b, 0x40]), "ESC @ en tête");
        assert!(bytes.windows(2).any(|w| w == [0x1d, 0x56]), "coupe (GS V)");
        let path = std::env::temp_dir().join(format!("printr-test-{}.bin", std::process::id()));
        Target::Dump(path.clone()).write(&bytes).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_device_is_not_available() {
        assert!(!Target::Device("/nonexistent/printr-test".into()).available());
        assert!(Target::Dump("/tmp/x".into()).available());
    }
}
