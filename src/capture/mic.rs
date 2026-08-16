use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;

use super::{CaptureFormat, RawChunk};

pub struct MicStream {
    _stream: cpal::Stream,
    pub format: CaptureFormat,
}

pub fn start(tx: Sender<RawChunk>) -> Result<MicStream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device"))?;

    let config = device.default_input_config().context("default_input_config")?;
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.clone().into();

    let format = CaptureFormat {
        sample_rate: stream_config.sample_rate.0,
        channels: stream_config.channels,
    };
    tracing::info!(?format, "mic format");

    let err_fn = |err| tracing::error!(error = ?err, "mic stream error");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let fmt = format.clone();
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    let _ = tx.send(RawChunk {
                        samples: data.to_vec(),
                        format: fmt.clone(),
                    });
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let fmt = format.clone();
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    let samples = data
                        .iter()
                        .map(|sample| *sample as f32 / i16::MAX as f32)
                        .collect();
                    let _ = tx.send(RawChunk {
                        samples,
                        format: fmt.clone(),
                    });
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let fmt = format.clone();
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    let samples = data
                        .iter()
                        .map(|sample| (*sample as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0))
                        .collect();
                    let _ = tx.send(RawChunk {
                        samples,
                        format: fmt.clone(),
                    });
                },
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("unsupported mic sample format: {other:?}")),
    };

    stream.play().context("mic stream play")?;
    Ok(MicStream {
        _stream: stream,
        format,
    })
}
