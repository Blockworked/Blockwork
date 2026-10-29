//! Android touch vocabulary uses physical pixels on the default display.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchAction {
    Tap,
    LongPress,
    Swipe,
    Back,
    Home,
    Recents,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gesture {
    pub action: TouchAction,
    pub x: f64,
    pub y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub duration_ms: u64,
}
