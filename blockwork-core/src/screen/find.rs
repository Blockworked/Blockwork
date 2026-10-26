//! Finding a small image inside a screenshot: an exact RGB pass preserves
//! matches that downsampling would hide. For approximate matches, a coarse
//! RGB search picks candidates for the full-resolution comparison.

use super::Image;

/// Candidates the coarse pass hands to the full-size check.
const CANDIDATES: usize = 12;
/// Coarse positions under the limit kept per phase when every match is
/// wanted, and how many distinct spots of them get the full-size check.
const ALL_CANDIDATES: usize = 50_000;
const ALL_SPOTS: usize = 1024;
/// Most matches [`find_all`] reports.
pub const MAX_MATCHES: usize = 200;

/// One place `needle` matches: its top-left pixel and mean per-channel
/// difference (0-255).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match {
    pub x: u32,
    pub y: u32,
    pub diff: f64,
}

/// Top-left pixel of the best match of `needle` in `hay`, if its pixels
/// differ by at most `tolerance` percent on average.
pub fn find(hay: &Image, needle: &Image, tolerance: f64) -> Option<(u32, u32)> {
    if needle.width == 0 || needle.height == 0 || needle.width > hay.width || needle.height > hay.height {
        return None;
    }
    // Neither grayscale nor downsampled color can distinguish every texture.
    // An exact copy must not depend on surviving a bounded coarse shortlist.
    let mut exact = None;
    exact_positions(hay, needle, |x, y| {
        exact = Some((x, y));
        false
    });
    if exact.is_some() {
        return exact;
    }
    let max_diff = tolerance.clamp(0.0, 100.0) / 100.0 * 255.0;
    if max_diff == 0.0 {
        return None;
    }
    // Shrink both as far as the needle stays recognizable.
    let factor = [4, 2, 1].into_iter().find(|f| needle.width / f >= 7 && needle.height / f >= 7).unwrap_or(1);
    let small_hay = Rgb::shrink(hay, factor);

    // A match can sit at any offset from the shrunken grid, so the needle is
    // shrunk once per offset; the right one lines up exactly.
    let mut candidates: Vec<(u64, i64, i64)> = Vec::new();
    for py in 0..factor {
        for px in 0..factor {
            let phase = needle.crop(px, py, needle.width - px, needle.height - py);
            let small_needle = Rgb::shrink(&phase, factor);
            let area = (small_needle.width as u64 * small_needle.height as u64).max(1);
            for (sum, cx, cy) in coarse(&small_hay, &small_needle, Some(CANDIDATES), u64::MAX) {
                // Compare per pixel, since phases shrink to different sizes.
                let score = sum * 1024 / area;
                candidates.push((score, (cx * factor) as i64 - px as i64, (cy * factor) as i64 - py as i64));
            }
        }
    }
    candidates.sort_unstable();
    candidates.truncate(CANDIDATES);

    let mut best: Option<(f64, u32, u32)> = None;
    let (max_x, max_y) = ((hay.width - needle.width) as i64, (hay.height - needle.height) as i64);
    // Flat areas can give several phases the same coarse score. Refine a
    // whole downsampled cell so choosing one phase cannot lose its neighbors.
    let radius = factor.saturating_sub(1).max(1) as i64;
    for (_, cx, cy) in candidates {
        for y in (cy - radius).max(0)..=(cy + radius).min(max_y) {
            for x in (cx - radius).max(0)..=(cx + radius).min(max_x) {
                let (x, y) = (x as u32, y as u32);
                let limit = best.map_or(max_diff, |(d, ..)| d.min(max_diff));
                if let Some(diff) = color_diff(hay, needle, x, y, limit)
                    && best.is_none_or(|(d, ..)| diff < d)
                {
                    best = Some((diff, x, y));
                }
            }
        }
    }
    best.map(|(_, x, y)| (x, y))
}

/// Every place `needle` matches in `hay` within `tolerance` percent, best
/// first. A match overlapping a better one by more than half the needle's
/// width and height is the same match, so it's left out.
pub fn find_all(hay: &Image, needle: &Image, tolerance: f64) -> Vec<Match> {
    if needle.width == 0 || needle.height == 0 || needle.width > hay.width || needle.height > hay.height {
        return Vec::new();
    }
    let (half_w, half_h) = (needle.width.div_ceil(2), needle.height.div_ceil(2));
    let overlaps = move |a: &Match, x: u32, y: u32| a.x.abs_diff(x) < half_w && a.y.abs_diff(y) < half_h;
    let mut found: Vec<Match> = Vec::new();
    exact_positions(hay, needle, |x, y| {
        if !found.iter().any(|m| overlaps(m, x, y)) {
            found.push(Match { x, y, diff: 0.0 });
        }
        found.len() < MAX_MATCHES
    });
    let max_diff = tolerance.clamp(0.0, 100.0) / 100.0 * 255.0;
    if max_diff > 0.0 {
        let factor = [4, 2, 1].into_iter().find(|f| needle.width / f >= 7 && needle.height / f >= 7).unwrap_or(1);
        let small_hay = Rgb::shrink(hay, factor);
        // Averaging never adds difference, so a real match's coarse score
        // stays under the limit; the slack covers rounding and cropped edges.
        let mut candidates: Vec<(u64, i64, i64)> = Vec::new();
        for py in 0..factor {
            for px in 0..factor {
                let phase = needle.crop(px, py, needle.width - px, needle.height - py);
                let small_needle = Rgb::shrink(&phase, factor);
                let area = (small_needle.width as u64 * small_needle.height as u64).max(1);
                let cap = ((max_diff + 2.0) * 1.1 * 3.0 * area as f64) as u64 + 1;
                for (sum, cx, cy) in coarse(&small_hay, &small_needle, None, cap) {
                    candidates.push((sum * 1024 / area, (cx * factor) as i64 - px as i64, (cy * factor) as i64 - py as i64));
                }
            }
        }
        candidates.sort_unstable();
        // One spot per would-be match: the best position, with the others
        // that overlap it by over half a needle left to it.
        let mut spots: Vec<(i64, i64)> = Vec::new();
        for (_, cx, cy) in candidates {
            if spots.len() >= ALL_SPOTS {
                break;
            }
            let near = |&(x, y): &(i64, i64)| (x - cx).unsigned_abs() < half_w as u64 && (y - cy).unsigned_abs() < half_h as u64;
            if !spots.iter().any(near) {
                spots.push((cx, cy));
            }
        }
        let (max_x, max_y) = ((hay.width - needle.width) as i64, (hay.height - needle.height) as i64);
        let radius = factor.saturating_sub(1).max(1) as i64;
        for (cx, cy) in spots {
            let mut best: Option<Match> = None;
            for y in (cy - radius).max(0)..=(cy + radius).min(max_y) {
                for x in (cx - radius).max(0)..=(cx + radius).min(max_x) {
                    let limit = best.map_or(max_diff, |m| m.diff.min(max_diff));
                    if let Some(diff) = color_diff(hay, needle, x as u32, y as u32, limit)
                        && best.is_none_or(|m| diff < m.diff)
                    {
                        best = Some(Match { x: x as u32, y: y as u32, diff });
                    }
                }
            }
            found.extend(best);
        }
    }
    // A needle that is mostly background is "close" to any blank patch of
    // that background on average, so a match must also share its shape.
    let shape = Shape::of(needle);
    found.retain(|m| m.diff == 0.0 || shape.matches(hay, needle, m.x, m.y));
    // Best first, ties in reading order, then drop what overlaps a better one.
    found.sort_by(|a, b| a.diff.total_cmp(&b.diff).then(a.y.cmp(&b.y)).then(a.x.cmp(&b.x)));
    let mut kept: Vec<Match> = Vec::new();
    for m in found {
        if kept.len() >= MAX_MATCHES {
            break;
        }
        if !kept.iter().any(|k| overlaps(k, m.x, m.y)) {
            kept.push(m);
        }
    }
    kept
}

/// A needle's own colors around their mean, to tell a real match from a
/// patch that is merely about as bright.
struct Shape {
    mean: [f64; 3],
    /// Mean per-channel distance from `mean`: how far a featureless patch
    /// of the needle's average color would be from it.
    spread: f64,
}

impl Shape {
    fn of(needle: &Image) -> Shape {
        let pixels = needle.rgba.chunks_exact(4);
        let n = (needle.width as f64 * needle.height as f64).max(1.0);
        let mut mean = [0.0; 3];
        for px in pixels.clone() {
            (0..3).for_each(|c| mean[c] += px[c] as f64 / n);
        }
        let spread = pixels.map(|px| (0..3).map(|c| (px[c] as f64 - mean[c]).abs()).sum::<f64>()).sum::<f64>() / (n * 3.0);
        Shape { mean, spread }
    }

    /// Whether the patch at (`x`, `y`), with its own mean taken off, is
    /// closer to the needle's shape than a blank patch would be by half.
    fn matches(&self, hay: &Image, needle: &Image, x: u32, y: u32) -> bool {
        // A flat needle has no shape to compare.
        if self.spread < 1.0 {
            return true;
        }
        let n = needle.width as f64 * needle.height as f64;
        let patch = |row: u32| {
            let h = ((y + row) as usize * hay.width as usize + x as usize) * 4;
            &hay.rgba[h..h + needle.width as usize * 4]
        };
        let mut mean = [0.0; 3];
        for row in 0..needle.height {
            for px in patch(row).chunks_exact(4) {
                (0..3).for_each(|c| mean[c] += px[c] as f64 / n);
            }
        }
        let mut sum = 0.0;
        for row in 0..needle.height {
            let n_row = &needle.rgba[(row * needle.width * 4) as usize..((row + 1) * needle.width * 4) as usize];
            for (a, b) in patch(row).chunks_exact(4).zip(n_row.chunks_exact(4)) {
                sum += (0..3).map(|c| ((a[c] as f64 - mean[c]) - (b[c] as f64 - self.mean[c])).abs()).sum::<f64>();
            }
        }
        sum / (n * 3.0) < self.spread / 2.0
    }
}

/// Sorts matches the way text is read: rows top to bottom, each row left
/// to right. Matches whose tops are less than half a needle apart share a
/// row, so a row of buttons a pixel out of line still reads left to right.
pub fn reading_order(matches: &mut [Match], needle_height: u32) {
    matches.sort_by_key(|m| (m.y, m.x));
    let band = needle_height.div_ceil(2).max(1);
    let mut start = 0;
    while start < matches.len() {
        let top = matches[start].y;
        let end = start + matches[start..].iter().take_while(|m| m.y - top < band).count();
        matches[start..end].sort_by_key(|m| m.x);
        start = end;
    }
}

/// Calls `each` with every exact match's top-left pixel, in reading order,
/// until it returns false. Most positions are rejected with two RGB pixels
/// before comparing full rows; choosing a second pixel unlike the first also
/// rejects flat backgrounds cheaply when the template has a small mark.
fn exact_positions(hay: &Image, needle: &Image, mut each: impl FnMut(u32, u32) -> bool) {
    let first = needle.pixel(0, 0);
    let anchor = needle.rgba.chunks_exact(4).enumerate().max_by_key(|(_, pixel)| {
        pixel[..3].iter().zip(first).map(|(a, b)| a.abs_diff(b) as u32).sum::<u32>()
    }).map(|(index, _)| index).unwrap_or(0);
    let (ax, ay) = (anchor % needle.width as usize, anchor / needle.width as usize);
    let anchor_color = needle.pixel(ax as u32, ay as u32);
    let row_bytes = needle.width as usize * 4;
    for y in 0..=hay.height - needle.height {
        for x in 0..=hay.width - needle.width {
            if hay.pixel(x, y) != first || hay.pixel(x + ax as u32, y + ay as u32) != anchor_color {
                continue;
            }
            let matches = (0..needle.height).all(|row| {
                let h = ((y + row) as usize * hay.width as usize + x as usize) * 4;
                let n = row as usize * row_bytes;
                hay.rgba[h..h + row_bytes].chunks_exact(4)
                    .zip(needle.rgba[n..n + row_bytes].chunks_exact(4))
                    .all(|(a, b)| a[..3] == b[..3])
            });
            if matches && !each(x, y) {
                return;
            }
        }
    }
}

/// Mean per-channel difference of `needle` placed at (`x`, `y`), or `None`
/// once it's clearly above `limit`.
fn color_diff(hay: &Image, needle: &Image, x: u32, y: u32, limit: f64) -> Option<f64> {
    let total = (needle.width as u64 * needle.height as u64 * 3) as f64;
    let cap = (limit * total) as u64;
    let row_len = needle.width as usize * 4;
    let mut sum = 0u64;
    for row in 0..needle.height {
        let h = ((y + row) as usize * hay.width as usize + x as usize) * 4;
        let n = row as usize * row_len;
        for (a, b) in hay.rgba[h..h + row_len].chunks_exact(4).zip(needle.rgba[n..n + row_len].chunks_exact(4)) {
            sum += a[0].abs_diff(b[0]) as u64 + a[1].abs_diff(b[1]) as u64 + a[2].abs_diff(b[2]) as u64;
        }
        if sum > cap {
            return None;
        }
    }
    Some(sum as f64 / total)
}

struct Rgb {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 3]>,
}

impl Rgb {
    /// Preserve all three channels, averaged over `factor`x`factor` blocks.
    fn shrink(image: &Image, factor: u32) -> Rgb {
        let (width, height) = (image.width / factor, image.height / factor);
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        let area = factor * factor;
        for by in 0..height {
            for bx in 0..width {
                let mut sum = [0u32; 3];
                for dy in 0..factor {
                    let row = (by * factor + dy) as usize * image.width as usize;
                    for dx in 0..factor {
                        let i = (row + (bx * factor + dx) as usize) * 4;
                        let px = &image.rgba[i..i + 3];
                        for c in 0..3 {
                            sum[c] += px[c] as u32;
                        }
                    }
                }
                pixels.push(sum.map(|value| (value / area) as u8));
            }
        }
        Rgb { width, height, pixels }
    }
}

/// Positions by RGB difference below `cap`, as (sum, x, y), split across
/// threads by row: the best `keep`, or with `None` every one (up to
/// [`ALL_CANDIDATES`]).
fn coarse(hay: &Rgb, needle: &Rgb, keep: Option<usize>, cap: u64) -> Vec<(u64, u32, u32)> {
    if needle.width == 0 || needle.height == 0 || needle.width > hay.width || needle.height > hay.height {
        return Vec::new();
    }
    let rows = hay.height - needle.height + 1;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(rows as usize).max(1) as u32;
    let chunk = rows.div_ceil(threads);
    let mut all: Vec<(u64, u32, u32)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (start, end) = (t * chunk, ((t + 1) * chunk).min(rows));
                scope.spawn(move || coarse_rows(hay, needle, start, end, keep, cap))
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    all.sort_unstable();
    all.truncate(keep.unwrap_or(ALL_CANDIDATES));
    all
}

fn coarse_rows(hay: &Rgb, needle: &Rgb, start: u32, end: u32, keep: Option<usize>, limit: u64) -> Vec<(u64, u32, u32)> {
    let Some(keep) = keep else {
        return coarse_rows_under(hay, needle, start, end, limit);
    };
    let mut best: Vec<(u64, u32, u32)> = Vec::with_capacity(keep + 1);
    let (nw, nh) = (needle.width as usize, needle.height as usize);
    for y in start..end {
        for x in 0..=hay.width - needle.width {
            let cap = if best.len() == keep { best[keep - 1].0 } else { limit };
            let mut sum = 0u64;
            for row in 0..nh {
                let h = (y as usize + row) * hay.width as usize + x as usize;
                let n = row * nw;
                sum += hay.pixels[h..h + nw].iter().zip(&needle.pixels[n..n + nw])
                    .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>()).sum::<u64>();
                if sum >= cap {
                    break;
                }
            }
            if sum < cap {
                let at = best.partition_point(|(s, ..)| *s <= sum);
                best.insert(at, (sum, x, y));
                best.truncate(keep);
            }
        }
    }
    best
}

/// Every position in rows `start..end` whose sum stays below `limit`.
fn coarse_rows_under(hay: &Rgb, needle: &Rgb, start: u32, end: u32, limit: u64) -> Vec<(u64, u32, u32)> {
    let mut under = Vec::new();
    let (nw, nh) = (needle.width as usize, needle.height as usize);
    for y in start..end {
        for x in 0..=hay.width - needle.width {
            let mut sum = 0u64;
            for row in 0..nh {
                let h = (y as usize + row) * hay.width as usize + x as usize;
                let n = row * nw;
                sum += hay.pixels[h..h + nw].iter().zip(&needle.pixels[n..n + nw])
                    .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>()).sum::<u64>();
                if sum >= limit {
                    break;
                }
            }
            if sum < limit {
                under.push((sum, x, y));
                if under.len() >= ALL_CANDIDATES {
                    return under;
                }
            }
        }
    }
    under
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A noisy but deterministic picture, so every spot looks different.
    fn noise(width: u32, height: u32, seed: u32) -> Image {
        let mut image = Image::new(width, height);
        let mut state = seed.wrapping_mul(2654435761).max(1);
        for px in image.rgba.chunks_exact_mut(4) {
            for c in px.iter_mut().take(3) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *c = (state >> 24) as u8;
            }
        }
        image
    }

    #[test]
    fn finds_an_exact_copy_at_any_alignment() {
        let hay = noise(300, 200, 7);
        for (x, y) in [(0, 0), (101, 57), (277, 171), (13, 3)] {
            let needle = hay.crop(x, y, 23, 29);
            assert_eq!(find(&hay, &needle, 0.0), Some((x, y)), "at {x},{y}");
        }
    }

    #[test]
    fn tolerance_decides_whether_a_changed_copy_counts() {
        let hay = noise(200, 120, 3);
        let mut needle = hay.crop(60, 40, 30, 20);
        // Brighten every pixel a little.
        needle.rgba.chunks_exact_mut(4).for_each(|p| p.iter_mut().take(3).for_each(|c| *c = c.saturating_add(12)));
        assert_eq!(find(&hay, &needle, 0.0), None);
        assert_eq!(find(&hay, &needle, 10.0), Some((60, 40)));
        assert_eq!(find(&hay, &noise(30, 20, 99), 10.0), None, "unrelated pixels never match");
        assert_eq!(find(&hay, &noise(300, 20, 99), 100.0), None, "bigger than the screen");
    }

    /// `hay` with `needle` copied over it at (`x`, `y`), each channel moved by `shift`.
    fn paste(hay: &mut Image, needle: &Image, x: u32, y: u32, shift: i16) {
        for row in 0..needle.height {
            for col in 0..needle.width {
                let n = ((row * needle.width + col) * 4) as usize;
                let h = (((y + row) * hay.width + x + col) * 4) as usize;
                for c in 0..3 {
                    hay.rgba[h + c] = (needle.rgba[n + c] as i16 + shift).clamp(0, 255) as u8;
                }
            }
        }
    }

    fn spots(matches: &[Match]) -> Vec<(u32, u32)> {
        matches.iter().map(|m| (m.x, m.y)).collect()
    }

    #[test]
    fn finds_every_exact_copy() {
        let needle = noise(20, 16, 42);
        let mut hay = noise(400, 300, 5);
        for (x, y) in [(300, 20), (10, 200), (150, 120), (40, 22)] {
            paste(&mut hay, &needle, x, y, 0);
        }
        let mut all = find_all(&hay, &needle, 0.0);
        assert_eq!(all.len(), 4);
        assert!(all.iter().all(|m| m.diff == 0.0));
        reading_order(&mut all, needle.height);
        // (40, 22) and (300, 20) share a row: their tops are 2 px apart.
        assert_eq!(spots(&all), [(40, 22), (300, 20), (150, 120), (10, 200)]);
    }

    #[test]
    fn finds_every_changed_copy_best_first() {
        let needle = noise(32, 24, 8);
        let mut hay = noise(500, 260, 9);
        paste(&mut hay, &needle, 50, 40, 20);
        paste(&mut hay, &needle, 300, 180, 6);
        paste(&mut hay, &needle, 420, 30, 12);
        assert!(find_all(&hay, &needle, 0.0).is_empty());
        let all = find_all(&hay, &needle, 10.0);
        assert_eq!(spots(&all), [(300, 180), (420, 30), (50, 40)], "closest first");
        assert_eq!(find(&hay, &needle, 10.0), Some((300, 180)), "find agrees on the best");
        // A tighter tolerance keeps only the closer copies.
        assert_eq!(spots(&find_all(&hay, &needle, 6.0)), [(300, 180), (420, 30)]);
    }

    #[test]
    fn copies_side_by_side_stay_apart() {
        let needle = noise(12, 12, 77);
        let mut hay = noise(200, 60, 1);
        for i in 0..5 {
            paste(&mut hay, &needle, 20 + i * 12, 30, 0);
        }
        let mut all = find_all(&hay, &needle, 5.0);
        reading_order(&mut all, needle.height);
        assert_eq!(spots(&all), (0..5).map(|i| (20 + i * 12, 30)).collect::<Vec<_>>());
    }

    /// A flat button: a gray face, a dark border and a small white arrow.
    fn button(tint: i16) -> Image {
        let mut image = Image::new(24, 18);
        for y in 0..18 {
            for x in 0..24 {
                let edge = x == 0 || y == 0 || x == 23 || y == 17;
                let arrow = (5..13).contains(&y) && x >= 8 + (y - 5) && x < 16 - (y - 5);
                let v = if edge { 90 } else if arrow { 250 } else { 200 };
                let i = ((y * 24 + x) * 4) as usize;
                image.rgba[i..i + 3].fill((v + tint).clamp(0, 255) as u8);
            }
        }
        image
    }

    #[test]
    fn finds_every_button_on_a_flat_page() {
        let mut page = Image::new(1280, 720);
        page.rgba.chunks_exact_mut(4).for_each(|p| p[..3].fill(245));
        let places = [(100, 100), (500, 100), (900, 104), (100, 400), (500, 400), (1200, 690)];
        for (i, &(x, y)) in places.iter().enumerate() {
            // The last one is hovered: a little lighter.
            paste(&mut page, &button(0), x, y, if i == places.len() - 1 { 10 } else { 0 });
        }
        let mut all = find_all(&page, &button(0), 10.0);
        reading_order(&mut all, 18);
        assert_eq!(spots(&all), places);
        assert_eq!(spots(&find_all(&page, &button(0), 0.0)).len(), 5, "the hovered one isn't exact");
        // Mostly page around a small mark: blank page is close on average but has no mark.
        let mark = page.crop(92, 92, 40, 34);
        let mut all = find_all(&page, &mark, 10.0);
        reading_order(&mut all, 34);
        assert_eq!(spots(&all), places.map(|(x, y)| (x - 8, y - 8)));
    }
}
