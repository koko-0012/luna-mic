//! Output abstraction: a processed mono stream is routed to a render endpoint.
//! A future signed virtual-device transport can replace this sink without changing DSP.
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    FromSample, SizedSample,
};
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

#[derive(Default)]
pub struct OutputCounters {
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
    pub queued: AtomicU32,
    pub failed: AtomicBool,
}
pub struct AudioSink {
    producer: HeapProd<f32>,
    pub counters: Arc<OutputCounters>,
}
impl AudioSink {
    /// Never blocks. Overflow drops the newest frame and reports it in diagnostics.
    pub fn push(&mut self, x: f32) {
        if self.producer.try_push(x).is_err() {
            self.counters.overruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub struct RenderOutput {
    pub stream: cpal::Stream,
    pub sink: AudioSink,
    pub rate: u32,
}
impl RenderOutput {
    pub fn open(id: Option<&str>, input_rate: u32) -> Result<Self, String> {
        let device = crate::devices::find(id, false)?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("Output format unavailable: {e}"))?;
        let rate = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.clone().into();
        let (producer, consumer) = HeapRb::<f32>::new(input_rate as usize / 5).split();
        let counters = Arc::new(OutputCounters::default());
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                build::<f32>(&device, &config, consumer, input_rate, counters.clone())
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config, consumer, input_rate, counters.clone())
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(&device, &config, consumer, input_rate, counters.clone())
            }
            f => {
                return Err(format!(
                    "Unsupported output sample format: {f:?}. Use a float32 or 16-bit endpoint."
                ))
            }
        }?;
        stream
            .play()
            .map_err(|e| format!("Could not start output: {e}"))?;
        Ok(Self {
            stream,
            sink: AudioSink { producer, counters },
            rate,
        })
    }
}

struct Resampler {
    queue: HeapCons<f32>,
    ratio: f64,
    phase: f64,
    a: f32,
    b: f32,
    started: bool,
    target: usize,
    counters: Arc<OutputCounters>,
}
impl Resampler {
    fn next(&mut self, step: f64) -> f32 {
        if !self.started {
            if self.queue.occupied_len() < self.target {
                return 0.;
            }
            self.a = self.queue.try_pop().unwrap_or(0.);
            self.b = self.queue.try_pop().unwrap_or(0.);
            self.started = true;
        }
        let x = self.a + (self.b - self.a) * self.phase as f32;
        self.phase += step;
        while self.phase >= 1. {
            self.phase -= 1.;
            self.a = self.b;
            match self.queue.try_pop() {
                Some(v) => self.b = v,
                None => {
                    self.counters.underruns.fetch_add(1, Ordering::Relaxed);
                    self.started = false;
                    self.phase = 0.;
                    self.a = 0.;
                    self.b = 0.;
                    return 0.;
                }
            }
        }
        // Every endpoint has a final format safety clamp, including raw monitoring.
        x.clamp(-1., 1.)
    }
}
fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: HeapCons<f32>,
    input_rate: u32,
    counters: Arc<OutputCounters>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let error = counters.clone();
    let mut resampler = Resampler {
        queue,
        ratio: input_rate as f64 / config.sample_rate.0 as f64,
        phase: 0.,
        a: 0.,
        b: 0.,
        started: false,
        target: input_rate as usize / 50,
        counters,
    };
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let occupied = resampler.queue.occupied_len();
                resampler
                    .counters
                    .queued
                    .store(occupied as u32, Ordering::Relaxed);
                // Small clock drift between independently clocked WASAPI endpoints is
                // corrected slowly through the resampling ratio, rather than unbounded delay.
                let drift = ((occupied as f64 - resampler.target as f64) / resampler.target as f64
                    * 0.002)
                    .clamp(-0.005, 0.005);
                let step = resampler.ratio * (1. + drift);
                for frame in data.chunks_mut(channels) {
                    let x = resampler.next(step);
                    for sample in frame {
                        *sample = T::from_sample(x);
                    }
                }
            },
            move |_| {
                error.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("Could not open output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resampling_and_underflow() {
        let (mut p, q) = HeapRb::<f32>::new(1000).split();
        for _ in 0..500 {
            p.try_push(0.25).unwrap();
        }
        let c = Arc::new(OutputCounters::default());
        let mut r = Resampler {
            queue: q,
            ratio: 1.,
            phase: 0.,
            a: 0.,
            b: 0.,
            started: false,
            target: 10,
            counters: c.clone(),
        };
        for _ in 0..200 {
            assert!((r.next(2.) - 0.25).abs() < 1e-5);
        }
        for _ in 0..100 {
            r.next(2.);
        }
        assert!(c.underruns.load(Ordering::Relaxed) > 0);
        assert_eq!(r.next(1.), 0.);
    }
}
