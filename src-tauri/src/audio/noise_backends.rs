//! Frame engines. Construction happens before capture starts. Every process call
//! accepts/returns float PCM on the signed-16-bit scale used by the frame adapter.
use crate::settings::NoiseMethod;
pub enum Backend {
    #[cfg(feature = "noise-suppression")]
    Rnnoise(Box<nnnoiseless::DenoiseState<'static>>),
    #[cfg(feature = "speex")]
    Speex(Box<Speex>),
    #[cfg(feature = "webrtc")]
    Webrtc(Box<Webrtc>),
    #[cfg(feature = "deepfilter")]
    Deepfilter(Box<super::deep_worker::DeepWorker>),
}
impl Backend {
    pub fn new(method: NoiseMethod) -> Result<Self, String> {
        match method {
            #[cfg(feature = "noise-suppression")]
            NoiseMethod::Rnnoise => Ok(Self::Rnnoise(nnnoiseless::DenoiseState::new())),
            #[cfg(feature = "speex")]
            NoiseMethod::Speex => Ok(Self::Speex(Box::new(Speex::new()?))),
            #[cfg(feature = "webrtc")]
            NoiseMethod::Webrtc => {
                let config = sonora::Config {
                    pipeline: sonora::config::Pipeline {
                        maximum_internal_processing_rate:
                            sonora::config::MaxProcessingRate::Rate48kHz,
                        ..Default::default()
                    },
                    noise_suppression: Some(sonora::config::NoiseSuppression {
                        level: sonora::config::NoiseSuppressionLevel::High,
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let make = || {
                    sonora::AudioProcessing::builder()
                        .config(config.clone())
                        .capture_config(sonora::StreamConfig::new(48000, 1))
                        .render_config(sonora::StreamConfig::new(48000, 1))
                        .build()
                };
                let mut probe = make();
                let (mut max, mut peak) = (0f32, 120usize);
                for frame in 0..5 {
                    let mut impulse = [0.; 480];
                    if frame == 0 {
                        impulse[120] = 0.5;
                    }
                    let mut output = [0.; 480];
                    probe
                        .process_capture_f32(&[&impulse], &mut [&mut output])
                        .map_err(|error| format!("WebRTC initialization failed: {error:?}"))?;
                    for (i, sample) in output.into_iter().enumerate() {
                        if sample.abs() > max {
                            max = sample.abs();
                            peak = frame * 480 + i;
                        }
                    }
                }
                // Measure filter-bank/NS group delay before capture; use a fresh
                // state afterward so the probe is never played to the headphones.
                let delay = 480 + peak.saturating_sub(120);
                Ok(Self::Webrtc(Box::new(Webrtc {
                    state: make(),
                    delay,
                })))
            }
            #[cfg(feature = "deepfilter")]
            NoiseMethod::Deepfilter => Ok(Self::Deepfilter(Box::new(
                super::deep_worker::DeepWorker::new()?,
            ))),
            #[allow(unreachable_patterns)]
            _ => Err("This noise-removal engine was not included in this build.".into()),
        }
    }
    pub fn process(&mut self, input: &[f32; 480], output: &mut [f32; 480]) {
        let _ = input;
        match self {
            #[cfg(feature = "noise-suppression")]
            Self::Rnnoise(state) => {
                state.process_frame(output, input);
            }
            #[cfg(feature = "speex")]
            Self::Speex(state) => state.process(input, output),
            #[cfg(feature = "webrtc")]
            Self::Webrtc(state) => {
                let normalized = input.map(|sample| sample / 32768.);
                let mut dest = [0.; 480];
                if state
                    .state
                    .process_capture_f32(&[&normalized], &mut [&mut dest])
                    .is_err()
                {
                    output.fill(0.);
                    return;
                }
                for (out, value) in output.iter_mut().zip(dest) {
                    *out = value * 32768.;
                }
            }
            #[cfg(feature = "deepfilter")]
            Self::Deepfilter(state) => state.process(input, output),
            #[allow(unreachable_patterns)]
            _ => output.fill(0.),
        }
    }
    pub fn delay_samples(&self) -> usize {
        match self {
            #[cfg(feature = "deepfilter")]
            Self::Deepfilter(state) => state.delay_samples(),
            #[cfg(feature = "webrtc")]
            Self::Webrtc(state) => state.delay,
            #[allow(unreachable_patterns)]
            _ => 960,
        }
    }
    pub fn problems(&self) -> u64 {
        match self {
            #[cfg(feature = "deepfilter")]
            Self::Deepfilter(state) => state.misses(),
            #[allow(unreachable_patterns)]
            _ => 0,
        }
    }
}
#[cfg(feature = "webrtc")]
pub struct Webrtc {
    state: sonora::AudioProcessing,
    delay: usize,
}

#[cfg(feature = "speex")]
pub struct Speex {
    state: std::ptr::NonNull<std::ffi::c_void>,
    buffer: [i16; 480],
}
#[cfg(feature = "speex")]
unsafe extern "C" {
    fn speex_preprocess_state_init(frame_size: i32, sampling_rate: i32) -> *mut std::ffi::c_void;
    fn speex_preprocess_state_destroy(state: *mut std::ffi::c_void);
    fn speex_preprocess_run(state: *mut std::ffi::c_void, samples: *mut i16) -> i32;
    fn speex_preprocess_ctl(
        state: *mut std::ffi::c_void,
        request: i32,
        value: *mut std::ffi::c_void,
    ) -> i32;
}
// The C state has exactly one owner; it is never concurrently shared or cloned.
#[cfg(feature = "speex")]
unsafe impl Send for Speex {}
#[cfg(feature = "speex")]
impl Speex {
    fn new() -> Result<Self, String> {
        let state = std::ptr::NonNull::new(unsafe { speex_preprocess_state_init(480, 48000) })
            .ok_or("SpeexDSP initialization failed")?;
        let result = Self {
            state,
            buffer: [0; 480],
        };
        // Official speex_preprocess.h: denoise=0, AGC=2, VAD=4, noise level=18.
        for (request, mut value) in [(0, 1), (2, 0), (18, -30)] {
            let status = unsafe {
                speex_preprocess_ctl(
                    result.state.as_ptr(),
                    request,
                    (&mut value as *mut i32).cast(),
                )
            };
            if status != 0 {
                return Err("SpeexDSP parameter initialization failed".into());
            }
        }
        Ok(result)
    }
    fn process(&mut self, input: &[f32; 480], output: &mut [f32; 480]) {
        for (dest, sample) in self.buffer.iter_mut().zip(input) {
            *dest = sample.clamp(-32768., 32767.).round() as i16;
        }
        unsafe {
            speex_preprocess_run(self.state.as_ptr(), self.buffer.as_mut_ptr());
        }
        for (dest, sample) in output.iter_mut().zip(self.buffer) {
            *dest = sample as f32;
        }
    }
}
#[cfg(feature = "speex")]
impl Drop for Speex {
    fn drop(&mut self) {
        unsafe {
            speex_preprocess_state_destroy(self.state.as_ptr());
        }
    }
}
