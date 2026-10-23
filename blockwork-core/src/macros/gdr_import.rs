//! Imports Geometry Dash replays in the GDR format (msgpack, or the
//! `.gdr.json` variant, as written by zBot, xdBot and others; binary GDR2 included) as a Blockwork macro: each click becomes a `Wait` plus a
//! mouse press/release, timed in whole physics steps for a frame-locked host.

use super::{Instruction, InstructionKind, Macro};
use crate::input::types::{Direction, InputToken, MacroButton};
use crate::input::value::Value;

/// The msgpack subset GDR uses.
#[derive(Debug, Clone, PartialEq)]
enum Mp {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bin,
    Array(Vec<Mp>),
    Map(Vec<(String, Mp)>),
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    depth: usize,
}

/// Deeper nesting than any replay has; past it the file is malformed, and
/// recursing on would overflow the stack.
const MAX_DEPTH: usize = 64;

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or("unexpected end of file")?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn uint(&mut self, n: usize) -> Result<u64, String> {
        Ok(self.take(n)?.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)))
    }

    fn str(&mut self, n: usize) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn array(&mut self, n: usize) -> Result<Mp, String> {
        self.nested(|r| (0..n).map(|_| r.value()).collect::<Result<_, _>>().map(Mp::Array))
    }

    fn map(&mut self, n: usize) -> Result<Mp, String> {
        self.nested(|r| r.entries(n))
    }

    fn nested(&mut self, f: impl FnOnce(&mut Self) -> Result<Mp, String>) -> Result<Mp, String> {
        if self.depth >= MAX_DEPTH {
            return Err("the file is nested too deeply".into());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    fn entries(&mut self, n: usize) -> Result<Mp, String> {
        // The count comes from the file: every entry takes at least two
        // bytes, so a bigger count can't be real and mustn't size the Vec.
        let mut entries = Vec::with_capacity(n.min((self.data.len() - self.pos) / 2));
        for _ in 0..n {
            let key = match self.value()? {
                Mp::Str(s) => s,
                _ => return Err("non-string map key".into()),
            };
            entries.push((key, self.value()?));
        }
        Ok(Mp::Map(entries))
    }

    fn value(&mut self) -> Result<Mp, String> {
        let tag = self.take(1)?[0];
        Ok(match tag {
            0x00..=0x7f => Mp::Int(i64::from(tag)),
            0x80..=0x8f => return self.map(usize::from(tag & 0x0f)),
            0x90..=0x9f => return self.array(usize::from(tag & 0x0f)),
            0xa0..=0xbf => Mp::Str(self.str(usize::from(tag & 0x1f))?),
            0xc0 => Mp::Nil,
            0xc2 => Mp::Bool(false),
            0xc3 => Mp::Bool(true),
            0xc4 | 0xc5 | 0xc6 => {
                let n = self.uint(1 << (tag - 0xc4))? as usize;
                self.take(n)?;
                Mp::Bin
            }
            0xca => Mp::Float(f64::from(f32::from_bits(self.uint(4)? as u32))),
            0xcb => Mp::Float(f64::from_bits(self.uint(8)?)),
            0xcc..=0xcf => Mp::Int(self.uint(1 << (tag - 0xcc))? as i64),
            0xd0 => Mp::Int(i64::from(self.uint(1)? as u8 as i8)),
            0xd1 => Mp::Int(i64::from(self.uint(2)? as u16 as i16)),
            0xd2 => Mp::Int(i64::from(self.uint(4)? as u32 as i32)),
            0xd3 => Mp::Int(self.uint(8)? as i64),
            0xd9 | 0xda | 0xdb => {
                let n = self.uint(1 << (tag - 0xd9))? as usize;
                Mp::Str(self.str(n)?)
            }
            0xdc => {
                let n = self.uint(2)? as usize;
                return self.array(n);
            }
            0xdd => {
                let n = self.uint(4)? as usize;
                return self.array(n);
            }
            0xde => {
                let n = self.uint(2)? as usize;
                return self.map(n);
            }
            0xdf => {
                let n = self.uint(4)? as usize;
                return self.map(n);
            }
            0xe0..=0xff => Mp::Int(i64::from(tag as i8)),
            other => return Err(format!("unsupported msgpack tag 0x{other:02x}")),
        })
    }
}

impl Mp {
    fn get(&self, key: &str) -> Option<&Mp> {
        match self {
            Mp::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// First present key, for fields different bots name differently.
    fn get_any(&self, keys: &[&str]) -> Option<&Mp> {
        keys.iter().find_map(|k| self.get(k))
    }

    fn from_json(value: &serde_json::Value) -> Mp {
        use serde_json::Value as J;
        match value {
            J::Null => Mp::Nil,
            J::Bool(b) => Mp::Bool(*b),
            J::Number(n) => n.as_i64().map(Mp::Int).unwrap_or_else(|| Mp::Float(n.as_f64().unwrap_or(0.0))),
            J::String(s) => Mp::Str(s.clone()),
            J::Array(a) => Mp::Array(a.iter().map(Mp::from_json).collect()),
            J::Object(o) => Mp::Map(o.iter().map(|(k, v)| (k.clone(), Mp::from_json(v))).collect()),
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            Mp::Int(i) => Some(*i as f64),
            Mp::Float(f) => Some(*f),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Mp::Int(i) => Some(*i),
            Mp::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
}

/// One recorded click, whatever the container format.
struct Click {
    /// The recorder logged an extra update at this input's frame (xdBot).
    extra_update: bool,
    frame: i64,
    down: bool,
    player2: bool,
    button: i64,
}

struct Replay {
    fps: f64,
    level: Option<String>,
    clicks: Vec<Click>,
}

/// xdBot stores the recorded player state of every frame in its extension:
/// a record count, then per frame `[frame varint][x][y][f32][01][12 bytes][01]`
/// (big-endian floats). A frame whose x didn't move from the previous one
/// was counted by the recorder without the game taking a physics step, so
/// playback keyed to real steps must skip it. Returns those frame numbers
/// in ascending order (empty if the data isn't in the expected shape).
fn xdbot_stalled_frames(ext: &[u8]) -> Vec<i64> {
    fn varint(data: &[u8], pos: &mut usize) -> Option<u64> {
        let mut value = 0u64;
        for i in 0..10 {
            let byte = *data.get(*pos)?;
            *pos += 1;
            value |= u64::from(byte & 0x7f) << (7 * i);
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    let mut pos = 0;
    let Some(count) = varint(ext, &mut pos) else { return Vec::new() };
    let mut stalled = Vec::new();
    let mut prev_x: Option<u32> = None;
    let mut expected = 1u64;
    for _ in 0..count {
        let Some(frame) = varint(ext, &mut pos) else { break };
        let Some(state) = ext.get(pos..pos + 26) else { break };
        // Frames must run 1, 2, 3...; anything else isn't the layout above.
        if frame != expected {
            return Vec::new();
        }
        let x = u32::from_be_bytes([state[0], state[1], state[2], state[3]]);
        if prev_x == Some(x) {
            stalled.push(frame as i64);
        }
        prev_x = Some(x);
        pos += 26;
        expected += 1;
    }
    stalled
}

#[cfg(test)]
pub(crate) fn stalled_for_test(ext: &[u8]) -> Vec<i64> {
    xdbot_stalled_frames(ext)
}

/// Binary GDR2 (`GDR` + version 2): LEB128 varints, length-prefixed strings,
/// big-endian floats, then delta-packed inputs (player 1 first, then 2).
fn parse_binary(bytes: &[u8]) -> Result<Replay, String> {
    struct Bin<'a> {
        data: &'a [u8],
        pos: usize,
    }
    impl Bin<'_> {
        fn take(&mut self, n: usize) -> Result<&[u8], String> {
            let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or("unexpected end of file")?;
            let slice = &self.data[self.pos..end];
            self.pos = end;
            Ok(slice)
        }
        fn varint(&mut self) -> Result<u64, String> {
            let mut value = 0u64;
            for i in 0..10 {
                let byte = self.take(1)?[0];
                value |= u64::from(byte & 0x7f) << (7 * i);
                if byte & 0x80 == 0 {
                    return Ok(value);
                }
            }
            Err("varint too long".into())
        }
        fn string(&mut self) -> Result<String, String> {
            let n = self.varint()? as usize;
            Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
        }
    }

    let mut r = Bin { data: bytes, pos: 3 };
    let version = r.varint()?;
    if version != 2 {
        return Err(format!("unsupported GDR version {version}"));
    }
    let input_tag = r.string()?;
    r.string()?; // author
    r.string()?; // description
    r.take(4)?; // duration
    r.varint()?; // game version
    let fps = f64::from_bits(u64::from_be_bytes(r.take(8)?.try_into().map_err(|_| "bad framerate")?));
    r.varint()?; // seed
    r.varint()?; // coins
    r.varint()?; // ldm
    let platformer = r.varint()? != 0;
    let bot_name = r.string()?;
    r.varint()?; // bot version
    r.varint()?; // level id
    let level = r.string()?;
    let ext = r.varint()? as usize;
    let ext_data = r.take(ext)?;
    let stalled = if bot_name.eq_ignore_ascii_case("xdBot") { xdbot_stalled_frames(ext_data) } else { Vec::new() };
    let deaths = r.varint()?;
    for _ in 0..deaths {
        r.varint()?;
    }
    r.varint()?; // input count
    let mut p1_left = r.varint()?;

    let mut clicks = Vec::new();
    let mut prev = 0u64;
    while r.pos < r.data.len() {
        let packed = r.varint()?;
        let (delta, button, down) = if platformer {
            (packed >> 3, ((packed >> 1) & 3) as i64, packed & 1 == 1)
        } else {
            (packed >> 1, 1, packed & 1 == 1)
        };
        let frame = prev + delta;
        prev = frame;
        clicks.push(Click { extra_update: false, frame: frame as i64, down, player2: p1_left == 0, button });
        if !input_tag.is_empty() {
            let n = r.varint()? as usize;
            r.take(n)?;
        }
        if p1_left > 0 {
            p1_left -= 1;
            if p1_left == 0 {
                prev = 0;
            }
        }
    }
    // Player 2's inputs follow player 1's; interleave them by frame.
    clicks.sort_by_key(|c| c.frame);
    // Frame numbers count stalled frames that never became a physics step.
    let mut last_extra_frame = None;
    for click in &mut clicks {
        // One extra update per stalled frame, however many inputs it holds.
        click.extra_update = stalled.binary_search(&click.frame).is_ok() && last_extra_frame != Some(click.frame);
        if click.extra_update {
            last_extra_frame = Some(click.frame);
        }
        click.frame -= stalled.partition_point(|f| *f <= click.frame) as i64;
    }
    Ok(Replay { fps, level: Some(level).filter(|l| !l.is_empty()), clicks })
}

/// Msgpack (GDR1 / zBot style) or `.gdr.json`.
fn parse_structured(bytes: &[u8]) -> Result<Replay, String> {
    let root = if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        Mp::from_json(&serde_json::from_slice(bytes).map_err(|e| format!("invalid JSON replay: {e}"))?)
    } else {
        Reader { data: bytes, pos: 0, depth: 0 }.value()?
    };
    let inputs = match root.get("inputs") {
        Some(Mp::Array(inputs)) => inputs,
        _ => return Err("not a GDR replay (no inputs)".into()),
    };
    let fps = root.get("framerate").and_then(Mp::as_f64).filter(|f| *f > 0.0).unwrap_or(240.0);
    let mut clicks = Vec::with_capacity(inputs.len());
    for input in inputs {
        clicks.push(Click {
            extra_update: false,
            frame: input.get("frame").and_then(Mp::as_i64).ok_or("input without a frame")?,
            down: matches!(input.get("down"), Some(Mp::Bool(true))),
            player2: matches!(input.get_any(&["2p", "player2"]), Some(Mp::Bool(true))),
            button: input.get_any(&["btn", "button"]).and_then(Mp::as_i64).unwrap_or(1),
        });
    }
    let level = match root.get_any(&["level", "levelInfo"]).and_then(|l| l.get("name")) {
        Some(Mp::Str(name)) if !name.is_empty() => Some(name.clone()),
        _ => None,
    };
    Ok(Replay { fps, level, clicks })
}

/// Turns inputs recorded against a host's own clock into macro code:
/// `(time_ms, secondary, down)` per input, in order. Same shape the GDR
/// import produces, so recorded and imported macros play back identically.
pub fn step_inputs_to_instructions(inputs: &[(f64, bool, bool)]) -> Vec<Instruction> {
    let mut code = Vec::with_capacity(inputs.len() * 2);
    let mut last = 0.0f64;
    for &(time_ms, secondary, down) in inputs {
        if time_ms > last + 1e-9 {
            code.push(Instruction::new(InstructionKind::Wait(Value::number(time_ms - last))));
            last = time_ms;
        }
        let button = if secondary { MacroButton::Right } else { MacroButton::Left };
        let direction = if down { Direction::Press } else { Direction::Release };
        code.push(Instruction::new(InstructionKind::Token(InputToken::Button(button, direction))));
    }
    code
}

/// Builds a macro from GDR bytes (binary GDR2, msgpack, or JSON).
/// `fallback_name` names it when the replay carries no level name.
pub fn import_gdr(bytes: &[u8], fallback_name: &str) -> Result<Macro, String> {
    let replay = if bytes.starts_with(b"GDR") { parse_binary(bytes)? } else { parse_structured(bytes)? };
    let ms_per_frame = 1000.0 / replay.fps;

    // The player with the most clicks is the main one (left button). Some
    // bots flag a single-player run as player 2, which a one-player level
    // would never act on.
    let p2_clicks = replay.clicks.iter().filter(|c| c.player2 && c.button == 1).count();
    let p1_clicks = replay.clicks.iter().filter(|c| !c.player2 && c.button == 1).count();
    let swap_players = p2_clicks > p1_clicks;

    let mut code = Vec::with_capacity(replay.clicks.len() * 2);
    let mut last_frame = 0i64;
    for click in &replay.clicks {
        // Only the jump button (1); platformer left/right have no click equivalent.
        if click.button != 1 {
            continue;
        }
        // The secondary player is the right mouse button, the main one the left.
        let secondary = click.player2 != swap_players;
        // Clicks that carried an extra update use the Middle/Back buttons.
        let button = match (secondary, click.extra_update) {
            (false, false) => MacroButton::Left,
            (true, false) => MacroButton::Right,
            (false, true) => MacroButton::Middle,
            (true, true) => MacroButton::Back,
        };
        if click.frame > last_frame {
            let wait = (click.frame - last_frame) as f64 * ms_per_frame;
            code.push(Instruction::new(InstructionKind::Wait(Value::number(wait))));
            last_frame = click.frame;
        }
        let direction = if click.down { Direction::Press } else { Direction::Release };
        code.push(Instruction::new(InstructionKind::Token(InputToken::Button(button, direction))));
    }
    if code.is_empty() {
        return Err("replay has no jump inputs".into());
    }
    let level = replay.level.unwrap_or_else(|| fallback_name.to_string());
    let description = format!("Imported GDR replay ({} clicks, {} fps)", replay.clicks.len(), replay.fps);
    Ok(Macro::new(format!("{level} (frame-locked)"), description, code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_clicks_as_step_timed_waits() {
        // {"framerate": 240.0f32, "inputs": [{"btn":1,"down":true,"frame":24,"2p":false}]}
        let mut data = vec![0x82, 0xa9];
        data.extend_from_slice(b"framerate");
        data.push(0xca);
        data.extend_from_slice(&240.0f32.to_be_bytes());
        data.push(0xa6);
        data.extend_from_slice(b"inputs");
        data.extend_from_slice(&[0x91, 0x83, 0xa5]);
        data.extend_from_slice(b"frame");
        data.extend_from_slice(&[24, 0xa4]);
        data.extend_from_slice(b"down");
        data.extend_from_slice(&[0xc3, 0xa2]);
        data.extend_from_slice(b"2p");
        data.extend_from_slice(&[0xc2]);
        let mac = import_gdr(&data, "Test").unwrap();
        let strand = &mac.graph.strands[0].instructions;
        assert_eq!(strand.len(), 3);
        assert!(matches!(&strand[1].kind, InstructionKind::Wait(_)));
        assert!(matches!(
            &strand[2].kind,
            InstructionKind::Token(InputToken::Button(MacroButton::Left, Direction::Press))
        ));
    }

    #[test]
    fn imports_json_variant_with_alias_keys() {
        let json = br#"{"framerate":240,"inputs":[{"frame":10,"button":1,"player2":true,"down":true},{"frame":12,"button":2,"down":true}]}"#;
        let mac = import_gdr(json, "Json").unwrap();
        let strand = &mac.graph.strands[0].instructions;
        assert_eq!(strand.len(), 3, "the non-jump button is skipped");
        // A lone player 2 is treated as the main player.
        assert!(matches!(
            &strand[2].kind,
            InstructionKind::Token(InputToken::Button(MacroButton::Left, Direction::Press))
        ));
    }

    #[test]
    fn imports_binary_gdr2_with_two_players() {
        // GDR v2, no input tag, empty strings, 240 fps, 2 inputs for p1 (down@10, up@20),
        // 1 for p2 (down@15). Varints: delta<<1 | down.
        let mut d = b"GDR".to_vec();
        d.extend_from_slice(&[2, 0, 0, 0]); // version, tag, author, description
        d.extend_from_slice(&1.0f32.to_be_bytes());
        d.push(0); // game version
        d.extend_from_slice(&240.0f64.to_be_bytes());
        d.extend_from_slice(&[0, 0, 0, 0]); // seed, coins, ldm, platformer
        d.extend_from_slice(&[0, 0, 0, 0]); // bot name, bot version, level id, level name
        d.extend_from_slice(&[0, 0]); // extension size, deaths
        d.extend_from_slice(&[3, 2]); // input count, p1 count
        d.extend_from_slice(&[(10 << 1) | 1, (10 << 1), (15 << 1) | 1]);
        let mac = import_gdr(&d, "Bin").unwrap();
        let strand = &mac.graph.strands[0].instructions;
        // WhenRan, wait, p1 down, wait, p2 down, wait, p1 up
        assert_eq!(strand.len(), 7);
        assert!(matches!(
            &strand[4].kind,
            InstructionKind::Token(InputToken::Button(MacroButton::Right, Direction::Press))
        ));
    }

    #[test]
    fn xdbot_stalled_frames_are_detected() {
        // count 3; frame 1 x=0, frame 2 x=1, frame 3 x=1 (stalled)
        let mut ext = vec![3u8];
        for (frame, x) in [(1u8, 0.0f32), (2, 1.0), (3, 1.0)] {
            ext.push(frame);
            ext.extend_from_slice(&x.to_be_bytes());
            ext.extend_from_slice(&[0u8; 8]);
            ext.push(1);
            ext.extend_from_slice(&[0u8; 12]);
            ext.push(1);
        }
        assert_eq!(stalled_for_test(&ext), vec![3]);
    }

    #[test]
    fn rejects_huge_counts_without_allocating() {
        // map32 / array32 claiming ~4 billion entries in a few bytes.
        for tag in [0xdf, 0xdd] {
            let data = [tag, 0xff, 0xff, 0xff, 0xff, 0xa1, b'a'];
            assert!(import_gdr(&data, "bad").is_err());
        }
    }

    #[test]
    fn rejects_deep_nesting_without_overflowing() {
        let data = vec![0x91; 200_000];
        assert!(import_gdr(&data, "bad").is_err());
    }
}
