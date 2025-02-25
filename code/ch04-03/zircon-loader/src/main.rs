#![deny(warnings, unused_must_use)]

extern crate log;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use structopt::StructOpt;
use zircon_loader::*;
use zircon_object::object::*;
use zircon_object::task::Process;

#[derive(Debug, StructOpt)]
#[structopt()]
struct Opt {
    #[structopt(parse(from_os_str))]
    prebuilt_path: PathBuf,
    #[structopt(default_value = "")]
    cmdline: String,
}

#[async_std::main]
async fn main() {
    kernel_hal_unix::init();
    init_logger();
    let opt = Opt::from_args();
    let images = open_images(&opt.prebuilt_path).expect("failed to read file");
    let proc: Arc<dyn KernelObject> = run_userboot(&images, &opt.cmdline);
    drop(images);
    let proc = proc.downcast_arc::<Process>().unwrap();
    proc.wait_for_end().await;
}

fn open_images(path: &Path) -> std::io::Result<Images<Vec<u8>>> {
    Ok(Images {
        userboot: std::fs::read(path.join("userboot-libos.so"))?,
        vdso: std::fs::read(path.join("libzircon-libos.so"))?,
        zbi: std::fs::read(path.join("bringup.zbi"))?,
    })
}

fn init_logger() {
    env_logger::builder()
        .format(|buf, record| {
            use env_logger::fmt::Color;
            use log::Level;
            use std::io::Write;

            let (tid, pid) = kernel_hal::Thread::get_tid();
            let mut style = buf.style();
            match record.level() {
                Level::Trace => style.set_color(Color::Black).set_intense(true),
                Level::Debug => style.set_color(Color::White),
                Level::Info => style.set_color(Color::Green),
                Level::Warn => style.set_color(Color::Yellow),
                Level::Error => style.set_color(Color::Red).set_bold(true),
            };
            let now = kernel_hal_unix::timer_now();
            let level = style.value(record.level());
            let args = record.args();
            writeln!(buf, "[{:?} {:>5} {}:{}] {}", now, level, pid, tid, args)
        })
        .init();
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[async_std::test]
//     async fn userboot() {
//         kernel_hal_unix::init();

//         let opt = Opt {
//             prebuilt_path: PathBuf::from("../prebuilt/zircon/x64"),
//             cmdline: String::from(""),
//         };
//         let images = open_images(&opt.prebuilt_path).expect("failed to read file");

//         let proc: Arc<dyn KernelObject> = run_userboot(&images, &opt.cmdline);
//         drop(images);

//         let proc = proc.downcast_arc::<Process>().unwrap();
//         proc.wait_for_end().await;
//     }
// }
