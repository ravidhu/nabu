pub mod mic;
pub mod system;

#[derive(Debug, Clone)]
pub struct CaptureFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct RawChunk {
    pub samples: Vec<f32>,
    #[allow(dead_code)]
    pub format: CaptureFormat,
}
