use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

mod renderer;
mod sprite;

use renderer::TinySkiaRenderer;

thread_local! {
    static RENDERER: RefCell<Option<TinySkiaRenderer>> = RefCell::new(None);
}

/// Helper function to extract f64 values from JS objects with a default fallback
fn get_f64(object: &JsValue, key: &str, default: f64) -> f64 {
    js_sys::Reflect::get(object, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(default)
}

#[wasm_bindgen]
pub fn worker_init(num_workers: usize, canvas_width: u32, canvas_height: u32) {
    console_error_panic_hook::set_once();
    RENDERER.with(|renderer| {
        *renderer.borrow_mut() = Some(TinySkiaRenderer::new(
            num_workers,
            canvas_width,
            canvas_height,
        ));
    });
}

#[wasm_bindgen]
pub fn worker_render(frame_no: u64, width: u32, height: u32, fps: f64) -> js_sys::Uint8Array {
    let buffer_size = (width * height * 4) as usize;
    let mut buffer = vec![0u8; buffer_size];

    RENDERER.with(|renderer| {
        if let Some(ref mut r) = *renderer.borrow_mut() {
            r.render_to_rgba(&mut buffer, width, height, frame_no, fps);
        }
    });

    js_sys::Uint8Array::from(&buffer[..])
}

#[wasm_bindgen(start)]
pub fn worker_main() {
    console_error_panic_hook::set_once();

    let global: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let global_clone = global.clone();

    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        let data = event.data();

        if data.is_object() {
            // Try as command object
            if let Some(cmd) = js_sys::Reflect::get(&data, &JsValue::from_str("cmd"))
                .ok()
                .and_then(|v| v.as_string())
            {
                match cmd.as_str() {
                    "init" => {
                        let num_workers = get_f64(&data, "num_workers", 1.0) as usize;
                        let canvas_width = get_f64(&data, "width", 800.0) as u32;
                        let canvas_height = get_f64(&data, "height", 600.0) as u32;

                        worker_init(num_workers, canvas_width, canvas_height);
                        global_clone
                            .post_message(&JsValue::from_str("ready"))
                            .unwrap();
                    }
                    "render" => {
                        let frame_no = get_f64(&data, "frame_no", 0.0) as u64;
                        let width = get_f64(&data, "width", 800.0) as u32;
                        let height = get_f64(&data, "height", 600.0) as u32;
                        let fps = get_f64(&data, "fps", 0.0);

                        let buffer_array = worker_render(frame_no, width, height, fps);

                        // Send back frame with frame number
                        let response = js_sys::Object::new();
                        js_sys::Reflect::set(
                            &response,
                            &JsValue::from_str("frame_no"),
                            &JsValue::from_f64(frame_no as f64),
                        )
                        .unwrap();
                        js_sys::Reflect::set(
                            &response,
                            &JsValue::from_str("buffer"),
                            &buffer_array,
                        )
                        .unwrap();

                        global_clone.post_message(&response).unwrap();
                    }
                    _ => {}
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    global.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // Signal to main thread that worker is ready
    global.post_message(&JsValue::from_str("loaded")).unwrap();
}
