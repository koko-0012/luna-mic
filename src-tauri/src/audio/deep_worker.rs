//! DeepFilterNet owns non-Send inference state and may allocate per frame. Keep
//! that state on a dedicated worker; capture exchanges fixed-size frames only.
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc,
};
use std::thread::{self, JoinHandle};
const LAG: u64 = 2;
#[derive(Clone, Copy)]
struct Frame {
    sequence: u64,
    samples: [f32; 480],
}
pub struct DeepWorker {
    pending: Option<Frame>,
    input: HeapProd<Frame>,
    output: HeapCons<Frame>,
    sequence: u64,
    delay: usize,
    stop: Arc<AtomicBool>,
    misses: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}
impl DeepWorker {
    pub fn new() -> Result<Self, String> {
        let (input, mut incoming) = HeapRb::<Frame>::new(8).split();
        let (mut outgoing, output) = HeapRb::<Frame>::new(8).split();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let misses = Arc::new(AtomicU64::new(0));
        let errors = misses.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("luna-deepfilter".into())
            .spawn(move || {
                let initialized = std::panic::catch_unwind(|| {
                    df::tract::DfTract::new(
                        df::tract::DfParams::default(),
                        &df::tract::RuntimeParams::default(),
                    )
                    .map_err(|error| error.to_string())
                });
                let mut model = match initialized {
                    Ok(Ok(model)) => model,
                    Ok(Err(error)) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                    Err(_) => {
                        let _ =
                            ready_tx.send(Err("DeepFilterNet model initialization failed".into()));
                        return;
                    }
                };
                if model.sr != 48000 || model.hop_size != 480 {
                    let _ = ready_tx.send(Err("Unsupported DeepFilterNet model format".into()));
                    return;
                }
                let delay = model.fft_size - model.hop_size
                    + (model.lookahead + LAG as usize + 1) * model.hop_size;
                let mut enhanced = [0.; 480];
                let _ = model.process(
                    ndarray::ArrayView2::from_shape((1, 480), &[0.; 480]).unwrap(),
                    ndarray::ArrayViewMut2::from_shape((1, 480), &mut enhanced).unwrap(),
                );
                let _ = ready_tx.send(Ok(delay));
                while !stop_worker.load(Ordering::Acquire) {
                    let Some(frame) = incoming.try_pop() else {
                        thread::park();
                        continue;
                    };
                    let normalized = frame.samples.map(|sample| sample / 32768.);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        model.process(
                            ndarray::ArrayView2::from_shape((1, 480), &normalized).unwrap(),
                            ndarray::ArrayViewMut2::from_shape((1, 480), &mut enhanced).unwrap(),
                        )
                    }));
                    if !matches!(result, Ok(Ok(_))) {
                        enhanced.fill(0.);
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                    let result = Frame {
                        sequence: frame.sequence,
                        samples: enhanced.map(|sample| {
                            if sample.is_finite() {
                                sample * 32768.
                            } else {
                                0.
                            }
                        }),
                    };
                    if outgoing.try_push(result).is_err() {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        let delay = match ready_rx.recv_timeout(std::time::Duration::from_secs(45)) {
            Ok(Ok(delay)) => delay,
            other => {
                stop.store(true, Ordering::Release);
                worker.thread().unpark();
                return Err(match other {
                    Ok(Err(error)) => error,
                    _ => "DeepFilterNet initialization timed out".into(),
                });
            }
        };
        Ok(Self {
            input,
            output,
            sequence: 0,
            pending: None,
            delay,
            stop,
            misses,
            worker: Some(worker),
        })
    }
    pub fn process(&mut self, input: &[f32; 480], output: &mut [f32; 480]) {
        let sequence = self.sequence;
        self.sequence += 1;
        if self
            .input
            .try_push(Frame {
                sequence,
                samples: *input,
            })
            .is_err()
        {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
        output.fill(0.);
        if sequence < LAG {
            return;
        }
        let wanted = sequence - LAG;
        // Bounded queue: discard old late results instead of building up delay.
        while let Some(frame) = self.pending.take().or_else(|| self.output.try_pop()) {
            if frame.sequence == wanted {
                *output = frame.samples;
                return;
            }
            if frame.sequence > wanted {
                self.pending = Some(frame);
                self.misses.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
    }
    pub fn delay_samples(&self) -> usize {
        self.delay
    }
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}
impl Drop for DeepWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
