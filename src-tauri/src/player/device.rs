use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, DeviceId, SampleFormat, StreamConfig, SupportedStreamConfigRange};

pub(super) struct DeviceIdentity {
    pub(super) id: DeviceId,
    pub(super) name: String,
}

impl DeviceIdentity {
    pub(super) fn same_device_as(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub(super) fn default_output_device() -> Result<Device, String> {
    cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "No default output device available".to_string())
}

pub(super) fn device_identity(device: &Device) -> Result<DeviceIdentity, String> {
    let id = device
        .id()
        .map_err(|e| format!("Failed to identify output device: {}", e))?;
    let name = device
        .description()
        .ok()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|| "<unknown-device>".to_string());

    Ok(DeviceIdentity { id, name })
}

pub(super) fn default_output_device_identity() -> Result<DeviceIdentity, String> {
    let device = default_output_device()?;
    device_identity(&device)
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
        return Ok((config, default_format, config.sample_rate));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(id: &str, name: &str) -> DeviceIdentity {
        DeviceIdentity {
            id: DeviceId::new(cpal::default_host().id(), id),
            name: name.to_string(),
        }
    }

    #[test]
    fn device_identity_ignores_display_name_changes() {
        let current = identity("device-a", "Old name");
        let renamed = identity("device-a", "New name");

        assert!(current.same_device_as(&renamed));
    }

    #[test]
    fn device_identity_distinguishes_same_name_devices() {
        let first = identity("device-a", "Same name");
        let second = identity("device-b", "Same name");

        assert!(!first.same_device_as(&second));
    }
}
