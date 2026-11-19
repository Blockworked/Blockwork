//! The variable and list blocks.

use super::*;
use blockstitch_core::graph::{DictItem, dict_remove, dict_set, parse_json_object};

/// Runs one variable or list block. `Some(flow)` ends the strand with it; `None` goes on to the next block.
pub(super) fn run_data_block(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Flow>, String> {
    match &ins.kind {
        InstructionKind::SetVariable(name, value) => {
            match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                Ok(result) => {
                    if let Ok(mut vars) = ctx.variables.lock() {
                        vars.insert(name.clone(), result);
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Set variable", e)?,
            }
        }
        InstructionKind::ChangeVariable(name, value) => {
            match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                // The delta must be numeric - text/bool are a deliberate no-op.
                Ok(Evaluated::Text(_) | Evaluated::Bool(_)) => {}
                Ok(Evaluated::Number(delta)) => {
                    if let Ok(mut vars) = ctx.variables.lock() {
                        // Non-numeric (or missing) current value coerces
                        // to 0 before adding, same leniency Scratch uses.
                        let current = vars
                            .get(name)
                            .and_then(|e| e.as_number().ok())
                            .unwrap_or(0.0);
                        vars.insert(name.clone(), Evaluated::Number(current + delta));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Change variable", e)?,
            }
        }
        InstructionKind::SetClipboard(value) => {
            match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                Ok(text) => {
                    if let Err(e) = crate::clipboard::set_text(text) {
                        soft_fail(ctx, &ins.id, "Set clipboard", e)?;
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Set clipboard", e)?,
            }
        }
        InstructionKind::SaveClipboard => match crate::clipboard::save() {
            Ok(saved) => {
                saved_clipboards().insert(ctx.source.macro_id.clone(), saved);
            }
            Err(e) => soft_fail(ctx, &ins.id, "Save clipboard", e)?,
        },
        InstructionKind::RestoreClipboard => {
            let saved = saved_clipboards().get(&ctx.source.macro_id).cloned();
            let restored = match saved {
                Some(saved) => crate::clipboard::restore(&saved),
                None => Err("nothing saved yet (use save clipboard first)".into()),
            };
            if let Err(e) = restored {
                soft_fail(ctx, &ins.id, "Restore clipboard", e)?;
            }
        }
        InstructionKind::AddToList { value, name } => {
            match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                Ok(value) => match ListItem::from_evaluated(value) {
                    Some(item) => {
                        if let Ok(mut lists) = ctx.lists.lock()
                            && let Some(list) = lists.get_mut(name)
                        {
                            list.push(item);
                        }
                    }
                    None => soft_fail(
                        ctx,
                        &ins.id,
                        "Add to list",
                        "list items must be number or text",
                    )?,
                },
                Err(e) => soft_fail(ctx, &ins.id, "Add to list", e)?,
            }
        }
        InstructionKind::DeleteOfList { index, name } => {
            match ctx.resolve(index, depth).and_then(|v| v.eval_number()) {
                Ok(index) => {
                    if let Ok(mut lists) = ctx.lists.lock()
                        && let Some(list) = lists.get_mut(name)
                        && let Some(index) = list_index(index, list.len(), false)
                    {
                        list.remove(index);
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Delete list item", e)?,
            }
        }
        InstructionKind::DeleteAllOfList { name } => {
            if let Ok(mut lists) = ctx.lists.lock()
                && let Some(list) = lists.get_mut(name)
            {
                list.clear();
            }
        }
        InstructionKind::ShiftList { name, amount } => {
            match ctx.resolve(amount, depth).and_then(|v| v.eval_number()) {
                Ok(amount) => {
                    if let Ok(mut lists) = ctx.lists.lock()
                        && let Some(list) = lists.get_mut(name)
                        && !list.is_empty()
                    {
                        let len = list.len();
                        let distance = amount.round() as isize;
                        if distance >= 0 {
                            list.rotate_right(distance as usize % len);
                        } else {
                            list.rotate_left(distance.unsigned_abs() % len);
                        }
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Shift list", e)?,
            }
        }
        InstructionKind::InsertIntoList { value, index, name } => {
            let value = ctx.resolve(value, depth).and_then(|v| v.eval());
            let index = ctx.resolve(index, depth).and_then(|v| v.eval_number());
            match (value, index) {
                (Ok(value), Ok(index)) => match ListItem::from_evaluated(value) {
                    Some(item) => {
                        if let Ok(mut lists) = ctx.lists.lock()
                            && let Some(list) = lists.get_mut(name)
                            && let Some(index) = list_index(index, list.len(), true)
                        {
                            list.insert(index, item);
                        }
                    }
                    None => soft_fail(
                        ctx,
                        &ins.id,
                        "Insert into list",
                        "list items must be number or text",
                    )?,
                },
                (Err(e), _) | (_, Err(e)) => soft_fail(ctx, &ins.id, "Insert into list", e)?,
            }
        }
        InstructionKind::ReplaceItemOfList { index, name, value } => {
            let index = ctx.resolve(index, depth).and_then(|v| v.eval_number());
            let value = ctx.resolve(value, depth).and_then(|v| v.eval());
            match (index, value) {
                (Ok(index), Ok(value)) => match ListItem::from_evaluated(value) {
                    Some(item) => {
                        if let Ok(mut lists) = ctx.lists.lock()
                            && let Some(list) = lists.get_mut(name)
                            && let Some(index) = list_index(index, list.len(), false)
                        {
                            list[index] = item;
                        }
                    }
                    None => soft_fail(
                        ctx,
                        &ins.id,
                        "Replace list item",
                        "list items must be number or text",
                    )?,
                },
                (Err(e), _) | (_, Err(e)) => soft_fail(ctx, &ins.id, "Replace list item", e)?,
            }
        }
        InstructionKind::ReverseList { name } => {
            if let Ok(mut lists) = ctx.lists.lock()
                && let Some(list) = lists.get_mut(name)
            {
                list.reverse();
            }
        }
        InstructionKind::SplitIntoList {
            text,
            separator,
            name,
        } => {
            let parts = ctx
                .resolve(text, depth)
                .and_then(|v| v.eval_text())
                .and_then(|text| {
                    Ok(crate::text::split(
                        &text,
                        &ctx.resolve(separator, depth)?.eval_text()?,
                    ))
                });
            match parts {
                Ok(parts) => {
                    if let Ok(mut lists) = ctx.lists.lock() {
                        lists.insert(
                            name.clone(),
                            parts.into_iter().map(ListItem::Text).collect(),
                        );
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Split", e)?,
            }
        }
        InstructionKind::SetDictKey { name, key, value } => {
            let key = ctx.resolve(key, depth).and_then(|v| v.eval_text());
            let value = ctx.resolve(value, depth).and_then(|v| v.eval());
            match (key, value) {
                (Ok(key), Ok(value)) => match DictItem::from_evaluated(value) {
                    Some(item) => {
                        if let Ok(mut dicts) = ctx.dicts.lock()
                            && let Some(entries) = dicts.get_mut(name)
                        {
                            dict_set(entries, key, item);
                        }
                    }
                    None => soft_fail(
                        ctx,
                        &ins.id,
                        "Set dict key",
                        "dict values must be number or text",
                    )?,
                },
                (Err(e), _) | (_, Err(e)) => soft_fail(ctx, &ins.id, "Set dict key", e)?,
            }
        }
        InstructionKind::DeleteDictKey { name, key } => {
            match ctx.resolve(key, depth).and_then(|v| v.eval_text()) {
                Ok(key) => {
                    if let Ok(mut dicts) = ctx.dicts.lock()
                        && let Some(entries) = dicts.get_mut(name)
                    {
                        dict_remove(entries, &key);
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Delete dict key", e)?,
            }
        }
        InstructionKind::ClearDict { name } => {
            if let Ok(mut dicts) = ctx.dicts.lock()
                && let Some(entries) = dicts.get_mut(name)
            {
                entries.clear();
            }
        }
        InstructionKind::DictFromJson { text, name } => {
            match ctx
                .resolve(text, depth)
                .and_then(|v| v.eval_text())
                .and_then(|text| parse_json_object(&text))
            {
                Ok(entries) => {
                    if let Ok(mut dicts) = ctx.dicts.lock()
                        && let Some(slot) = dicts.get_mut(name)
                    {
                        *slot = entries;
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Dict from JSON", e)?,
            }
        }
        _ => unreachable!("run_data_block got another block"),
    }
    Ok(None)
}

/// What each macro's `SaveClipboard` kept, by macro id.
fn saved_clipboards() -> std::sync::MutexGuard<'static, HashMap<String, crate::clipboard::Saved>> {
    static SAVED: OnceLock<Mutex<HashMap<String, crate::clipboard::Saved>>> = OnceLock::new();
    SAVED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
