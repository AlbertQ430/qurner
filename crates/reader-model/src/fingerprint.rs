//! SHA-256 内容指纹值类型。
//!
//! 指纹用于按原始文件字节识别重复书籍（见 SRS FR-016）。本模块只提供值类型，
//! 文件流式计算在 TASK-M1D 实现。

use std::fmt;
use std::str::FromStr;

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// SHA-256 摘要的字节数。
pub const SHA256_BYTES: usize = 32;

/// SHA-256 摘要的小写十六进制字符数。
pub const SHA256_HEX_LEN: usize = SHA256_BYTES * 2;

/// 稳定的书籍内容指纹，保存 32 字节 SHA-256 摘要。
///
/// 文本表示始终为小写 64 字符十六进制字符串；解析时接受大小写。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BookFingerprint([u8; SHA256_BYTES]);

/// 指纹文本解析错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FingerprintError {
    /// 文本长度不是 64 个字符。
    #[error("内容指纹必须是 64 个十六进制字符，实际为 {0} 个字符")]
    InvalidLength(usize),
    /// 文本包含非十六进制字符。
    #[error("内容指纹包含非十六进制字符")]
    InvalidHex,
}

impl BookFingerprint {
    /// 由 SHA-256 摘要字节构造指纹。
    #[must_use]
    pub const fn from_sha256(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }

    /// 返回原始 32 字节摘要。
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// 返回小写 64 字符十六进制字符串。
    #[must_use]
    pub fn to_hex(self) -> String {
        use std::fmt::Write as _;

        let mut output = String::with_capacity(SHA256_HEX_LEN);
        for byte in self.0 {
            let _ = write!(output, "{byte:02x}");
        }
        output
    }
}

impl FromStr for BookFingerprint {
    type Err = FingerprintError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != SHA256_HEX_LEN {
            return Err(FingerprintError::InvalidLength(value.len()));
        }

        let mut bytes = [0_u8; SHA256_BYTES];
        for (index, chunk) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let high = hex_digit(chunk[0]).ok_or(FingerprintError::InvalidHex)?;
            let low = hex_digit(chunk[1]).ok_or(FingerprintError::InvalidHex)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for BookFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for BookFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BookFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

impl Serialize for BookFingerprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for BookFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FingerprintVisitor;

        impl Visitor<'_> for FingerprintVisitor {
            type Value = BookFingerprint;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("一个 64 字符的 SHA-256 十六进制字符串")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(FingerprintVisitor)
    }
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn fingerprint_abc() -> BookFingerprint {
        SHA256_ABC.parse().expect("固定 SHA-256 文本应可解析")
    }

    #[test]
    fn display_and_parse_round_trip() {
        assert_eq!(fingerprint_abc().to_string(), SHA256_ABC);
        assert_eq!(fingerprint_abc().to_hex(), SHA256_ABC);
        assert_eq!(SHA256_ABC.parse::<BookFingerprint>(), Ok(fingerprint_abc()));
    }

    #[test]
    fn parse_accepts_uppercase_hex() {
        let uppercase = SHA256_ABC.to_uppercase();
        assert_eq!(uppercase.parse::<BookFingerprint>(), Ok(fingerprint_abc()));
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert_eq!(
            "abc".parse::<BookFingerprint>(),
            Err(FingerprintError::InvalidLength(3))
        );
        assert_eq!(
            "z".repeat(SHA256_HEX_LEN).parse::<BookFingerprint>(),
            Err(FingerprintError::InvalidHex)
        );
    }

    #[test]
    fn serializes_as_lowercase_hex_string() {
        let json = serde_json::to_string(&fingerprint_abc()).expect("序列化应成功");
        assert_eq!(json, format!("\"{SHA256_ABC}\""));

        let restored: BookFingerprint = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(restored, fingerprint_abc());
    }

    #[test]
    fn deserialization_rejects_invalid_values() {
        assert!(serde_json::from_str::<BookFingerprint>("\"not-a-fingerprint\"").is_err());
        assert!(serde_json::from_str::<BookFingerprint>("123").is_err());
    }
}
