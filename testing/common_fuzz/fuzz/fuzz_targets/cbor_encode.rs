#![no_main]
use std::fs::File;
use std::path::Path;
use std::io::{Write, Cursor};


use common_fuzz::cbor::Payload;
use fvm_ipld_encoding as encoding;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|p: Payload| {
    let out = encoding::to_vec(&p).expect("all payloads must be possible to encode");

    //let mut f = File::create(Path::new("artifacts/cbor_encode/bytes_produced_but_wont_decode.cbor")).unwrap();
    //f.write(out.as_slice()).unwrap();

    let p2 = encoding::from_reader::<Payload, _>(Cursor::new(&out)).expect("everything that encodes must decode");
    let out2 = encoding::to_vec(&p2).expect("decoded payload must be possible to encode2");
    if !out.eq(&out2) {
        panic!("repeated encodings must be stable");
    }
});
