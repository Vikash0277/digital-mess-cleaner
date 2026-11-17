//! Small formatting helpers used in answers and logs.

pub fn bytes(value: i64) -> String {
    let v = value.max(0) as f64;
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut idx = 0usize;
    let mut n = v;
    while n >= 1024.0 && idx < UNITS.len() - 1 {
        n /= 1024.0;
        idx += 1;
    }
    if idx == 0 || n >= 100.0 {
        format!("{n:.0} {}", UNITS[idx])
    } else {
        format!("{n:.1} {}", UNITS[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1024), "1.0 KB");
        assert_eq!(bytes(1024 * 1024 * 3 / 2), "1.5 MB");
    }
}
