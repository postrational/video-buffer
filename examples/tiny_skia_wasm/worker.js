// Worker wrapper that loads the WASM module
import init from './dist/tiny_skia_wasm_worker.js';

// Initialize the WASM module when worker starts
// The worker_main function is marked with #[wasm_bindgen(start)]
// so it runs automatically after init()
await init();
