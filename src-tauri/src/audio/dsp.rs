//! Mono, allocation-free sample processing. All state belongs to the capture callback.
//! Parameters arrive as a fixed-size Copy value, never through a callback mutex.
use crate::settings::Parameters;
use serde::Serialize;
use std::f32::consts::PI;

pub const FREQUENCIES: [f32; 8] = [60., 120., 250., 500., 1000., 2000., 4000., 8000.];
pub fn db(amplitude: f32) -> f32 {
    20. * amplitude.max(0.000001).log10()
}
pub fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.)
}
fn coefficient(ms: f32, rate: f32) -> f32 {
    (-1. / (ms * 0.001 * rate)).exp()
}
fn clean(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.
    } else {
        x
    }
}

/// An extension point for optional processors. `configure` must not allocate when
/// invoked by the callback; heavy model initialization belongs to stream setup.
pub trait Processor {
    fn configure(&mut self, parameters: &Parameters);
    fn process(&mut self, sample: f32) -> f32;
}

pub struct Gain {
    target: f32,
    current: f32,
    smoothing: f32,
}
impl Gain {
    fn new(rate: f32) -> Self {
        Self {
            target: 1.,
            current: 1.,
            smoothing: coefficient(5., rate),
        }
    }
}
impl Processor for Gain {
    fn configure(&mut self, p: &Parameters) {
        self.target = if p.gain_on { amplitude(p.gain_db) } else { 1. };
    }
    fn process(&mut self, x: f32) -> f32 {
        self.current = self.target + self.smoothing * (self.current - self.target);
        x * self.current
    }
}

pub struct Gate {
    rate: f32,
    threshold: f32,
    attack: f32,
    release: f32,
    hold_samples: u32,
    detector: f32,
    detector_decay: f32,
    remaining: u32,
    pub open: bool,
    gain: f32,
    enabled: bool,
}
impl Gate {
    fn new(rate: f32) -> Self {
        Self {
            rate,
            threshold: 0.,
            attack: 0.,
            release: 0.,
            hold_samples: 0,
            detector: 0.,
            detector_decay: coefficient(15., rate),
            remaining: 0,
            open: false,
            gain: 0.,
            enabled: true,
        }
    }
}
impl Processor for Gate {
    fn configure(&mut self, p: &Parameters) {
        self.enabled = p.gate_on;
        self.threshold = amplitude(p.gate_db);
        self.attack = coefficient(p.gate_attack, self.rate);
        self.release = coefficient(p.gate_release, self.rate);
        self.hold_samples = (p.gate_hold * 0.001 * self.rate) as u32;
    }
    fn process(&mut self, x: f32) -> f32 {
        self.detector = x.abs().max(self.detector * self.detector_decay);
        let threshold = if self.open {
            self.threshold * 0.708
        } else {
            self.threshold
        };
        if self.detector >= threshold {
            self.open = true;
            self.remaining = self.hold_samples;
        } else if self.remaining > 0 {
            self.remaining -= 1;
        } else {
            self.open = false;
        }
        let target = if self.open || !self.enabled { 1. } else { 0. };
        let c = if target > self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain = clean(target + c * (self.gain - target));
        x * self.gain
    }
}

#[derive(Clone, Copy)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    target_b: [f32; 3],
    target_a: [f32; 2],
    z: [f32; 2],
}
impl Default for Biquad {
    fn default() -> Self {
        Self {
            b: [1., 0., 0.],
            a: [0.; 2],
            target_b: [1., 0., 0.],
            target_a: [0.; 2],
            z: [0.; 2],
        }
    }
}
impl Biquad {
    fn set(&mut self, rate: f32, freq: f32, gain: f32) {
        // RBJ peaking biquad, Q=1. Fixed bands are clamped below Nyquist.
        let a = 10_f32.powf(gain / 40.);
        let omega = 2. * PI * freq.min(rate * 0.45) / rate;
        let alpha = omega.sin() / 2.;
        let cos = omega.cos();
        let a0 = 1. + alpha / a;
        self.target_b = [(1. + alpha * a) / a0, -2. * cos / a0, (1. - alpha * a) / a0];
        self.target_a = [-2. * cos / a0, (1. - alpha / a) / a0];
    }
    fn process(&mut self, x: f32) -> f32 {
        for i in 0..3 {
            self.b[i] += 0.002 * (self.target_b[i] - self.b[i]);
        }
        for i in 0..2 {
            self.a[i] += 0.002 * (self.target_a[i] - self.a[i]);
        }
        let y = self.b[0] * x + self.z[0];
        self.z[0] = clean(self.b[1] * x - self.a[0] * y + self.z[1]);
        self.z[1] = clean(self.b[2] * x - self.a[1] * y);
        y
    }
}
pub struct Equalizer {
    rate: f32,
    bands: [Biquad; 8],
}
impl Equalizer {
    fn new(rate: f32) -> Self {
        Self {
            rate,
            bands: [Biquad::default(); 8],
        }
    }
}
impl Processor for Equalizer {
    fn configure(&mut self, p: &Parameters) {
        for (i, b) in self.bands.iter_mut().enumerate() {
            b.set(
                self.rate,
                FREQUENCIES[i],
                if p.eq_on { p.eq[i] } else { 0. },
            );
        }
    }
    fn process(&mut self, mut x: f32) -> f32 {
        for band in &mut self.bands {
            x = band.process(x);
        }
        x
    }
}

pub struct Compressor {
    rate: f32,
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    enabled: bool,
    pub reduction: f32,
}
impl Compressor {
    fn new(rate: f32) -> Self {
        Self {
            rate,
            threshold: -18.,
            ratio: 3.,
            attack: 0.,
            release: 0.,
            makeup: 1.,
            enabled: false,
            reduction: 0.,
        }
    }
}
impl Processor for Compressor {
    fn configure(&mut self, p: &Parameters) {
        self.threshold = p.compressor_db;
        self.ratio = p.ratio;
        self.attack = coefficient(p.compressor_attack, self.rate);
        self.release = coefficient(p.compressor_release, self.rate);
        self.makeup = if p.compressor_on {
            amplitude(p.makeup_db)
        } else {
            1.
        };
        self.enabled = p.compressor_on;
    }
    fn process(&mut self, x: f32) -> f32 {
        let over = db(x.abs()) - self.threshold;
        let knee = 6.;
        let target = if !self.enabled || over <= -knee / 2. {
            0.
        } else if over < knee / 2. {
            (1. - 1. / self.ratio) * (over + knee / 2.).powi(2) / (2. * knee)
        } else {
            over * (1. - 1. / self.ratio)
        };
        let c = if target > self.reduction {
            self.attack
        } else {
            self.release
        };
        self.reduction = target + c * (self.reduction - target);
        x * amplitude(-self.reduction) * self.makeup
    }
}
pub struct Limiter {
    ceiling: f32,
    release: f32,
    gain: f32,
    enabled: bool,
    pub active: bool,
}
impl Limiter {
    fn new(rate: f32) -> Self {
        Self {
            ceiling: amplitude(-1.),
            release: coefficient(80., rate),
            gain: 1.,
            enabled: true,
            active: false,
        }
    }
}
impl Processor for Limiter {
    fn configure(&mut self, p: &Parameters) {
        self.ceiling = amplitude(p.ceiling_db);
        self.enabled = p.limiter_on;
    }
    fn process(&mut self, x: f32) -> f32 {
        let target = if self.enabled {
            (self.ceiling / x.abs().max(1e-9)).min(1.)
        } else {
            1.
        };
        self.gain = if target < self.gain {
            target
        } else {
            target + self.release * (self.gain - target)
        };
        self.active = self.enabled && self.gain < 0.999;
        let out = x * self.gain;
        if self.enabled {
            out.clamp(-self.ceiling, self.ceiling)
        } else {
            out
        }
    }
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Levels {
    pub suppression_problems: u64,
    pub raw_db: f32,
    pub gate_input_db: f32,
    pub gate_detector_db: f32,
    pub output_db: f32,
    pub peak_db: f32,
    pub clipped: bool,
    pub gate_open: bool,
    pub reduction_db: f32,
    pub limiter_active: bool,
    pub frames: u64,
}
pub struct Chain {
    gain: Gain,
    suppression: super::suppression::Suppression,
    gate: Gate,
    eq: Equalizer,
    compressor: Compressor,
    limiter: Limiter,
    params: Parameters,
    configured: bool,
    blend: f32,
    blend_coefficient: f32,
    pub gate_input: f32,
    pub gain_output: f32,
}
impl Chain {
    pub fn new(rate: f32, p: Parameters) -> Self {
        Self::new_with_method(rate, p, crate::settings::NoiseMethod::Rnnoise)
    }
    pub fn new_with_method(rate: f32, p: Parameters, method: crate::settings::NoiseMethod) -> Self {
        let mut s = Self {
            gain: Gain::new(rate),
            suppression: super::suppression::Suppression::new_with_method(rate as u32, method),
            gate: Gate::new(rate),
            eq: Equalizer::new(rate),
            compressor: Compressor::new(rate),
            limiter: Limiter::new(rate),
            params: p,
            configured: false,
            blend: if p.bypass { 0. } else { 1. },
            blend_coefficient: coefficient(5., rate),
            gate_input: 0.,
            gain_output: 0.,
        };
        s.configure(p);
        s
    }
    pub fn configure(&mut self, p: Parameters) {
        if self.configured && self.params == p {
            return;
        }
        self.configured = true;
        self.params = p;
        self.gain.configure(&p);
        self.gate.configure(&p);
        self.eq.configure(&p);
        self.compressor.configure(&p);
        self.limiter.configure(&p);
    }
    pub fn process(&mut self, raw: f32) -> f32 {
        let raw = if raw.is_finite() { raw } else { 0. };
        let x = self.gain.process(raw);
        self.gain_output = x;
        let x = self.suppression.process(
            x,
            self.params.suppression_on && !self.params.bypass,
            self.params.suppression_strength,
        );
        self.gate_input = x;
        let x = self.gate.process(x);
        let x = self.eq.process(x);
        let x = self.compressor.process(x);
        let x = self.limiter.process(x);
        let target = if self.params.bypass { 0. } else { 1. };
        self.blend = target + self.blend_coefficient * (self.blend - target);
        if !self.params.enabled {
            0.
        } else {
            raw * (1. - self.blend) + x * self.blend
        }
    }
    pub fn gate_open(&self) -> bool {
        !self.params.gate_on || self.params.bypass || self.gate.open
    }
    pub fn suppression_supported(&self) -> bool {
        self.suppression.supported()
    }
    pub fn suppression_delay(&self) -> u32 {
        (self.suppression.delay as f32 / 48.) as u32
    }
    pub fn suppression_error(&self) -> Option<&str> {
        self.suppression.error.as_deref()
    }
    pub fn suppression_problems(&self) -> u64 {
        self.suppression.problems()
    }
    pub fn reduction(&self) -> f32 {
        if self.params.bypass {
            0.
        } else {
            self.compressor.reduction
        }
    }
    pub fn limiting(&self) -> bool {
        !self.params.bypass && self.limiter.active
    }
}

/// Fixed 20 ms telemetry windows so small WASAPI callbacks do not erase peaks.
pub struct Meter {
    rate: u32,
    n: u32,
    frames: u64,
    raw: f64,
    gate: f64,
    out: f64,
    peak: f32,
    clip: bool,
    limiting: bool,
    gr: f32,
    gate_peak: f32,
}
impl Meter {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            n: 0,
            frames: 0,
            raw: 0.,
            gate: 0.,
            out: 0.,
            peak: 0.,
            clip: false,
            limiting: false,
            gr: 0.,
            gate_peak: 0.,
        }
    }
    pub fn push(&mut self, raw: f32, processed: f32, chain: &Chain) -> Option<Levels> {
        self.n += 1;
        self.frames += 1;
        self.raw += (raw as f64).powi(2);
        self.gate += (chain.gate_input as f64).powi(2);
        self.gate_peak = self.gate_peak.max(chain.gate.detector);
        self.out += (processed as f64).powi(2);
        self.peak = self
            .peak
            .max(raw.abs())
            .max(processed.abs())
            .max(chain.gate_input.abs());
        self.peak = self.peak.max(chain.gain_output.abs());
        self.clip |= self.peak >= 0.999;
        self.limiting |= chain.limiting();
        self.gr = self.gr.max(chain.reduction());
        if self.n < self.rate / 50 {
            return None;
        }
        let l = Levels {
            suppression_problems: chain.suppression_problems(),
            raw_db: db((self.raw / self.n as f64).sqrt() as f32),
            gate_input_db: db((self.gate / self.n as f64).sqrt() as f32),
            gate_detector_db: db(self.gate_peak),
            output_db: db((self.out / self.n as f64).sqrt() as f32),
            peak_db: db(self.peak),
            clipped: self.clip,
            gate_open: chain.gate_open(),
            reduction_db: self.gr,
            limiter_active: self.limiting,
            frames: self.frames,
        };
        self.n = 0;
        self.raw = 0.;
        self.gate = 0.;
        self.out = 0.;
        self.peak = 0.;
        self.clip = false;
        self.limiting = false;
        self.gr = 0.;
        self.gate_peak = 0.;
        Some(l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_short_peak_opens_the_gate_even_when_average_is_below_cutoff() {
        let p = Parameters {
            gate_db: -40.,
            ..Parameters::default()
        };
        let mut chain = Chain::new(48000., p);
        let mut meter = Meter::new(48000);
        let mut result = None;
        for i in 0..960 {
            let raw = if i == 0 { 0.05 } else { 0. };
            let output = chain.process(raw);
            result = meter.push(raw, output, &chain).or(result);
        }
        let levels = result.unwrap();
        assert!(levels.gate_input_db < p.gate_db);
        assert!(levels.gate_detector_db > p.gate_db);
        assert!(levels.gate_open);
    }
    #[test]
    fn bypass_and_disabled() {
        let mut p = Parameters {
            bypass: true,
            gain_db: 18.,
            eq: [12.; 8],
            ..Parameters::default()
        };
        let mut c = Chain::new(48000., p);
        for _ in 0..1000 {
            assert_eq!(c.process(0.2), 0.2);
        }
        p.enabled = false;
        c.configure(p);
        assert_eq!(c.process(0.2), 0.);
    }
    #[test]
    fn gate_opens_holds_and_closes() {
        let p = Parameters::default();
        let mut g = Gate::new(48000.);
        g.configure(&p);
        for _ in 0..4800 {
            g.process(0.1);
        }
        assert!(g.open);
        for _ in 0..4800 {
            g.process(0.);
        }
        assert!(g.open);
        for _ in 0..96000 {
            g.process(0.);
        }
        assert!(!g.open);
        assert!(g.gain < 0.0001);
    }
    #[test]
    fn compressor_reduces_steady_signal() {
        let p = Parameters {
            compressor_on: true,
            compressor_db: -20.,
            ratio: 4.,
            ..Parameters::default()
        };
        let mut c = Compressor::new(48000.);
        c.configure(&p);
        let mut out = 0.;
        for _ in 0..48000 {
            out = c.process(1.);
        }
        assert!((db(out) + 15.).abs() < 0.1);
    }
    #[test]
    fn limiter_bounds_transients() {
        let mut c = Chain::new(
            48000.,
            Parameters {
                gain_db: 18.,
                eq: [12.; 8],
                gate_on: false,
                ..Parameters::default()
            },
        );
        for i in 0..96000 {
            let x = (i as f32 * 0.13).sin();
            let y = c.process(x);
            assert!(y.is_finite() && y.abs() <= amplitude(-1.) + 1e-6);
        }
    }
    #[test]
    fn eq_bypass_unity() {
        let mut eq = Equalizer::new(48000.);
        eq.configure(&Parameters {
            eq_on: false,
            eq: [12.; 8],
            ..Parameters::default()
        });
        for i in 0..1000 {
            let x = (i as f32).sin();
            assert!((eq.process(x) - x).abs() < 1e-5);
        }
    }
    #[test]
    fn silence_and_nan_are_finite() {
        let mut c = Chain::new(44100., Parameters::default());
        assert!(c.process(f32::NAN).is_finite());
        for _ in 0..10000 {
            assert!(c.process(0.).is_finite());
        }
    }
}
