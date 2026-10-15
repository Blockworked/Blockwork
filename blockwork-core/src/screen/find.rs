//! Finding a small image inside a screenshot: a coarse grayscale search on
//! shrunken copies picks a few candidates, then a full-color comparison
//! around each one decides.

use super::Image;

/// Candidates the coarse pass hands to the full-size check.
const CANDIDATES: usize = 12;

/// Top-left pixel of the best match of `needle` in `hay`, if its pixels
/// differ by at most `tolerance` percent on average.
pub fn find(hay: &Image, needle: &Image, tolerance: f64) -> Option<(u32, u32)> {
    if needle.width == 0 || needle.height == 0 || needle.width > hay.width || needle.height > hay.height {
        return None;
    }
    let max_diff = tolerance.clamp(0.0, 100.0) / 100.0 * 255.0;
    // Shrink both as far as the needle stays recognizable.
    let factor = [4, 2, 1].into_iter().find(|f| needle.width / f >= 7 && needle.height / f >= 7).unwrap_or(1);
    let small_hay = Gray::shrink(hay, factor);

    // A match can sit at any offset from the shrunken grid, so the needle is
    // shrunk once per offset; the right one lines up exactly.
    let mut candidates: Vec<(u64, i64, i64)> = Vec::new();
    for py in 0..factor {
        for px in 0..factor {
            let phase = needle.crop(px, py, needle.width - px, needle.height - py);
            let small_needle = Gray::shrink(&phase, factor);
            let area = (small_needle.width as u64 * small_needle.height as u64).max(1);
            for (sum, cx, cy) in coarse(&small_hay, &small_needle) {
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
    for (_, cx, cy) in candidates {
        for y in (cy - 1).max(0)..=(cy + 1).min(max_y) {
            for x in (cx - 1).max(0)..=(cx + 1).min(max_x) {
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

struct Gray {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Gray {
    /// Grayscale, averaged over `factor`x`factor` blocks.
    fn shrink(image: &Image, factor: u32) -> Gray {
        let (width, height) = (image.width / factor, image.height / factor);
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        let area = factor * factor;
        for by in 0..height {
            for bx in 0..width {
                let mut sum = 0u32;
                for dy in 0..factor {
                    let row = (by * factor + dy) as usize * image.width as usize;
                    for dx in 0..factor {
                        let i = (row + (bx * factor + dx) as usize) * 4;
                        let px = &image.rgba[i..i + 3];
                        sum += (px[0] as u32 * 77 + px[1] as u32 * 150 + px[2] as u32 * 29) >> 8;
                    }
                }
                pixels.push((sum / area) as u8);
            }
        }
        Gray { width, height, pixels }
    }
}

/// The best few positions by grayscale difference, as (sum, x, y), split
/// across threads by row.
fn coarse(hay: &Gray, needle: &Gray) -> Vec<(u64, u32, u32)> {
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
                scope.spawn(move || coarse_rows(hay, needle, start, end))
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    all.sort_unstable();
    all.truncate(CANDIDATES);
    all
}

fn coarse_rows(hay: &Gray, needle: &Gray, start: u32, end: u32) -> Vec<(u64, u32, u32)> {
    let mut best: Vec<(u64, u32, u32)> = Vec::with_capacity(CANDIDATES + 1);
    let (nw, nh) = (needle.width as usize, needle.height as usize);
    for y in start..end {
        for x in 0..=hay.width - needle.width {
            let cap = if best.len() == CANDIDATES { best[CANDIDATES - 1].0 } else { u64::MAX };
            let mut sum = 0u64;
            for row in 0..nh {
                let h = (y as usize + row) * hay.width as usize + x as usize;
                let n = row * nw;
                sum += hay.pixels[h..h + nw].iter().zip(&needle.pixels[n..n + nw]).map(|(a, b)| a.abs_diff(*b) as u64).sum::<u64>();
                if sum >= cap {
                    break;
                }
            }
            if sum < cap {
                let at = best.partition_point(|(s, ..)| *s <= sum);
                best.insert(at, (sum, x, y));
                best.truncate(CANDIDATES);
            }
        }
    }
    best
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
}
