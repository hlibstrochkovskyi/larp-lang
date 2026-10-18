#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| larp_fuzz_support::formatter(data));
