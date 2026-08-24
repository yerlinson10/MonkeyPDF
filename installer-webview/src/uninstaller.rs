#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
use std::{
    cell::RefCell,
    env, fs,
    path::PathBuf,
    process::Command,
    rc::Rc,
    thread,
};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Icon, WindowBuilder},
};
use wry::WebViewBuilder;

const UI_SHELL: &str = include_str!("../ui/uninstall.html");
const LOGO: &str = include_str!("../ui/logo.svg");
const ICON_PNG: &[u8] = include_bytes!("../../src-tauri/icons/32x32.png");

fn ui_html() -> String {
    UI_SHELL.replace("<!--LOGO-->", LOGO)
}

fn window_icon() -> Option<Icon> {
    let img = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

#[derive(Deserialize)]
struct Msg {
    op: String,
    #[serde(default)]
    data: bool,
}

enum Host {
    Ipc(String),
    Finished(Result<(), String>),
}

fn json_eval(webview: &wry::WebView, v: serde_json::Value) {
    let payload = v.to_string();
    let _ = webview.evaluate_script(&format!("window.__host && window.__host({payload})"));
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn relaunch_from_temp() -> bool {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--ui") {
        return false;
    }
    let exe = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let instdir = match exe.parent() {
        Some(p) => p.to_path_buf(),
        None => return false,
    };
    let tmp = env::temp_dir().join("MonkeyPDF-uninstaller.exe");
    if fs::copy(&exe, &tmp).is_err() {
        return false;
    }
    let _ = Command::new(&tmp)
        .arg("--ui")
        .arg(instdir)
        .spawn();
    true
}

fn spawn_uninstall(instdir: PathBuf, wipe_data: bool, proxy: tao::event_loop::EventLoopProxy<Host>) {
    thread::spawn(move || {
        let result = (|| {
            let engine = instdir.join("unins000.exe");
            if !engine.is_file() {
                return Err(
                    "No encuentro el motor de desinstalación. Vuelve a instalar MonkeyPDF y prueba otra vez.".into(),
                );
            }
            let tmp = env::temp_dir().join("MonkeyPDF-unengine.exe");
            fs::copy(&engine, &tmp).map_err(|e| format!("No pude copiar el motor: {e}"))?;
            let dir_s = instdir
                .to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .to_string();
            let mut cmd = Command::new(&tmp);
            cmd.arg("/S").arg("/P");
            if wipe_data {
                cmd.arg("/DATA");
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(format!("_?={dir_s}"));
            }
            #[cfg(not(windows))]
            {
                cmd.arg(format!("_?={dir_s}"));
            }
            let status = cmd
                .status()
                .map_err(|e| format!("No pude iniciar el motor: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "El motor devolvió {status}. Cierra MonkeyPDF si está abierto e inténtalo de nuevo."
                ));
            }
            Ok(())
        })();
        let _ = proxy.send_event(Host::Finished(result));
    });
}

fn main() {
    if relaunch_from_temp() {
        return;
    }

    let instdir = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(exe_dir);

    let event_loop = EventLoopBuilder::<Host>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("MonkeyPDF Uninstall")
        .with_inner_size(tao::dpi::LogicalSize::new(760.0, 480.0))
        .with_resizable(false)
        .with_window_icon(window_icon())
        .build(&event_loop)
        .expect("window");

    let proxy_ipc = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(ui_html())
        .with_ipc_handler(move |req| {
            let body = req.body().clone();
            let _ = proxy_ipc.send_event(Host::Ipc(body));
        })
        .build(&window)
        .expect("webview");

    let webview = Rc::new(RefCell::new(webview));
    let instdir = Rc::new(instdir);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(Host::Ipc(body)) => {
                let parsed: Msg = match serde_json::from_str(&body) {
                    Ok(m) => m,
                    Err(_) => return,
                };
                match parsed.op.as_str() {
                    "quit" => *control_flow = ControlFlow::Exit,
                    "uninstall" => {
                        json_eval(
                            &webview.borrow(),
                            serde_json::json!({"op":"progress","text":"Quitando MonkeyPDF…"}),
                        );
                        spawn_uninstall(instdir.as_ref().clone(), parsed.data, proxy.clone());
                    }
                    _ => {}
                }
            }
            Event::UserEvent(Host::Finished(Ok(()))) => {
                json_eval(&webview.borrow(), serde_json::json!({"op":"done"}));
            }
            Event::UserEvent(Host::Finished(Err(e))) => {
                json_eval(
                    &webview.borrow(),
                    serde_json::json!({"op":"error","text": e}),
                );
            }
            _ => {}
        }
    });
}
