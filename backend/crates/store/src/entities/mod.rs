//! 实体层 — 与数据库表一一对应（sea-orm）
//!
//! 使用约定：
//! - `use crate::entities::user::{Entity as Users, Column as UserCol, Model as UserModel};`
//! - 时间列统一 `chrono::NaiveDateTime`（SQLite 存 naive 文本，UTC 语义）

pub mod background;
pub mod comment;
pub mod comment_like;
pub mod document;
pub mod document_chunk;
pub mod gallery_album;
pub mod media_image;
pub mod music_setting;
pub mod post_like;
pub mod post_stat;
pub mod talk;
pub mod user;
pub mod visit_event;

pub mod site_entry;
