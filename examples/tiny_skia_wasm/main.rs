use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use video_buffer::backends::WasmCanvasBackend;
use video_buffer::{DisplayPresenter, FrameQueue, PixelFormat};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Worker};

const NUM_WORKERS: usize = 10;
const MAX_QUEUED_FRAMES: usize = 60; // Maximum pre-rendered frames to keep
const MAX_FPS: f64 = 90.0; // Maximum render framerate

struct WasmApp {
    // Presentation layer (handles timing, conversion, and canvas blitting)
    presenter: DisplayPresenter<WasmCanvasBackend>,

    // Frame queue from workers (pre-rendered frames waiting to be displayed)
    frame_queue: FrameQueue,

    workers: Vec<Worker>,
    workers_ready: usize,
    next_render_frame: u64, // Next frame number to request from workers

    // FPS tracking
    frame_times: VecDeque<f64>, // Timestamps of actual presented frames
    fps: f64,                   // Measured display FPS

    // Canvas dimensions (needed for worker init)
    width: u32,
    height: u32,
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

        // Create backend and presenter
        let backend = WasmCanvasBackend::new(ctx);
        let presenter = DisplayPresenter::new(backend, width, height, PixelFormat::Rgba8)?
            .with_max_fps(MAX_FPS);

        // Create multiple workers
        let worker_options = web_sys::WorkerOptions::new();
        worker_options.set_type(web_sys::WorkerType::Module);

        let mut workers = Vec::new();
        for _ in 0..NUM_WORKERS {
            let worker = Worker::new_with_options("./worker.js", &worker_options)?;
            workers.push(worker);
        }

        Ok(Self {
            presenter,
            frame_queue: FrameQueue::new(MAX_QUEUED_FRAMES),
            workers,
            workers_ready: 0,
            next_render_frame: 0,
            frame_times: VecDeque::new(),
            fps: 0.0,
            width,
            height,
        })
    }

    fn init_workers(
        app: Rc<RefCell<Self>>,
        on_workers_ready: Box<dyn Fn()>,
    ) -> Result<(), JsValue> {
        let on_workers_ready = Rc::new(on_workers_ready);

        for worker_id in 0..NUM_WORKERS {
            let worker = app.borrow().workers[worker_id].clone();
            let on_workers_ready_handle = Rc::clone(&on_workers_ready);
            let app_clone = Rc::clone(&app);

            let process_worker_message =
                Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
                    let data = event.data();

                    if let Some(msg) = data.as_string() {
                        if msg == "loaded" {
                            // Send init with worker_id, num_workers, and canvas dimensions
                            let app_ref = app_clone.borrow();
                            let init_obj = build_js_object(&[
                                ("cmd", JsValue::from_str("init")),
                                ("worker_id", JsValue::from_f64(worker_id as f64)),
                                ("num_workers", JsValue::from_f64(NUM_WORKERS as f64)),
                                ("width", JsValue::from_f64(app_ref.width as f64)),
                                ("height", JsValue::from_f64(app_ref.height as f64)),
                            ]);
                            drop(app_ref);
                            worker.post_message(&init_obj).unwrap();
                        } else if msg == "ready" {
                            let mut app_mut = app_clone.borrow_mut();
                            app_mut.workers_ready += 1;
                            if app_mut.workers_ready == NUM_WORKERS {
                                drop(app_mut);
                                on_workers_ready_handle();
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

            let log_worker_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
                web_sys::console::error_1(&format!("Worker {} failed to load", worker_id).into());
                web_sys::console::log_1(&event);
            }) as Box<dyn FnMut(_)>);

            app.borrow().workers[worker_id]
                .set_onmessage(Some(process_worker_message.as_ref().unchecked_ref()));
            app.borrow().workers[worker_id]
                .set_onerror(Some(log_worker_error.as_ref().unchecked_ref()));
            process_worker_message.forget();
            log_worker_error.forget();
        }

        Ok(())
    }

    fn on_frame_ready(&mut self, frame_no: u64, buffer: Vec<u8>) {
        // Add frame to queue
        self.frame_queue.push(frame_no, buffer);

        // Request more frames if needed
        self.request_frames();
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
            let request_obj = build_js_object(&[
                ("cmd", JsValue::from_str("render")),
                ("frame_no", JsValue::from_f64(frame_no as f64)),
                ("width", JsValue::from_f64(self.width as f64)),
                ("height", JsValue::from_f64(self.height as f64)),
                ("fps", JsValue::from_f64(self.fps)),
            ]);

            if let Err(e) = self.workers[worker_id].post_message(&request_obj) {
                web_sys::console::error_1(&format!("Failed to post message: {:?}", e).into());
                break;
            }

            self.next_render_frame += 1;
        }
    }

    fn present_frame(&mut self) -> Result<bool, JsValue> {
        let now = js_sys::Date::now();

        // Try to get the next frame from the queue
        if let Some(buffer) = self.frame_queue.pop_ready() {
            let presented = self.presenter.present_frame(&buffer, now)?;

            if presented {
                self.update_fps(now);
            }

            // Request more frames to keep queue filled
            self.request_frames();

            Ok(presented)
        } else {
            Ok(false) // No frame available yet
        }
    }

    fn update_fps(&mut self, now: f64) {
        self.frame_times.push_back(now);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }
        if self.frame_times.len() >= 2 {
            let time_span = self.frame_times.back().unwrap() - self.frame_times.front().unwrap();
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
    // Enables the closure to schedule itself recursively
    let present_and_reschedule: Rc<RefCell<Option<Closure<dyn FnMut()>>>> =
        Rc::new(RefCell::new(None));
    let present_and_reschedule_handle = Rc::clone(&present_and_reschedule);
    let app_handle = Rc::clone(&app);

    let present_frame_loop = Closure::wrap(Box::new(move || {
        let _ = app_handle.borrow_mut().present_frame();

        if let Some(scheduler) = present_and_reschedule_handle.borrow().as_ref() {
            let _ = web_sys::window()
                .unwrap()
                .request_animation_frame(scheduler.as_ref().unchecked_ref());
        }
    }) as Box<dyn FnMut()>);

    // Schedule first frame
    web_sys::window()
        .unwrap()
        .request_animation_frame(present_frame_loop.as_ref().unchecked_ref())
        .unwrap();

    present_and_reschedule
        .borrow_mut()
        .replace(present_frame_loop);
}

/// Helper function to build JS objects from key-value pairs
fn build_js_object(fields: &[(&str, JsValue)]) -> js_sys::Object {
    let obj = js_sys::Object::new();
    for (key, value) in fields {
        js_sys::Reflect::set(&obj, &JsValue::from_str(key), value).unwrap();
    }
    obj
}
