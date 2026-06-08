#![no_main]
use libfuzzer_sys::fuzz_target;
use omni::pipeline::collapse::{collapse, CollapseMode};

fuzz_target!(|data: &str| {
    // Fuzz the collapse pipeline with random UTF-8 strings
    // If there is any hidden byte-slicing panic, this will find it.
    let _ = collapse(data, &CollapseMode::Generic);
    let _ = collapse(data, &CollapseMode::Build);
});
