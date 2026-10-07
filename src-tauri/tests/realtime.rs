use luna_mic_core::{audio::dsp::Chain, settings::Parameters};
use ringbuf::{traits::*, HeapRb};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static TRACK:Cell<bool>=const{Cell::new(false)};
    static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};
}
struct CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.try_with(|flag| flag.get()).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.try_with(|flag| flag.get()).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
        System.realloc(ptr, layout, size)
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn processing_parameter_updates_and_routing_do_not_allocate() {
    let mut methods: Vec<_> = luna_mic_core::audio::suppression::methods(48000)
        .into_iter()
        .filter(|method| method.available)
        .map(|method| method.id)
        .collect();
    if methods.is_empty() {
        methods.push(luna_mic_core::settings::NoiseMethod::Rnnoise);
    }
    for method in methods {
        let mut p = Parameters::default();
        let mut chain = Chain::new_with_method(48000., p, method);
        let (mut producer, mut consumer) = HeapRb::<f32>::new(1024).split();
        let (mut controls, mut read) = triple_buffer::triple_buffer(&p);
        ALLOCATIONS.with(|n| n.set(0));
        TRACK.with(|flag| flag.set(true));
        let mut finite = true;
        for buffer in 0..200 {
            // Include enabling/disabling RNNoise and its first non-silent neural frame.
            p.suppression_on = buffer < 150;
            p.eq[buffer % 8] = (buffer % 24) as f32 - 12.;
            controls.write(p);
            chain.configure(*read.read());
            for i in 0..480 {
                let sample = (i as f32 * 0.15).sin() * 0.1;
                let out = chain.process(sample);
                finite &= out.is_finite();
                let _ = producer.try_push(out);
            }
            while consumer.try_pop().is_some() {}
        }
        TRACK.with(|flag| flag.set(false));
        assert_eq!(
            ALLOCATIONS.with(|n| n.get()),
            0,
            "Callback allocated with {method:?}"
        );
        assert!(finite);
    }
}
