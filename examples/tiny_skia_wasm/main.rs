use std::cell::RefCell;
use std::rc::Rc;
use video_buffer::{FrameQueue, PixelFormat, TripleBuffer};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, Worker};

const NUM_WORKERS: usize = 4;
const MAX_QUEUED_FRAMES: usize = 8; // Maximum pre-rendered frames to keep
const MAX_FPS: f64 = 66.0; // Maximum display framerate
const MIN_FRAME_TIME_MS: f64 = 1000.0 / MAX_FPS; // Minimum time between frames (~12.5ms)

struct WasmApp {
    ctx: CanvasRenderingContext2d,
    width: u32,
    height: u32,

    // Triple buffer for display (manages front/back/ready buffers)
    triple_buffer: TripleBuffer,

    // Frame queue from workers (pre-rendered frames waiting to be displayed)
    frame_queue: FrameQueue,

    workers: Vec<Worker>,
    workers_ready: usize,
    next_render_frame: u64, // Next frame number to request from workers
    last_present_time: f64, // Timestamp of last frame presentation

    // FPS tracking
    frame_times: Vec<f64>, // Timestamps of actual presented frames
    fps: f64,       // Measured display FPS
}

impl WasmApp {
    fn new(canvas_id: &str, width: u32, height: u32) -> Result<Self, JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()?;

        canvas.set_width(width);
        canvas.set_height(height);

        let ctx = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Create triple buffer (RGBA8 format for canvas)
        let triple_buffer = TripleBuffer::new(width, height, PixelFormat::Rgba8);

        // Create multiple workers
        let worker_options = web_sys::WorkerOptions::new();
        worker_options.set_type(web_sys::WorkerType::Module);

        let mut workers = Vec::new();
        for _ in 0..NUM_WORKERS {
            let worker = Worker::new_with_options("./worker.js", &worker_options)?;
            workers.push(worker);
        }

        Ok(Self {
            ctx,
            width,
            height,
            triple_buffer,
            frame_queue: FrameQueue::new(MAX_QUEUED_FRAMES),
            workers,
            workers_ready: 0,
            next_render_frame: 0,
            last_present_time: 0.0,
            frame_times: Vec::new(),
            fps: 0.0,
        })
    }

    fn init_workers(app: Rc<RefCell<Self>>, callback: Box<dyn Fn()>) -> Result<(), JsValue> {
        let callback = Rc::new(callback);

        for worker_id in 0..NUM_WORKERS {
            let worker = app.borrow().workers[worker_id].clone();
            let callback_clone = Rc::clone(&callback);
            let app_clone = Rc::clone(&app);

            let onmessage = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
                let data = event.data();

                if let Some(msg) = data.as_string() {
                    if msg == "loaded" {
                        // Send init with worker_id, num_workers, and canvas dimensions
                        let app_ref = app_clone.borrow();
                        let init_obj = js_sys::Object::new();
                        js_sys::Reflect::set(
                            &init_obj,
                            &JsValue::from_str("cmd"),
                            &JsValue::from_str("init"),
                        )
                        .unwrap();
                        js_sys::Reflect::set(
                            &init_obj,
                            &JsValue::from_str("worker_id"),
                            &JsValue::from_f64(worker_id as f64),
                        )
                        .unwrap();
                        js_sys::Reflect::set(
                            &init_obj,
                            &JsValue::from_str("num_workers"),
                            &JsValue::from_f64(NUM_WORKERS as f64),
                        )
                        .unwrap();
                        js_sys::Reflect::set(
                            &init_obj,
                            &JsValue::from_str("width"),
                            &JsValue::from_f64(app_ref.width as f64),
                        )
                        .unwrap();
                        js_sys::Reflect::set(
                            &init_obj,
                            &JsValue::from_str("height"),
                            &JsValue::from_f64(app_ref.height as f64),
                        )
                        .unwrap();
                        drop(app_ref);
                        worker.post_message(&init_obj).unwrap();
                    } else if msg == "ready" {
                        let mut app_mut = app_clone.borrow_mut();
                        app_mut.workers_ready += 1;
                        if app_mut.workers_ready == NUM_WORKERS {
                            drop(app_mut);
                            callback_clone();
                        }
                    }
                } else if data.is_object() {
                    // Try to extract frame data (object with frame_no and buffer)
                    if let (Ok(frame_no_val), Ok(buffer_val)) = (
                        js_sys::Reflect::get(&data, &JsValue::from_str("frame_no")),
                        js_sys::Reflect::get(&data, &JsValue::from_str("buffer")),
                    ) {
                        if let (Some(frame_no), Ok(array)) = (
                            frame_no_val.as_f64(),
                            buffer_val.dyn_into::<js_sys::Uint8Array>(),
                        ) {
                            let mut buffer = vec![0u8; array.length() as usize];
                            array.copy_to(&mut buffer);
                            app_clone
                                .borrow_mut()
                                .on_frame_ready(frame_no as u64, buffer);
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);

            let onerror = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
                web_sys::console::error_1(&format!("Worker {} failed to load", worker_id).into());
                web_sys::console::log_1(&event);
            }) as Box<dyn FnMut(_)>);

            app.borrow().workers[worker_id].set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            app.borrow().workers[worker_id].set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onmessage.forget();
            onerror.forget();
        }

        Ok(())
    }

    fn on_frame_ready(&mut self, frame_no: u64, buffer: Vec<u8>) {
        // Add frame to queue
        self.frame_queue.push(frame_no, buffer);

        // Request more frames if needed
        self.request_frames();
    }

    fn prepare_next_frame(&mut self) -> bool {
        // Try to get the next frame in sequence from the queue
        if let Some(buffer) = self.frame_queue.pop_ready() {
            // Copy frame to triple buffer's render buffer
            {
                let mut render_buf = self.triple_buffer.render_buffer();
                render_buf.copy_from_slice(&buffer);
            }

            // Commit the render
            self.triple_buffer.commit_render();

            // Request more frames to keep queue filled
            self.request_frames();

            return true; // Frame is ready
        }

        false // No frame available yet
    }

    fn request_frames(&mut self) {
        // Only request frames if we don't have too many in flight
        // next_render_frame - next_display_frame tells us how many frames are requested/queued total
        while (self.next_render_frame - self.frame_queue.next_frame_number())
            < MAX_QUEUED_FRAMES as u64
        {
            let frame_no = self.next_render_frame;
            let worker_id = (frame_no as usize) % NUM_WORKERS;

            // Send render request to worker
            let request_obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &request_obj,
                &JsValue::from_str("cmd"),
                &JsValue::from_str("render"),
            )
            .unwrap();
            js_sys::Reflect::set(
                &request_obj,
                &JsValue::from_str("frame_no"),
                &JsValue::from_f64(frame_no as f64),
            )
            .unwrap();
            js_sys::Reflect::set(
                &request_obj,
                &JsValue::from_str("width"),
                &JsValue::from_f64(self.width as f64),
            )
            .unwrap();
            js_sys::Reflect::set(
                &request_obj,
                &JsValue::from_str("height"),
                &JsValue::from_f64(self.height as f64),
            )
            .unwrap();
            js_sys::Reflect::set(
                &request_obj,
                &JsValue::from_str("fps"),
                &JsValue::from_f64(self.fps),
            )
            .unwrap();

            if let Err(e) = self.workers[worker_id].post_message(&request_obj) {
                web_sys::console::error_1(&format!("Failed to post message: {:?}", e).into());
                break;
            }

            self.next_render_frame += 1;
        }
    }

    fn present_frame(&mut self) -> Result<bool, JsValue> {
        let now = js_sys::Date::now();
        let elapsed = now - self.last_present_time;

        // Only present if enough time has elapsed
        if elapsed < MIN_FRAME_TIME_MS {
            return Ok(false); // Frame was skipped
        }

        // Prepare the next frame from the queue
        if !self.prepare_next_frame() {
            // No frame available yet, skip
            return Ok(false);
        }

        // Swap ready buffer to present buffer
        self.triple_buffer.commit_present();

        // Get the present buffer and blit to canvas
        let present_buf = self.triple_buffer.present_buffer();

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&present_buf),
            self.width,
            self.height,
        )?;
        drop(present_buf);

        self.ctx.put_image_data(&image_data, 0.0, 0.0)?;

        self.last_present_time = now;

        self.update_fps(now);

        Ok(true) // Frame was presented
    }

    fn update_fps(&mut self, now: f64) {
        self.frame_times.push(now);
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }
        if self.frame_times.len() >= 2 {
            let time_span = self.frame_times.last().unwrap() - self.frame_times.first().unwrap();
            if time_span > 0.0 {
                self.fps = (self.frame_times.len() as f64 - 1.0) / (time_span / 1000.0);
            }
        }
    }
}

#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    // Default 800x600 window for non-fullscreen demo
    start_custom("canvas", 800, 600)
}

#[wasm_bindgen]
pub fn start_custom(canvas_id: &str, width: u32, height: u32) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let app = Rc::new(RefCell::new(WasmApp::new(canvas_id, width, height)?));

    // Initialize workers
    let app_clone = Rc::clone(&app);

    WasmApp::init_workers(
        Rc::clone(&app),
        Box::new(move || {
            // Once workers are ready, start requesting frames
            app_clone.borrow_mut().request_frames();
            start_render_loop(Rc::clone(&app_clone));
        }),
    )?;

    Ok(())
}

fn start_render_loop(app: Rc<RefCell<WasmApp>>) {
    let closure: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let closure_handle = Rc::clone(&closure);
    let app_handle = Rc::clone(&app);

    let callback = Closure::wrap(Box::new(move || {
        let _ = app_handle.borrow_mut().present_frame();

        if let Some(cb) = closure_handle.borrow().as_ref() {
            let _ = web_sys::window()
                .unwrap()
                .request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }) as Box<dyn FnMut()>);

    closure.borrow_mut().replace(callback);

    {
        let callback_ref = closure.borrow();
        if let Some(callback) = callback_ref.as_ref() {
            web_sys::window()
                .unwrap()
                .request_animation_frame(callback.as_ref().unchecked_ref())
                .unwrap();
        }
    }
}
