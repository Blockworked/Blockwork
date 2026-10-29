//! Image search, object detection and aiming for the screen blocks.

use super::*;

/// A detection query from a detect block's slots.
#[allow(clippy::too_many_arguments)]
/// What an image block looks for and which match it wants.
pub(super) struct ImageSearch<'a> {
    pub(super) image: &'a str,
    pub(super) tolerance: Option<&'a Value>,
    pub(super) pick: Option<&'a ImageMatch>,
    pub(super) region: Option<&'a str>,
}

/// The screen point of the match `search` picks, if there is one, and how
/// many matches there are (only counted when `count` asks or the pick needs them).
pub(super) fn find_picked_image(
    ctx: &mut ExecCtx,
    depth: u32,
    search: &ImageSearch,
    count: bool,
) -> Result<(Option<(f64, f64)>, usize), String> {
    let tolerance = optional_number(ctx, depth, search.tolerance, 10.0)?;
    let needle = crate::screen::image::load_cached(search.image)?;
    let region = crate::detect::parse_region(search.region.unwrap_or(""))?;
    if search.pick.is_none() && !count {
        let at = crate::screen::find_image_in(&needle, tolerance, region)?;
        return Ok((at, at.map_or(0, |_| 1)));
    }
    let target = match search.pick {
        None => MatchTarget::Best,
        Some(m) => match m.mode {
            ImagePick::Nth => MatchTarget::Nth(ctx.resolve(&m.index, depth)?.eval_number()?),
            ImagePick::NearMouse => {
                let (x, y) = crate::screen::cursor()?;
                MatchTarget::Near(x as f64, y as f64)
            }
            ImagePick::NearPoint => MatchTarget::Near(
                ctx.resolve(&m.near_x, depth)?.eval_number()?,
                ctx.resolve(&m.near_y, depth)?.eval_number()?,
            ),
        },
    };
    let matches = crate::screen::find_image_all(&needle, tolerance, region)?;
    let picked = pick_match(&matches, target)?;
    Ok((picked.map(|m| (m.x, m.y)), matches.len()))
}

/// Which of several matches an image block wants, with its numbers resolved.
#[derive(Debug, Clone, Copy)]
pub(super) enum MatchTarget {
    Best,
    Nth(f64),
    Near(f64, f64),
}

/// The match `target` names among `matches` (in reading order), if any.
pub(super) fn pick_match(
    matches: &[crate::screen::ScreenMatch],
    target: MatchTarget,
) -> Result<Option<crate::screen::ScreenMatch>, String> {
    let distance = |m: &crate::screen::ScreenMatch, x: f64, y: f64| (m.x - x).hypot(m.y - y);
    Ok(match target {
        // min_by keeps the first of equals: reading order breaks ties.
        MatchTarget::Best => matches
            .iter()
            .min_by(|a, b| a.diff.total_cmp(&b.diff))
            .copied(),
        MatchTarget::Nth(n) => {
            let n = n.round();
            if n == 0.0 || !n.is_finite() {
                return Err("match numbers count from 1 (or from -1 at the end)".into());
            }
            let at = if n > 0.0 {
                n - 1.0
            } else {
                matches.len() as f64 + n
            };
            (at >= 0.0)
                .then(|| matches.get(at as usize).copied())
                .flatten()
        }
        MatchTarget::Near(x, y) => matches
            .iter()
            .min_by(|a, b| distance(a, x, y).total_cmp(&distance(b, x, y)))
            .copied(),
    })
}

// One argument per field of the detection block.
#[allow(clippy::too_many_arguments)]
pub(super) fn detect_query(
    ctx: &mut ExecCtx,
    depth: u32,
    pattern: &Value,
    by: &str,
    detector: &Value,
    label: &Value,
    confidence: &Value,
    sort: &str,
    region: &str,
) -> Result<crate::detect::Query, String> {
    Ok(crate::detect::Query {
        pattern: ctx.resolve(pattern, depth)?.eval_text()?,
        by: by.parse()?,
        detector: ctx.resolve(detector, depth)?.eval_text()?,
        label: ctx.resolve(label, depth)?.eval_text()?,
        min_score: (ctx.resolve(confidence, depth)?.eval_number()? / 100.0).clamp(0.0, 1.0),
        sort: sort.parse()?,
        region: crate::detect::parse_region(region)?,
    })
}

/// One tracked detection pass, written to `list` in the query's order.
pub(super) fn detect_into_list(
    ctx: &ExecCtx,
    query: &crate::detect::Query,
    list: &str,
    capture: &dyn crate::detect::WindowCapture,
) -> Result<crate::detect::Frame, String> {
    let frame = crate::detect::detect_with(query, capture)?;
    let kept = crate::detect::overlay::filter(
        &overlay_scope(ctx, list),
        frame.detections,
        frame.crosshair,
    );
    let mut found = crate::detect::track::update(&ctx.source.macro_id, list, query, kept);
    crate::detect::sort(&mut found, query.sort, frame.crosshair);
    if !list.is_empty()
        && let Ok(mut lists) = ctx.lists.lock()
    {
        lists.insert(
            list.to_string(),
            found.iter().map(|d| ListItem::Text(d.to_item())).collect(),
        );
    }
    Ok(crate::detect::Frame {
        detections: found,
        ..frame
    })
}

/// Overlay spots are per macro and list, like tracks.
pub(super) fn overlay_scope(ctx: &ExecCtx, list: &str) -> String {
    format!("{}\n{list}", ctx.source.macro_id)
}

pub(super) fn aim_controller(
    ctx: &mut ExecCtx,
    depth: u32,
    gain: &Value,
    max_step: &Value,
    deadzone: &Value,
) -> Result<crate::detect::AimController, String> {
    Ok(crate::detect::AimController {
        gain: ctx.resolve(gain, depth)?.eval_number()?,
        max_step: ctx.resolve(max_step, depth)?.eval_number()?,
        deadzone: ctx.resolve(deadzone, depth)?.eval_number()?,
    })
}

/// Macros allowed to aim this session, by id.
pub(super) static AIM_ALLOWED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Asks once per macro per session before aim blocks move the mouse.
/// `Ok(false)` means the user said no (already logged as the block's error).
pub(super) fn confirm_aim(ctx: &mut ExecCtx, block_id: &str) -> Result<bool, String> {
    let id = ctx.source.macro_id.clone();
    if AIM_ALLOWED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&id)
    {
        return Ok(true);
    }
    let message = "This macro moves the mouse toward what it sees on screen. \
        Only allow it where automation is allowed, never against other players. Let it aim?";
    let request =
        crate::prompt::Request::new(crate::prompt::Kind::Confirm, &ctx.source.name, message);
    match ask(ctx, block_id, "Aim", request)? {
        Some(reply) if reply.answer == "yes" => {
            AIM_ALLOWED
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(id);
            Ok(true)
        }
        Some(_) => {
            soft_fail(ctx, block_id, "Aim", "you didn't allow aiming")?;
            Ok(false)
        }
        None => Ok(false),
    }
}

/// Follows track `target` until it's held centered for `settle_ms`: frames
/// are detected back to back while a mover thread glides the pointer at a
/// high rate. Returns `hit`, `timeout`, `lost` or `stopped`.
pub(super) fn aim_at(
    ctx: &ExecCtx,
    query: &crate::detect::Query,
    list: &str,
    target: u64,
    aim: crate::detect::AimController,
    timeout_ms: f64,
    settle_ms: f64,
) -> Result<&'static str, String> {
    let started = Instant::now();
    let glide = Arc::new(Mutex::new(crate::detect::Glide::new(aim, rand::random())));
    let done = Arc::new(AtomicBool::new(false));
    let mover = {
        let (glide, done, emulator, stop_flag) = (
            Arc::clone(&glide),
            Arc::clone(&done),
            Arc::clone(&ctx.emulator),
            ctx.stop_flag.clone(),
        );
        std::thread::Builder::new()
            .name("aim-glide".into())
            .spawn(move || {
                raise_current_thread_priority();
                let tick = Duration::from_secs_f64(GLIDE_TICK);
                let mut next = Instant::now();
                while !done.load(Ordering::Relaxed)
                    && !stop_flag.as_ref().is_some_and(stop_requested)
                {
                    let (dx, dy) = glide
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .tick(started.elapsed().as_secs_f64(), GLIDE_TICK);
                    if (dx, dy) != (0, 0)
                        && emulator
                            .lock()
                            .map_err(|e| e.to_string())
                            .and_then(|mut em| em.move_mouse_rel(dx, dy))
                            .is_err()
                    {
                        break;
                    }
                    next += tick;
                    let now = Instant::now();
                    if next > now {
                        spin_sleeper().sleep(next - now);
                    } else {
                        next = now;
                    }
                }
            })
            .map_err(|e| e.to_string())?
    };
    let result = follow(
        ctx, query, list, target, &glide, started, aim, timeout_ms, settle_ms,
    );
    done.store(true, Ordering::Relaxed);
    let _ = mover.join();
    result
}

/// The detection side of [`aim_at`].
#[allow(clippy::too_many_arguments)]
pub(super) fn follow(
    ctx: &ExecCtx,
    query: &crate::detect::Query,
    list: &str,
    target: u64,
    glide: &Mutex<crate::detect::Glide>,
    started: Instant,
    aim: crate::detect::AimController,
    timeout_ms: f64,
    settle_ms: f64,
) -> Result<&'static str, String> {
    let mut centered_since: Option<Instant> = None;
    let mut stats = AimStats::default();
    let prefetch = crate::detect::Prefetch::start(&query.pattern, query.by);
    let mut target = target;
    let mut overlay = crate::detect::overlay::OverlayCheck::default();
    loop {
        if ctx.stop_flag.as_ref().is_some_and(stop_requested) {
            stats.report(&ctx.source);
            return Ok("stopped");
        }
        let frame = detect_into_list(ctx, query, list, &prefetch)?;
        stats.add(&frame);
        glide
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_aim(aim.for_window(frame.window_height));
        // The frame shows the game as it was a little before the capture.
        let captured = prefetch
            .captured_at()
            .map_or(0.0, |at| at.duration_since(started).as_secs_f64())
            - FRAME_LAG;
        // Turning the view moves everything on screen, so the tracker can
        // give the same target a new id; take over the box where it should be.
        let expected = glide
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .expected_px(started.elapsed().as_secs_f64());
        let found = frame
            .detections
            .iter()
            .find(|d| d.id == Some(target))
            .or_else(|| {
                let (ex, ey) = expected?;
                let off = |d: &&crate::detect::Detection| {
                    let (x, y) = d.center();
                    (x - frame.crosshair.0 - ex).hypot(y - frame.crosshair.1 - ey)
                };
                frame
                    .detections
                    .iter()
                    .filter(|d| off(d) <= d.h.max(RELOCK_MIN_PX))
                    .min_by(|a, b| off(a).total_cmp(&off(b)))
            });
        if let Some(id) = found.and_then(|d| d.id) {
            if id != target {
                overlay.reset();
            }
            target = id;
        }
        match found {
            // Missed this frame; give up once the tracker drops it.
            None => {
                centered_since = None;
                glide
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .hold(started.elapsed().as_secs_f64());
                if !crate::detect::track::alive(&ctx.source.macro_id, list, target) {
                    stats.report(&ctx.source);
                    return Ok("lost");
                }
            }
            Some(d) => {
                let (x, y) = d.center();
                let error = (x - frame.crosshair.0, y - frame.crosshair.1);
                let (turned, now_turned) = {
                    let g = glide.lock().unwrap_or_else(|e| e.into_inner());
                    (g.turned_px(captured), g.turned_px(captured + FRAME_LAG))
                };
                tracing::debug!(
                    "aim frame at {:.3} s: {:.1},{:.1} px off, turned {:.1},{:.1} px",
                    captured + FRAME_LAG,
                    error.0,
                    error.1,
                    now_turned.0,
                    now_turned.1
                );
                if overlay.is_overlay(captured, error, turned) {
                    // Part of the game's HUD: chasing it would only spin the view.
                    tracing::debug!(
                        "aim: skipping a box fixed to the screen at {:.0},{:.0} px",
                        error.0,
                        error.1
                    );
                    crate::detect::overlay::remember(&overlay_scope(ctx, list), d, frame.crosshair);
                    stats.report(&ctx.source);
                    return Ok("lost");
                }
                glide
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .observe(captured, error);
                if error.0.hypot(error.1) <= aim.deadzone.max(0.0) {
                    if centered_since
                        .get_or_insert_with(Instant::now)
                        .elapsed()
                        .as_secs_f64()
                        * 1000.0
                        >= settle_ms
                    {
                        stats.report(&ctx.source);
                        return Ok("hit");
                    }
                } else {
                    centered_since = None;
                }
            }
        }
        if timeout_ms > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout_ms {
            stats.report(&ctx.source);
            return Ok("timeout");
        }
    }
}

/// Frame timings for one aim, logged when it ends.
#[derive(Default)]
pub(super) struct AimStats {
    frames: u32,
    capture_ms: f64,
    detect_ms: f64,
    started: Option<Instant>,
}

impl AimStats {
    fn add(&mut self, frame: &crate::detect::Frame) {
        self.started.get_or_insert_with(Instant::now);
        self.frames += 1;
        self.capture_ms += frame.capture_ms;
        self.detect_ms += frame.detect_ms;
    }

    fn report(&self, source: &crate::run_log::Source) {
        let (Some(started), n) = (self.started, self.frames.max(1) as f64) else {
            return;
        };
        tracing::info!(
            "{}: aimed for {:.1} s at {:.0} frames/s (waited {:.1} ms for capture, model {:.1} ms)",
            source.name,
            started.elapsed().as_secs_f64(),
            self.frames as f64 / started.elapsed().as_secs_f64().max(1e-3),
            self.capture_ms / n,
            self.detect_ms / n
        );
    }
}

/// Runs one image, detection or aim block. `Some(flow)` ends the strand with it; `None` goes on to the next block.
pub(super) fn run_screen_block(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Flow>, String> {
    match &ins.kind {
        InstructionKind::FindImage {
            image,
            tolerance,
            x_variable,
            y_variable,
            pick,
            region,
            count_variable,
        } => {
            let search = ImageSearch {
                image,
                tolerance: tolerance.as_ref(),
                pick: pick.as_ref(),
                region: region.as_deref(),
            };
            let count_variable = count_variable.as_deref().unwrap_or("");
            match find_picked_image(ctx, depth, &search, !count_variable.is_empty()) {
                Ok((at, count)) => {
                    let (x, y) = at.map_or((-1.0, -1.0), |(x, y)| (x.round(), y.round()));
                    set_variable(ctx, x_variable, Evaluated::Number(x));
                    set_variable(ctx, y_variable, Evaluated::Number(y));
                    set_variable(ctx, count_variable, Evaluated::Number(count as f64));
                }
                Err(e) => soft_fail(ctx, &ins.id, "Find image", e)?,
            }
        }
        InstructionKind::ClickImage {
            image,
            tolerance,
            button,
            pick,
            region,
        } => {
            let clicked = (|| {
                let search = ImageSearch {
                    image,
                    tolerance: tolerance.as_ref(),
                    pick: pick.as_ref(),
                    region: region.as_deref(),
                };
                let mode = pick.as_ref().map(|m| m.mode);
                let (at, count) =
                    find_picked_image(ctx, depth, &search, mode == Some(ImagePick::Nth))?;
                let (x, y) = at.ok_or_else(|| match (mode, count) {
                    (_, 0) => "the image isn't on screen".to_string(),
                    (Some(ImagePick::Nth), 1) => "the image is on screen only once".to_string(),
                    (Some(ImagePick::Nth), n) => format!("the image is on screen only {n} times"),
                    _ => "the image isn't on screen".to_string(),
                })?;
                let mut em = ctx.emulator.lock().map_err(|e| e.to_string())?;
                let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
                em.click_at(x.round() as i32, y.round() as i32, button.clone(), &stopped)
            })();
            if let Err(e) = clicked {
                soft_fail(ctx, &ins.id, "Click image", e)?;
            }
        }
        InstructionKind::WaitForImage {
            image,
            tolerance,
            appear,
            timeout,
            region,
            error_timeout,
        } => {
            let setup = (|| {
                let tolerance = optional_number(ctx, depth, tolerance.as_ref(), 10.0)?;
                let timeout = optional_number(ctx, depth, timeout.as_ref(), 0.0)?;
                let error_timeout = optional_number(ctx, depth, error_timeout.as_ref(), 0.0)?;
                let region = crate::detect::parse_region(region.as_deref().unwrap_or(""))?;
                Ok::<_, String>((
                    crate::screen::image::load_cached(image)?,
                    tolerance,
                    timeout,
                    region,
                    error_timeout,
                ))
            })();
            let (needle, tolerance, timeout, region, error_timeout) = match setup {
                Ok(setup) => setup,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Wait for image", e)?;
                    return Ok(Some(Flow::Normal));
                }
            };
            let started = Instant::now();
            loop {
                match crate::screen::find_image_in(&needle, tolerance, region) {
                    Ok(at) if at.is_some() == *appear => break,
                    Ok(_) => {}
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for image", e)?;
                        break;
                    }
                }
                let what = if *appear {
                    "Wait for image: the image didn't appear"
                } else {
                    "Wait for image: the image didn't go away"
                };
                if wait_limit_reached(ctx, &ins.id, started, timeout, error_timeout, what)? {
                    break;
                }
                if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                    break;
                }
            }
        }
        InstructionKind::DetectObjectsInWindow {
            pattern,
            by,
            detector,
            label,
            confidence,
            sort,
            results_list,
            count_variable,
            region,
        } => {
            let found = detect_query(
                ctx, depth, pattern, by, detector, label, confidence, sort, region,
            )
            .and_then(|query| detect_into_list(ctx, &query, results_list, &crate::detect::Screen));
            match found {
                Ok(frame) => set_variable(
                    ctx,
                    count_variable,
                    Evaluated::Number(frame.detections.len() as f64),
                ),
                Err(e) => soft_fail(ctx, &ins.id, "Detect objects", e)?,
            }
        }
        InstructionKind::WaitForDetection {
            pattern,
            by,
            detector,
            label,
            confidence,
            appear,
            timeout,
            region,
        } => {
            let setup = detect_query(
                ctx, depth, pattern, by, detector, label, confidence, "Score", region,
            )
            .and_then(|query| Ok((query, ctx.resolve(timeout, depth)?.eval_number()?.max(0.0))));
            let (query, timeout) = match setup {
                Ok(setup) => setup,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Wait for detection", e)?;
                    return Ok(None);
                }
            };
            let started = Instant::now();
            loop {
                match crate::detect::detect_on_screen(&query) {
                    Ok(frame) if frame.detections.is_empty() != *appear => break,
                    Ok(_) => {}
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for detection", e)?;
                        break;
                    }
                }
                if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                    let what = if *appear {
                        "didn't appear"
                    } else {
                        "didn't go away"
                    };
                    soft_fail(
                        ctx,
                        &ins.id,
                        "Wait for detection",
                        format!("it {what} within {timeout} ms"),
                    )?;
                    break;
                }
                if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                    break;
                }
            }
        }
        InstructionKind::AimStep {
            pattern,
            by,
            target_x,
            target_y,
            gain,
            max_step,
            deadzone,
        } => {
            if !confirm_aim(ctx, &ins.id)? {
                return Ok(None);
            }
            let done = (|| {
                let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                let target = (
                    ctx.resolve(target_x, depth)?.eval_number()?,
                    ctx.resolve(target_y, depth)?.eval_number()?,
                );
                let aim = aim_controller(ctx, depth, gain, max_step, deadzone)?;
                let window = crate::window::bounds(&pattern, by.parse()?)?;
                let crosshair = (
                    window.x + window.width / 2.0,
                    window.y + window.height / 2.0,
                );
                if let Some((dx, dy)) = aim.for_window(window.height).step(target, crosshair) {
                    ctx.emulator
                        .lock()
                        .map_err(|e| e.to_string())?
                        .move_mouse_rel(dx, dy)?;
                }
                Ok::<_, String>(())
            })();
            if let Err(e) = done {
                soft_fail(ctx, &ins.id, "Aim", e)?;
            }
        }
        InstructionKind::AimAtDetection {
            results_list,
            index,
            gain,
            max_step,
            deadzone,
            timeout,
            settle_ms,
            status_variable,
        } => {
            if !confirm_aim(ctx, &ins.id)? {
                return Ok(None);
            }
            let setup = (|| {
                let query = crate::detect::track::query_for(&ctx.source.macro_id, results_list)
                    .ok_or_else(|| {
                        format!("detect objects into the list \"{results_list}\" first")
                    })?;
                let n = ctx.resolve(index, depth)?.eval_number()?;
                let item = ctx
                    .lists
                    .lock()
                    .ok()
                    .and_then(|lists| {
                        Some(
                            lists
                                .get(results_list)?
                                .get((n.round() as usize).wrapping_sub(1))?
                                .evaluated()
                                .as_text(),
                        )
                    })
                    .ok_or_else(|| format!("the list \"{results_list}\" has no item {n}"))?;
                let target = crate::detect::Detection::from_item(&item)?
                    .id
                    .ok_or("that item has no tracking id; fill the list with a detect block")?;
                let aim = aim_controller(ctx, depth, gain, max_step, deadzone)?;
                let timeout = ctx.resolve(timeout, depth)?.eval_number()?.max(0.0);
                let settle = ctx.resolve(settle_ms, depth)?.eval_number()?.max(0.0);
                Ok::<_, String>((query, target, aim, timeout, settle))
            })();
            let result = setup.and_then(|(query, target, aim, timeout, settle)| {
                aim_at(ctx, &query, results_list, target, aim, timeout, settle)
            });
            match result {
                Ok(status) => {
                    set_variable(ctx, status_variable, Evaluated::Text(status.to_string()))
                }
                Err(e) => soft_fail(ctx, &ins.id, "Aim at detection", e)?,
            }
        }
        _ => unreachable!("run_screen_block got another block"),
    }
    Ok(None)
}
