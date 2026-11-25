/// Generates a deterministic short-hash Node ID (`n_xxxx`) from `hwnd`, `RuntimeId`, and `role`.
///
/// When `runtime_id` is empty, `fallback_key` (e.g. tree path + name) is mixed in so sibling
/// elements without a UIA RuntimeId still receive distinct IDs.
pub fn short_node_id(hwnd: isize, runtime_id: &[i32], role: &str, fallback_key: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in hwnd.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    if runtime_id.is_empty() {
        hash ^= 0xff;
        hash = hash.wrapping_mul(FNV_PRIME);
        for byte in fallback_key.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    } else {
        for part in runtime_id {
            for byte in part.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
    }

    for byte in role.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let folded = ((hash >> 32) ^ hash) as u16;
    format!("n_{folded:04x}")
}
