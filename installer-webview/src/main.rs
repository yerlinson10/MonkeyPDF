#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
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

const UI_SHELL: &str = include_str!("../ui/index.html");
const LOGO: &str = include_str!("../ui/logo.svg");
const ENGINE: &[u8] = include_bytes!("../engine.exe");
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
    path: Option<String>,
}

enum Host {
    Ipc(String),
    Finished(Result<PathBuf, String>),
}

fn default_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("MonkeyPDF")
}

fn json_eval(webview: &wry::WebView, v: serde_json::Value) {
    let payload = v.to_string();
    let _ = webview.evaluate_script(&format!("window.__host && window.__host({payload})"));
}

fn spawn_install(dest: PathBuf, proxy: tao::event_loop::EventLoopProxy<Host>) {
    thread::spawn(move || {
        let result = (|| {
            if ENGINE.is_empty() {
                return Err(
                    "Falta el motor NSIS. Ejecuta npm run wrap:installer después del build.".into(),
                );
            }
            let dest_s = dest
                .to_string_lossy()
                .trim()
                .trim_matches('"')
                .trim_end_matches(['\\', '/'])
                .to_string();
            let dest = PathBuf::from(&dest_s);
            std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

            let tmp = std::env::temp_dir().join("MonkeyPDF-engine.exe");
            std::fs::write(&tmp, ENGINE).map_err(|e| format!("No pude extraer el motor: {e}"))?;

            // NSIS: /D= must be last and unquoted. Do not wrap this in `cmd /C`
            // (that quoting breaks /D= and the installer exits 1).
            let mut cmd = Command::new(&tmp);
            cmd.arg("/S").arg("/UPDATE");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(format!("/D={dest_s}"));
            }
            #[cfg(not(windows))]
            {
                cmd.arg(format!("/D={dest_s}"));
            }
            let status = cmd
                .status()
                .map_err(|e| format!("No pude iniciar el motor: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "El motor de instalación devolvió {status}. Prueba otra carpeta o cierra MonkeyPDF si está abierto."
                ));
            }
            Ok(dest)
        })();
        let _ = proxy.send_event(Host::Finished(result));
    });
}

fn main() {
    let event_loop = EventLoopBuilder::<Host>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("MonkeyPDF Setup")
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
    let dest_hold = Rc::new(RefCell::new(default_dir()));
    let picking = Rc::new(Cell::new(false));
    let window = Rc::new(window);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if !picking.get() {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(Host::Ipc(body)) => {
                let parsed: Msg = match serde_json::from_str(&body) {
                    Ok(m) => m,
                    Err(_) => return,
                };
                match parsed.op.as_str() {
                    "ready" => {
                        let p = default_dir();
                        *dest_hold.borrow_mut() = p.clone();
                        json_eval(
                            &webview.borrow(),
                            serde_json::json!({"op":"defaults","path": p.to_string_lossy()}),
                        );
                    }
                    "browse" => {
                        picking.set(true);
                        let start = dest_hold.borrow().clone();
                        let start = if start.is_dir() {
                            start
                        } else {
                            start
                                .parent()
                                .filter(|p| p.is_dir())
                                .map(|p| p.to_path_buf())
                                .unwrap_or(start)
                        };
                        let mut dlg = rfd::FileDialog::new();
                        if start.is_dir() {
                            dlg = dlg.set_directory(&start);
                        }
                        let folder = dlg.set_parent(window.as_ref()).pick_folder();
                        picking.set(false);
                        if let Some(folder) = folder {
                            *dest_hold.borrow_mut() = folder.clone();
                            json_eval(
                                &webview.borrow(),
                                serde_json::json!({"op":"picked","path": folder.to_string_lossy()}),
                            );
                        }
                    }
                    "quit" => *control_flow = ControlFlow::Exit,
                    "open" => {
                        let exe = dest_hold.borrow().join("MonkeyPDF.exe");
                        let _ = Command::new(exe).spawn();
                        *control_flow = ControlFlow::Exit;
                    }
                    "install" => {
                        let path = parsed.path.map(PathBuf::from).unwrap_or_else(default_dir);
                        *dest_hold.borrow_mut() = path.clone();
                        json_eval(
                            &webview.borrow(),
                            serde_json::json!({"op":"progress","text":"Copiando archivos…"}),
                        );
                        spawn_install(path, proxy.clone());
                    }
                    _ => {}
                }
            }
            Event::UserEvent(Host::Finished(Ok(path))) => {
                *dest_hold.borrow_mut() = path.clone();
                json_eval(
                    &webview.borrow(),
                    serde_json::json!({"op":"done","path": path.to_string_lossy()}),
                );
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
