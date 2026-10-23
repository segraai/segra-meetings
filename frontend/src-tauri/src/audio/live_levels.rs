// audio/live_levels.rs
//
// Live input levels of the active recording, for the recording UI's level meters.
// The pipeline records the loudest chunk per source; the frontend polls and resets.

use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};

use super::recording_state::DeviceType;

static MIC_RMS: AtomicU32 = AtomicU32::new(0);
static MIC_PEAK: AtomicU32 = AtomicU32::new(0);
static SYSTEM_RMS: AtomicU32 = AtomicU32::new(0);
static SYSTEM_PEAK: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct RecordingAudioLevels {
    pub mic_rms: f32,
    pub mic_peak: f32,
    pub system_rms: f32,
    pub system_peak: f32,
}

/// Keeps the larger of the stored and new value (levels are non-negative, so the
/// f32 bit patterns order the same way as the floats).
fn store_max(slot: &AtomicU32, value: f32) {
    slot.fetch_max(value.max(0.0).to_bits(), Ordering::Relaxed);
}

fn take(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.swap(0, Ordering::Relaxed))
}

/// Record the level of one raw capture chunk.
pub fn record_chunk(device_type: &DeviceType, samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mut sum_squares = 0.0f32;
    let mut peak = 0.0f32;
    for &sample in samples {
        sum_squares += sample * sample;
        peak = peak.max(sample.abs());
    }
    let rms = (sum_squares / samples.len() as f32).sqrt();

    let (rms_slot, peak_slot) = match device_type {
        DeviceType::Microphone => (&MIC_RMS, &MIC_PEAK),
        DeviceType::System => (&SYSTEM_RMS, &SYSTEM_PEAK),
    };
    store_max(rms_slot, rms);
    store_max(peak_slot, peak);
}

/// Loudest levels since the previous call, then reset.
pub fn take_levels() -> RecordingAudioLevels {
    RecordingAudioLevels {
        mic_rms: take(&MIC_RMS),
        mic_peak: take(&MIC_PEAK),
        system_rms: take(&SYSTEM_RMS),
        system_peak: take(&SYSTEM_PEAK),
    }
}

/// Clear stale levels when a recording starts.
pub fn reset() {
    take_levels();
}

#[tauri::command]
pub async fn get_recording_audio_levels() -> RecordingAudioLevels {
    take_levels()
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test touches the shared statics so parallel tests can't interleave.
    #[test]
    fn keeps_loudest_chunk_per_source_until_taken() {
        reset();
        record_chunk(&DeviceType::Microphone, &[0.5, -0.5]);
        record_chunk(&DeviceType::Microphone, &[0.1, -0.1]);
        record_chunk(&DeviceType::System, &[0.0, -0.8]);

        let levels = take_levels();
        assert!((levels.mic_rms - 0.5).abs() < 1e-6);
        assert!((levels.mic_peak - 0.5).abs() < 1e-6);
        assert!((levels.system_peak - 0.8).abs() < 1e-6);

        assert_eq!(
            take_levels(),
            RecordingAudioLevels { mic_rms: 0.0, mic_peak: 0.0, system_rms: 0.0, system_peak: 0.0 }
        );
    }
}
