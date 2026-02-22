use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, SampleFormat, StreamConfig, SupportedStreamConfigRange};

pub(super) fn default_output_device() -> Result<Device, String> {
    cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "No default output device available".to_string())
}

#[allow(deprecated)]
pub(super) fn device_name(device: &Device) -> Option<String> {
    device.name().ok()
}

pub(super) fn default_output_device_name() -> Option<String> {
    default_output_device().ok().and_then(|d| device_name(&d))
}

fn nearest_sample_rate(range: &SupportedStreamConfigRange, target: u32) -> u32 {
    target.clamp(range.min_sample_rate(), range.max_sample_rate())
}

pub(super) fn choose_stream_config(
    device: &Device,
    requested_sr: Option<u32>,
) -> Result<(StreamConfig, SampleFormat, u32), String> {
    let default_config = device
        .default_output_config()
        .map_err(|e| format!("Failed to get default output config: {}", e))?;
    let default_format = default_config.sample_format();
    let default_channels = default_config.channels();

    if requested_sr.is_none() {
        let config = default_config.config();
        return Ok((config.clone(), default_format, config.sample_rate));
    }

    let target_sr = requested_sr.unwrap();
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| format!("Failed to query supported output configs: {}", e))?
        .collect();

    if ranges.is_empty() {
        return Err("No supported output configs found".to_string());
    }

    let mut candidates: Vec<&SupportedStreamConfigRange> = ranges
        .iter()
        .filter(|x| x.channels() == default_channels && x.sample_format() == default_format)
        .collect();
    if candidates.is_empty() {
        candidates = ranges
            .iter()
            .filter(|x| x.sample_format() == default_format)
            .collect();
    }
    if candidates.is_empty() {
        candidates = ranges.iter().collect();
    }

    let mut best: Option<(&SupportedStreamConfigRange, u32, u32, bool)> = None;
    for range in candidates {
        let sr = nearest_sample_rate(range, target_sr);
        let diff = sr.abs_diff(target_sr);
        let is_greater_or_eq = sr >= target_sr;
        let should_replace = match best {
            None => true,
            Some((_, _, best_diff, best_is_greater_or_eq)) => {
                diff < best_diff
                    || (diff == best_diff && is_greater_or_eq && !best_is_greater_or_eq)
            }
        };

        if should_replace {
            best = Some((range, sr, diff, is_greater_or_eq));
        }
    }

    let (range, sample_rate, _, _) = best.unwrap();
    let config = range.with_sample_rate(sample_rate).config();
    Ok((config, range.sample_format(), sample_rate))
}
