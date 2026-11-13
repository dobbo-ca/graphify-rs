use super::*;

// New: FIPS 180-4 example digests.
#[test]
#[ignore = "graphify-rs-u2o.3"]
fn sha256_fips_vectors() {
    let million_a = vec![b'a'; 1_000_000];
    let cases: [(&str, &[u8], &str); 4] = [
        (
            "empty",
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "448-bit",
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            "million a",
            &million_a,
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ];
    for (name, msg, want) in cases {
        assert_eq!(hex(&sha256(msg)), want, "case {name}");
    }
}

// New: RFC 4231 section 4 HMAC-SHA-256 test cases 1 to 7.
#[test]
#[ignore = "graphify-rs-u2o.3"]
fn hmac_rfc4231_cases_1_to_7() {
    let key4: Vec<u8> = (0x01..=0x19).collect();
    let cases: [(&str, Vec<u8>, Vec<u8>, &str); 7] = [
        (
            "case 1",
            vec![0x0b; 20],
            b"Hi There".to_vec(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        ),
        (
            "case 2",
            b"Jefe".to_vec(),
            b"what do ya want for nothing?".to_vec(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        ),
        (
            "case 3",
            vec![0xaa; 20],
            vec![0xdd; 50],
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe",
        ),
        (
            "case 4",
            key4,
            vec![0xcd; 50],
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b",
        ),
        (
            // Truncated to 128 bits: only the first 16 bytes are compared.
            "case 5",
            vec![0x0c; 20],
            b"Test With Truncation".to_vec(),
            "a3b6167473100ee06e0c796c2955552b",
        ),
        (
            "case 6",
            vec![0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        ),
        (
            "case 7",
            vec![0xaa; 131],
            b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.".to_vec(),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2",
        ),
    ];
    for (name, key, data, want) in cases {
        let mac = hmac_sha256(&key, &data);
        let got = hex(&mac[..want.len() / 2]);
        assert_eq!(got, want, "{name}");
    }
}

// New: lowercase hex encoding.
#[test]
#[ignore = "graphify-rs-u2o.3"]
fn hex_lowercase() {
    assert_eq!(hex(&[]), "", "empty");
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff", "mixed bytes");
}
