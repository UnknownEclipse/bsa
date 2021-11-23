use super::Hash;

#[test]
fn test_hash_file_name() {
    let cases: &[(&[u8], Option<Hash>)] = &[
        (
            b"fxambblowingfog01.nif",
            Some(Hash {
                last: 49,
                last2: 176,
                len: 17,
                first: 102,
                crc: 17588009,
            }),
        ),
        (
            b"dog.dds",
            Some(Hash {
                last: 231,
                last2: 239,
                len: 3,
                first: 100,
                crc: 2379983301,
            }),
        ),
    ];

    for &(filename, hash) in cases {
        assert_eq!(Hash::from_filename(filename), hash);
    }
}

#[test]
fn test_hash_dir_name() {
    let cases: &[(&[u8], Option<Hash>)] = &[(
        b"meshes\\dungeons\\mines\\caveshaft",
        Some(Hash {
            last: 116,
            last2: 102,
            len: 31,
            first: 109,
            crc: 743299860,
        }),
    )];

    for &(filename, hash) in cases {
        assert_eq!(Hash::from_dirname(filename), hash);
    }
}
