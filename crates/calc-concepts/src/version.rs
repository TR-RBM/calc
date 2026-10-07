const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const SEPARATOR: u8 = 0;

fn fold(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

pub fn concept_set_version(files: &[(&str, &[u8])]) -> u64 {
    let mut ordered: Vec<&(&str, &[u8])> = files.iter().collect();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    ordered
        .into_iter()
        .fold(FNV_OFFSET_BASIS, |hash, (path, bytes)| {
            let hash = fold(hash, path.as_bytes());
            let hash = fold(hash, &[SEPARATOR]);
            let hash = fold(hash, bytes);
            fold(hash, &[SEPARATOR])
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_STRING_FNV1A: u64 = 0xcbf2_9ce4_8422_2325;
    const LETTER_A_FNV1A: u64 = 0xaf63_dc4c_8601_ec8c;

    #[test]
    fn fold_matches_published_fnv1a_values() {
        assert_eq!(fold(FNV_OFFSET_BASIS, b""), EMPTY_STRING_FNV1A);
        assert_eq!(fold(FNV_OFFSET_BASIS, b"a"), LETTER_A_FNV1A);
    }

    #[test]
    fn version_does_not_depend_on_file_order() {
        let forward = [("a.md", b"x".as_slice()), ("b.md", b"y".as_slice())];
        let backward = [("b.md", b"y".as_slice()), ("a.md", b"x".as_slice())];
        assert_eq!(
            concept_set_version(&forward),
            concept_set_version(&backward)
        );
    }

    #[test]
    fn version_changes_when_content_changes() {
        let before = [("a.md", b"x".as_slice())];
        let after = [("a.md", b"z".as_slice())];
        assert_ne!(concept_set_version(&before), concept_set_version(&after));
    }

    #[test]
    fn moving_a_byte_between_path_and_content_changes_the_version() {
        let first = [("ab", b"c".as_slice())];
        let second = [("a", b"bc".as_slice())];
        assert_ne!(concept_set_version(&first), concept_set_version(&second));
    }
}
