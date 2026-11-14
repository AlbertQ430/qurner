//! 仅供集成测试使用的 EPUB 夹具生成器。
//!
//! 不进入 `epub-core` 公共 API，不生成或提交真实 `.epub` 文件。

#![allow(dead_code)]

pub mod fixtures;
pub mod zip_writer;
