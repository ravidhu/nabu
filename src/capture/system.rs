use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use screencapturekit::{
    output::CMSampleBuffer,
    shareable_content::SCShareableContent,
    stream::{
        configuration::SCStreamConfiguration, content_filter::SCContentFilter,
        output_trait::SCStreamOutputTrait, output_type::SCStreamOutputType, SCStream,
    },
};

use super::{CaptureFormat, RawChunk};

pub struct SystemStream {
    pub format: CaptureFormat,
    _stream: SCStream,
}

struct AudioDelegate {
    tx: Sender<RawChunk>,
    format: CaptureFormat,
}

impl SCStreamOutputTrait for AudioDelegate {
    fn did_output_sample_buffer(&self, sample_buffer: CMSampleBuffer, _of_type: SCStreamOutputType) {
        let Ok(buf_list) = sample_buffer.get_audio_buffer_list() else {
            return;
        };

        // SCK delivers non-interleaved LPCM float32 LE: one AudioBuffer per channel.
        // Collect each channel's samples, then interleave (L R L R …) so the
        // downstream MonoResampler receives the format it expects.
        let channels: Vec<Vec<f32>> = (0..buf_list.num_buffers())
            .filter_map(|index| buf_list.get(index))
            .map(|buf| {
                buf.data()
                    .chunks_exact(4)
                    .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    .collect()
            })
            .collect();

        if channels.is_empty() {
            return;
        }

        let n_frames = channels[0].len();
        let n_ch = channels.len();
        let mut samples = Vec::with_capacity(n_frames * n_ch);
        for frame in 0..n_frames {
            for channel in &channels {
                samples.push(*channel.get(frame).unwrap_or(&0.0));
            }
        }

        let _ = self.tx.try_send(RawChunk {
            samples,
            format: self.format.clone(),
        });
    }
}

#[cfg(target_os = "macos")]
pub fn start(tx: Sender<RawChunk>) -> Result<SystemStream> {
    let format = CaptureFormat {
        sample_rate: 48_000,
        channels: 2,
    };

    let config = SCStreamConfiguration::new()
        .set_captures_audio(true)
        .map_err(|err| anyhow!("SCStreamConfiguration::set_captures_audio: {err:?}"))?
        .set_channel_count(2)
        .map_err(|err| anyhow!("SCStreamConfiguration::set_channel_count: {err:?}"))?
        .set_sample_rate(48_000)
        .map_err(|err| anyhow!("SCStreamConfiguration::set_sample_rate: {err:?}"))?;

    let content = SCShareableContent::get().map_err(|err| anyhow!("SCShareableContent::get: {err:?}"))?;
    let displays = content.displays();
    let display = displays.first().ok_or_else(|| anyhow!("no display found"))?;

    let filter = SCContentFilter::new().with_display_excluding_windows(display, &[]);

    let mut stream = SCStream::new(&filter, &config);
    stream.add_output_handler(
        AudioDelegate {
            tx,
            format: format.clone(),
        },
        SCStreamOutputType::Audio,
    );
    stream
        .start_capture()
        .map_err(|err| anyhow!("SCStream::start_capture: {err:?}"))?;

    Ok(SystemStream {
        format,
        _stream: stream,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn start(_tx: Sender<RawChunk>) -> Result<SystemStream> {
    anyhow::bail!("system audio capture only implemented for macOS")
}
