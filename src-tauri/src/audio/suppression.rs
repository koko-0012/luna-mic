//! RNNoise frame adapter. Only 48 kHz is supported; other formats pass through.
//! Allocate model/FFT state during stream setup, never while processing samples.
const FRAME: usize = 480;
const DELAY: usize = FRAME * 2; // 10 ms frame assembly + 10 ms overlap synthesis.
const MAX_DELAY: usize = 8192;
use super::noise_backends::Backend;
use crate::settings::NoiseMethod;
use serde::Serialize;
#[derive(Clone, Serialize)]
pub struct MethodInfo {
    pub id: NoiseMethod,
    pub name: &'static str,
    pub available: bool,
    pub description: &'static str,
}
pub fn available_method(rate: u32, method: NoiseMethod) -> bool {
    rate == 48000
        && match method {
            NoiseMethod::Rnnoise => cfg!(feature = "noise-suppression"),
            NoiseMethod::Speex => cfg!(feature = "speex"),
            NoiseMethod::Webrtc => cfg!(feature = "webrtc"),
            NoiseMethod::Deepfilter => cfg!(feature = "deepfilter"),
        }
}
pub fn methods(rate: u32) -> Vec<MethodInfo> {
    [
        (
            NoiseMethod::Rnnoise,
            "RNNoise",
            "Lightweight neural speech cleanup",
        ),
        (
            NoiseMethod::Speex,
            "SpeexDSP",
            "Traditional suppression, approximately 30 dB maximum",
        ),
        (
            NoiseMethod::Webrtc,
            "WebRTC",
            "Call-style suppression; echo and auto gain remain off",
        ),
        (
            NoiseMethod::Deepfilter,
            "DeepFilterNet 3",
            "Heavier neural cleanup on an isolated worker; extra latency",
        ),
    ]
    .into_iter()
    .map(|(id, name, description)| MethodInfo {
        id,
        name,
        description,
        available: available_method(rate, id),
    })
    .collect()
}
pub fn available(rate: u32) -> bool {
    available_method(rate, NoiseMethod::Rnnoise)
}
pub struct Suppression {
    state: Option<Backend>,
    pub error: Option<String>,
    pub delay: usize,
    supported: bool,
    input: [f32; FRAME],
    wet: [f32; FRAME],
    dry: [f32; MAX_DELAY],
    position: usize,
    dry_position: usize,
    mix: f32,
    flush_frames: usize,
}
impl Suppression {
    pub fn new(rate: u32) -> Self {
        Self::new_with_method(rate, NoiseMethod::Rnnoise)
    }
    pub fn new_with_method(rate: u32, method: NoiseMethod) -> Self {
        let (state, error) = if available_method(rate, method) {
            match Backend::new(method) {
                Ok(mut state) => {
                    state.process(&[0.; FRAME], &mut [0.; FRAME]);
                    (Some(state), None)
                }
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        let delay = state
            .as_ref()
            .map(Backend::delay_samples)
            .unwrap_or(DELAY)
            .min(MAX_DELAY);
        let supported = state.is_some();
        Self {
            state,
            error,
            delay,
            supported,
            input: [0.; FRAME],
            wet: [0.; FRAME],
            dry: [0.; MAX_DELAY],
            position: 0,
            dry_position: 0,
            mix: 0.,
            flush_frames: 0,
        }
    }
    pub fn process(&mut self, sample: f32, on: bool, strength: f32) -> f32 {
        if !self.supported {
            return sample;
        }
        if on {
            self.flush_frames = 4;
        }
        let dry = self.dry[self.dry_position];
        self.dry[self.dry_position] = sample;
        self.dry_position = (self.dry_position + 1) % self.delay;
        let wet = self.wet[self.position] / 32768.;
        self.input[self.position] = sample.clamp(-1., 1.) * 32768.;
        self.position += 1;
        if self.position == FRAME {
            self.position = 0;
            if (on || self.mix > 0.0001 || self.flush_frames > 0) && self.state.is_some() {
                if let Some(state) = &mut self.state {
                    if on || self.mix > 0.0001 {
                        state.process(&self.input, &mut self.wet);
                    } else {
                        // Flush the old overlap/pitch history after fading off.
                        // Otherwise re-enabling could briefly replay an old frame.
                        state.process(&[0.; FRAME], &mut self.wet);
                        self.flush_frames -= 1;
                        self.wet.fill(0.);
                    }
                }
            }
        }
        // Smooth on/off; keep a dry history so strength blends time-aligned audio.
        // At zero mix the off path has no added latency and does no neural inference.
        let target = if on { 1. } else { 0. };
        self.mix += 0.002 * (target - self.mix);
        if self.mix < 0.0001 && !on {
            self.mix = 0.;
            return sample;
        }
        let denoised = dry * (1. - strength) + wet * strength;
        sample * (1. - self.mix) + denoised * self.mix
    }
    pub fn supported(&self) -> bool {
        self.supported
    }
    pub fn problems(&self) -> u64 {
        self.state.as_ref().map(Backend::problems).unwrap_or(0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_compiled_engines_initialize_and_produce_finite_frames() {
        for method in methods(48000).into_iter().filter(|method| method.available) {
            let mut engine = Suppression::new_with_method(48000, method.id);
            assert!(engine.supported(), "{}: {:?}", method.name, engine.error);
            println!(
                "{} ready: estimated {} ms frame delay",
                method.name,
                engine.delay / 48
            );
            for frame in 0..12 {
                for i in 0..480 {
                    let sample = ((frame * 480 + i) as f32 * 0.026).sin() * 0.1;
                    assert!(
                        engine.process(sample, true, 1.).is_finite(),
                        "{}",
                        method.name
                    );
                }
                if method.id == NoiseMethod::Deepfilter {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    }
    #[test]
    fn off_and_unsupported_are_exact() {
        for rate in [44100, 48000] {
            let mut s = Suppression::new(rate);
            for i in 0..2000 {
                let x = (i as f32 * 0.1).sin() * 0.1;
                assert_eq!(s.process(x, false, 1.), x);
            }
        }
    }
    #[cfg(feature = "noise-suppression")]
    #[test]
    fn reenable_after_silence_does_not_replay_previous_audio() {
        let mut s = Suppression::new(48000);
        for i in 0..12000 {
            s.process((i as f32 * 0.05).sin() * 0.2, true, 1.);
        }
        for _ in 0..12000 {
            s.process(0., false, 1.);
        }
        for _ in 0..2000 {
            assert!(s.process(0., true, 1.).abs() < 0.0001);
        }
    }
    #[cfg(feature = "noise-suppression")]
    #[test]
    fn full_strength_reduces_generated_stationary_noise() {
        let mut s = Suppression::new(48000);
        let mut seed = 1u32;
        let (mut input, mut output) = (0f64, 0f64);
        for i in 0..96000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = ((seed >> 8) as f32 / 16777216. - 0.5) * 0.02;
            let y = s.process(x, true, 1.);
            assert!(y.is_finite());
            if i > 48000 {
                input += (x as f64).powi(2);
                output += (y as f64).powi(2);
            }
        }
        assert!(
            output < input * 0.9,
            "RNNoise should reduce this stationary noise: {output}/{input}"
        );
    }
}
