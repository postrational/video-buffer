use std::collections::HashMap;

/// Stores frames keyed by their sequence number and yields them in order.
pub struct FrameQueue {
    next_frame: u64,
    frames: HashMap<u64, Vec<u8>>,
    max_len: usize,
}

impl FrameQueue {
    pub fn new(max_len: usize) -> Self {
        assert!(max_len > 0, "max_len must be greater than 0");

        Self {
            next_frame: 0,
            frames: HashMap::new(),
            max_len,
        }
    }

    pub fn next_frame_number(&self) -> u64 {
        self.next_frame
    }

    pub fn push(&mut self, frame_no: u64, frame: Vec<u8>) -> bool {
        if frame_no < self.next_frame {
            return false;
        }

        if self.frames.len() >= self.max_len && !self.frames.contains_key(&frame_no) {
            return false;
        }

        self.frames.insert(frame_no, frame);
        true
    }

    pub fn pop_ready(&mut self) -> Option<Vec<u8>> {
        if let Some(frame) = self.frames.remove(&self.next_frame) {
            self.next_frame += 1;
            Some(frame)
        } else {
            None
        }
    }
}
