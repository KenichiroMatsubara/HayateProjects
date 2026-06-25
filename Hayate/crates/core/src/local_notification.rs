//! local notification capability 契約（ADR-0119）。モデル: `flutter_local_notifications`
//! （`show(id, title, body, ...)` / `cancel(id)` / `cancelAll()`）。権限は別途（ADR-0119）。

use crate::capability::CapabilityError;

/// 表示するローカル通知（common 部分集合）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNotification {
    pub id: i32,
    pub title: String,
    pub body: String,
}

/// ローカル通知の表示・取り消し。
pub trait LocalNotifications {
    fn show(&mut self, notification: LocalNotification) -> Result<(), CapabilityError>;
    fn cancel(&mut self, id: i32) -> Result<(), CapabilityError>;
    fn cancel_all(&mut self) -> Result<(), CapabilityError>;
}
