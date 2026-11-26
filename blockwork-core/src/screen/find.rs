//! Finding a small image inside a screenshot: an exact RGB pass preserves
//! matches that downsampling would hide. For approximate matches, a coarse
//! RGB search picks candidates for the full-resolution comparison.

use std::cell::OnceCell;

use super::Image;

/// Candidates the coarse pass hands to the full-size check.
const CANDIDATES: usize = 12;
/// Coarse positions under the limit kept when every match is wanted, and
/// how many distinct spots of them get the full-size check.
const ALL_CANDIDATES: usize = 50_000;
const ALL_SPOTS: usize = 1024;
/// Most matches [`find_all`] reports.
pub const MAX_MATCHES: usize = 200;

// Resolution and DPI ratios between common display sizes and scale settings.
const DISPLAY_SCALES: [f64; 12] = [
    2.0 / 3.0,
    1.5,
    0.5,
    2.0,
    0.8,
    1.25,
    0.75,
    4.0 / 3.0,
    5.0 / 6.0,
    1.2,
    0.625,
    1.6,
];

struct Search<'a> {
    hay: &'a Image,
    coarse: [OnceCell<Rgb>; 4],
}

impl<'a> Search<'a> {
    fn new(hay: &'a Image) -> Self {
        Self {
            hay,
            coarse: Default::default(),
        }
    }

    fn shrink(&self, factor: u32) -> &Rgb {
        self.coarse[factor.trailing_zeros() as usize].get_or_init(|| Rgb::shrink(self.hay, factor))
    }

    fn scaled_templates(&self, needle: &'a Image) -> impl Iterator<Item = Image> + use<'a> {
        // A solid swatch has no shape from which to infer a different size.
        let scalable = needle.width > 0 && needle.height > 0 && Shape::of(needle).spread >= 1.0;
        let (max_width, max_height) = (self.hay.width, self.hay.height);
        let mut seen = vec![(needle.width, needle.height)];
        DISPLAY_SCALES.into_iter().filter_map(move |scale| {
            let width = (needle.width as f64 * scale).round() as u32;
            let height = (needle.height as f64 * scale).round() as u32;
            if !scalable
                || width == 0
                || height == 0
                || width > max_width
                || height > max_height
                || seen.contains(&(width, height))
            {
                return None;
            }
            seen.push((width, height));
            let resized = needle.resize(width, height);
            (Shape::of(&resized).spread >= 1.0).then_some(resized)
        })
    }
}

/// Shrink factor for the coarse pass: shrink both images until the needle
/// is about this small on its short side. Searching every sub-grid offset
/// costs factor*factor coarse scans (64 at factor 8), which on a large
/// multi-monitor haystack hangs the daemon for minutes - one phase with a
/// wide enough refine radius is enough since the full-res check covers
/// the grid offset.
fn shrink_factor(needle: &Image) -> u32 {
    [8, 4, 2, 1]
        .into_iter()
        .find(|f| needle.width / f >= 7 && needle.height / f >= 7)
        .unwrap_or(1)
}

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
    best_match(&Search::new(hay), needle, tolerance, false).map(|m| (m.x, m.y))
}

fn best_match(search: &Search<'_>, needle: &Image, tolerance: f64, scaled: bool) -> Option<Match> {
    let hay = search.hay;
    if needle.width == 0
        || needle.height == 0
        || needle.width > hay.width
        || needle.height > hay.height
    {
        return None;
    }
    // Neither grayscale nor downsampled color can distinguish every texture.
    // An exact copy must not depend on surviving a bounded coarse shortlist.
    let mut exact = None;
    exact_positions(hay, needle, |x, y| {
        exact = Some(Match { x, y, diff: 0.0 });
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
    let factor = shrink_factor(needle);
    let small_hay = search.shrink(factor);
    let small_needle = Rgb::shrink(needle, factor);
    let area = (small_needle.width as u64 * small_needle.height as u64).max(1);
    let mut candidates: Vec<(u64, i64, i64)> = Vec::new();
    let cap = if scaled {
        coarse_limit(max_diff, area)
    } else {
        u64::MAX
    };
    for (sum, cx, cy) in coarse(small_hay, &small_needle, Some(CANDIDATES), cap) {
        candidates.push((
            sum * 1024 / area,
            (cx * factor) as i64,
            (cy * factor) as i64,
        ));
    }
    candidates.sort_unstable();
    candidates.truncate(CANDIDATES);

    let mut best: Option<(f64, u32, u32)> = None;
    let (max_x, max_y) = (
        (hay.width - needle.width) as i64,
        (hay.height - needle.height) as i64,
    );
    // One phase can be off by up to factor-1 pixels from the shrunken
    // grid; refining one cell in every direction covers it.
    let radius = factor as i64;
    let shape = scaled.then(|| Shape::of(needle));
    for (_, cx, cy) in candidates {
        for y in (cy - radius).max(0)..=(cy + radius).min(max_y) {
            for x in (cx - radius).max(0)..=(cx + radius).min(max_x) {
                let (x, y) = (x as u32, y as u32);
                let limit = best.map_or(max_diff, |(d, ..)| d.min(max_diff));
                if let Some(diff) = color_diff(hay, needle, x, y, limit)
                    && best.is_none_or(|(d, ..)| diff < d)
                    && shape
                        .as_ref()
                        .is_none_or(|shape| shape.matches(hay, needle, x, y))
                {
                    best = Some((diff, x, y));
                }
            }
        }
    }
    best.map(|(diff, x, y)| Match { x, y, diff })
}

/// Every place `needle` matches in `hay` within `tolerance` percent, best
/// first. A match overlapping a better one by more than half the needle's
/// width and height is the same match, so it's left out.
pub fn find_all(hay: &Image, needle: &Image, tolerance: f64) -> Vec<Match> {
    all_matches(&Search::new(hay), needle, tolerance)
}

fn all_matches(search: &Search<'_>, needle: &Image, tolerance: f64) -> Vec<Match> {
    let hay = search.hay;
    if needle.width == 0
        || needle.height == 0
        || needle.width > hay.width
        || needle.height > hay.height
    {
        return Vec::new();
    }
    let (half_w, half_h) = (needle.width.div_ceil(2), needle.height.div_ceil(2));
    let overlaps =
        move |a: &Match, x: u32, y: u32| a.x.abs_diff(x) < half_w && a.y.abs_diff(y) < half_h;
    let mut found: Vec<Match> = Vec::new();
    exact_positions(hay, needle, |x, y| {
        if !found.iter().any(|m| overlaps(m, x, y)) {
            found.push(Match { x, y, diff: 0.0 });
        }
        found.len() < MAX_MATCHES
    });
    let max_diff = tolerance.clamp(0.0, 100.0) / 100.0 * 255.0;
    if max_diff > 0.0 {
        let factor = shrink_factor(needle);
        let small_hay = search.shrink(factor);
        let small_needle = Rgb::shrink(needle, factor);
        let area = (small_needle.width as u64 * small_needle.height as u64).max(1);
        let cap = coarse_limit(max_diff, area);
        let mut candidates: Vec<(u64, i64, i64)> = Vec::new();
        for (sum, cx, cy) in coarse(small_hay, &small_needle, None, cap) {
            candidates.push((
                sum * 1024 / area,
                (cx * factor) as i64,
                (cy * factor) as i64,
            ));
        }
        candidates.sort_unstable();
        // One spot per would-be match: the best position, with the others
        // that overlap it by over half a needle left to it.
        let mut spots: Vec<(i64, i64)> = Vec::new();
        for (_, cx, cy) in candidates {
            if spots.len() >= ALL_SPOTS {
                break;
            }
            let near = |&(x, y): &(i64, i64)| {
                (x - cx).unsigned_abs() < half_w as u64 && (y - cy).unsigned_abs() < half_h as u64
            };
            if !spots.iter().any(near) {
                spots.push((cx, cy));
            }
        }
        let (max_x, max_y) = (
            (hay.width - needle.width) as i64,
            (hay.height - needle.height) as i64,
        );
        let radius = factor as i64;
        for (cx, cy) in spots {
            let mut best: Option<Match> = None;
            for y in (cy - radius).max(0)..=(cy + radius).min(max_y) {
                for x in (cx - radius).max(0)..=(cx + radius).min(max_x) {
                    let limit = best.map_or(max_diff, |m| m.diff.min(max_diff));
                    if let Some(diff) = color_diff(hay, needle, x as u32, y as u32, limit)
                        && best.is_none_or(|m| diff < m.diff)
                    {
                        best = Some(Match {
                            x: x as u32,
                            y: y as u32,
                            diff,
                        });
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
    found.sort_by(|a, b| {
        a.diff
            .total_cmp(&b.diff)
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
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

fn coarse_limit(max_diff: f64, area: u64) -> u64 {
    ((max_diff + 2.0) * 1.1 * 3.0 * area as f64) as u64 + 1
}

pub(super) struct ScaledMatch {
    pub hit: Match,
    pub width: u32,
    pub height: u32,
}

impl ScaledMatch {
    fn new(hit: Match, image: &Image) -> Self {
        Self {
            hit,
            width: image.width,
            height: image.height,
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (
            self.hit.x as f64 + self.width as f64 / 2.0,
            self.hit.y as f64 + self.height as f64 / 2.0,
        )
    }

    fn overlaps(&self, other: &Self) -> bool {
        let (x, y) = self.center();
        let (other_x, other_y) = other.center();
        (x - other_x).abs() < f64::from(self.width.min(other.width).div_ceil(2))
            && (y - other_y).abs() < f64::from(self.height.min(other.height).div_ceil(2))
    }
}

pub(super) fn find_scaled(hay: &Image, needle: &Image, tolerance: f64) -> Option<ScaledMatch> {
    let search = Search::new(hay);
    let mut best =
        best_match(&search, needle, tolerance, false).map(|hit| ScaledMatch::new(hit, needle));
    if best.as_ref().is_some_and(|m| m.hit.diff == 0.0) {
        return best;
    }
    for variant in search.scaled_templates(needle) {
        let limit = best
            .as_ref()
            .map_or(tolerance, |m| m.hit.diff * 100.0 / 255.0);
        if let Some(hit) = best_match(&search, &variant, limit, true)
            && best.as_ref().is_none_or(|m| hit.diff < m.hit.diff)
        {
            best = Some(ScaledMatch::new(hit, &variant));
            if hit.diff == 0.0 {
                break;
            }
        }
    }
    best
}

pub(super) fn find_all_scaled(hay: &Image, needle: &Image, tolerance: f64) -> Vec<ScaledMatch> {
    let search = Search::new(hay);
    let at_scale = |image: &Image| {
        all_matches(&search, image, tolerance)
            .into_iter()
            .map(|hit| ScaledMatch::new(hit, image))
            .collect::<Vec<_>>()
    };
    let mut found = at_scale(needle);
    for variant in search.scaled_templates(needle) {
        found.extend(at_scale(&variant));
    }
    // Keep the best fit at each spot. Stable sorting favors the original size on ties.
    found.sort_by(|a, b| a.hit.diff.total_cmp(&b.hit.diff));
    let mut kept: Vec<ScaledMatch> = Vec::new();
    for hit in found {
        if kept.len() >= MAX_MATCHES {
            break;
        }
        if !kept.iter().any(|other| other.overlaps(&hit)) {
            kept.push(hit);
        }
    }
    reading_order_by(&mut kept, |m| (m.hit.x, m.hit.y, m.height));
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
        let pixels = needle.rgba.as_chunks::<4>().0.iter();
        let n = (needle.width as f64 * needle.height as f64).max(1.0);
        let mut mean = [0.0; 3];
        for px in pixels.clone() {
            (0..3).for_each(|c| mean[c] += px[c] as f64 / n);
        }
        let spread = pixels
            .map(|px| (0..3).map(|c| (px[c] as f64 - mean[c]).abs()).sum::<f64>())
            .sum::<f64>()
            / (n * 3.0);
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
            for px in patch(row).as_chunks::<4>().0.iter() {
                (0..3).for_each(|c| mean[c] += px[c] as f64 / n);
            }
        }
        let mut sum = 0.0;
        for row in 0..needle.height {
            let n_row = &needle.rgba
                [(row * needle.width * 4) as usize..((row + 1) * needle.width * 4) as usize];
            for (a, b) in patch(row)
                .as_chunks::<4>()
                .0
                .iter()
                .zip(n_row.as_chunks::<4>().0.iter())
            {
                sum += (0..3)
                    .map(|c| ((a[c] as f64 - mean[c]) - (b[c] as f64 - self.mean[c])).abs())
                    .sum::<f64>();
            }
        }
        sum / (n * 3.0) < self.spread / 2.0
    }
}

/// Sorts matches the way text is read: rows top to bottom, each row left
/// to right. Matches whose tops are less than half a needle apart share a
/// row, so a row of buttons a pixel out of line still reads left to right.
pub fn reading_order(matches: &mut [Match], needle_height: u32) {
    reading_order_by(matches, |m| (m.x, m.y, needle_height));
}

fn reading_order_by<T>(matches: &mut [T], geometry: impl Fn(&T) -> (u32, u32, u32)) {
    matches.sort_by_key(|m| {
        let (x, y, _) = geometry(m);
        (y, x)
    });
    let mut start = 0;
    while start < matches.len() {
        let (_, top, height) = geometry(&matches[start]);
        let band = height.div_ceil(2).max(1);
        let end = start
            + matches[start..]
                .iter()
                .take_while(|m| geometry(m).1 - top < band)
                .count();
        matches[start..end].sort_by_key(|m| geometry(m).0);
        start = end;
    }
}

/// Calls `each` with every exact match's top-left pixel, in reading order,
/// until it returns false. Most positions are rejected with two RGB pixels
/// before comparing full rows; choosing a second pixel unlike the first also
/// rejects flat backgrounds cheaply when the template has a small mark.
fn exact_positions(hay: &Image, needle: &Image, mut each: impl FnMut(u32, u32) -> bool) {
    let first = needle.pixel(0, 0);
    let anchor = needle
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .max_by_key(|(_, pixel)| {
            pixel[..3]
                .iter()
                .zip(first)
                .map(|(a, b)| a.abs_diff(b) as u32)
                .sum::<u32>()
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    let (ax, ay) = (
        anchor % needle.width as usize,
        anchor / needle.width as usize,
    );
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
                hay.rgba[h..h + row_bytes]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(needle.rgba[n..n + row_bytes].as_chunks::<4>().0.iter())
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
        for (a, b) in hay.rgba[h..h + row_len]
            .as_chunks::<4>()
            .0
            .iter()
            .zip(needle.rgba[n..n + row_len].as_chunks::<4>().0.iter())
        {
            sum += a[0].abs_diff(b[0]) as u64
                + a[1].abs_diff(b[1]) as u64
                + a[2].abs_diff(b[2]) as u64;
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
        Rgb {
            width,
            height,
            pixels,
        }
    }
}

/// Positions by RGB difference below `cap`, as (sum, x, y), split across
/// threads by row: the best `keep`, or with `None` every one (up to
/// [`ALL_CANDIDATES`]).
fn coarse(hay: &Rgb, needle: &Rgb, keep: Option<usize>, cap: u64) -> Vec<(u64, u32, u32)> {
    if needle.width == 0
        || needle.height == 0
        || needle.width > hay.width
        || needle.height > hay.height
    {
        return Vec::new();
    }
    let rows = hay.height - needle.height + 1;
    // Cap threads so a wide desktop does not oversubscribe the daemon.
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8)
        .min(rows as usize)
        .max(1) as u32;
    let chunk = rows.div_ceil(threads);
    let mut all: Vec<(u64, u32, u32)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (start, end) = (t * chunk, ((t + 1) * chunk).min(rows));
                scope.spawn(move || coarse_rows(hay, needle, start, end, keep, cap))
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_default())
            .collect()
    });
    all.sort_unstable();
    all.truncate(keep.unwrap_or(ALL_CANDIDATES));
    all
}

fn coarse_rows(
    hay: &Rgb,
    needle: &Rgb,
    start: u32,
    end: u32,
    keep: Option<usize>,
    limit: u64,
) -> Vec<(u64, u32, u32)> {
    let Some(keep) = keep else {
        return coarse_rows_under(hay, needle, start, end, limit);
    };
    let mut best: Vec<(u64, u32, u32)> = Vec::with_capacity(keep + 1);
    let (nw, nh) = (needle.width as usize, needle.height as usize);
    for y in start..end {
        for x in 0..=hay.width - needle.width {
            let cap = best.get(keep - 1).map_or(limit, |&(sum, ..)| sum);
            let mut sum = 0u64;
            for row in 0..nh {
                let h = (y as usize + row) * hay.width as usize + x as usize;
                let n = row * nw;
                sum += hay.pixels[h..h + nw]
                    .iter()
                    .zip(&needle.pixels[n..n + nw])
                    .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>())
                    .sum::<u64>();
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
fn coarse_rows_under(
    hay: &Rgb,
    needle: &Rgb,
    start: u32,
    end: u32,
    limit: u64,
) -> Vec<(u64, u32, u32)> {
    let mut under = Vec::new();
    let (nw, nh) = (needle.width as usize, needle.height as usize);
    for y in start..end {
        for x in 0..=hay.width - needle.width {
            let mut sum = 0u64;
            for row in 0..nh {
                let h = (y as usize + row) * hay.width as usize + x as usize;
                let n = row * nw;
                sum += hay.pixels[h..h + nw]
                    .iter()
                    .zip(&needle.pixels[n..n + nw])
                    .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>())
                    .sum::<u64>();
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
        for px in image.rgba.as_chunks_mut::<4>().0.iter_mut() {
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
        needle
            .rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|p| p.iter_mut().take(3).for_each(|c| *c = c.saturating_add(12)));
        assert_eq!(find(&hay, &needle, 0.0), None);
        assert_eq!(find(&hay, &needle, 10.0), Some((60, 40)));
        assert_eq!(
            find(&hay, &noise(30, 20, 99), 10.0),
            None,
            "unrelated pixels never match"
        );
        assert_eq!(
            find(&hay, &noise(300, 20, 99), 100.0),
            None,
            "bigger than the screen"
        );
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
    fn finds_a_saved_image_after_a_display_scale_change() {
        let needle = button(0).scale_up(3);
        let saved = needle.to_data_uri().unwrap();
        let needle = Image::load(&saved).unwrap();
        let mut hay = Image::new(168, 104);
        // Render the same button independently at 1440p (two thirds of 4K).
        paste(&mut hay, &button(0).scale_up(2), 99, 49, 7);
        assert_eq!(find(&hay, &needle, 10.0), None);
        assert_eq!(find_scaled(&hay, &needle, 1.0).map(|m| m.center()), None);
        assert_eq!(
            find_scaled(&hay, &needle, 10.0).map(|m| m.center()),
            Some((123.0, 67.0))
        );
        assert_eq!(
            find_all_scaled(&hay, &needle, 10.0)
                .iter()
                .map(ScaledMatch::center)
                .collect::<Vec<_>>(),
            [(123.0, 67.0)]
        );

        let mut hay = Image::new(168, 104);
        paste(&mut hay, &needle, 17, 23, 0);
        let smaller_needle = button(0).scale_up(2);
        assert_eq!(
            find_scaled(&hay, &smaller_needle, 10.0).map(|m| m.center()),
            Some((53.0, 50.0))
        );
    }

    #[test]
    fn counts_mixed_scale_matches_once_in_reading_order() {
        let needle = button(0).scale_up(3);
        let mut hay = Image::new(196, 132);
        paste(&mut hay, &needle, 110, 6, 0);
        paste(&mut hay, &button(0).scale_up(2), 25, 9, 0);
        paste(&mut hay, &needle, 50, 75, 0);
        let matches = find_all_scaled(&hay, &needle, 10.0);
        assert_eq!(
            matches.iter().map(ScaledMatch::center).collect::<Vec<_>>(),
            [(49.0, 27.0), (146.0, 33.0), (86.0, 102.0)]
        );
        assert_eq!(
            find_scaled(&hay, &needle, 10.0).map(|m| m.center()),
            Some((146.0, 33.0))
        );
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
        assert_eq!(
            spots(&all),
            [(300, 180), (420, 30), (50, 40)],
            "closest first"
        );
        assert_eq!(
            find(&hay, &needle, 10.0),
            Some((300, 180)),
            "find agrees on the best"
        );
        // A tighter tolerance keeps only the closer copies.
        assert_eq!(
            spots(&find_all(&hay, &needle, 6.0)),
            [(300, 180), (420, 30)]
        );
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
        assert_eq!(
            spots(&all),
            (0..5).map(|i| (20 + i * 12, 30)).collect::<Vec<_>>()
        );
    }

    /// A flat button: a gray face, a dark border and a small white arrow.
    fn button(tint: i16) -> Image {
        let mut image = Image::new(24, 18);
        for y in 0..18 {
            for x in 0..24 {
                let edge = x == 0 || y == 0 || x == 23 || y == 17;
                let arrow = (5..13).contains(&y) && x >= 8 + (y - 5) && x < 16 - (y - 5);
                let v = if edge {
                    90
                } else if arrow {
                    250
                } else {
                    200
                };
                let i = ((y * 24 + x) * 4) as usize;
                image.rgba[i..i + 3].fill((v + tint).clamp(0, 255) as u8);
            }
        }
        image
    }

    #[test]
    fn finds_every_button_on_a_flat_page() {
        let mut page = Image::new(1280, 720);
        page.rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|p| p[..3].fill(245));
        let places = [
            (100, 100),
            (500, 100),
            (900, 104),
            (100, 400),
            (500, 400),
            (1200, 690),
        ];
        for (i, &(x, y)) in places.iter().enumerate() {
            // The last one is hovered: a little lighter.
            paste(
                &mut page,
                &button(0),
                x,
                y,
                if i == places.len() - 1 { 10 } else { 0 },
            );
        }
        let mut all = find_all(&page, &button(0), 10.0);
        reading_order(&mut all, 18);
        assert_eq!(spots(&all), places);
        assert_eq!(
            spots(&find_all(&page, &button(0), 0.0)).len(),
            5,
            "the hovered one isn't exact"
        );
        // Mostly page around a small mark: blank page is close on average but has no mark.
        let mark = page.crop(92, 92, 40, 34);
        let mut all = find_all(&page, &mark, 10.0);
        reading_order(&mut all, 34);
        assert_eq!(spots(&all), places.map(|(x, y)| (x - 8, y - 8)));
    }
}
