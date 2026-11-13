//! Ported from crypto/sha256/sha256_test.go (go1.27.4). Go licence: BSD-3-Clause,
//! Copyright 2009 The Go Authors.
//! sha256_test.go has 13 Test funcs (6 ported, 7 not ported) plus 1 added (`fips_180_4_vectors`):
//! - TestGolden: ported (SHA-256 rows only; SHA-224 rows skipped)
//! - TestGoldenMarshal: not ported: marshal/unmarshal not reached by graphify-go
//! - TestLarge: ported
//! - TestExtraLarge: ported
//! - TestMarshalTypeMismatch: not ported: marshal/unmarshal not reached by graphify-go
//! - TestSize: ported (SHA-224 half skipped)
//! - TestBlockSize: ported
//! - TestLargeHashes: ported (the Go test unmarshals a saved state; `Digest` has no unmarshal, so
//!   the saved h/tail/len are finalized with `block_with(Tier::Scalar, ..)` and the FIPS padding)
//! - TestAllocations: not ported: Go allocation counts
//! - TestCgo: not ported: cgo
//! - TestHash: not ported: `cryptotest.TestHash` hash.Hash interface helper
//! - TestExtraMethods: not ported: `cryptotest.NoExtraMethods`: Go method-set check
//! - TestAllocatonsWithTypeAsserts: not ported: Go allocation counts

use gostd::sha256::{BLOCK_SIZE, Digest, SIZE, Tier, block_with, sum256};
use std::fmt::Write as _;

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        write!(s, "{x:02x}").unwrap();
        s
    })
}

/// (digest, input) rows of Go's `golden`.
const GOLDEN: &[(&str, &str)] = &[
    (
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "",
    ),
    (
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
        "a",
    ),
    (
        "fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603",
        "ab",
    ),
    (
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "abc",
    ),
    (
        "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589",
        "abcd",
    ),
    (
        "36bbe50ed96841d10443bcb670d6554f0a34b761be67ec9c4a8ad2c0c44ca42c",
        "abcde",
    ),
    (
        "bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721",
        "abcdef",
    ),
    (
        "7d1a54127b222502f5b79b5fb0803061152a44f92b37e23c6527baf665d4da9a",
        "abcdefg",
    ),
    (
        "9c56cc51b374c3ba189210d5b6d4bf57790d351c96c47c02190ecf1e430635ab",
        "abcdefgh",
    ),
    (
        "19cc02f26df43cc571bc9ed7b0c4d29224a3ec229529221725ef76d021c8326f",
        "abcdefghi",
    ),
    (
        "72399361da6a7754fec986dca5b7cbaf1c810a28ded4abaf56b2106d06cb78b0",
        "abcdefghij",
    ),
    (
        "a144061c271f152da4d151034508fed1c138b8c976339de229c3bb6d4bbb4fce",
        "Discard medicine more than two years old.",
    ),
    (
        "6dae5caa713a10ad04b46028bf6dad68837c581616a1589a265a11288d4bb5c4",
        "He who has a shady past knows that nice guys finish last.",
    ),
    (
        "ae7a702a9509039ddbf29f0765e70d0001177914b86459284dab8b348c2dce3f",
        "I wouldn't marry him with a ten foot pole.",
    ),
    (
        "6748450b01c568586715291dfa3ee018da07d36bb7ea6f180c1af6270215c64f",
        "Free! Free!/A trip/to Mars/for 900/empty jars/Burma Shave",
    ),
    (
        "14b82014ad2b11f661b5ae6a99b75105c2ffac278cd071cd6c05832793635774",
        "The days of the digital watch are numbered.  -Tom Stoppard",
    ),
    (
        "7102cfd76e2e324889eece5d6c41921b1e142a4ac5a2692be78803097f6a48d8",
        "Nepal premier won't resign.",
    ),
    (
        "23b1018cd81db1d67983c5f7417c44da9deb582459e378d7a068552ea649dc9f",
        "For every action there is an equal and opposite government program.",
    ),
    (
        "8001f190dfb527261c4cfcab70c98e8097a7a1922129bc4096950e57c7999a5a",
        "His money is twice tainted: 'taint yours and 'taint mine.",
    ),
    (
        "8c87deb65505c3993eb24b7a150c4155e82eee6960cf0c3a8114ff736d69cad5",
        "There is no reason for any individual to have a computer in their home. -Ken Olsen, 1977",
    ),
    (
        "bfb0a67a19cdec3646498b2e0f751bddc41bba4b7f30081b0b932aad214d16d7",
        "It's a tiny change to the code and not completely disgusting. - Bob Manchek",
    ),
    (
        "7f9a0b9bf56332e19f5a0ec1ad9c1425a153da1c624868fda44561d6b74daf36",
        "size:  a.out:  bad magic",
    ),
    (
        "b13f81b8aad9e3666879af19886140904f7f429ef083286195982a7588858cfc",
        "The major problem is with sendmail.  -Mark Horton",
    ),
    (
        "b26c38d61519e894480c70c8374ea35aa0ad05b2ae3d6674eec5f52a69305ed4",
        "Give me a rock, paper and scissors and I will move the world.  CCFestoon",
    ),
    (
        "049d5e26d4f10222cd841a119e38bd8d2e0d1129728688449575d4ff42b842c1",
        "If the enemy is within range, then so are you.",
    ),
    (
        "0e116838e3cc1c1a14cd045397e29b4d087aa11b0853fc69ec82e90330d60949",
        "It's well we cannot hear the screams/That we create in others' dreams.",
    ),
    (
        "4f7d8eb5bcf11de2a56b971021a444aa4eafd6ecd0f307b5109e4e776cd0fe46",
        "You remind me of a TV show, but that's all right: I watch it anyway.",
    ),
    (
        "61c0cc4c4bd8406d5120b3fb4ebc31ce87667c162f29468b3c779675a85aebce",
        "C is as portable as Stonehedge!!",
    ),
    (
        "1fb2eb3688093c4a3f80cd87a5547e2ce940a4f923243a79a2a1e242220693ac",
        "Even if I could be Shakespeare, I think I should still choose to be Faraday. - A. Huxley",
    ),
    (
        "395585ce30617b62c80b93e8208ce866d4edc811a177fdb4b82d3911d8696423",
        "The fugacity of a constituent in a mixture of gases at a given temperature is proportional to its mole fraction.  Lewis-Randall Rule",
    ),
    (
        "4f9b189a13d030838269dce846b16a1ce9ce81fe63e65de2f636863336a98fe6",
        "How can you write a big system without C++?  -Paul Glick",
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_golden() {
    for &(out, input) in GOLDEN {
        let got = hex(&sum256(input.as_bytes()));
        assert_eq!(got, out, "Sum256 function: sha256({input})");
        let mut c = Digest::new();
        for j in 0..3 {
            if j < 2 {
                c.write(input.as_bytes());
            } else {
                let (a, b) = input.as_bytes().split_at(input.len() / 2);
                c.write(a);
                c.sum();
                c.write(b);
            }
            assert_eq!(hex(&c.sum()), out, "sha256[{j}]({input})");
            c.reset();
        }
    }
}

/// Go `TestLarge` and `TestExtraLarge` share this body.
fn large(n: usize, ok: &str) {
    const OFFSETS: usize = 4;
    let mut block = vec![0u8; n + OFFSETS];
    let mut c = Digest::new();
    for offset in 0..OFFSETS {
        for i in 0..n {
            block[offset + i] = b'0' + (i % 10) as u8;
        }
        let mut block_size = 10;
        while block_size <= n {
            let blocks = n / block_size;
            let b = &block[offset..offset + block_size];
            c.reset();
            for _ in 0..blocks {
                c.write(b);
            }
            assert_eq!(hex(&c.sum()), ok, "offset={offset}, blockSize={block_size}");
            block_size *= 10;
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_large() {
    // sha256sum of "0123456789" * 1000
    large(
        10000,
        "4c207598af7a20db0e3334dd044399a40e467cb81b37f7ba05a4f76dcbd8fd59",
    );
}

#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_extra_large() {
    // sha256sum of "0123456789" * 10000
    large(
        100000,
        "aca9e593cc629cbaa94cd5a07dc029424aad93e5129e5d11f8dcd2f139c16cc0",
    );
}

#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_size() {
    assert_eq!(SIZE, 32);
    assert_eq!(Digest::new().sum().len(), SIZE);
}

#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_block_size() {
    assert_eq!(BLOCK_SIZE, 64);
}

/// Saved states of Go's `largeUnmarshalTests` (h words, buffered tail `x[..len % 64]`, byte
/// count, expected sum). Lengths above 2^32 exercise the 32nd-bit problem of golang/go#29517.
#[test]
#[ignore = "graphify-rs-aa6.18"]
fn test_large_hashes() {
    let tail39 = [0u8; 39];
    let seq39: Vec<u8> = (0..39).collect();
    let tests: [([u32; 8], &[u8], u64, &str); 3] = [
        (
            [
                0x7958afb7, 0x042a8faa, 0x9b78c523, 0x1feb94fd, 0x7a31affb, 0x6bd6970a, 0xc933cf02,
                0x0b2ea5e4,
            ],
            &tail39[..],
            7_115_087_207,
            "f5e06371f0c115e9968455c8e48a318aba548b9f15676fa41de123f7d1c99c55",
        ),
        (
            [
                0x24933375, 0x0a560be2, 0xf73a3021, 0xcab3a413, 0xd32036dc, 0x4242b519, 0xcd3dc168,
                0xee3db49c,
            ],
            &b"@ABCDE"[..],
            7_070_038_086,
            "a280b08df5eba060fcd0eb3d29320bbc038afb95781661f91bbfd0a6fc9fdd6e",
        ),
        (
            [
                0x9f128747, 0xf2df3c82, 0xa0112f2a, 0x57022649, 0x4b576c68, 0x0395b1ab, 0x0c0af65a,
                0x65f91d1b,
            ],
            &seq39[..],
            6_464_878_887,
            "d2fffb762f105ab71e2d70069346c44c38c4fe183aad8cfcf5a76397c0457806",
        ),
    ];
    for (i, (mut h, tail, len, want)) in tests.into_iter().enumerate() {
        assert_eq!(tail.len() as u64, len % 64);
        let mut m = tail.to_vec();
        m.push(0x80);
        while m.len() % 64 != 56 {
            m.push(0);
        }
        m.extend_from_slice(&(len << 3).to_be_bytes());
        assert!(block_with(Tier::Scalar, &mut h, &m));
        let sum: Vec<u8> = h.iter().flat_map(|w| w.to_be_bytes()).collect();
        assert_eq!(hex(&sum), want, "test {i} sum mismatch");
    }
}

/// Added: FIPS 180-4 example vectors.
#[test]
#[ignore = "graphify-rs-aa6.18"]
fn fips_180_4_vectors() {
    let million_a = vec![b'a'; 1_000_000];
    let cases: [(&[u8], &str); 4] = [
        (
            &b""[..],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            &b"abc"[..],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            &b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"[..],
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            &million_a[..],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ];
    for (input, want) in cases {
        assert_eq!(hex(&sum256(input)), want);
        let mut d = Digest::new();
        d.write(input);
        assert_eq!(hex(&d.sum()), want);
    }
}
