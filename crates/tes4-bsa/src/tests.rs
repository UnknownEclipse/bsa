pub mod hash {
    use crate::hash::{hash_directory_name, hash_file_name, Hash};

    #[test]
    pub fn test_hash_file_name() {
        let cases: &[(&str, Option<Hash>)] = &[
            (
                "fxambblowingfog01.nif",
                Some(Hash {
                    last: 49,
                    last2: 176,
                    len: 17,
                    first: 102,
                    crc: 17588009,
                }),
            ),
            (
                "dog.dds",
                Some(Hash {
                    last: 231,
                    last2: 239,
                    len: 3,
                    first: 100,
                    crc: 2379983301,
                }),
            ),
        ];

        for &(file_name, hash) in cases {
            assert_eq!(hash_file_name(file_name), hash);
        }
    }

    #[test]
    pub fn test_hash_directory_name() {
        let cases: &[(&str, Option<Hash>)] = &[
            (
                "meshes/dungeons/mines/caveshaft",
                Some(Hash {
                    last: 116,
                    last2: 102,
                    len: 31,
                    first: 109,
                    crc: 743299860,
                }),
            ),
            (
                "meshes\\dungeons\\mines\\caveshaft",
                Some(Hash {
                    last: 116,
                    last2: 102,
                    len: 31,
                    first: 109,
                    crc: 743299860,
                }),
            ),
            (
                "meshes/DUNGEONS\\mines\\CAVEshaft",
                Some(Hash {
                    last: 116,
                    last2: 102,
                    len: 31,
                    first: 109,
                    crc: 743299860,
                }),
            ),
            (
                "meshes/DUNGEONS\\\\\\mines\\CAVEshaft/",
                Some(Hash {
                    last: 116,
                    last2: 102,
                    len: 31,
                    first: 109,
                    crc: 743299860,
                }),
            ),
            ("meshes/../dungeons/caveshaft", None),
            ("/meshes/", None),
            ("meshes/🚀", None),
            ("meshes/./caves", None),
            ("", None),
            (
                &(0..50)
                    .map(|_| "this/is/a/name/that/exceeds/maximum/path/length/limitations/")
                    .collect::<String>(),
                None,
            ),
        ];

        for &(dir_name, hash) in cases {
            dbg!(dir_name);
            dbg!(hash);
            assert_eq!(hash_directory_name(dir_name), hash);
        }
    }
}
